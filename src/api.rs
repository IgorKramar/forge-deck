use crate::{
    demo::Demo,
    model::{ApiError, Method, Request, Response},
};
use serde_json::Value;
use std::{
    io::{self, Read, Write},
    net::Ipv4Addr,
    os::{fd::AsRawFd, unix::process::CommandExt},
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

pub struct Api {
    host: String,
    demo: Option<Demo>,
    program: PathBuf,
    timeout: Duration,
    output_limit: usize,
}

impl Api {
    pub fn new(host: &str, demo: bool) -> Result<Self, ApiError> {
        validate_host(host).map_err(|message| ApiError {
            message,
            uncertain: false,
            http_status: None,
        })?;
        Ok(Self {
            host: host.to_owned(),
            demo: demo.then(Demo::new),
            program: "glab".into(),
            timeout: Duration::from_secs(20),
            output_limit: 2 * 1024 * 1024,
        })
    }

    pub fn execute(
        &mut self,
        request: &Request,
        cancel: &AtomicBool,
    ) -> Result<Response, ApiError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(error("Запрос отменён", false));
        }
        if let Some(demo) = &mut self.demo {
            return demo.execute(request).map(clean_response);
        }
        let deadline = Instant::now() + self.timeout;
        let response = match request {
            Request::Json {
                endpoint,
                method,
                body,
            } => self.json(endpoint, *method, body.as_ref(), cancel, deadline),
            Request::Text { endpoint } => {
                let captured = self.run(endpoint, Method::Get, None, cancel, deadline)?;
                if endpoint.contains("/repository/blobs/")
                    && (captured.stdout.bytes.contains(&0)
                        || std::str::from_utf8(&captured.stdout.bytes).is_err())
                {
                    return Ok(Response::Text {
                        body: "Бинарный файл: просмотр содержимого в терминале отключён.".into(),
                        truncated: captured.stdout.truncated,
                    });
                }
                Ok(Response::Text {
                    body: String::from_utf8_lossy(&captured.stdout.bytes).into_owned(),
                    truncated: captured.stdout.truncated,
                })
            }
            Request::Projects(ids) => {
                let mut projects = Vec::with_capacity(ids.len());
                for id in ids {
                    let response = self.json(
                        &format!("projects/{id}"),
                        Method::Get,
                        None,
                        cancel,
                        deadline,
                    );
                    let value = match response {
                        Ok(Response::Json(value)) => value,
                        Err(error) if matches!(error.http_status, Some(403 | 404)) => {
                            serde_json::json!({"id":id,"name":format!("Недоступен #{id}"),"path_with_namespace":"* — убрать из избранного","_unavailable":true})
                        }
                        Err(error) => return Err(error),
                        _ => unreachable!(),
                    };
                    if value.get("id").and_then(Value::as_u64) != Some(*id) {
                        return Err(error("GitLab вернул неверный проект для избранного", false));
                    }
                    projects.push(value);
                }
                Ok(Response::Json(Value::Array(projects)))
            }
        }?;
        Ok(clean_response(response))
    }

    fn json(
        &self,
        endpoint: &str,
        method: Method,
        body: Option<&Value>,
        cancel: &AtomicBool,
        deadline: Instant,
    ) -> Result<Response, ApiError> {
        let captured = self.run(endpoint, method, body, cancel, deadline)?;
        if captured.stdout.truncated {
            return Err(error(
                "Ответ GitLab превышает 2 МиБ. Сузьте поиск или откройте отдельный ресурс.",
                method != Method::Get,
            ));
        }
        let value = serde_json::from_slice(&captured.stdout.bytes)
            .map_err(|_| error("GitLab вернул некорректный JSON", method != Method::Get))?;
        Ok(Response::Json(value))
    }

    fn run(
        &self,
        endpoint: &str,
        method: Method,
        body: Option<&Value>,
        cancel: &AtomicBool,
        deadline: Instant,
    ) -> Result<Captured, ApiError> {
        validate_endpoint(endpoint).map_err(|message| error(message, false))?;
        if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return Err(error("Запрос отменён до отправки", false));
        }
        let payload = body
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| error("Не удалось подготовить JSON запроса", false))?;
        if payload.as_ref().is_some_and(|data| data.len() > 32 * 1024) {
            return Err(error("Тело запроса слишком велико", false));
        }
        let mutation = method != Method::Get;
        let mut command = Command::new(&self.program);
        command.args(["api", "--hostname", &self.host, "--method", method.name()]);
        if payload.is_some() {
            command.args(["--input", "-", "--header", "Content-Type: application/json"]);
        }
        command
            .arg(endpoint)
            .process_group(0)
            .stdin(if payload.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = command.spawn().map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                error("Не найден glab. Установите GitLab CLI и выполните glab auth login для выбранного сервера.", false)
            } else {
                error("Не удалось запустить glab", false)
            }
        })?;
        let mut running = RunningChild::new(child);
        let mut stdout = running
            .child
            .stdout
            .take()
            .ok_or_else(|| error("Не открыт stdout glab", mutation))?;
        let mut stderr = running
            .child
            .stderr
            .take()
            .ok_or_else(|| error("Не открыт stderr glab", mutation))?;
        let mut stdin = running.child.stdin.take();
        nonblocking(&stdout)
            .and_then(|()| nonblocking(&stderr))
            .and_then(|()| {
                if let Some(stream) = &stdin {
                    nonblocking(stream)
                } else {
                    Ok(())
                }
            })
            .map_err(|_| error("Не удалось настроить неблокирующий обмен с glab", mutation))?;
        let mut captured = Captured {
            stdout: Capture::default(),
            stderr: Capture::default(),
        };
        let mut status: Option<ExitStatus> = None;
        let mut stdout_done = false;
        let mut stderr_done = false;
        let mut written = 0;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(error("Ожидание ответа отменено", mutation));
            }
            if Instant::now() >= deadline {
                return Err(error("GitLab не ответил за отведённое время", mutation));
            }
            if let (Some(stream), Some(data)) = (&mut stdin, &payload) {
                match stream.write(&data[written..]) {
                    Ok(count) => written += count,
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    Err(_) => return Err(error("Не удалось передать запрос glab", mutation)),
                }
                if written == data.len() {
                    stdin = None;
                }
            }
            if !stdout_done {
                stdout_done = drain(&mut stdout, &mut captured.stdout, self.output_limit)
                    .map_err(|_| error("Не удалось прочитать ответ glab", mutation))?;
            }
            if !stderr_done {
                stderr_done = drain(&mut stderr, &mut captured.stderr, 16 * 1024)
                    .map_err(|_| error("Не удалось прочитать состояние glab", mutation))?;
            }
            if status.is_none() {
                status = running
                    .child
                    .try_wait()
                    .map_err(|_| error("Не удалось дождаться glab", mutation))?;
            }
            if let Some(status) = status.filter(|_| stdout_done && stderr_done && stdin.is_none()) {
                running.armed = false;
                if !status.success() {
                    return Err(classify_error(&captured.stderr.bytes, mutation));
                }
                return Ok(captured);
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

struct RunningChild {
    child: Child,
    armed: bool,
}
impl RunningChild {
    fn new(child: Child) -> Self {
        Self { child, armed: true }
    }
}
impl Drop for RunningChild {
    fn drop(&mut self) {
        if self.armed {
            // Потомки CLI могут удерживать pipe после завершения родителя.
            unsafe {
                libc::kill(-(self.child.id() as i32), libc::SIGKILL);
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn nonblocking(stream: &impl AsRawFd) -> io::Result<()> {
    let descriptor = stream.as_raw_fd();
    // Дескриптор принадлежит открытому pipe и остаётся живым на время fcntl.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[derive(Default)]
struct Capture {
    bytes: Vec<u8>,
    truncated: bool,
}
struct Captured {
    stdout: Capture,
    stderr: Capture,
}

fn drain(stream: &mut impl Read, capture: &mut Capture, limit: usize) -> io::Result<bool> {
    let mut buffer = [0_u8; 8192];
    // Ограничение порции оставляет время для проверки отмены при непрерывном выводе.
    for _ in 0..8 {
        match stream.read(&mut buffer) {
            Ok(0) => return Ok(true),
            Ok(count) => {
                let keep = count.min(limit.saturating_sub(capture.bytes.len()));
                capture.bytes.extend_from_slice(&buffer[..keep]);
                capture.truncated |= keep < count;
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(false)
}

fn error(message: impl Into<String>, uncertain: bool) -> ApiError {
    ApiError {
        message: message.into(),
        uncertain,
        http_status: None,
    }
}

fn classify_error(stderr: &[u8], mutation: bool) -> ApiError {
    let message = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    let status = |code: &str| {
        message.contains(&format!("http {code}"))
            || message.contains(&format!("status code: {code}"))
    };
    if status("401")
        || message.contains("401 unauthorized")
        || message.contains("not logged")
        || message.contains("auth login")
    {
        error(
            "Нет авторизации GitLab. Выполните glab auth login для выбранного сервера.",
            false,
        )
    } else if status("403") || message.contains("403 forbidden") {
        ApiError {
            http_status: Some(403),
            ..error("GitLab отклонил запрос: недостаточно прав (403)", false)
        }
    } else if status("404") || message.contains("404 not found") {
        ApiError {
            http_status: Some(404),
            ..error("Ресурс GitLab не найден или недоступен (404)", false)
        }
    } else if status("429") || message.contains("429 too many requests") {
        error(
            "Достигнут лимит запросов GitLab (429). Повторите чтение позже.",
            false,
        )
    } else if status("400")
        || status("422")
        || message.contains("400 bad request")
        || message.contains("422 unprocessable")
    {
        error(
            "GitLab отклонил переданные значения. Проверьте настройки и права.",
            false,
        )
    } else {
        error(
            "glab завершился с ошибкой. Проверьте подключение и выбранный сервер.",
            mutation,
        )
    }
}

pub fn validate_host(host: &str) -> Result<(), String> {
    let invalid = || {
        "Укажите только имя сервера и необязательный порт, например gitlab.example.org:443"
            .to_owned()
    };
    if host.is_empty()
        || host.len() > 259
        || !host.is_ascii()
        || host.starts_with('-')
        || host.contains(['/', '@', '?', '#'])
        || host.chars().any(char::is_whitespace)
    {
        return Err(invalid());
    }
    let domain = if let Some((domain, port)) = host.split_once(':') {
        if port.parse::<u16>().ok().filter(|p| *p > 0).is_none() {
            return Err(invalid());
        }
        domain
    } else {
        host
    };
    if domain.is_empty() || domain.len() > 253 {
        return Err(invalid());
    }
    if domain.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return domain
            .parse::<Ipv4Addr>()
            .map(|_| ())
            .map_err(|_| invalid());
    }
    if domain.split('.').any(|label| {
        label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    }) {
        return Err(invalid());
    }
    Ok(())
}

fn validate_endpoint(endpoint: &str) -> Result<(), String> {
    if endpoint.is_empty()
        || endpoint.len() > 8192
        || endpoint.starts_with(['-', '/'])
        || endpoint.contains("://")
        || endpoint
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
        || endpoint
            .split('?')
            .next()
            .is_some_and(|path| path.split('/').any(|part| part == ".."))
    {
        Err("Некорректный путь GitLab API".into())
    } else {
        Ok(())
    }
}

pub fn sanitize_text(text: &str) -> String {
    terminal_text(text, true)
}

fn terminal_text(text: &str, expand_tabs: bool) -> String {
    struct Output {
        text: String,
        expand_tabs: bool,
    }
    impl vte::Perform for Output {
        fn print(&mut self, character: char) {
            if !character.is_control() {
                self.text.push(character);
            }
        }
        fn execute(&mut self, byte: u8) {
            match byte {
                b'\n' => self.text.push('\n'),
                b'\t' => self
                    .text
                    .push_str(if self.expand_tabs { "    " } else { "\t" }),
                _ => (),
            }
        }
    }
    let mut output = Output {
        text: String::with_capacity(text.len()),
        expand_tabs,
    };
    vte::Parser::new().advance(&mut output, text.as_bytes());
    output.text
}

fn clean_response(response: Response) -> Response {
    match response {
        Response::Json(mut value) => {
            clean_json(&mut value);
            Response::Json(value)
        }
        Response::Text { body, truncated } => Response::Text {
            body: sanitize_text(&body),
            truncated,
        },
    }
}

fn clean_json(value: &mut Value) {
    match value {
        Value::String(text) => *text = terminal_text(text, false),
        Value::Array(values) => values.iter_mut().for_each(clean_json),
        Value::Object(values) => values.values_mut().for_each(clean_json),
        _ => (),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{fs, os::unix::fs::PermissionsExt, sync::Arc, time::SystemTime};

    struct FakeCli {
        directory: PathBuf,
        path: PathBuf,
    }
    impl FakeCli {
        fn new(body: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory =
                std::env::temp_dir().join(format!("deck-api-test-{}-{unique}", std::process::id()));
            fs::create_dir(&directory).unwrap();
            let path = directory.join("glab");
            fs::write(&path, format!("#!/usr/bin/python3\n{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self { directory, path }
        }
        fn client(&self) -> Api {
            let mut api = Api::new("gitlab.example.org", false).unwrap();
            api.program = self.path.clone();
            api.timeout = Duration::from_secs(2);
            api
        }
    }
    impl Drop for FakeCli {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn host_and_terminal_text_are_validated_at_the_boundary() {
        for host in ["gitlab.example.org", "localhost:8080", "127.0.0.1:443"] {
            assert!(validate_host(host).is_ok(), "{host}");
        }
        for host in [
            "",
            "--help",
            "https://gitlab.com",
            "user@gitlab.com",
            "x/y",
            "x\ny",
            "x:0",
            "x:65536",
            "999.999.999.999",
        ] {
            assert!(validate_host(host).is_err(), "{host}");
        }
        assert_eq!(
            sanitize_text("\x1b[31mred\x1b[0m\x1b]52;c;bad\x07\nnext\tline\0"),
            "red\nnext    line"
        );
    }

    #[test]
    fn body_uses_stdin_and_json_values_are_sanitized() {
        let fake = FakeCli::new(
            "import json, sys\nassert '--input' in sys.argv\nassert '--method' in sys.argv\nassert 'PUT' in sys.argv\nassert 'Новое описание' not in sys.argv\ndata=json.load(sys.stdin)\nassert data == {'description':'Новое описание'}\nprint(json.dumps({'nested':[{'name':'\\x1b[31mПривет\\x1b[0m','description':'строка 1\\n\\tстрока 2'}]}))",
        );
        let result = fake
            .client()
            .execute(
                &Request::put("projects/101", json!({"description":"Новое описание"})),
                &AtomicBool::new(false),
            )
            .unwrap();
        let Response::Json(value) = result else {
            panic!("expected json")
        };
        assert_eq!(value["nested"][0]["name"], "Привет");
        assert_eq!(value["nested"][0]["description"], "строка 1\n\tстрока 2");
    }

    #[test]
    fn null_bad_json_and_errors_do_not_leak_cli_output() {
        let fake = FakeCli::new("print('null')");
        assert!(matches!(
            fake.client()
                .execute(&Request::get("projects/101"), &AtomicBool::new(false))
                .unwrap(),
            Response::Json(Value::Null)
        ));
        let bad = FakeCli::new("print('not-json SECRET')");
        let error = bad
            .client()
            .execute(
                &Request::post("projects/101/jobs/9271/cancel"),
                &AtomicBool::new(false),
            )
            .unwrap_err();
        assert!(error.uncertain);
        assert!(!error.message.contains("SECRET"));
        let denied = FakeCli::new(
            "import sys\nprint('401 Unauthorized (HTTP 401) PRIVATE-TOKEN: SECRET',file=sys.stderr)\nsys.exit(1)",
        );
        let error = denied
            .client()
            .execute(&Request::get("projects"), &AtomicBool::new(false))
            .unwrap_err();
        assert!(!error.uncertain);
        assert!(error.message.contains("авторизации"));
        assert!(!error.message.contains("SECRET"));
        let failed = FakeCli::new(
            "import sys\nprint('SECRET unknown failure',file=sys.stderr)\nsys.exit(2)",
        );
        assert!(
            failed
                .client()
                .execute(
                    &Request::post("projects/101/jobs/9271/cancel"),
                    &AtomicBool::new(false)
                )
                .unwrap_err()
                .uncertain
        );
    }

    #[test]
    fn limits_both_pipes_without_deadlocking_and_marks_text_truncated() {
        let fake =
            FakeCli::new("import sys\nsys.stdout.write('a'*500000)\nsys.stderr.write('b'*500000)");
        let mut api = fake.client();
        api.output_limit = 128;
        let response = api
            .execute(
                &Request::text("projects/101/jobs/9271/trace"),
                &AtomicBool::new(false),
            )
            .unwrap();
        let Response::Text { body, truncated } = response else {
            panic!("expected text")
        };
        assert_eq!(body.len(), 128);
        assert!(truncated);
        assert!(
            api.execute(&Request::get("projects"), &AtomicBool::new(false))
                .is_err()
        );
        let binary = FakeCli::new("import sys\nsys.stdout.buffer.write(b'hello\\x00world')");
        let Response::Text { body, .. } = binary
            .client()
            .execute(
                &Request::text("projects/101/repository/blobs/123/raw"),
                &AtomicBool::new(false),
            )
            .unwrap()
        else {
            panic!("expected text")
        };
        assert!(body.starts_with("Бинарный файл"));
        assert!(!body.contains("hello"));
    }

    #[test]
    fn timeout_and_cancel_reap_cli_even_when_descendant_keeps_pipes() {
        let fake = FakeCli::new(
            "import os, time\nif os.fork() == 0:\n    time.sleep(10)\n    os._exit(0)\nos._exit(0)",
        );
        let mut api = fake.client();
        api.timeout = Duration::from_millis(80);
        let start = Instant::now();
        let error = api
            .execute(
                &Request::post("projects/101/jobs/9271/cancel"),
                &AtomicBool::new(false),
            )
            .unwrap_err();
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(error.uncertain);
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let raised = thread::spawn(move || {
            thread::sleep(Duration::from_millis(40));
            flag.store(true, Ordering::Relaxed);
        });
        api.timeout = Duration::from_secs(2);
        let start = Instant::now();
        let error = api.execute(&Request::get("projects"), &cancel).unwrap_err();
        raised.join().unwrap();
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(!error.uncertain);
        assert!(error.message.contains("отменено"));
    }

    #[test]
    fn blocked_stdin_obeys_the_same_deadline_and_requests_validate_before_spawn() {
        let fake = FakeCli::new("import time\ntime.sleep(10)");
        let mut api = fake.client();
        api.timeout = Duration::from_millis(80);
        let start = Instant::now();
        let error = api
            .execute(
                &Request::put("projects/101", json!({"description":"x".repeat(30000)})),
                &AtomicBool::new(false),
            )
            .unwrap_err();
        assert!(error.uncertain);
        assert!(start.elapsed() < Duration::from_secs(1));
        for endpoint in [
            "--help",
            "https://elsewhere.example/api",
            "/projects",
            "projects/../tokens",
            "projects\n--help",
        ] {
            assert!(
                !api.execute(&Request::get(endpoint), &AtomicBool::new(false))
                    .unwrap_err()
                    .uncertain
            );
        }
    }

    #[test]
    fn favorite_batch_keeps_server_errors_and_checks_identity() {
        let fake = FakeCli::new(
            "import json,sys\nproject_id=int(sys.argv[-1].split('/')[-1])\nprint(json.dumps({'id':project_id}))",
        );
        let Response::Json(projects) = fake
            .client()
            .execute(&Request::Projects(vec![101, 102]), &AtomicBool::new(false))
            .unwrap()
        else {
            panic!("expected JSON")
        };
        assert_eq!(projects, json!([{"id":101},{"id":102}]));
        let missing = FakeCli::new(
            "import json,sys\nproject_id=int(sys.argv[-1].split('/')[-1])\nif project_id==101:\n print('HTTP 404',file=sys.stderr)\n sys.exit(1)\nprint(json.dumps({'id':project_id}))",
        );
        let Response::Json(projects) = missing
            .client()
            .execute(&Request::Projects(vec![101, 102]), &AtomicBool::new(false))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(projects[0]["id"], 101);
        assert_eq!(projects[0]["_unavailable"], true);
        assert_eq!(projects[1]["id"], 102);
        let failed = FakeCli::new("import sys\nprint('HTTP 401',file=sys.stderr)\nsys.exit(1)");
        assert!(
            failed
                .client()
                .execute(&Request::Projects(vec![101, 102]), &AtomicBool::new(false))
                .is_err()
        );
        let wrong = FakeCli::new("print('{\"id\":999}')");
        assert!(
            wrong
                .client()
                .execute(&Request::Projects(vec![101]), &AtomicBool::new(false))
                .is_err()
        );
    }
}
