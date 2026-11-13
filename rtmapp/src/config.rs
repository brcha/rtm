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
    /// The Git-or-Pijul choice as versions before RTM-124 wrote it, for `file_name`. `load` moves
    /// it into that file's entry, so it is never written again.
    pub vcs: Option<String>,
    /// Minutes between background syncs (RTM-20); 0 syncs only on open, on exit and on demand.
    pub sync_interval_minutes: u64,
    /// Check for a newer release at start and daily (RTM-138). On unless set to false.
    pub check_for_updates: bool,
    /// `"stable"` or `"beta"`: the releases the update check offers. Anything else counts as
    /// stable.
    pub update_channel: String,
    /// The sidebar's Projects and Contexts sections are collapsed (RTM-120).
    pub projects_collapsed: bool,
    pub contexts_collapsed: bool,
    /// The saved session (RTM-124): the todo.txt files the app keeps; `file_name` names the active
    /// one and stays for older versions, which ignore this list. Last, so the plain keys are
    /// written before the tables.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileEntry>,
}

/// One todo.txt of the saved session (`[[files]]`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct FileEntry {
    pub path: String,
    /// The label of the file's tab (RTM-127), set by hand; without it the tab shows the file name.
    pub name: Option<String>,
    /// `"git"` or `"pijul"`: the user's choice when the file's directory belongs to both.
    pub vcs: Option<String>,
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
            check_for_updates: true,
            update_channel: "stable".to_owned(),
            projects_collapsed: false,
            contexts_collapsed: false,
            files: Vec::new(),
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
    /// Whether the update check offers betas; an unknown channel counts as stable.
    pub fn beta(&self) -> bool {
        self.update_channel == "beta"
    }

    /// Reads the config. A missing or invalid file gives the defaults. Nothing is written: a
    /// migrated config reaches the file on the next save.
    pub fn load(path: &Path) -> Self {
        let mut config: AppConfig = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default();
        config.migrate();
        config
    }

    /// Brings a config from an older version (or edited by one) into the list of files: an entry
    /// for `file_name` when it has none, and the top-level `vcs`, which an older version wrote for
    /// `file_name`, moved into that entry. Entries without a path are dropped. Windows paths saved
    /// in the verbatim form (`\\?\C:\…`) take the plain one, and an entry that then repeats an
    /// earlier one is dropped.
    fn migrate(&mut self) {
        let plain = |path: &str| plain_path(Path::new(path)).to_string_lossy().into_owned();
        self.files.retain(|entry| !entry.path.is_empty());
        for entry in &mut self.files {
            entry.path = plain(&entry.path);
        }
        let mut seen = std::collections::HashSet::new();
        self.files
            .retain(|entry| seen.insert(PathBuf::from(&entry.path)));
        self.file_name = self.file_name.as_deref().map(plain);
        let Some(file) = self.file_name.clone() else {
            self.vcs = None;
            return;
        };
        if self.entry(Path::new(&file)).is_none() {
            self.files.push(FileEntry {
                path: file.clone(),
                ..FileEntry::default()
            });
        }
        if let Some(vcs) = self.vcs.take() {
            self.set_vcs(&file, &vcs);
        }
    }

    /// The entry of `file`. Paths are compared as given: the app keeps them resolved, in the plain
    /// Windows form (`plain_path`).
    pub fn entry(&self, file: &Path) -> Option<&FileEntry> {
        self.files
            .iter()
            .find(|entry| Path::new(&entry.path) == file)
    }

    fn entry_mut(&mut self, file: &Path) -> Option<&mut FileEntry> {
        self.files
            .iter_mut()
            .find(|entry| Path::new(&entry.path) == file)
    }

    /// Stores the Git-or-Pijul choice for `file`, adding its entry if it has none.
    pub fn set_vcs(&mut self, file: &str, vcs: &str) {
        if self.entry(Path::new(file)).is_none() {
            self.files.push(FileEntry {
                path: file.to_owned(),
                ..FileEntry::default()
            });
        }
        if let Some(entry) = self.entry_mut(Path::new(file)) {
            entry.vcs = Some(vcs.to_owned());
        }
    }

    /// Sets the tab label of `file`, or clears it with `None`. A file without an entry is left
    /// alone: every tab has one.
    pub fn set_name(&mut self, file: &str, name: Option<&str>) {
        if let Some(entry) = self.entry_mut(Path::new(file)) {
            entry.name = name.map(str::to_owned);
        }
    }

    /// Adds `file` to the saved session, unless it is listed already.
    pub fn add_file(&mut self, file: &str) {
        if self.entry(Path::new(file)).is_none() {
            self.files.push(FileEntry {
                path: file.to_owned(),
                ..FileEntry::default()
            });
        }
    }

    /// Removes `file` from the saved session; `file_name` is cleared when it named it.
    pub fn remove_file(&mut self, file: &str) {
        self.files
            .retain(|entry| Path::new(&entry.path) != Path::new(file));
        if self.file_name.as_deref() == Some(file) {
            self.file_name = None;
        }
    }

    /// Makes `file` the active file, the one shown at the next start.
    pub fn set_active(&mut self, file: &str) {
        self.file_name = Some(file.to_owned());
    }

    /// Renames the entry of `old` to `file`, keeping its name and choice: the path the config
    /// gave, resolved (symlinks, `..`) when the file was loaded. `file_name` follows.
    pub fn rename(&mut self, old: &str, file: &str) {
        if old == file {
            return;
        }
        if self.file_name.as_deref() == Some(old) {
            self.file_name = Some(file.to_owned());
        }
        if self.entry(Path::new(file)).is_some() {
            // Both spellings are listed: keep the resolved one, with the newer choice `migrate`
            // gave the old spelling.
            let vcs = self
                .entry(Path::new(old))
                .and_then(|entry| entry.vcs.clone());
            self.files
                .retain(|entry| Path::new(&entry.path) != Path::new(old));
            if let Some(vcs) = vcs {
                self.set_vcs(file, &vcs);
            }
        } else if let Some(entry) = self.entry_mut(Path::new(old)) {
            entry.path = file.to_owned();
        }
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
