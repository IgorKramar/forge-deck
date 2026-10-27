use gitlab_deck::i18n::Language;

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
    let count = 7;
    assert_eq!(
        gitlab_deck::tr!(Language::En, "{count} projects", "{count} проектов"),
        "7 projects"
    );
}
