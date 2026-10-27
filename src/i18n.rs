#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Language {
    #[default]
    En,
    Ru,
}

impl Language {
    pub fn text<'a>(self, en: &'a str, ru: &'a str) -> &'a str {
        match self {
            Self::En => en,
            Self::Ru => ru,
        }
    }

    pub fn code(self) -> &'static str {
        self.text("en", "ru")
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "en" => Some(Self::En),
            "ru" => Some(Self::Ru),
            _ => None,
        }
    }

    pub fn from_locale(locale: &str) -> Self {
        let language = locale.split(['_', '-', '.', '@']).next().unwrap_or("");
        if language.eq_ignore_ascii_case("ru") {
            Self::Ru
        } else {
            Self::En
        }
    }

    pub fn from_environment() -> Self {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .filter_map(|key| std::env::var(key).ok())
            .find(|value| !value.is_empty())
            .map(|locale| Self::from_locale(&locale))
            .unwrap_or_default()
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::En => Self::Ru,
            Self::Ru => Self::En,
        }
    }
}

#[macro_export]
macro_rules! tr {
    ($language:expr, $en:literal, $ru:literal $(, $argument:expr)* $(,)?) => {
        match $language {
            $crate::i18n::Language::En => format!($en $(, $argument)*),
            $crate::i18n::Language::Ru => format!($ru $(, $argument)*),
        }
    };
}
