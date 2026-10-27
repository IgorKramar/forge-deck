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

impl Language {
    pub fn error_message(self, message: &str) -> String {
        match message {
            "Запрос отменён" | "Request canceled" => self.text("Request canceled", "Запрос отменён").into(),
            "Запрос отменён до отправки" | "Request canceled before sending" => self.text("Request canceled before sending", "Запрос отменён до отправки").into(),
            "GitLab вернул неверный проект для избранного" | "GitLab returned the wrong project for favorites" => self.text("GitLab returned the wrong project for favorites", "GitLab вернул неверный проект для избранного").into(),
            "Ответ GitLab превышает 2 МиБ. Сузьте поиск или откройте отдельный ресурс." | "GitLab response exceeds 2 MiB. Narrow the search or open a single resource." => self.text("GitLab response exceeds 2 MiB. Narrow the search or open a single resource.", "Ответ GitLab превышает 2 МиБ. Сузьте поиск или откройте отдельный ресурс.").into(),
            "GitLab вернул некорректный JSON" | "GitLab returned invalid JSON" => self.text("GitLab returned invalid JSON", "GitLab вернул некорректный JSON").into(),
            "Не удалось подготовить JSON запроса" | "Could not prepare the JSON request" => self.text("Could not prepare the JSON request", "Не удалось подготовить JSON запроса").into(),
            "Тело запроса слишком велико" | "Request body is too large" => self.text("Request body is too large", "Тело запроса слишком велико").into(),
            "Не найден glab. Установите GitLab CLI и выполните glab auth login для выбранного сервера." | "glab was not found. Install GitLab CLI and run glab auth login for the selected server." => self.text("glab was not found. Install GitLab CLI and run glab auth login for the selected server.", "Не найден glab. Установите GitLab CLI и выполните glab auth login для выбранного сервера.").into(),
            "Не удалось запустить glab" | "Could not start glab" => self.text("Could not start glab", "Не удалось запустить glab").into(),
            "Не открыт stdout glab" | "Could not open glab stdout" => self.text("Could not open glab stdout", "Не открыт stdout glab").into(),
            "Не открыт stderr glab" | "Could not open glab stderr" => self.text("Could not open glab stderr", "Не открыт stderr glab").into(),
            "Не удалось настроить неблокирующий обмен с glab" | "Could not configure nonblocking communication with glab" => self.text("Could not configure nonblocking communication with glab", "Не удалось настроить неблокирующий обмен с glab").into(),
            "Ожидание ответа отменено" | "Waiting for the response was canceled" => self.text("Waiting for the response was canceled", "Ожидание ответа отменено").into(),
            "GitLab не ответил за отведённое время" | "GitLab did not respond before the timeout" => self.text("GitLab did not respond before the timeout", "GitLab не ответил за отведённое время").into(),
            "Не удалось передать запрос glab" | "Could not send the request to glab" => self.text("Could not send the request to glab", "Не удалось передать запрос glab").into(),
            "Не удалось прочитать ответ glab" | "Could not read the glab response" => self.text("Could not read the glab response", "Не удалось прочитать ответ glab").into(),
            "Не удалось прочитать состояние glab" | "Could not read glab status" => self.text("Could not read glab status", "Не удалось прочитать состояние glab").into(),
            "Не удалось дождаться glab" | "Could not wait for glab" => self.text("Could not wait for glab", "Не удалось дождаться glab").into(),
            "Нет авторизации GitLab. Выполните glab auth login для выбранного сервера." | "Not authenticated with GitLab. Run glab auth login for the selected server." => self.text("Not authenticated with GitLab. Run glab auth login for the selected server.", "Нет авторизации GitLab. Выполните glab auth login для выбранного сервера.").into(),
            "GitLab отклонил запрос: недостаточно прав (403)" | "GitLab rejected the request: insufficient permissions (403)" => self.text("GitLab rejected the request: insufficient permissions (403)", "GitLab отклонил запрос: недостаточно прав (403)").into(),
            "Ресурс GitLab не найден или недоступен (404)" | "GitLab resource was not found or is unavailable (404)" => self.text("GitLab resource was not found or is unavailable (404)", "Ресурс GitLab не найден или недоступен (404)").into(),
            "Достигнут лимит запросов GitLab (429). Повторите чтение позже." | "GitLab rate limit reached (429). Retry reading later." => self.text("GitLab rate limit reached (429). Retry reading later.", "Достигнут лимит запросов GitLab (429). Повторите чтение позже.").into(),
            "GitLab отклонил переданные значения. Проверьте настройки и права." | "GitLab rejected the values. Check settings and permissions." => self.text("GitLab rejected the values. Check settings and permissions.", "GitLab отклонил переданные значения. Проверьте настройки и права.").into(),
            "glab завершился с ошибкой. Проверьте подключение и выбранный сервер." | "glab failed. Check the connection and selected server." => self.text("glab failed. Check the connection and selected server.", "glab завершился с ошибкой. Проверьте подключение и выбранный сервер.").into(),
            "Укажите только имя сервера и необязательный порт, например gitlab.example.org:443" | "Enter only a hostname and optional port, for example gitlab.example.org:443" => self.text("Enter only a hostname and optional port, for example gitlab.example.org:443", "Укажите только имя сервера и необязательный порт, например gitlab.example.org:443").into(),
            "Некорректный путь GitLab API" | "Invalid GitLab API path" => self.text("Invalid GitLab API path", "Некорректный путь GitLab API").into(),
            "Изменяется ровно одно поле проекта" | "Exactly one project field must be changed" => self.text("Exactly one project field must be changed", "Изменяется ровно одно поле проекта").into(),
            "Это поле недоступно для редактирования" | "This field is not available for editing" => self.text("This field is not available for editing", "Это поле недоступно для редактирования").into(),
            "Недопустимое значение настройки" | "Invalid setting value" => self.text("Invalid setting value", "Недопустимое значение настройки").into(),
            "Неверный JSON-тип настройки" | "Invalid JSON type for the setting" => self.text("Invalid JSON type for the setting", "Неверный JSON-тип настройки").into(),
            "Джоба не ожидает ручного запуска" | "The job is not waiting for a manual start" => self.text("The job is not waiting for a manual start", "Джоба не ожидает ручного запуска").into(),
            "Демонстрационный ресурс не найден (404)" | "Demo resource was not found (404)" => self.text("Demo resource was not found (404)", "Демонстрационный ресурс не найден (404)").into(),
            "Некорректный URL" | "Invalid URL" => self.text("Invalid URL", "Некорректный URL").into(),
            "Некорректный UTF-8 URL" | "Invalid UTF-8 URL" => self.text("Invalid UTF-8 URL", "Некорректный UTF-8 URL").into(),
            "Некорректная страница" | "Invalid page" => self.text("Invalid page", "Некорректная страница").into(),
            "Некорректный размер страницы" | "Invalid page size" => self.text("Invalid page size", "Некорректный размер страницы").into(),
            "Некорректная пагинация" | "Invalid pagination" => self.text("Invalid pagination", "Некорректная пагинация").into(),
            _ => message.to_owned(),
        }
    }
}
