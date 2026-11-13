use super::support_tests::*;
use super::*;

/// Writes `name` in `dir` with `contents` and returns its canonical path.
fn write(dir: &Path, name: &str, contents: &str) -> PathBuf {
    std::fs::write(dir.join(name), contents).unwrap();
    resolve(&dir.join(name)).unwrap()
}

/// A config listing `files`, in that order, with `active` as `file_name`.
fn session(files: &[&Path], active: &Path) -> AppConfig {
    let mut config = AppConfig::default();
    for file in files {
        config.add_file(&file.to_string_lossy());
    }
    config.set_active(&active.to_string_lossy());
    config
}

/// An app on the files of `config`, as at startup, without sync.
fn app_on_session(dir: &Path, mut config: AppConfig) -> App {
    config.sync = false;
    let mut app = bare_app(dir, None);
    (app.files, app.active) = open_session(&mut config);
    app.config = config;
    app
}

fn paths(app: &App) -> Vec<&Path> {
    app.files.iter().map(|file| file.path.as_path()).collect()
}

#[test]
fn every_saved_file_opens_and_file_name_is_shown() {
    let dir = temp_dir("tabs startup");
    let todo = write(&dir, "todo.txt", "Alpha\n");
    let work = write(&dir, "work.txt", "Report\n");

    let app = app_on_session(&dir, session(&[&todo, &work], &work));

    assert_eq!(paths(&app), [todo.as_path(), work.as_path()]);
    assert_eq!(app.library().unwrap().items[0].description, "Report");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_missing_file_keeps_its_tab_with_its_error() {
    let dir = temp_dir("tabs missing");
    let todo = write(&dir, "todo.txt", "Alpha\n");
    let missing = dir.join("missing.txt");

    let app = app_on_session(&dir, session(&[&missing, &todo], &missing));

    assert_eq!(paths(&app), [missing.as_path(), todo.as_path()]);
    let shown = app.file().unwrap();
    assert!(shown.library.is_none());
    assert!(shown.error.as_deref().unwrap().contains("missing.txt"));
    assert!(!dir.join("config.toml").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn two_entries_for_one_file_open_one_tab() {
    let dir = temp_dir("tabs duplicate");
    let todo = write(&dir, "todo.txt", "Alpha\n");
    let detour = dir.join("sub/../todo.txt");

    let app = app_on_session(&dir, session(&[&detour, &todo], &detour));

    assert_eq!(paths(&app), [todo.as_path()]);
    assert_eq!(app.config.files.len(), 1);
    assert_eq!(app.config.file_name.as_deref(), todo.to_str());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn loading_an_open_file_switches_to_its_tab() {
    let dir = temp_dir("tabs load open");
    let todo = write(&dir, "todo.txt", "Alpha\n");
    let work = write(&dir, "work.txt", "Report\n");
    let mut app = app_on_session(&dir, session(&[&todo, &work], &work));

    app.open_file(&dir.join("sub/../todo.txt"));

    assert_eq!(paths(&app), [todo.as_path(), work.as_path()]);
    assert_eq!(app.file().unwrap().path, todo);
    let config = AppConfig::load(&dir.join("config.toml"));
    assert_eq!(config.file_name.as_deref(), todo.to_str());
    assert_eq!(config.files.len(), 2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn loading_a_file_missing_at_startup_loads_it_into_its_tab() {
    let dir = temp_dir("tabs load missing");
    let todo = write(&dir, "todo.txt", "Alpha\n");
    let late = dir.join("sub/../late.txt");
    let mut app = app_on_session(&dir, session(&[&late, &todo], &todo));
    assert!(app.files.first().unwrap().error.is_some());

    let late = write(&dir, "late.txt", "Late item\n");
    app.open_file(&late);

    assert_eq!(paths(&app), [late.as_path(), todo.as_path()]);
    let shown = app.file().unwrap();
    assert_eq!(shown.path, late);
    assert_eq!(shown.error, None);
    assert_eq!(app.library().unwrap().items[0].description, "Late item");
    let config = AppConfig::load(&dir.join("config.toml"));
    let entries: Vec<_> = config
        .files
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    assert_eq!(entries, [late.to_str().unwrap(), todo.to_str().unwrap()]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_new_file_over_an_open_one_is_refused() {
    let dir = temp_dir("tabs new over open");
    let todo = write(&dir, "todo.txt", "Alpha\n");
    let mut app = app_on_session(&dir, session(&[&todo], &todo));

    app.create_file(&todo);

    assert!(app.error.as_deref().unwrap().contains("close its tab"));
    assert_eq!(file_lines(&dir), ["Alpha"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn closing_the_active_tab_shows_the_one_to_its_right() {
    let dir = temp_dir("tabs close active");
    let one = write(&dir, "one.txt", "One\n");
    let two = write(&dir, "two.txt", "Two\n");
    let three = write(&dir, "three.txt", "Three\n");
    let mut app = app_on_session(&dir, session(&[&one, &two, &three], &two));

    app.close(&two);

    assert_eq!(paths(&app), [one.as_path(), three.as_path()]);
    assert_eq!(app.file().unwrap().path, three);
    let config = AppConfig::load(&dir.join("config.toml"));
    assert_eq!(config.file_name.as_deref(), three.to_str());
    assert_eq!(config.files.len(), 2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn closing_the_rightmost_active_tab_shows_the_one_to_its_left() {
    let dir = temp_dir("tabs close last active");
    let one = write(&dir, "one.txt", "One\n");
    let two = write(&dir, "two.txt", "Two\n");
    let mut app = app_on_session(&dir, session(&[&one, &two], &two));

    app.close(&two);

    assert_eq!(app.file().unwrap().path, one);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn closing_a_tab_left_of_the_active_one_keeps_it_shown() {
    let dir = temp_dir("tabs close left");
    let one = write(&dir, "one.txt", "One\n");
    let two = write(&dir, "two.txt", "Two\n");
    let mut app = app_on_session(&dir, session(&[&one, &two], &two));

    app.close(&one);

    assert_eq!(paths(&app), [two.as_path()]);
    assert_eq!(app.file().unwrap().path, two);
    let config = AppConfig::load(&dir.join("config.toml"));
    assert_eq!(config.file_name.as_deref(), two.to_str());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn closing_the_last_tab_leaves_no_file_and_an_empty_session() {
    let dir = temp_dir("tabs close only");
    let todo = write(&dir, "todo.txt", "Alpha\n");
    let mut app = app_on_session(&dir, session(&[&todo], &todo));
    app.new_item = "Written first".to_owned();
    app.add_item();

    app.close(&todo);

    assert!(app.file().is_none());
    assert!(file_lines(&dir).last().unwrap().ends_with("Written first"));
    let config = AppConfig::load(&dir.join("config.toml"));
    assert_eq!(config.file_name, None);
    assert_eq!(config.files, []);
    app.new_item = "Nowhere".to_owned();
    app.add_item();
    assert_eq!(app.error.as_deref(), Some("Please load a file first"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn closing_a_file_missing_at_startup_removes_its_entry() {
    let dir = temp_dir("tabs close missing");
    let todo = write(&dir, "todo.txt", "Alpha\n");
    let missing = dir.join("missing.txt");
    let mut app = app_on_session(&dir, session(&[&missing, &todo], &todo));

    app.close(&missing);

    assert_eq!(paths(&app), [todo.as_path()]);
    let config = AppConfig::load(&dir.join("config.toml"));
    assert_eq!(config.files.len(), 1);
    assert_eq!(config.file_name.as_deref(), todo.to_str());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn switching_tabs_shows_the_file_and_saves_it_as_active() {
    let dir = temp_dir("tabs switch");
    let todo = write(&dir, "todo.txt", "Alpha\n");
    let work = write(&dir, "work.txt", "Report\n");
    let mut app = app_on_session(&dir, session(&[&todo, &work], &todo));

    app.switch(&work);
    app.new_item = "Send invoice".to_owned();
    app.add_item();

    assert_eq!(app.file().unwrap().path, work);
    assert!(
        std::fs::read_to_string(&work)
            .unwrap()
            .contains("Send invoice")
    );
    assert_eq!(file_lines(&dir), ["Alpha"]);
    let config = AppConfig::load(&dir.join("config.toml"));
    assert_eq!(config.file_name.as_deref(), work.to_str());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn switching_waits_for_the_edit_dialog() {
    let dir = temp_dir("tabs switch editing");
    let todo = write(&dir, "todo.txt", "Alpha\n");
    let work = write(&dir, "work.txt", "Report\n");
    let mut app = app_on_session(&dir, session(&[&todo, &work], &todo));
    open_editor(&mut app, 0);

    app.switch(&work);

    assert_eq!(app.file().unwrap().path, todo);
    let _ = std::fs::remove_dir_all(dir);
}

// Renaming a tab (RTM-151).

#[test]
fn renaming_a_tab_labels_it_and_saves_the_name_in_its_entry() {
    let dir = temp_dir("tabs rename");
    let todo = write(&dir, "todo.txt", "Alpha\n");
    let mut app = app_on_session(&dir, session(&[&todo], &todo));

    app.start_rename(&todo);
    assert_eq!(app.renaming, Some((todo.clone(), "todo.txt".to_owned())));
    app.rename_tab(&todo, "  Personal ");

    assert_eq!(app.renaming, None);
    assert_eq!(app.files[0].name, "Personal");
    let config = AppConfig::load(&dir.join("config.toml"));
    assert_eq!(
        config.entry(&todo).unwrap().name.as_deref(),
        Some("Personal")
    );
    assert_eq!(std::fs::read_to_string(&todo).unwrap(), "Alpha\n");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_empty_name_gives_the_tab_its_file_name_back() {
    let dir = temp_dir("tabs rename empty");
    let todo = write(&dir, "todo.txt", "Alpha\n");
    let mut config = session(&[&todo], &todo);
    config.set_name(&todo.to_string_lossy(), Some("Personal"));
    let mut app = app_on_session(&dir, config);
    assert_eq!(app.files[0].name, "Personal");

    app.rename_tab(&todo, "   ");

    assert_eq!(app.files[0].name, "todo.txt");
    let config = AppConfig::load(&dir.join("config.toml"));
    assert_eq!(config.entry(&todo).unwrap().name, None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_tab_whose_file_is_missing_can_be_renamed() {
    let dir = temp_dir("tabs rename missing");
    let todo = write(&dir, "todo.txt", "Alpha\n");
    let missing = dir.join("missing.txt");
    let mut app = app_on_session(&dir, session(&[&todo, &missing], &todo));

    app.rename_tab(&missing, "Old");

    assert_eq!(app.files[1].name, "Old");
    let config = AppConfig::load(&dir.join("config.toml"));
    assert_eq!(config.entry(&missing).unwrap().name.as_deref(), Some("Old"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn keeping_the_label_unchanged_writes_nothing() {
    let dir = temp_dir("tabs rename unchanged");
    let todo = write(&dir, "todo.txt", "Alpha\n");
    let mut app = app_on_session(&dir, session(&[&todo], &todo));

    app.start_rename(&todo);
    app.rename_tab(&todo, "todo.txt");

    assert_eq!(app.renaming, None);
    assert_eq!(app.files[0].name, "todo.txt");
    assert_eq!(app.config.entry(&todo).unwrap().name, None);
    assert!(!dir.join("config.toml").exists());
    let _ = std::fs::remove_dir_all(dir);
}
