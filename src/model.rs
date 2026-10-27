use crate::i18n::Language;
use serde_json::Value;
use std::collections::BTreeSet;

pub const PAGE_SIZE: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Projects,
    Groups,
    Favorites,
    Overview,
    Repository,
    File,
    MergeRequests,
    MrDetails,
    MrDiff,
    MrNotes,
    Pipelines,
    Jobs,
    Trace,
    Settings,
    Branches,
}

impl View {
    pub fn title(self) -> &'static str {
        self.title_in(Language::En)
    }

    pub fn title_in(self, language: Language) -> &'static str {
        match self {
            Self::Projects => language.text("Projects", "Проекты"),
            Self::Groups => language.text("Groups", "Группы"),
            Self::Favorites => language.text("Favorites", "Избранное"),
            Self::Overview => language.text("Overview", "Обзор"),
            Self::Repository => language.text("Repository", "Репозиторий"),
            Self::File => language.text("File", "Файл"),
            Self::MergeRequests => "Merge requests",
            Self::MrDetails => "Merge request",
            Self::MrDiff => language.text("Diff", "Дифф"),
            Self::MrNotes => language.text("Discussions", "Обсуждения"),
            Self::Pipelines => language.text("Pipelines", "Пайплайны"),
            Self::Jobs => language.text("Jobs", "Джобы"),
            Self::Trace => language.text("Job log", "Лог джобы"),
            Self::Settings => language.text("Settings", "Настройки"),
            Self::Branches => language.text("Branches", "Ветки"),
        }
    }

    pub fn is_text(self) -> bool {
        matches!(
            self,
            Self::File | Self::MrDetails | Self::MrDiff | Self::MrNotes | Self::Trace
        )
    }
}

#[derive(Clone, Debug)]
pub struct Context {
    pub view: View,
    pub project: Option<Value>,
    pub group: Option<Value>,
    pub pipeline: Option<Value>,
    pub mr: Option<Value>,
    pub job: Option<Value>,
    pub file: Option<Value>,
    pub path: String,
    pub page: u32,
    pub query: String,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            view: View::Projects,
            project: None,
            group: None,
            pipeline: None,
            mr: None,
            job: None,
            file: None,
            path: String::new(),
            page: 1,
            query: String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
}

impl Method {
    pub fn name(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Request {
    Json {
        endpoint: String,
        method: Method,
        body: Option<Value>,
    },
    Text {
        endpoint: String,
    },
    Projects(Vec<u64>),
}

impl Request {
    pub fn get(endpoint: impl Into<String>) -> Self {
        Self::Json {
            endpoint: endpoint.into(),
            method: Method::Get,
            body: None,
        }
    }
    pub fn text(endpoint: impl Into<String>) -> Self {
        Self::Text {
            endpoint: endpoint.into(),
        }
    }
    pub fn post(endpoint: impl Into<String>) -> Self {
        Self::Json {
            endpoint: endpoint.into(),
            method: Method::Post,
            body: None,
        }
    }
    pub fn put(endpoint: impl Into<String>, body: Value) -> Self {
        Self::Json {
            endpoint: endpoint.into(),
            method: Method::Put,
            body: Some(body),
        }
    }
    pub fn is_mutation(&self) -> bool {
        matches!(
            self,
            Self::Json {
                method: Method::Post | Method::Put,
                ..
            }
        )
    }
}

#[derive(Debug)]
pub enum Response {
    Json(Value),
    Text { body: String, truncated: bool },
}

#[derive(Debug, Clone)]
pub struct ApiError {
    pub message: String,
    pub uncertain: bool,
    pub http_status: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Main,
    Details,
}

#[derive(Clone, Debug)]
pub enum InputPurpose {
    Search,
    Setting(usize),
    Host,
}

#[derive(Clone, Debug)]
pub enum Popup {
    Help,
    Input {
        title: String,
        value: String,
        purpose: InputPurpose,
    },
    Confirm {
        title: String,
        body: String,
        request: Request,
    },
}

#[derive(Clone, Debug)]
pub struct State {
    pub language: crate::i18n::Language,
    pub host: String,
    pub demo: bool,
    pub context: Context,
    pub rows: Vec<Value>,
    pub object: Value,
    pub text: String,
    pub selected: usize,
    pub scroll: u16,
    pub scroll_x: u16,
    pub focus: Focus,
    pub sidebar_selected: usize,
    pub loading: bool,
    pub status: String,
    pub error: Option<String>,
    pub popup: Option<Popup>,
    pub popup_scroll: u16,
    pub favorites: BTreeSet<u64>,
    pub tick: u64,
}

impl State {
    pub fn new(host: String, demo: bool) -> Self {
        Self {
            language: crate::i18n::Language::En,
            host,
            demo,
            context: Context::default(),
            rows: Vec::new(),
            object: Value::Null,
            text: String::new(),
            selected: 0,
            scroll: 0,
            scroll_x: 0,
            focus: Focus::Main,
            sidebar_selected: 0,
            loading: false,
            status: String::new(),
            error: None,
            popup: None,
            popup_scroll: 0,
            favorites: BTreeSet::new(),
            tick: 0,
        }
    }
    pub fn selected(&self) -> Option<&Value> {
        self.rows.get(self.selected)
    }
    pub fn project_id(&self) -> Option<u64> {
        self.context.project.as_ref()?.get("id")?.as_u64()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SettingKind {
    Text { required: bool, max: usize },
    Choice(&'static [&'static str]),
    Bool,
    Number { min: u64 },
}

#[derive(Clone, Copy, Debug)]
pub struct Setting {
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub kind: SettingKind,
    pub help: &'static str,
}

pub const SETTINGS: &[Setting] = &[
    Setting {
        key: "name",
        label: "Название проекта",
        group: "Общие",
        kind: SettingKind::Text {
            required: true,
            max: 255,
        },
        help: "Понятное название проекта. Путь репозитория не меняется.",
    },
    Setting {
        key: "description",
        label: "Описание",
        group: "Общие",
        kind: SettingKind::Text {
            required: false,
            max: 4096,
        },
        help: "Описание проекта в каталоге и на странице проекта.",
    },
    Setting {
        key: "visibility",
        label: "Видимость",
        group: "Общие",
        kind: SettingKind::Choice(&["private", "internal", "public"]),
        help: "Кто может видеть проект. Изменение может открыть доступ к содержимому.",
    },
    Setting {
        key: "default_branch",
        label: "Основная ветка",
        group: "Репозиторий",
        kind: SettingKind::Text {
            required: true,
            max: 255,
        },
        help: "Существующая ветка, открываемая по умолчанию.",
    },
    Setting {
        key: "merge_method",
        label: "Способ слияния",
        group: "Merge requests",
        kind: SettingKind::Choice(&["merge", "rebase_merge", "ff"]),
        help: "merge, полулинейная история rebase_merge или fast-forward ff.",
    },
    Setting {
        key: "squash_option",
        label: "Объединение коммитов",
        group: "Merge requests",
        kind: SettingKind::Choice(&["default_off", "default_on", "never", "always"]),
        help: "Правило squash при слиянии merge request.",
    },
    Setting {
        key: "only_allow_merge_if_pipeline_succeeds",
        label: "Требовать успешный CI",
        group: "Merge requests",
        kind: SettingKind::Bool,
        help: "Не разрешать слияние до успешного пайплайна.",
    },
    Setting {
        key: "only_allow_merge_if_all_discussions_are_resolved",
        label: "Требовать закрытия обсуждений",
        group: "Merge requests",
        kind: SettingKind::Bool,
        help: "Все обсуждения должны быть разрешены перед слиянием.",
    },
    Setting {
        key: "remove_source_branch_after_merge",
        label: "Удалять исходную ветку",
        group: "Merge requests",
        kind: SettingKind::Bool,
        help: "Значение по умолчанию для удаления ветки после слияния.",
    },
    Setting {
        key: "issues_access_level",
        label: "Доступ к issues",
        group: "Возможности",
        kind: SettingKind::Choice(&["disabled", "private", "enabled"]),
        help: "disabled — выключены; private — участникам; enabled — по видимости проекта.",
    },
    Setting {
        key: "merge_requests_access_level",
        label: "Доступ к merge requests",
        group: "Возможности",
        kind: SettingKind::Choice(&["disabled", "private", "enabled"]),
        help: "Уровень доступа к merge requests проекта.",
    },
    Setting {
        key: "wiki_access_level",
        label: "Доступ к Wiki",
        group: "Возможности",
        kind: SettingKind::Choice(&["disabled", "private", "enabled"]),
        help: "Уровень доступа к Wiki проекта.",
    },
    Setting {
        key: "builds_access_level",
        label: "Доступ к CI / CD",
        group: "CI / CD",
        kind: SettingKind::Choice(&["disabled", "private", "enabled"]),
        help: "Уровень доступа к CI / CD проекта.",
    },
    Setting {
        key: "shared_runners_enabled",
        label: "Общие раннеры",
        group: "CI / CD",
        kind: SettingKind::Bool,
        help: "Разрешить общие раннеры инстанса для этого проекта.",
    },
    Setting {
        key: "build_timeout",
        label: "Тайм-аут джобы, секунды",
        group: "CI / CD",
        kind: SettingKind::Number { min: 600 },
        help: "Ограничение продолжительности джобы. Минимум 600 секунд.",
    },
];

impl Setting {
    pub fn label_in(&self, language: Language) -> &'static str {
        if language == Language::Ru {
            return self.label;
        }
        match self.key {
            "name" => "Project name",
            "description" => "Description",
            "visibility" => "Visibility",
            "default_branch" => "Default branch",
            "merge_method" => "Merge method",
            "squash_option" => "Squash commits",
            "only_allow_merge_if_pipeline_succeeds" => "Require successful CI",
            "only_allow_merge_if_all_discussions_are_resolved" => "Require resolved discussions",
            "remove_source_branch_after_merge" => "Delete source branch",
            "issues_access_level" => "Issues access",
            "merge_requests_access_level" => "Merge request access",
            "wiki_access_level" => "Wiki access",
            "builds_access_level" => "CI / CD access",
            "shared_runners_enabled" => "Shared runners",
            "build_timeout" => "Job timeout, seconds",
            _ => self.label,
        }
    }

    pub fn group_in(&self, language: Language) -> &'static str {
        let english = match self.group {
            "Общие" => "General",
            "Репозиторий" => "Repository",
            "Возможности" => "Features",
            other => other,
        };
        language.text(english, self.group)
    }

    pub fn help_in(&self, language: Language) -> &'static str {
        if language == Language::Ru {
            return self.help;
        }
        match self.key {
            "name" => "Human-readable project name. The repository path stays unchanged.",
            "description" => "Project description shown in the catalog and project overview.",
            "visibility" => "Who can see the project. Changing this may expose its contents.",
            "default_branch" => "Existing branch opened by default.",
            "merge_method" => "merge, semi-linear rebase_merge, or fast-forward ff.",
            "squash_option" => "Squash policy when merging a merge request.",
            "only_allow_merge_if_pipeline_succeeds" => {
                "Require a successful pipeline before merging."
            }
            "only_allow_merge_if_all_discussions_are_resolved" => {
                "All discussions must be resolved before merging."
            }
            "remove_source_branch_after_merge" => {
                "Default choice for deleting the source branch after merging."
            }
            "issues_access_level" => {
                "disabled — off; private — members; enabled — follows project visibility."
            }
            "merge_requests_access_level" => "Access level for project merge requests.",
            "wiki_access_level" => "Access level for the project Wiki.",
            "builds_access_level" => "Access level for project CI / CD.",
            "shared_runners_enabled" => "Allow instance runners for this project.",
            "build_timeout" => "Maximum job duration. At least 600 seconds.",
            _ => self.help,
        }
    }

    pub fn parse(&self, input: &str) -> Result<Value, String> {
        self.parse_in(input, Language::En)
    }

    pub fn parse_in(&self, input: &str, language: Language) -> Result<Value, String> {
        if input
            .chars()
            .any(|c| c.is_control() && !(self.key == "description" && matches!(c, '\n' | '\t')))
        {
            return Err(language
                .text(
                    "Control characters are not allowed",
                    "Управляющие символы недопустимы",
                )
                .into());
        }
        match self.kind {
            SettingKind::Text { required, max } => {
                if required && input.trim().is_empty() {
                    return Err(language
                        .text("The value cannot be empty", "Значение не может быть пустым")
                        .into());
                }
                if input.chars().count() > max {
                    return Err(crate::tr!(
                        language,
                        "At most {max} characters",
                        "Не более {max} символов"
                    ));
                }
                Ok(Value::String(input.into()))
            }
            SettingKind::Choice(values) if values.contains(&input) => {
                Ok(Value::String(input.into()))
            }
            SettingKind::Choice(values) => Err(crate::tr!(
                language,
                "Allowed values: {}",
                "Допустимо: {}",
                values.join(", ")
            )),
            SettingKind::Bool => input.parse::<bool>().map(Value::Bool).map_err(|_| {
                language
                    .text("Enter true or false", "Введите true или false")
                    .into()
            }),
            SettingKind::Number { min } => input
                .parse::<u64>()
                .ok()
                .filter(|n| *n >= min && *n <= i32::MAX as u64)
                .map(Value::from)
                .ok_or_else(|| {
                    crate::tr!(
                        language,
                        "An integer from {min} to {}",
                        "Целое число от {min} до {}",
                        i32::MAX
                    )
                }),
        }
    }
}

pub fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("—")
}
pub fn number(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}
pub fn value_label(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "—".into(),
        other => other.to_string(),
    }
}
pub fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_setting_boundaries_and_encodes_paths() {
        assert!(SETTINGS[0].parse(" ").is_err());
        assert_eq!(SETTINGS[1].parse("Описание").unwrap(), "Описание");
        assert!(SETTINGS[2].parse("everyone").is_err());
        assert!(SETTINGS.last().unwrap().parse("599").is_err());
        assert_eq!(SETTINGS.last().unwrap().parse("600").unwrap(), 600);
        assert!(SETTINGS[1].parse("x\u{1b}]0;bad").is_err());
        assert_eq!(
            encode("group/проект &?"),
            "group%2F%D0%BF%D1%80%D0%BE%D0%B5%D0%BA%D1%82%20%26%3F"
        );
    }

    #[test]
    fn titles_and_settings_have_both_languages_without_changing_values() {
        assert_eq!(View::Projects.title(), "Projects");
        assert_eq!(View::Projects.title_in(Language::Ru), "Проекты");
        for setting in SETTINGS {
            for text in [
                setting.label_in(Language::En),
                setting.group_in(Language::En),
                setting.help_in(Language::En),
            ] {
                assert!(
                    !text
                        .chars()
                        .any(|c| ('А'..='я').contains(&c) || matches!(c, 'Ё' | 'ё'))
                );
            }
            assert_eq!(setting.label_in(Language::Ru), setting.label);
            assert_eq!(setting.help_in(Language::Ru), setting.help);
        }
        assert_eq!(
            SETTINGS[1].parse_in("Моё описание", Language::En).unwrap(),
            "Моё описание"
        );
        assert_eq!(
            SETTINGS[2].parse_in("private", Language::En).unwrap(),
            "private"
        );
        assert!(
            SETTINGS[0]
                .parse_in("", Language::En)
                .unwrap_err()
                .contains("empty")
        );
        assert!(
            SETTINGS[0]
                .parse_in("", Language::Ru)
                .unwrap_err()
                .contains("пустым")
        );
        assert!(
            SETTINGS[1]
                .parse_in("x\u{1b}", Language::En)
                .unwrap_err()
                .contains("Control")
        );
        assert!(
            SETTINGS[2]
                .parse_in("invalid", Language::En)
                .unwrap_err()
                .contains("Allowed")
        );
        assert!(
            SETTINGS[6]
                .parse_in("invalid", Language::En)
                .unwrap_err()
                .contains("true or false")
        );
        assert!(
            SETTINGS
                .last()
                .unwrap()
                .parse_in("1", Language::En)
                .unwrap_err()
                .contains("integer")
        );
    }
}
