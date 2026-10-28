use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use gitlab_deck::{
    api::Api,
    app::{App, Effect},
    i18n::Language,
    model::{Popup, View},
};

fn settle(app: &mut App, api: &mut Api, effects: Vec<Effect>) {
    let mut queue = std::collections::VecDeque::from(effects);
    while let Some(effect) = queue.pop_front() {
        if let Effect::Fetch {
            id,
            request,
            cancel,
        } = effect
        {
            let response = api.execute(&request, &cancel);
            queue.extend(app.accept(id, response));
        }
    }
    assert!(!app.state.loading);
    assert!(app.state.error.is_none(), "{:?}", app.state.error);
}

fn press(app: &mut App, api: &mut Api, key: KeyCode) {
    let effects = app.key(KeyEvent::new(key, KeyModifiers::NONE));
    settle(app, api, effects);
}

#[test]
fn complete_demo_navigation_and_confirmed_changes() {
    for language in [Language::En, Language::Ru] {
        let mut app = App::new("gitlab.example.org".into(), true);
        app.state.language = language;
        let mut api = Api::new_in(&app.state.host, true, language).unwrap();
        let initial = app.refresh();
        settle(&mut app, &mut api, initial);
        assert_eq!(app.state.rows.len(), 7);
        press(&mut app, &mut api, KeyCode::Enter);
        assert_eq!(app.state.context.view, View::Overview);
        press(&mut app, &mut api, KeyCode::Char('p'));
        press(&mut app, &mut api, KeyCode::Enter);
        assert_eq!(app.state.context.view, View::Jobs);
        press(&mut app, &mut api, KeyCode::Enter);
        assert_eq!(app.state.context.view, View::Trace);
        assert!(!app.state.text.is_empty());
        press(&mut app, &mut api, KeyCode::Esc);
        let manual = app
            .state
            .rows
            .iter()
            .position(|r| r["status"] == "manual")
            .unwrap();
        app.state.selected = manual;
        press(&mut app, &mut api, KeyCode::Char('S'));
        assert!(matches!(app.state.popup, Some(Popup::Confirm { .. })));
        press(&mut app, &mut api, KeyCode::Esc);
        assert_eq!(app.state.rows[manual]["status"], "manual");
        press(&mut app, &mut api, KeyCode::Char('S'));
        press(&mut app, &mut api, KeyCode::Char('y'));
        assert_eq!(app.state.rows[manual]["status"], "pending");

        press(&mut app, &mut api, KeyCode::Char('m'));
        press(&mut app, &mut api, KeyCode::Enter);
        assert_eq!(app.state.context.view, View::MrDetails);
        press(&mut app, &mut api, KeyCode::Char('d'));
        assert!(app.state.text.contains("diff --git"));
        press(&mut app, &mut api, KeyCode::Char('c'));
        assert_eq!(app.state.context.view, View::MrNotes);
        assert!(app.state.text.contains("Igor Kramar"));
        press(&mut app, &mut api, KeyCode::Char('f'));
        press(&mut app, &mut api, KeyCode::Enter);
        assert_eq!(app.state.context.path, "src");
        press(&mut app, &mut api, KeyCode::Enter);
        assert_eq!(app.state.context.view, View::File);
        assert!(app.state.text.contains("pub fn health"));

        press(&mut app, &mut api, KeyCode::Char('s'));
        let original = app.state.object.clone();
        app.state.selected = 1;
        press(&mut app, &mut api, KeyCode::Char('e'));
        let effects = app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        settle(&mut app, &mut api, effects);
        app.paste("Новая строка\nи ещё одна\tстрока");
        press(&mut app, &mut api, KeyCode::Enter);
        press(&mut app, &mut api, KeyCode::Esc);
        assert_eq!(app.state.object, original);
        press(&mut app, &mut api, KeyCode::Char('e'));
        let effects = app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        settle(&mut app, &mut api, effects);
        app.paste("Новая строка\nи ещё одна\tстрока");
        press(&mut app, &mut api, KeyCode::Enter);
        press(&mut app, &mut api, KeyCode::Char('y'));
        let mut expected = original;
        expected["description"] = "Новая строка\nи ещё одна\tстрока".into();
        assert_eq!(app.state.object, expected);

        press(&mut app, &mut api, KeyCode::Char('2'));
        assert_eq!(app.state.rows.len(), 2);
        press(&mut app, &mut api, KeyCode::Enter);
        assert_eq!(app.state.context.view, View::Projects);
        assert_eq!(app.state.rows.len(), 4);
    }
}

#[test]
fn github_navigation_settings_and_ci_follow_native_contracts() {
    use gitlab_deck::{
        model::{Method, Request, Response},
        provider::Provider,
    };
    for language in [Language::En, Language::Ru] {
        let mut app = App::new("github.example.org".into(), true);
        app.state.provider = Provider::Github;
        app.state.language = language;
        let mut api = Api::new_for(&app.state.host, true, language, Provider::Github).unwrap();
        let initial = app.refresh();
        settle(&mut app, &mut api, initial);
        assert_eq!(app.state.rows.len(), 3);
        press(&mut app, &mut api, KeyCode::Enter);
        press(&mut app, &mut api, KeyCode::Char('p'));
        press(&mut app, &mut api, KeyCode::Enter);
        press(&mut app, &mut api, KeyCode::Enter);
        assert_eq!(app.state.context.view, View::Trace);
        assert!(app.state.text.contains("cargo test"));
        app.key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert!(app.state.error.is_some());
        assert!(app.state.popup.is_none());
        app.key(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::NONE));
        assert!(app.state.popup.is_none());
        press(&mut app, &mut api, KeyCode::Char('r'));
        assert!(matches!(app.state.popup, Some(Popup::Confirm { .. })));
        press(&mut app, &mut api, KeyCode::Char('y'));
        assert_eq!(app.state.context.job.as_ref().unwrap()["id"], 9270);
        press(&mut app, &mut api, KeyCode::Char('m'));
        press(&mut app, &mut api, KeyCode::Enter);
        assert!(app.state.text.starts_with("#42"));
        press(&mut app, &mut api, KeyCode::Char('d'));
        assert!(app.state.text.contains("diff --git"));
        press(&mut app, &mut api, KeyCode::Char('c'));
        assert!(!app.state.text.is_empty());
        press(&mut app, &mut api, KeyCode::Char('f'));
        press(&mut app, &mut api, KeyCode::Enter);
        assert_eq!(app.state.context.view, View::File);
        assert!(app.state.text.contains("repository") || app.state.text.contains("репозиторий"));
        press(&mut app, &mut api, KeyCode::Char('b'));
        assert_eq!(app.state.rows.len(), 2);
        press(&mut app, &mut api, KeyCode::Char('s'));
        assert_eq!(app.state.settings().len(), 8);
        app.state.object["permissions"]["admin"] = false.into();
        app.key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        assert!(app.state.popup.is_none());
        assert!(app.state.error.is_some());
        app.state.object["permissions"]["admin"] = true.into();
        app.state.error = None;
        app.state.selected = 0;
        press(&mut app, &mut api, KeyCode::Char('e'));
        app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        app.paste("Текст GitHub — без перевода");
        press(&mut app, &mut api, KeyCode::Enter);
        let Some(Popup::Confirm {
            request:
                Request::Json {
                    method: Method::Patch,
                    endpoint,
                    body: Some(body),
                },
            ..
        }) = &app.state.popup
        else {
            panic!("expected native PATCH confirmation")
        };
        assert_eq!(endpoint, "repos/demo/api-gateway");
        assert_eq!(
            body,
            &serde_json::json!({"description":"Текст GitHub — без перевода"})
        );
        let pending = app.key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        assert!(app.mutation_pending());
        assert!(
            app.key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE))
                .is_empty()
        );
        assert_eq!(app.state.provider, Provider::Github);
        settle(&mut app, &mut api, pending);
        assert_eq!(
            app.state.object["description"],
            "Текст GitHub — без перевода"
        );
        api.set_language(language.toggle());
        press(&mut app, &mut api, KeyCode::Char('L'));
        assert_eq!(
            app.state.object["description"],
            "Текст GitHub — без перевода"
        );
        press(&mut app, &mut api, KeyCode::Char('2'));
        assert_eq!(app.state.rows.len(), 1);
        press(&mut app, &mut api, KeyCode::Enter);
        assert_eq!(app.state.rows.len(), 3);
        press(&mut app, &mut api, KeyCode::Char('3'));
        assert_eq!(app.state.selected().unwrap()["_project"]["id"], 0);
        press(&mut app, &mut api, KeyCode::Enter);
        assert_eq!(
            app.state.context.project.as_ref().unwrap()["full_name"],
            "demo/api-gateway"
        );
        assert_eq!(app.state.context.view, View::MrDetails);
        assert_eq!(app.state.project_id(), Some(101));
        press(&mut app, &mut api, KeyCode::Char('*'));
        assert!(app.state.favorites.contains(&101));
        press(&mut app, &mut api, KeyCode::Char('p'));
        press(&mut app, &mut api, KeyCode::Char('*'));
        assert!(!app.state.favorites.contains(&101));
        press(&mut app, &mut api, KeyCode::Char('f'));
        press(&mut app, &mut api, KeyCode::Char('*'));
        assert!(app.state.favorites.contains(&101));
        let old = app.refresh();
        let old_id = old
            .iter()
            .find_map(|e| {
                if let Effect::Fetch { id, .. } = e {
                    Some(*id)
                } else {
                    None
                }
            })
            .unwrap();
        let next = app.connect(Provider::Gitlab, "gitlab.example.org".into());
        assert!(!next.is_empty());
        app.accept(old_id, Ok(Response::Json(serde_json::json!({"id":999}))));
        assert_eq!(app.state.provider, Provider::Gitlab);
        assert!(app.state.object.is_null());
    }
}
