"""Проверка настоящего PTY без открытия окон: python3 tests/terminal_smoke.py."""
import fcntl
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


def smoke(binary, columns, rows, language, provider):
    master, slave = pty.openpty()
    original = termios.tcgetattr(slave)
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))

    def terminal_session():
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

    output = bytearray()

    def drain(seconds):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            if select.select([master], [], [], min(0.05, until - time.monotonic()))[0]:
                try:
                    output.extend(os.read(master, 65536))
                except OSError:
                    break

    with tempfile.TemporaryDirectory(prefix="deck-pty-") as config:
        process = subprocess.Popen(
            [str(binary), "--demo", "--lang", language,"--provider",provider], stdin=slave, stdout=slave, stderr=slave,
            env={**os.environ, "TERM": "xterm-256color", "XDG_CONFIG_HOME": config},
            preexec_fn=terminal_session,
        )
        try:
            drain(1)
            assert process.poll() is None, output.decode(errors="replace")[-500:]
            if columns >= 80:
                assert ("github.example.org" if provider=="github" else "gitlab.example.org").encode() in output
                for key in [b"\r", b"p", b"\r", b"\r", b"?", b"q", b"s"]:
                    os.write(master, key)
                    drain(0.12)
                assert ("Settings" if language == "en" else "Настройки").encode() in output
                os.write(master, b"L")
                drain(0.2)
                assert ("Настройки" if language == "en" else "Settings").encode() in output
            else:
                assert b"80" in output and b"24" in output
            os.write(master, b"q")
            drain(0.3)
            assert process.wait(timeout=3) == 0
            assert termios.tcgetattr(slave) == original, "Терминал не восстановлен"
            assert b"\x1b[?1049l" in output, "Не закрыт alternate screen"
            assert b"\x1b[?2004l" in output, "Не выключена bracketed paste"
            assert not list(Path(config).rglob("config.json")), "Demo сохранил настройки"
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            os.close(master)
            os.close(slave)
    print(f"PTY {provider} {language} {columns}x{rows}: запуск, навигация, выход, восстановление — OK")


if __name__ == "__main__":
    executable = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/forge-deck").resolve()
    for provider in ["gitlab","github"]:
        for language in ["en", "ru"]:
            for size in [(160, 48), (100, 32), (72, 22)]:
                smoke(executable, *size, language,provider)
