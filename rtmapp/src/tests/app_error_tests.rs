//! The two error lines (RTM-125): the app line and the open file's line, and what clears each.

use super::support_tests::*;

#[test]
fn a_conflict_message_stays_after_a_config_save() {
    let dir = temp_dir("error conflict save");
    let mut app = app_with(&dir, "First\n");

    app.file_mut().close_conflicted();
    app.save_config();

    assert_eq!(app.error, None);
    let error = app.file().error.as_deref().unwrap();
    assert!(error.contains("resolve them by hand"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_failed_config_save_and_a_file_message_are_both_shown() {
    let dir = temp_dir("error both");
    let mut app = app_with(&dir, "First\n");
    // A regular file where the config's folder would be: creating the folder fails everywhere.
    app.config_path = dir.join("todo.txt").join("config.toml");

    app.file_mut().close_conflicted();
    app.save_config();

    let errors = errors(&app);
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(
        errors[0].starts_with("Could not save the config"),
        "{errors:?}"
    );
    assert!(errors[1].contains("resolve them by hand"), "{errors:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_successful_change_clears_both_lines() {
    let dir = temp_dir("error cleared");
    let mut app = app_with(&dir, "First\n");
    app.error = Some("Could not parse the item".to_owned());
    app.file_mut().error = Some("Not committed".to_owned());

    app.new_item = "Second".to_owned();
    app.add_item();

    assert_eq!(errors(&app), Vec::<&str>::new());
    assert_eq!(file_lines(&dir).len(), 2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn opening_another_file_clears_the_old_file_message() {
    let dir = temp_dir("error other file");
    std::fs::write(dir.join("other.txt"), "Other\n").unwrap();
    let mut app = app_with(&dir, "First\n");
    app.file_mut().error = Some("Not committed".to_owned());

    app.open_file(&dir.join("other.txt"));

    assert_eq!(app.file().error, None);
    assert_eq!(
        app.file().library.as_ref().unwrap().items[0].description,
        "Other"
    );
    let _ = std::fs::remove_dir_all(dir);
}
