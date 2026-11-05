use super::*;

/// A fresh directory whose name contains spaces, under the system temp dir.
pub(super) fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmapp test {} {name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    dir
}

/// An app on a temp todo.txt with `contents`. The config path is in the temp dir too, so a
/// test can never write the user's real config.
pub(super) fn app_with(dir: &Path, contents: &str) -> App {
    let file = dir.join("todo.txt");
    std::fs::write(&file, contents).unwrap();
    bare_app(dir, Some(load_library(&file).unwrap()))
}

/// An app on `library` with its config in `dir`, built directly: no window, no sync started.
pub(super) fn bare_app(dir: &Path, library: Option<TodoLibrary>) -> App {
    App {
        config: AppConfig::default(),
        config_path: dir.join("config.toml"),
        files: vec![OpenFile {
            library,
            ..OpenFile::default()
        }],
        active: 0,
        error: None,
        new_item: String::new(),
        editing: None,
        settings: None,
        reconnect: false,
        worker: None,
        sync_status: None,
        last_sync: Instant::now(),
        dialog: None,
        ctx: egui::Context::default(),
    }
}

/// The error lines shown, the app's first.
pub(super) fn errors(app: &App) -> Vec<&str> {
    [&app.error, &app.file().error]
        .into_iter()
        .flatten()
        .map(String::as_str)
        .collect()
}

pub(super) fn file_lines(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("todo.txt"))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Gives the app a file dialog that has already returned `path`.
pub(super) fn dialog_returned(app: &mut App, path: Option<PathBuf>, then: OnPath) {
    let (send, receive) = mpsc::channel();
    send.send(path).unwrap();
    app.dialog = Some((receive, then));
}

pub(super) fn open_editor(app: &mut App, index: usize) -> &mut EditForm {
    app.open_editor(index);
    app.editing.as_mut().unwrap()
}
