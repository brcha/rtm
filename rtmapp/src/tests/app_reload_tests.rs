//! Changes another program makes to an open file (RTM-142).

use super::support_tests::*;
use super::*;

fn descriptions(app: &App) -> Vec<String> {
    let library = app.file().unwrap().library.as_ref().unwrap();
    library
        .items
        .iter()
        .map(|item| item.description.clone())
        .collect()
}

#[test]
fn a_change_is_made_on_what_another_program_wrote() {
    let dir = temp_dir("reload before write");
    let mut app = app_with(&dir, "Alpha\n");
    std::fs::write(dir.join("todo.txt"), "Alpha\nFrom outside\n").unwrap();

    app.toggle(0);

    assert_eq!(errors(&app), Vec::<&str>::new());
    let lines = file_lines(&dir);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].starts_with("x ") && lines[0].ends_with("Alpha"),
        "{lines:?}"
    );
    assert_eq!(lines[1], "From outside");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_list_shows_what_another_program_wrote() {
    let dir = temp_dir("reload on timer");
    let mut app = app_with(&dir, "Alpha\n");
    std::fs::write(dir.join("todo.txt"), "Alpha\nFrom outside\n").unwrap();

    // Not yet due: nothing is read.
    let now = Instant::now();
    app.next_disk_check = now + DISK_CHECK;
    let ctx = app.ctx.clone();
    app.watch_files(&ctx, now);
    assert_eq!(descriptions(&app), ["Alpha"]);

    app.watch_files(&ctx, now + DISK_CHECK);
    assert_eq!(descriptions(&app), ["Alpha", "From outside"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_apps_own_writes_do_not_count_as_outside_changes() {
    let dir = temp_dir("reload own write");
    let mut app = app_with(&dir, "Alpha\n");
    app.new_item = "Bravo".to_owned();
    app.add_item();
    assert!(!app.file().unwrap().changed_on_disk());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_edit_is_refused_when_another_program_changed_its_item() {
    let dir = temp_dir("reload stale edit");
    let mut app = app_with(&dir, "Alpha\nBravo\n");
    open_editor(&mut app, 1).description = "Bravo, edited here".to_owned();
    std::fs::write(dir.join("todo.txt"), "Alpha\nBravo, edited there\n").unwrap();

    app.reload_changed();
    app.save_edit();

    let form = app.editing.as_ref().expect("the dialog stays open");
    assert!(form.error.as_deref().unwrap().contains("changed"));
    assert_eq!(file_lines(&dir), ["Alpha", "Bravo, edited there"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_file_being_checked_on_open_is_not_reloaded() {
    let dir = temp_dir("reload while checking");
    let mut app = app_with(&dir, "Alpha\n");
    app.file_mut().unwrap().checking = true;
    std::fs::write(dir.join("todo.txt"), "Alpha\nFrom outside\n").unwrap();

    app.reload_changed();

    assert_eq!(descriptions(&app), ["Alpha"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_unreadable_change_is_not_written_over() {
    let dir = temp_dir("reload unreadable");
    let mut app = app_with(&dir, "Alpha\n");
    // Another program replaced the file with something that cannot be read as text.
    std::fs::write(dir.join("todo.txt"), [0xff, 0xfe, 0x00, b'\n']).unwrap();

    app.toggle(0);

    assert_eq!(
        std::fs::read(dir.join("todo.txt")).unwrap(),
        [0xff, 0xfe, 0x00, b'\n']
    );
    assert!(app.file().unwrap().error.is_some());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_tab_with_changes_that_could_not_be_written_stays_open() {
    let dir = temp_dir("reload close pending");
    let mut app = app_with(&dir, "Alpha\n");
    std::fs::write(dir.join("todo.txt"), [0xff, 0xfe, 0x00, b'\n']).unwrap();
    app.toggle(0);
    let path = app.file().unwrap().path.clone();

    app.close(&path);

    let file = app.file().expect("the tab stays");
    assert_eq!(file.pending.len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}
