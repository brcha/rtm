//! `rtmapp mcp`: the MCP server (`rtmmcp`) on the todo.txt files of rtmapp's config, over stdin and
//! stdout. It opens no window; the app need not be running.

use std::path::{Path, PathBuf};

use rtmmcp::{Source, Sources};

use crate::config::{AppConfig, config_path};
use crate::sync;

/// Serves the configured files until the client closes stdin, then exits.
pub fn run() -> ! {
    if let Err(e) = rtmmcp::serve(sources(config_path())) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
    std::process::exit(0);
}

/// The files of the config at `path`, read again at every tool call (the file is small), so a file
/// added in the app is served without restarting the client.
fn sources(path: PathBuf) -> Sources {
    std::sync::Arc::new(move || {
        let files = configured(&AppConfig::load(&path));
        if files.is_empty() {
            return Err("No todo.txt files are configured in rtmapp".to_owned());
        }
        Ok(files)
    })
}

/// Every `[[files]]` entry of `config`, the active file (`file_name`) first, as the default.
fn configured(config: &AppConfig) -> Vec<Source> {
    let mut files: Vec<Source> = config
        .files
        .iter()
        .map(|entry| Source {
            path: PathBuf::from(&entry.path),
            name: entry.name.clone(),
            sync: config.sync,
            vcs: sync::chosen(config, Path::new(&entry.path)),
        })
        .collect();
    if let Some(active) = &config.file_name
        && let Some(i) = files.iter().position(|file| file.path == Path::new(active))
    {
        let file = files.remove(i);
        files.insert(0, file);
    }
    files
}

#[cfg(test)]
#[path = "tests/mcp_sources_tests.rs"]
mod sources_tests;
