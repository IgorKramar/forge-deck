use serde_json::{Value, json};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{collections::BTreeSet, fs, io::Write, path::PathBuf};

pub struct Preferences {
    path: PathBuf,
    data: Value,
}

impl Preferences {
    pub fn load() -> Result<Self, String> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .ok_or("Не найден домашний каталог для настроек")?;
        Self::load_path(base.join("gitlab-deck/config.json"))
    }

    fn load_path(path: PathBuf) -> Result<Self, String> {
        let data = match fs::read_to_string(&path) {
            Ok(text) => {
                let value: Value = serde_json::from_str(&text).map_err(|_| {
                    format!("Некорректный JSON в {}. Файл не изменён.", path.display())
                })?;
                if !value.is_object() {
                    return Err("Настройки должны содержать JSON-объект".into());
                }
                value
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
            Err(e) => return Err(format!("Не удалось прочитать настройки: {e}")),
        };
        Ok(Self { path, data })
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
        let mut data = self.data.clone();
        data["host"] = Value::String(host.into());
        if !data.get("favorites").is_some_and(Value::is_object) {
            data["favorites"] = json!({});
        }
        data["favorites"][host] = json!(favorites);
        let parent = self.path.parent().ok_or("Не найден каталог настроек")?;
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
                .map_err(|e| format!("Не удалось создать временный файл: {e}"))?;
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
        let mut preferences = Preferences::load_path(path.clone()).unwrap();
        preferences
            .save("gitlab.example.org", &BTreeSet::from([7, 42]))
            .unwrap();
        let loaded = Preferences::load_path(path.clone()).unwrap();
        assert_eq!(
            loaded.favorites("gitlab.example.org"),
            BTreeSet::from([7, 42])
        );
        assert!(loaded.favorites("another.example.org").is_empty());
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::write(&path, "broken").unwrap();
        assert!(Preferences::load_path(path.clone()).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "broken");
        fs::remove_dir_all(root).unwrap();
    }
}
