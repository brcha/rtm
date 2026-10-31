//! Loads, edits and saves todo.txt files through the public interface of `TodoLibrary`.

use std::fs;
use std::path::{Path, PathBuf};
use todotxt::{TodoItem, TodoLibrary};

const PARENT: &str = "12345678-1234-1234-1234-123456789abc";
const CHILD: &str = "87654321-4321-4321-4321-abc123456789";

/// A todo.txt holding `lines`, in an empty scratch directory for test `name`.
fn todo_file(name: &str, lines: &[&str]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("todotxt-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("todo.txt");
    fs::write(&file, lines.join("\n") + "\n").unwrap();
    file
}

fn load(file: &Path) -> TodoLibrary {
    let mut library = TodoLibrary::new(file.to_str().unwrap().to_owned());
    library.load().unwrap();
    library
}

fn file_lines(file: &Path) -> Vec<String> {
    fs::read_to_string(file)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn cleanup(file: &Path) {
    fs::remove_dir_all(file.parent().unwrap()).unwrap();
}

fn today() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

#[test]
fn a_loaded_file_is_saved_unchanged() {
    let lines = [
        "(A) 2026-01-05 Call the plumber +home @phone due:2026-01-10",
        "x 2026-01-04 2026-01-02 File the taxes pri:B +admin",
        "2026-01-03 Water the plants rec:+w t:2026-01-06",
        &format!("Plan the trip uuid:{PARENT}"),
        &format!("Book the hotel sub:{PARENT}"),
        "An item without dates",
    ];
    let file = todo_file("unchanged", &lines);

    let library = load(&file);
    assert_eq!(library.item_count(), lines.len());
    library.save().unwrap();

    assert_eq!(file_lines(&file), lines);
    cleanup(&file);
}

#[test]
fn edits_are_written_and_read_back() {
    let file = todo_file(
        "edits",
        &[
            "(B) 2026-01-05 Write the report +work",
            "2026-01-02 Buy milk @shop",
            "x 2026-01-04 2026-01-01 Old task",
        ],
    );

    let mut library = load(&file);
    library.add_item("Call mom".parse::<TodoItem>().unwrap());
    assert_eq!(library.complete_item(0), Some(false));
    assert_eq!(library.uncomplete_item(2), Some(()));
    library.save().unwrap();

    let today = today();
    assert_eq!(
        file_lines(&file),
        [
            format!("x {today} 2026-01-05 Write the report pri:B +work"),
            "2026-01-02 Buy milk @shop".to_owned(),
            "2026-01-01 Old task".to_owned(),
            format!("{today} Call mom"),
        ]
    );
    assert_eq!(load(&file).items, library.items);
    cleanup(&file);
}

#[test]
fn completing_a_recurring_item_saves_the_next_occurrence() {
    let file = todo_file(
        "recurring",
        &["2026-01-05 Take out the bins due:2026-01-07 rec:+w"],
    );

    let mut library = load(&file);
    assert_eq!(library.complete_item(0), Some(true));
    library.save().unwrap();

    // A strict recurrence counts from the old due date, not from today.
    let today = today();
    assert_eq!(
        file_lines(&file),
        [
            format!("x {today} 2026-01-05 Take out the bins due:2026-01-07 rec:+w"),
            format!("{today} Take out the bins due:2026-01-14 rec:+w"),
        ]
    );
    cleanup(&file);
}

#[test]
fn removing_an_item_saves_the_file_without_its_sub_items() {
    let file = todo_file(
        "remove",
        &[
            &format!("Plan the trip uuid:{PARENT}"),
            &format!("Book the hotel uuid:{CHILD} sub:{PARENT}"),
            &format!("Pack sub:{CHILD}"),
            "Buy milk",
        ],
    );

    let mut library = load(&file);
    assert_eq!(library.remove_item(0).map(|removed| removed.len()), Some(3));
    library.save().unwrap();

    assert_eq!(file_lines(&file), ["Buy milk"]);
    assert_eq!(load(&file).item_count(), 1);
    cleanup(&file);
}
