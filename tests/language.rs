use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use gitlab_deck::i18n::Language;
use gitlab_deck::{
    app::{App, Effect},
    model::{Popup, Request, View},
};
use serde_json::json;

#[test]
fn locale_selection_and_text_are_explicit() {
    assert_eq!(Language::from_locale("ru_RU.UTF-8"), Language::Ru);
    assert_eq!(Language::from_locale("en_GB.UTF-8"), Language::En);
    assert_eq!(Language::from_locale("C.UTF-8"), Language::En);
    assert_eq!(Language::from_locale("russian"), Language::En);
    assert_eq!(Language::En.text("Projects", "Проекты"), "Projects");
    assert_eq!(Language::Ru.text("Projects", "Проекты"), "Проекты");
    assert_eq!(Language::En.toggle(), Language::Ru);
    assert_eq!(Language::Ru.code(), "ru");
    let github_error = "GitHub rejected the request: insufficient permissions (403)";
    let russian = Language::Ru.error_message(github_error);
    assert!(russian.contains("GitHub отклонил"));
    assert_eq!(Language::En.error_message(&russian), github_error);
    assert_eq!(
        Language::Ru.error_message("GitHub user supplied text"),
        "GitHub user supplied text"
    );
    assert!(
        Language::Ru
            .error_message("Could not start gh")
            .contains("запустить gh")
    );
    let count = 7;
    assert_eq!(
        gitlab_deck::tr!(Language::En, "{count} projects", "{count} проектов"),
        "7 projects"
    );
}

#[test]
fn language_switch_preserves_content_and_confirmed_payloads() {
    let key = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let mut app = App::new("gitlab.example.org".into(), true);
    app.state.rows = vec![json!({"id":7,"name":"Проекты"})];
    app.state.text = "Projects / Проекты".into();
    let before = (app.state.rows.clone(), app.state.text.clone());
    assert!(
        app.key(key('L'))
            .iter()
            .any(|e| matches!(e, Effect::SavePreferences))
    );
    assert_eq!(app.state.language, Language::Ru);
    assert_eq!((app.state.rows.clone(), app.state.text.clone()), before);
    app.cancel();
    for language in [Language::En, Language::Ru] {
        app.state.language = language;
        app.state.context.view = View::Settings;
        app.state.context.project = Some(json!({"id":7,"path_with_namespace":"core/api"}));
        app.state.object = json!({"name":"unchanged","description":"Проекты","permissions":{"project_access":{"access_level":40}}});
        app.state.selected = 1;
        app.key(key('e'));
        assert!(
            matches!(&app.state.popup,Some(Popup::Input{title,..}) if title.contains(language.text("Description","Описание")))
        );
        app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        app.paste("New / Новое");
        app.key(enter);
        let Some(Popup::Confirm {
            title,
            body,
            request: Request::Json {
                body: Some(patch), ..
            },
        }) = &app.state.popup
        else {
            panic!("confirmation missing")
        };
        assert!(title.contains(language.text("Save", "Сохранить")));
        assert!(
            body.contains("core/api") && body.contains("Проекты") && body.contains("New / Новое")
        );
        assert_eq!(patch, &json!({"description":"New / Новое"}));
        app.key(enter);
        assert!(app.mutation_pending());
        app.key(key('L'));
        assert_eq!(app.state.language, language);
        app.cancel();
    }
}

#[test]
fn transport_errors_follow_language_without_translating_data() {
    use gitlab_deck::api::Api;
    use gitlab_deck::model::Response;
    use std::sync::atomic::AtomicBool;
    for language in [Language::En, Language::Ru] {
        let mut api = Api::new_in("gitlab.example.org", true, language).unwrap();
        let error = api
            .execute(&Request::get("projects"), &AtomicBool::new(true))
            .unwrap_err();
        assert!(error.message.contains(language.text("canceled", "отменён")));
        let Response::Json(project) = api
            .execute(&Request::get("projects/101"), &AtomicBool::new(false))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(project["name"], "api-gateway");
        api.execute(
            &Request::put("projects/101", json!({"description":"Projects / Проекты"})),
            &AtomicBool::new(false),
        )
        .unwrap();
        api.set_language(language.toggle());
        let Response::Json(project) = api
            .execute(&Request::get("projects/101"), &AtomicBool::new(false))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(project["description"], "Projects / Проекты");
    }
}
