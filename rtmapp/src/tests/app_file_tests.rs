use super::support_tests::*;
use super::*;

#[test]
fn loads_items_from_a_path_with_spaces_and_dots() {
    let dir = temp_dir("loads");
    std::fs::write(
        dir.join("todo list.txt"),
        "(A) Pay bill +Home @online due:2026-11-01\nx 2026-10-20 2026-10-19 Done thing\nPlain item\n",
    )
    .unwrap();

    let library = load_library(&dir.join("sub").join("..").join("todo list.txt")).unwrap();

    assert_eq!(library.item_count(), 3);
    assert!(library.items[1].done);
    assert!(!library.file_name.contains(".."));
    assert!(library.file_name.ends_with("todo list.txt"));
    assert!(
        !library.file_name.starts_with(r"\\?\"),
        "{}",
        library.file_name
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn missing_file_is_an_error_that_names_the_path() {
    let dir = temp_dir("missing");
    let error = load_library(&dir.join("nope.txt")).unwrap_err();
    assert!(error.contains("nope.txt"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn empty_file_gives_an_empty_library() {
    let dir = temp_dir("empty");
    std::fs::write(dir.join("todo.txt"), "").unwrap();
    assert_eq!(load_library(&dir.join("todo.txt")).unwrap().item_count(), 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_new_file_is_created_empty_opened_and_kept_in_the_config() {
    let dir = temp_dir("new file");
    let mut app = bare_app(&dir, None);

    app.create_file(&dir.join("sub/todo.txt"));

    assert_eq!(errors(&app), Vec::<&str>::new());
    assert_eq!(
        std::fs::read_to_string(dir.join("sub/todo.txt")).unwrap(),
        ""
    );
    assert_eq!(
        app.file().unwrap().library.as_ref().unwrap().item_count(),
        0
    );
    app.new_item = "Buy milk".to_owned();
    app.add_item();
    assert_eq!(errors(&app), Vec::<&str>::new());
    // What the next start reads.
    let name = AppConfig::load(&dir.join("config.toml")).file_name.unwrap();
    let canonical = resolve(&dir.join("sub/todo.txt")).unwrap();
    assert_eq!(Path::new(&name), canonical);
    let library = load_library(Path::new(&name)).unwrap();
    assert_eq!(library.items[0].description, "Buy milk");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_picked_file_is_opened_when_the_dialog_returns() {
    let dir = temp_dir("dialog pick");
    std::fs::write(dir.join("other.txt"), "Other item\n").unwrap();
    let mut app = app_with(&dir, "First\n");

    dialog_returned(&mut app, Some(dir.join("other.txt")), App::open_file);
    app.dialog_result();

    assert!(app.dialog.is_none());
    assert_eq!(errors(&app), Vec::<&str>::new());
    assert_eq!(
        app.file().unwrap().library.as_ref().unwrap().items[0].description,
        "Other item"
    );
    let name = AppConfig::load(&dir.join("config.toml")).file_name.unwrap();
    assert_eq!(Path::new(&name), resolve(&dir.join("other.txt")).unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn opening_another_file_adds_a_tab_and_an_entry() {
    let dir = temp_dir("add entry");
    std::fs::write(dir.join("other.txt"), "Other item\n").unwrap();
    let mut app = app_with(&dir, "First\n");
    let first = app.library().unwrap().file_name.clone();
    app.config.add_file(&first);
    app.config.set_active(&first);
    app.config.set_vcs(&first, "git");

    dialog_returned(&mut app, Some(dir.join("other.txt")), App::open_file);
    app.dialog_result();

    assert_eq!(app.files.len(), 2);
    assert_eq!(app.library().unwrap().items[0].description, "Other item");
    let config = AppConfig::load(&dir.join("config.toml"));
    let other = resolve(&dir.join("other.txt")).unwrap();
    assert_eq!(config.file_name.as_deref(), other.to_str());
    let [kept, added] = config.files.as_slice() else {
        panic!("two entries: {:?}", config.files);
    };
    assert_eq!(
        (kept.path.as_str(), kept.vcs.as_deref()),
        (first.as_str(), Some("git"))
    );
    assert_eq!(Path::new(&added.path), other);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_cancelled_dialog_changes_nothing() {
    let dir = temp_dir("dialog cancel");
    let mut app = app_with(&dir, "First\n");

    dialog_returned(&mut app, None, App::open_file);
    app.dialog_result();

    assert!(app.dialog.is_none());
    assert_eq!(errors(&app), Vec::<&str>::new());
    assert_eq!(
        app.file().unwrap().library.as_ref().unwrap().items[0].description,
        "First"
    );
    assert!(!dir.join("config.toml").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_file_named_in_the_save_dialog_is_created_and_opened() {
    let dir = temp_dir("dialog new");
    let mut app = bare_app(&dir, None);

    dialog_returned(&mut app, Some(dir.join("new.txt")), App::create_file);
    app.dialog_result();

    assert!(app.dialog.is_none());
    assert_eq!(errors(&app), Vec::<&str>::new());
    assert_eq!(std::fs::read_to_string(dir.join("new.txt")).unwrap(), "");
    assert_eq!(
        app.file().unwrap().library.as_ref().unwrap().item_count(),
        0
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_picked_file_waits_for_the_edit_dialog_to_close() {
    let dir = temp_dir("dialog editing");
    std::fs::write(dir.join("other.txt"), "Other item\n").unwrap();
    let mut app = app_with(&dir, "First\n");
    open_editor(&mut app, 0);

    dialog_returned(&mut app, Some(dir.join("other.txt")), App::open_file);
    app.dialog_result();
    assert!(app.dialog.is_some());
    assert_eq!(
        app.file().unwrap().library.as_ref().unwrap().items[0].description,
        "First"
    );

    app.editing = None;
    app.dialog_result();
    assert!(app.dialog.is_none());
    assert_eq!(
        app.file().unwrap().library.as_ref().unwrap().items[0].description,
        "Other item"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_new_file_replaces_an_existing_one() {
    let dir = temp_dir("new file replaces");
    std::fs::write(dir.join("todo.txt"), "Old item\n").unwrap();
    let mut app = bare_app(&dir, None);

    app.create_file(&dir.join("todo.txt"));

    assert_eq!(errors(&app), Vec::<&str>::new());
    assert_eq!(std::fs::read_to_string(dir.join("todo.txt")).unwrap(), "");
    assert_eq!(
        app.file().unwrap().library.as_ref().unwrap().item_count(),
        0
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_new_file_in_a_missing_folder_is_an_error() {
    let dir = temp_dir("new file missing");
    let mut app = bare_app(&dir, None);

    app.create_file(&dir.join("missing/todo.txt"));

    assert!(app.error.as_deref().unwrap().contains("missing"));
    assert!(app.file().is_none());
    assert!(!dir.join("config.toml").exists());
    let _ = std::fs::remove_dir_all(dir);
}
