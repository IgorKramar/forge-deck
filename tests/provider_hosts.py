"""Регрессия выбора сервера при G: только временные профили и подставные CLI."""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time


def check(binary, initial, saved, environment, expected):
    target = "github" if initial == "gitlab" else "gitlab"
    initial_host = "initial.example.org"
    with tempfile.TemporaryDirectory(prefix="forge-hosts-") as directory:
        root = Path(directory)
        config = root / "config" / "forge-deck" / "config.json"
        config.parent.mkdir(parents=True)
        config.write_text(json.dumps({"hosts": saved}))
        cli = root / "bin"
        cli.mkdir()
        log = cli / "calls.jsonl"
        for name in ["gh", "glab"]:
            path = cli / name
            path.write_text('#!/usr/bin/python3\nimport json,sys\nfrom pathlib import Path\np=Path(sys.argv[0])\nwith (p.parent/"calls.jsonl").open("a") as f: f.write(json.dumps({"cli":p.name,"host":sys.argv[sys.argv.index("--hostname")+1]})+"\\n")\nprint("[]")\n')
            path.chmod(0o700)
        env = {**os.environ, "TERM": "xterm-256color", "XDG_CONFIG_HOME": str(root / "config"),
               "PATH": str(cli) + os.pathsep + os.environ["PATH"]}
        for name in ["GH_HOST", "GITLAB_HOST"]:
            env.pop(name, None)
        env.update(environment)
        master, slave = pty.openpty()
        original = termios.tcgetattr(slave)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 160, 0, 0))

        def session():
            os.setsid()
            fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

        process = subprocess.Popen([str(binary), "--provider", initial, "--host", initial_host],
                                   stdin=slave, stdout=slave, stderr=slave, env=env,
                                   preexec_fn=session)

        def wait_calls(count):
            until = time.monotonic() + 5
            while time.monotonic() < until:
                if select.select([master], [], [], 0.02)[0]:
                    try:
                        os.read(master, 65536)
                    except OSError:
                        break
                calls = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
                if len(calls) >= count:
                    return calls
                assert process.poll() is None, "TUI завершился до запроса"
            raise AssertionError("CLI не получил ожидаемый запрос")

        try:
            calls = wait_calls(1)
            assert calls[0]["host"] == initial_host, "--host потерял приоритет"
            os.write(master, b"G")
            if expected is not None:
                calls = wait_calls(2)
                assert calls[-1] == {"cli": "gh" if target == "github" else "glab", "host": expected}, calls
            os.write(master, b"q")
            assert process.wait(timeout=3) == 0
            assert termios.tcgetattr(slave) == original
            data = json.loads(config.read_text())
            if expected is None:
                assert data["provider"] == initial and data["hosts"][initial] == initial_host, data
                assert target not in data["hosts"]
                assert len(log.read_text().splitlines()) == 1
            else:
                assert data["provider"] == target and data["hosts"][target] == expected
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            os.close(master)
            os.close(slave)
    print(f"Provider host {initial} → {target}: {expected} — OK")


if __name__ == "__main__":
    binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/forge-deck").resolve()
    check(binary, "gitlab", {}, {"GH_HOST": "enterprise.example.org"}, "enterprise.example.org")
    check(binary, "gitlab", {"github": "saved.example.org"}, {"GH_HOST": "enterprise.example.org"}, "saved.example.org")
    check(binary, "github", {}, {"GITLAB_HOST": "lab.example.org"}, "lab.example.org")
    check(binary, "gitlab", {}, {}, "github.com")
    check(binary, "gitlab", {}, {"GH_HOST": "https://wrong.example.org"}, None)
