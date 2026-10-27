use crate::i18n::Language;
use serde_json::{Value, json};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{collections::BTreeSet, fs, io::Write, path::PathBuf};

pub struct Preferences {
    ui_language: Language,
    path: PathBuf,
    data: Value,
}

impl Preferences {
    pub fn load() -> Result<Self, String> {
        Self::load_in(Language::En)
    }

    pub fn load_in(language: Language) -> Result<Self, String> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .ok_or(language.text(
                "Could not find the home directory for settings",
                "Не найден домашний каталог для настроек",
            ))?;
        Self::load_path(base.join("gitlab-deck/config.json"), language)
    }

    fn load_path(path: PathBuf, language: Language) -> Result<Self, String> {
        let data = match fs::read_to_string(&path) {
            Ok(text) => {
                let value: Value = serde_json::from_str(&text).map_err(|_| {
                    crate::tr!(
                        language,
                        "Invalid JSON in {}. The file was not changed.",
                        "Некорректный JSON в {}. Файл не изменён.",
                        path.display()
                    )
                })?;
                if !value.is_object() {
                    return Err(language
                        .text(
                            "Settings must contain a JSON object",
                            "Настройки должны содержать JSON-объект",
                        )
                        .into());
                }
                value
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
            Err(e) => {
                return Err(crate::tr!(
                    language,
                    "Could not read settings: {e}",
                    "Не удалось прочитать настройки: {e}"
                ));
            }
        };
        Ok(Self {
            path,
            data,
            ui_language: language,
        })
    }

    pub fn language(&self) -> Option<Language> {
        self.data
            .get("language")
            .and_then(Value::as_str)
            .and_then(Language::from_code)
    }

    pub fn set_language(&mut self, language: Language) {
        self.ui_language = language;
        self.data["language"] = language.code().into();
    }

    pub fn host(&self) -> Option<String> {
        self.data
            .get("host")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }

    pub fn favorites(&self, host: &str) -> BTreeSet<u64> {
        self.data
            .get("favorites")
            .and_then(|v| v.get(host))
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default()
    }

    pub fn save(&mut self, host: &str, favorites: &BTreeSet<u64>) -> Result<(), String> {
        let language = self.ui_language;
        let mut data = self.data.clone();
        data["host"] = Value::String(host.into());
        if !data.get("favorites").is_some_and(Value::is_object) {
            data["favorites"] = json!({});
        }
        data["favorites"][host] = json!(favorites);
        let parent = self.path.parent().ok_or(language.text(
            "Could not find the settings directory",
            "Не найден каталог настроек",
        ))?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        let temporary = parent.join(format!(".config-{}.tmp", std::process::id()));
        let result = (|| -> Result<(), String> {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)
                .map_err(|e| {
                    crate::tr!(
                        language,
                        "Could not create a temporary file: {e}",
                        "Не удалось создать временный файл: {e}"
                    )
                })?;
            let text = serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?;
            file.write_all(&text).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
            fs::rename(&temporary, &self.path).map_err(|e| e.to_string())?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
        self.data = data;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_roundtrip_and_corrupt_file_is_preserved() {
        let root = std::env::temp_dir().join(format!("deck-config-test-{}", std::process::id()));
        let path = root.join("config.json");
        let _ = fs::remove_dir_all(&root);
        let mut preferences = Preferences::load_path(path.clone(), Language::En).unwrap();
        preferences
            .save("gitlab.example.org", &BTreeSet::from([7, 42]))
            .unwrap();
        let loaded = Preferences::load_path(path.clone(), Language::En).unwrap();
        assert_eq!(
            loaded.favorites("gitlab.example.org"),
            BTreeSet::from([7, 42])
        );
        assert!(loaded.favorites("another.example.org").is_empty());
        assert_eq!(loaded.language(), None);
        let mut loaded = loaded;
        loaded.set_language(crate::i18n::Language::Ru);
        loaded
            .save("gitlab.example.org", &BTreeSet::from([7, 42]))
            .unwrap();
        let loaded = Preferences::load_path(path.clone(), Language::En).unwrap();
        assert_eq!(loaded.language(), Some(crate::i18n::Language::Ru));
        assert_eq!(
            loaded.favorites("gitlab.example.org"),
            BTreeSet::from([7, 42])
        );
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::write(&path, "broken").unwrap();
        assert!(Preferences::load_path(path.clone(), Language::En).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "broken");
        fs::remove_dir_all(root).unwrap();
    }
}
