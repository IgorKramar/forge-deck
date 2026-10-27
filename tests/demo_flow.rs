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
