use super::support_tests::*;
use super::*;

const PARENT: &str = "11111111-1111-4111-8111-111111111111";
const CHILD: &str = "22222222-2222-4222-8222-222222222222";

/// An app on `todo.txt` (`todo`, shown) and `work.txt` (`work`) in `dir`, both loaded, no sync.
/// Returns the app and the canonical path of `work.txt`.
fn two_files(dir: &Path, todo: &str, work: &str) -> (App, PathBuf) {
    let mut app = app_with(dir, todo);
    std::fs::write(dir.join("work.txt"), work).unwrap();
    let work = OpenFile::loaded(load_library(&dir.join("work.txt")).unwrap(), None);
    let path = work.path.clone();
    app.files.push(work);
    (app, path)
}

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn an_item_moves_to_the_other_file() {
    let dir = temp_dir("move simple");
    let (mut app, work) = two_files(&dir, "Alpha\nBravo\n", "Report\n");

    open_editor(&mut app, 1);
    app.move_item(&work);

    assert!(app.editing.is_none());
    assert_eq!(file_lines(&dir), ["Alpha"]);
    let moved = lines(&work);
    assert_eq!(moved.len(), 2, "{moved:?}");
    assert!(moved[1].ends_with("Bravo"), "{moved:?}");
    assert!(app.delivered.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_item_moves_with_its_sub_items() {
    let dir = temp_dir("move sub-items");
    let todo =
        format!("Plan the trip uuid:{PARENT}\nOther\nBook the hotel uuid:{CHILD} sub:{PARENT}\n");
    let (mut app, work) = two_files(&dir, &todo, "Report\n");

    open_editor(&mut app, 0);
    app.move_item(&work);

    assert_eq!(file_lines(&dir), ["Other"]);
    let moved = lines(&work);
    assert_eq!(moved.len(), 3, "{moved:?}");
    assert!(moved[1].contains("Plan the trip"), "{moved:?}");
    assert!(
        moved[2].contains("Book the hotel") && moved[2].contains(PARENT),
        "{moved:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_dialogs_edits_are_moved() {
    let dir = temp_dir("move edits");
    let (mut app, work) = two_files(&dir, "Alpha\nBravo\n", "Report\n");

    open_editor(&mut app, 1).description = "Bravo, renamed".to_owned();
    app.move_item(&work);

    assert_eq!(file_lines(&dir), ["Alpha"]);
    assert!(lines(&work)[1].ends_with("Bravo, renamed"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_invalid_field_refuses_the_move() {
    let dir = temp_dir("move invalid");
    let (mut app, work) = two_files(&dir, "Alpha\nBravo\n", "Report\n");

    open_editor(&mut app, 1).due = "someday".to_owned();
    app.move_item(&work);

    assert!(app.editing.as_ref().unwrap().error.is_some());
    assert_eq!(file_lines(&dir), ["Alpha", "Bravo"]);
    assert_eq!(lines(&work), ["Report"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_stale_item_refuses_the_move() {
    let dir = temp_dir("move stale");
    let (mut app, work) = two_files(&dir, "Alpha\nBravo\n", "Report\n");

    open_editor(&mut app, 1);
    // What a pull that changed the item sets.
    app.file_mut().unwrap().stale = true;
    app.move_item(&work);

    let error = app.editing.as_ref().unwrap().error.as_deref().unwrap();
    assert!(error.contains("A sync changed this item"), "{error}");
    assert_eq!(file_lines(&dir), ["Alpha", "Bravo"]);
    assert_eq!(lines(&work), ["Report"]);
    let _ = std::fs::remove_dir_all(dir);
}
