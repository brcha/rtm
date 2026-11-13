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
    /// Reads the config. A missing or invalid file gives the defaults. A Windows path saved in the
    /// verbatim form (`\\?\C:\…`) takes the plain one, written with the next save.
    pub fn load(path: &Path) -> Self {
        let mut config: AppConfig = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default();
        config.file_name = config
            .file_name
            .as_deref()
            .map(|file| plain_path(Path::new(file)).to_string_lossy().into_owned());
        config
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }
}

/// `path` in the form Windows users know: the verbatim `\\?\C:\dir\todo.txt` that
/// `canonicalize` returns as `C:\dir\todo.txt`, and `\\?\UNC\server\share\todo.txt` as
/// `\\server\share\todo.txt`. A path whose plain form would name another file keeps the verbatim
/// form: a `.` or `..` component (literal in a verbatim path), a name ending in a dot or a space,
/// containing `/`, or naming a device (`CON`, `NUL`, `COM1`, …). Anything else, a path already
/// plain included, is returned unchanged.
///
/// Unlike `dunce`, a long path is converted too: rtm handles long plain paths (the standard
/// library adds the prefix for its own file calls, and rtmsync strips it before running git).
#[cfg(windows)]
pub fn plain_path(path: &Path) -> PathBuf {
    use std::path::{Component, Prefix};
    let unchanged = || path.to_path_buf();
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return unchanged();
    };
    let mut plain = match prefix.kind() {
        Prefix::VerbatimDisk(drive) => format!("{}:", char::from(drive)),
        Prefix::VerbatimUNC(server, share) => match (server.to_str(), share.to_str()) {
            (Some(server), Some(share)) => format!(r"\\{server}\{share}"),
            _ => return unchanged(),
        },
        _ => return unchanged(),
    };
    if components.next() != Some(Component::RootDir) {
        return unchanged();
    }
    let mut names = 0;
    for component in components {
        let Component::Normal(name) = component else {
            return unchanged();
        };
        match name.to_str() {
            Some(name) if keeps_meaning(name) => {
                plain.push('\\');
                plain.push_str(name);
                names += 1;
            }
            _ => return unchanged(),
        }
    }
    if names == 0 {
        plain.push('\\');
    }
    PathBuf::from(plain)
}

/// On other systems paths have no verbatim form.
#[cfg(not(windows))]
pub fn plain_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

/// Whether `name` names the same file outside a verbatim path. Windows trims a trailing dot or
/// space, splits at `/` and maps device names (by the part before the first dot) to devices.
#[cfg(windows)]
fn keeps_meaning(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit());
    !device && !name.ends_with(['.', ' ']) && !name.contains('/')
}

#[cfg(test)]
#[path = "tests/config_file_tests.rs"]
mod file_tests;

#[cfg(test)]
#[path = "tests/config_paths_tests.rs"]
mod paths_tests;
