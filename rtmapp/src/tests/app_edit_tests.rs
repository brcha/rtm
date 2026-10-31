use super::support_tests::*;

#[test]
fn completing_a_recurring_item_saves_it_and_the_next_occurrence() {
    let dir = temp_dir("complete");
    // Strict recurrence, so the next due date follows the old one, not today.
    let mut app = app_with(&dir, "Pay rent due:2026-11-01 rec:+1w\n");

    app.toggle(0);

    assert_eq!(app.error, None);
    let lines = file_lines(&dir);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].starts_with("x ") && lines[0].contains("due:2026-11-01"));
    assert!(!lines[1].starts_with("x ") && lines[1].contains("due:2026-11-08"));
    // Re-read after completing, as in the Tauri app.
    assert_eq!(app.library.as_ref().unwrap().item_count(), 2);
    assert!(!dir.join("config.toml").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn uncompleting_clears_the_completion_date_on_disk() {
    let dir = temp_dir("uncomplete");
    let mut app = app_with(&dir, "x 2026-10-20 2026-10-19 Done thing\n");

    app.toggle(0);

    assert_eq!(app.error, None);
    let lines = file_lines(&dir);
    assert_eq!(lines.len(), 1);
    assert!(!lines[0].starts_with("x ") && !lines[0].contains("2026-10-20"));
    assert!(lines[0].contains("2026-10-19 Done thing"), "{}", lines[0]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn toggling_an_index_past_the_end_changes_nothing() {
    let dir = temp_dir("past-end");
    let mut app = app_with(&dir, "Only item\n");

    app.toggle(1);

    assert_eq!(app.error, None);
    assert_eq!(file_lines(&dir), ["Only item"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn adding_saves_the_trimmed_item_with_a_creation_date() {
    let dir = temp_dir("add");
    let mut app = app_with(&dir, "First\n");
    app.new_item = "  (B) Second +Proj due:2026-11-02  ".to_owned();

    app.add_item();

    assert_eq!(app.error, None);
    assert_eq!(app.new_item, "");
    let today = chrono::Local::now().date_naive().format("%Y-%m-%d");
    assert_eq!(
        file_lines(&dir),
        [
            "First".to_owned(),
            format!("(B) {today} Second +Proj due:2026-11-02")
        ]
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn blank_input_adds_nothing() {
    let dir = temp_dir("blank");
    let mut app = app_with(&dir, "First\n");
    app.new_item = "   ".to_owned();

    app.add_item();

    assert_eq!(app.error, None);
    assert_eq!(
        std::fs::read_to_string(dir.join("todo.txt")).unwrap(),
        "First\n"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn adding_without_a_file_is_an_error_and_writes_nothing() {
    let dir = temp_dir("no-file");
    let mut app = bare_app(&dir, None);
    app.new_item = "Task".to_owned();

    app.add_item();

    assert_eq!(app.error.as_deref(), Some("Please load a file first"));
    assert_eq!(app.new_item, "Task");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1); // only "sub"
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn saving_an_edit_writes_the_item_and_keeps_the_others() {
    let dir = temp_dir("edit");
    let mut app = app_with(&dir, "First\n(D) Second +Home\nThird\n");
    let form = open_editor(&mut app, 1);
    form.description = "Second, edited".into();
    form.due = "2026-11-02".into();

    app.save_edit();

    assert_eq!(app.error, None);
    assert!(app.editing.is_none());
    assert_eq!(
        file_lines(&dir),
        ["First", "(D) Second, edited +Home due:2026-11-02", "Third"]
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_invalid_edit_keeps_the_dialog_and_writes_nothing() {
    let dir = temp_dir("edit-invalid");
    let mut app = app_with(&dir, "First\n");
    open_editor(&mut app, 0).due = "tomorrow".into();

    app.save_edit();

    let form = app.editing.as_ref().expect("dialog stays open");
    assert_eq!(form.error.as_deref(), Some("Due date: expected YYYY-MM-DD"));
    assert_eq!(
        std::fs::read_to_string(dir.join("todo.txt")).unwrap(),
        "First\n"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// A parent (index 2) with a sub-item before it and one after it, between two plain items.
const LINKED: &str = "First\n\
    Child A sub:5a1f3c2e-8d4b-4e6f-9a7c-1b2d3e4f5a6b\n\
    Parent uuid:5a1f3c2e-8d4b-4e6f-9a7c-1b2d3e4f5a6b\n\
    Child B sub:5a1f3c2e-8d4b-4e6f-9a7c-1b2d3e4f5a6b\n\
    Last\n";

#[test]
fn asking_to_delete_counts_the_sub_items() {
    let dir = temp_dir("ask-delete");
    let mut app = app_with(&dir, LINKED);
    // load() drops a line whose uuid does not parse; make sure none was.
    assert_eq!(app.library.as_ref().unwrap().item_count(), 5);

    open_editor(&mut app, 2);
    app.ask_delete();
    assert_eq!(app.editing.as_ref().unwrap().confirm_delete, Some(2));

    open_editor(&mut app, 0);
    app.ask_delete();
    assert_eq!(app.editing.as_ref().unwrap().confirm_delete, Some(0));
    // Counting changes nothing.
    assert_eq!(app.library.as_ref().unwrap().item_count(), 5);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn deleting_removes_the_item_and_its_sub_items() {
    let dir = temp_dir("delete");
    let mut app = app_with(&dir, LINKED);
    assert_eq!(app.library.as_ref().unwrap().item_count(), 5);
    open_editor(&mut app, 2);

    app.delete_item();

    assert_eq!(app.error, None);
    assert!(app.editing.is_none());
    assert_eq!(file_lines(&dir), ["First", "Last"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn deleting_a_plain_item_removes_only_it() {
    let dir = temp_dir("delete-plain");
    let mut app = app_with(&dir, "First\nSecond\nThird\n");
    open_editor(&mut app, 1);

    app.delete_item();

    assert_eq!(app.error, None);
    assert!(app.editing.is_none());
    assert_eq!(file_lines(&dir), ["First", "Third"]);
    let _ = std::fs::remove_dir_all(dir);
}
