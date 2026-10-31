//! The config file in the OS config directory (`rtm/config.toml`).
//!
//! The schema is the one the former Tauri app wrote (replaced in RTM-39), so field names and
//! defaults must stay as they are: existing users' settings live in this file.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub file_name: Option<String>,
    pub show_completed_items: bool,
    pub show_future_items: bool,
    pub hide_no_date: bool,
    pub reverse_sort: bool,
    /// Commit, pull and push the file through its repository (RTM-64). On unless set to false.
    pub sync: bool,
    /// `"git"` or `"pijul"`: the user's choice when the file's directory belongs to both.
    pub vcs: Option<String>,
    /// Minutes between background syncs (RTM-20); 0 syncs only on open, on exit and on demand.
    pub sync_interval_minutes: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            file_name: None,
            show_completed_items: false,
            show_future_items: false,
            hide_no_date: false,
            reverse_sort: false,
            sync: true,
            vcs: None,
            sync_interval_minutes: 10,
        }
    }
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
#[path = "tests/config_file_tests.rs"]
mod file_tests;
