use crate::{i18n::Language, provider::Provider};
use serde_json::{Value, json};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

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
        Self::load_base(&base, language)
    }

    fn load_base(base: &Path, language: Language) -> Result<Self, String> {
        let target = base.join("forge-deck/config.json");
        match fs::metadata(&target) {
            Ok(_) => Self::load_path(target, language),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut preferences =
                    Self::load_path(base.join("gitlab-deck/config.json"), language)?;
                preferences.path = target;
                Ok(preferences)
            }
            Err(e) => Err(e.to_string()),
        }
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

    pub fn provider(&self) -> Provider {
        match self.data["provider"].as_str() {
            Some("github") => Provider::Github,
            _ => Provider::Gitlab,
        }
    }
    pub fn host(&self) -> Option<String> {
        self.host_for(Provider::Gitlab)
    }
    pub fn host_for(&self, provider: Provider) -> Option<String> {
        self.data
            .get("hosts")
            .and_then(|v| v.get(provider.code()))
            .and_then(Value::as_str)
            .or_else(|| {
                if provider == Provider::Gitlab {
                    self.data["host"].as_str()
                } else {
                    None
                }
            })
            .map(str::to_owned)
    }
    pub fn favorites(&self, host: &str) -> BTreeSet<u64> {
        self.favorites_for(Provider::Gitlab, host)
    }
    pub fn favorites_for(&self, provider: Provider, host: &str) -> BTreeSet<u64> {
        self.data
            .get("favorites")
            .and_then(|v| {
                v.get(format!("{}:{host}", provider.code())).or_else(|| {
                    if provider == Provider::Gitlab {
                        v.get(host)
                    } else {
                        None
                    }
                })
            })
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default()
    }

    pub fn save(&mut self, host: &str, favorites: &BTreeSet<u64>) -> Result<(), String> {
        self.save_for(Provider::Gitlab, host, favorites)
    }
    pub fn save_for(
        &mut self,
        provider: Provider,
        host: &str,
        favorites: &BTreeSet<u64>,
    ) -> Result<(), String> {
        let language = self.ui_language;
        let mut data = self.data.clone();
        data["provider"] = provider.code().into();
        if !data["hosts"].is_object() {
            data["hosts"] = json!({});
        }
        data["hosts"][provider.code()] = host.into();
        if provider == Provider::Gitlab {
            data["host"] = host.into();
        }
        if !data.get("favorites").is_some_and(Value::is_object) {
            data["favorites"] = json!({});
        }
        data["favorites"][format!("{}:{host}", provider.code())] = json!(favorites);
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
    fn migration_keeps_legacy_unknown_fields_and_provider_favorites_separate() {
        let root = std::env::temp_dir().join(format!("deck-migrate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let legacy = root.join("gitlab-deck/config.json");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        let old = r#"{"host":"same.example.org","language":"ru","favorites":{"same.example.org":[7]},"extension":{"keep":true}}"#;
        fs::write(&legacy, old).unwrap();
        let mut prefs = Preferences::load_base(&root, Language::En).unwrap();
        assert_eq!(prefs.provider(), Provider::Gitlab);
        assert_eq!(prefs.language(), Some(Language::Ru));
        assert_eq!(
            prefs.favorites_for(Provider::Gitlab, "same.example.org"),
            BTreeSet::from([7])
        );
        assert!(
            prefs
                .favorites_for(Provider::Github, "same.example.org")
                .is_empty()
        );
        prefs
            .save_for(Provider::Github, "same.example.org", &BTreeSet::from([42]))
            .unwrap();
        let loaded = Preferences::load_base(&root, Language::En).unwrap();
        assert_eq!(loaded.provider(), Provider::Github);
        assert_eq!(
            loaded.host_for(Provider::Gitlab).as_deref(),
            Some("same.example.org")
        );
        assert_eq!(
            loaded.favorites_for(Provider::Github, "same.example.org"),
            BTreeSet::from([42])
        );
        assert_eq!(
            loaded.favorites_for(Provider::Gitlab, "same.example.org"),
            BTreeSet::from([7])
        );
        assert_eq!(loaded.data["extension"]["keep"], true);
        assert_eq!(fs::read_to_string(&legacy).unwrap(), old);
        assert_eq!(
            fs::metadata(root.join("forge-deck/config.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        fs::write(root.join("forge-deck/config.json"), "broken").unwrap();
        assert!(Preferences::load_base(&root, Language::En).is_err());
        fs::remove_dir_all(root).unwrap();
    }
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
