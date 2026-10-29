//! The config file in the OS config directory (`rtm/config.toml`).
//!
//! The schema is the one the former Tauri app wrote (replaced in RTM-39), so field names and
//! defaults must stay as they are: existing users' settings live in this file.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub file_name: Option<String>,
    pub show_completed_items: bool,
    pub show_future_items: bool,
    pub hide_no_date: bool,
    pub reverse_sort: bool,
}

/// `<config dir>/rtm/config.toml`. Nothing is created here; `AppConfig::save` creates the directory.
pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("rtm")
        .join("config.toml")
}

impl AppConfig {
    /// Reads the config. A missing or invalid file gives the defaults.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("rtmapp-test-{}-{name}", std::process::id()))
            .join("rtm")
            .join("config.toml")
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_dir_all(path.parent().and_then(Path::parent).unwrap());
    }

    #[test]
    fn round_trip() {
        let path = temp_path("round-trip");
        let config = AppConfig {
            file_name: Some("/home/user/todo.txt".into()),
            show_completed_items: true,
            show_future_items: false,
            hide_no_date: true,
            reverse_sort: true,
        };
        config.save(&path).unwrap();
        assert_eq!(AppConfig::load(&path), config);
        cleanup(&path);
    }

    #[test]
    fn file_with_only_file_name() {
        let config: AppConfig = toml::from_str("file_name = \"/tmp/todo.txt\"").unwrap();
        assert_eq!(config.file_name.as_deref(), Some("/tmp/todo.txt"));
        assert!(!config.show_completed_items && !config.hide_no_date && !config.reverse_sort);
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let config: AppConfig = toml::from_str("future_option = 3\nreverse_sort = true").unwrap();
        assert!(config.reverse_sort);
    }

    #[test]
    fn missing_or_invalid_file_gives_defaults() {
        let path = temp_path("invalid");
        assert_eq!(AppConfig::load(&path), AppConfig::default());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "this is = = not toml").unwrap();
        assert_eq!(AppConfig::load(&path), AppConfig::default());
        cleanup(&path);
    }
}
