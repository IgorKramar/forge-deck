use clap::Parser;
use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind},
    execute,
};
use gitlab_deck::{
    api::Api,
    app::{App, Effect},
    config::Preferences,
    model::{ApiError, Request, Response},
    ui,
};
use std::{
    io,
    sync::{Arc, atomic::AtomicBool, mpsc},
    thread,
    time::Duration,
};

#[derive(Parser)]
#[command(version, about = "GitLab Deck — проекты, код и CI в терминале")]
struct Args {
    #[arg(long, help = "GitLab hostname[:port], например gitlab.example.org")]
    host: Option<String>,
    #[arg(long, help = "Демонстрационные данные без сети и учётных данных")]
    demo: bool,
    #[arg(
        long,
        help = "Проверить подключение чтением первой страницы проектов и выйти"
    )]
    check: bool,
}

struct Work {
    id: u64,
    host: String,
    demo: bool,
    request: Request,
    cancel: Arc<AtomicBool>,
}

fn effects(
    app: &mut App,
    list: Vec<Effect>,
    sender: &mpsc::Sender<Work>,
    preferences: &mut Preferences,
) -> bool {
    for effect in list {
        match effect {
            Effect::Exit => return true,
            Effect::Fetch {
                id,
                request,
                cancel,
            } => {
                let work = Work {
                    id,
                    host: app.state.host.clone(),
                    demo: app.state.demo,
                    request,
                    cancel,
                };
                if sender.send(work).is_err() {
                    app.cancel();
                    app.state.error = Some("Обработчик запросов завершился".into());
                }
            }
            Effect::HostChanged => {
                app.state.favorites = if app.state.demo {
                    Default::default()
                } else {
                    preferences.favorites(&app.state.host)
                };
                if !app.state.demo
                    && let Err(error) = preferences.save(&app.state.host, &app.state.favorites)
                {
                    app.state.error = Some(error);
                }
            }
            Effect::SavePreferences => {
                if !app.state.demo
                    && let Err(error) = preferences.save(&app.state.host, &app.state.favorites)
                {
                    app.state.error = Some(error);
                }
            }
        }
    }
    false
}

fn run(app: &mut App, preferences: &mut Preferences) -> io::Result<()> {
    let (send_work, receive_work) = mpsc::channel::<Work>();
    let (send_result, receive_result) = mpsc::channel::<(u64, Result<Response, ApiError>)>();
    let worker = thread::spawn(move || {
        let mut client: Option<(String, bool, Api)> = None;
        while let Ok(work) = receive_work.recv() {
            if work.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                continue;
            }
            if client
                .as_ref()
                .is_none_or(|(host, demo, _)| host != &work.host || *demo != work.demo)
            {
                match Api::new(&work.host, work.demo) {
                    Ok(api) => client = Some((work.host.clone(), work.demo, api)),
                    Err(error) => {
                        if send_result.send((work.id, Err(error))).is_err() {
                            break;
                        }
                        continue;
                    }
                }
            }
            if let Some((_, _, api)) = &mut client {
                let result = api.execute(&work.request, &work.cancel);
                if send_result.send((work.id, result)).is_err() {
                    break;
                }
            }
        }
    });
    let mut unknown_on_exit = false;
    let result = ratatui::run(|terminal| {
        execute!(io::stdout(), EnableBracketedPaste)?;
        let initial = app.refresh();
        effects(app, initial, &send_work, preferences);
        let mut redraw = true;
        loop {
            while let Ok((id, result)) = receive_result.try_recv() {
                let next = app.accept(id, result);
                effects(app, next, &send_work, preferences);
                redraw = true;
            }
            if redraw || app.state.loading {
                app.state.tick = app.state.tick.wrapping_add(1);
                terminal.draw(|frame| ui::draw(frame, &app.state))?;
                redraw = false;
            }
            if event::poll(Duration::from_millis(if app.state.loading {
                100
            } else {
                250
            }))? {
                redraw = true;
                let next = match event::read()? {
                    Event::Key(key) if key.kind != KeyEventKind::Release => app.key(key),
                    Event::Paste(text) => {
                        app.paste(&text);
                        Vec::new()
                    }
                    _ => Vec::new(),
                };
                if effects(app, next, &send_work, preferences) {
                    unknown_on_exit = app.mutation_pending();
                    break;
                }
            }
        }
        Ok(())
    });
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    unknown_on_exit |= app.mutation_pending();
    app.cancel();
    drop(send_work);
    if worker.join().is_err() && result.is_ok() {
        return Err(io::Error::other("Обработчик запросов аварийно завершился"));
    }
    if unknown_on_exit {
        eprintln!(
            "Запрос изменения был отправлен. Выход не отменяет его на сервере: проверьте результат в GitLab перед повтором."
        );
    }
    result
}

fn main() {
    let args = Args::parse();
    let result = (|| -> Result<(), String> {
        let mut preferences = Preferences::load()?;
        let host = args.host.unwrap_or_else(|| {
            if args.demo {
                "gitlab.example.org".into()
            } else {
                preferences
                    .host()
                    .or_else(|| std::env::var("GITLAB_HOST").ok())
                    .unwrap_or_else(|| "gitlab.com".into())
            }
        });
        gitlab_deck::api::validate_host(&host)?;
        if args.check {
            let mut api = Api::new(&host, args.demo).map_err(|e| e.message)?;
            let response = api
                .execute(
                    &Request::get("projects?simple=true&per_page=50&page=1"),
                    &AtomicBool::new(false),
                )
                .map_err(|e| e.message)?;
            let Response::Json(value) = response else {
                return Err("Ожидался JSON-список проектов".into());
            };
            let count = value.as_array().ok_or("Ожидался список проектов")?.len();
            println!(
                "GitLab Deck {} · {} · {}\nСоединение проверено: {count} проектов на первой странице.",
                env!("CARGO_PKG_VERSION"),
                host,
                if args.demo { "ДЕМО" } else { "GitLab" }
            );
            return Ok(());
        }
        let mut app = App::new(host, args.demo);
        if !args.demo {
            app.state.favorites = preferences.favorites(&app.state.host);
            preferences.save(&app.state.host, &app.state.favorites)?;
        }
        run(&mut app, &mut preferences).map_err(|e| e.to_string())
    })();
    if let Err(error) = result {
        eprintln!("GitLab Deck: {error}");
        std::process::exit(1);
    }
}
