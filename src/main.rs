use clap::Parser;
use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind},
    execute,
};
use gitlab_deck::{
    api::Api,
    app::{App, Effect},
    config::Preferences,
    i18n::Language,
    model::{ApiError, Request, Response},
    provider::Provider,
    ui,
};
use std::{
    io,
    sync::{Arc, atomic::AtomicBool, mpsc},
    thread,
    time::Duration,
};

#[derive(Parser)]
#[command(
    version,
    about = "Forge Deck — projects, code, and CI in your terminal"
)]
struct Args {
    #[arg(long, help = "Server hostname[:port], without scheme or path")]
    host: Option<String>,
    #[arg(
        long,
        value_enum,
        help = "Forge provider (gitlab or github); press G in the TUI to switch"
    )]
    provider: Option<Provider>,
    #[arg(
        long,
        value_enum,
        help = "Interface language (en or ru); press L in the TUI to switch"
    )]
    lang: Option<Language>,
    #[arg(long, help = "Demo data without network access or credentials")]
    demo: bool,
    #[arg(
        long,
        help = "Check the connection by reading the first project page, then exit"
    )]
    check: bool,
}

struct Work {
    id: u64,
    host: String,
    demo: bool,
    language: Language,
    provider: Provider,
    request: Request,
    cancel: Arc<AtomicBool>,
}

fn connection_host(preferences: &Preferences, provider: Provider) -> String {
    preferences
        .host_for(provider)
        .or_else(|| {
            std::env::var(if provider == Provider::Github {
                "GH_HOST"
            } else {
                "GITLAB_HOST"
            })
            .ok()
        })
        .unwrap_or_else(|| provider.default_host().into())
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
                    language: app.state.language,
                    provider: app.state.provider,
                    request,
                    cancel,
                };
                if sender.send(work).is_err() {
                    app.cancel();
                    app.state.error = Some(
                        app.state
                            .language
                            .text(
                                "The request worker has stopped",
                                "Обработчик запросов завершился",
                            )
                            .into(),
                    );
                }
            }
            Effect::SwitchProvider => {
                let provider = app.state.provider.toggle();
                let host = if app.state.demo {
                    provider.demo_host().into()
                } else {
                    connection_host(preferences, provider)
                };
                let next = app.connect(provider, host);
                if effects(app, next, sender, preferences) {
                    return true;
                }
            }
            Effect::HostChanged => {
                app.state.favorites = if app.state.demo {
                    Default::default()
                } else {
                    preferences.favorites_for(app.state.provider, &app.state.host)
                };
                if !app.state.demo
                    && let Err(error) = {
                        preferences.set_language(app.state.language);
                        preferences.save_for(
                            app.state.provider,
                            &app.state.host,
                            &app.state.favorites,
                        )
                    }
                {
                    app.state.error = Some(error);
                }
            }
            Effect::SavePreferences => {
                if !app.state.demo
                    && let Err(error) = {
                        preferences.set_language(app.state.language);
                        preferences.save_for(
                            app.state.provider,
                            &app.state.host,
                            &app.state.favorites,
                        )
                    }
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
        let mut client: Option<(String, bool, Provider, Api)> = None;
        while let Ok(work) = receive_work.recv() {
            if work.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                continue;
            }
            if client.as_ref().is_none_or(|(host, demo, provider, _)| {
                host != &work.host || *demo != work.demo || *provider != work.provider
            }) {
                match Api::new_for(&work.host, work.demo, work.language, work.provider) {
                    Ok(api) => client = Some((work.host.clone(), work.demo, work.provider, api)),
                    Err(error) => {
                        if send_result.send((work.id, Err(error))).is_err() {
                            break;
                        }
                        continue;
                    }
                }
            }
            if let Some((_, _, _, api)) = &mut client {
                api.set_language(work.language);
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
        return Err(io::Error::other(app.state.language.text(
            "The request worker crashed",
            "Обработчик запросов аварийно завершился",
        )));
    }
    if unknown_on_exit {
        eprintln!("{}", app.state.language.text(
            "A change request was sent. Exiting does not cancel it on the server: verify the result on the server before retrying.",
            "Запрос изменения был отправлен. Выход не отменяет его на сервере: проверьте результат на сервере перед повтором."
        ));
    }
    result
}

fn main() {
    let args = Args::parse();
    let initial_language = args.lang.unwrap_or_else(Language::from_environment);
    let mut error_language = initial_language;
    let result = (|| -> Result<(), String> {
        let mut preferences = Preferences::load_in(initial_language)?;
        let language = args
            .lang
            .or_else(|| preferences.language())
            .unwrap_or(initial_language);
        error_language = language;
        let provider = args.provider.unwrap_or_else(|| preferences.provider());
        let host = args.host.unwrap_or_else(|| {
            if args.demo {
                provider.demo_host().into()
            } else {
                connection_host(&preferences, provider)
            }
        });
        let mut app = App::new(host.clone(), args.demo);
        app.state.provider = provider;
        app.state.language = language;
        gitlab_deck::api::validate_host(&host)
            .map_err(|message| language.error_message(&message))?;
        if args.check {
            let mut api =
                Api::new_for(&host, args.demo, language, provider).map_err(|e| e.message)?;
            let response = api
                .execute(&app.request()?, &AtomicBool::new(false))
                .map_err(|e| e.message)?;
            let Response::Json(value) = response else {
                return Err(language
                    .text(
                        "Expected a JSON project list",
                        "Ожидался JSON-список проектов",
                    )
                    .into());
            };
            let count = value
                .as_array()
                .ok_or(language.text("Expected a project list", "Ожидался список проектов"))?
                .len();
            println!(
                "{}",
                gitlab_deck::tr!(
                    language,
                    "Forge Deck {} · {} · {}\nConnection verified: {count} projects on the first page.",
                    "Forge Deck {} · {} · {}\nСоединение проверено: {count} проектов на первой странице.",
                    env!("CARGO_PKG_VERSION"),
                    host,
                    if args.demo {
                        language.text("DEMO", "ДЕМО")
                    } else {
                        provider.name()
                    }
                )
            );
            return Ok(());
        }
        if !args.demo {
            app.state.favorites = preferences.favorites_for(app.state.provider, &app.state.host);
            {
                preferences.set_language(app.state.language);
                preferences.save_for(app.state.provider, &app.state.host, &app.state.favorites)
            }?;
        }
        run(&mut app, &mut preferences).map_err(|e| e.to_string())
    })();
    if let Err(error) = result {
        eprintln!("Forge Deck: {}", error_language.error_message(&error));
        std::process::exit(1);
    }
}
