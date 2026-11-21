use super::support_tests::*;
use super::*;

const WITH_DONE: &str = "Buy milk\nx 2026-09-15 Pay rent\nCall mom\nx 2026-10-20 Water plants\n";

fn archive_of(source: &Source) -> std::path::PathBuf {
    source.path.with_file_name("done.txt")
}

fn archive_lines(source: &Source) -> Vec<String> {
    std::fs::read_to_string(archive_of(source))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn test_every_done_item_moves_to_the_archive() {
    let source = source_with(WITH_DONE);
    let result = archive_items(&source, None).unwrap();
    assert_eq!(
        result,
        format!(
            "Archived 2 item(s) to '{}':\nx 2026-09-15 Pay rent\nx 2026-10-20 Water plants",
            archive_of(&source).display()
        )
    );
    assert_eq!(lines(&source), ["Buy milk", "Call mom"]);
    assert_eq!(
        archive_lines(&source),
        ["x 2026-09-15 Pay rent", "x 2026-10-20 Water plants"]
    );
    remove(source);
}

#[test]
fn test_a_cutoff_keeps_items_completed_on_or_after_it() {
    let source = source_with(&format!("{WITH_DONE}x Undated\nx 2026-10-01 On the day\n"));
    archive_items(&source, Some("2026-10-01".parse().unwrap())).unwrap();
    assert_eq!(
        lines(&source),
        [
            "Buy milk",
            "Call mom",
            "x 2026-10-20 Water plants",
            "x 2026-10-01 On the day"
        ]
    );
    assert_eq!(
        archive_lines(&source),
        ["x 2026-09-15 Pay rent", "x Undated"]
    );
    remove(source);
}

#[test]
fn test_nothing_to_archive_writes_nothing() {
    // A line `save` would write differently, to show the file is not rewritten.
    let content = "Buy milk   +groceries\nx 2026-10-20 Water plants\n";
    let source = source_with(content);
    let result = archive_items(&source, Some("2026-10-01".parse().unwrap())).unwrap();
    assert_eq!(
        result,
        format!("No items to archive in '{}'.", source.path.display())
    );
    assert_eq!(std::fs::read_to_string(&source.path).unwrap(), content);
    assert!(!archive_of(&source).exists());
    remove(source);
}

#[test]
fn test_another_file_archives_to_its_own_done_file() {
    let source = source_with("");
    let notes = source.path.with_file_name("notes.txt");
    std::fs::write(&notes, "Open\nx Done\n").unwrap();
    let notes = Source {
        path: notes,
        ..source.clone()
    };
    archive_items(&notes, None).unwrap();
    assert_eq!(lines(&notes), ["Open"]);
    assert_eq!(
        std::fs::read_to_string(notes.path.with_file_name("notes.done.txt")).unwrap(),
        "x Done\n"
    );
    assert!(!archive_of(&source).exists());
    remove(source);
}

#[test]
fn test_a_synced_file_and_its_archive_are_committed_together() {
    let source = synced_source_with(WITH_DONE);
    archive_items(&source, Some("2026-10-01".parse().unwrap())).unwrap();
    let dir = source.path.parent().unwrap();
    assert_eq!(
        git(dir, &["log", "-1", "--format=%s"]),
        "rtm: archive Pay rent"
    );
    assert_eq!(
        git(dir, &["show", "--name-only", "--format=", "HEAD"]),
        "done.txt\ntodo.txt"
    );
    assert_eq!(git(dir, &["status", "--porcelain"]), "");
    remove(source);
}

#[test]
fn test_an_edited_archive_refuses_the_archive() {
    let source = synced_source_with(WITH_DONE);
    let dir = source.path.parent().unwrap();
    std::fs::write(archive_of(&source), "x 2025-12-01 Old\n").unwrap();
    git(dir, &["add", "done.txt"]);
    git(dir, &["commit", "--quiet", "-m", "archive"]);
    std::fs::write(archive_of(&source), "x 2025-12-01 Old\nBy hand\n").unwrap();

    let error = archive_items(&source, None).unwrap_err();
    assert!(
        error.contains("done.txt") && error.contains("not committed by rtm"),
        "{error}"
    );
    assert!(error.ends_with("Nothing was changed."), "{error}");
    assert_eq!(lines(&source).len(), 4);
    assert_eq!(archive_lines(&source), ["x 2025-12-01 Old", "By hand"]);
    assert_eq!(git(dir, &["log", "-1", "--format=%s"]), "archive");
    remove(source);
}

#[test]
fn test_an_ignored_archive_is_left_out_with_a_note() {
    let source = synced_source_with(WITH_DONE);
    let dir = source.path.parent().unwrap();
    std::fs::write(dir.join(".gitignore"), "done.txt\n").unwrap();
    git(dir, &["add", ".gitignore"]);
    git(dir, &["commit", "--quiet", "-m", "ignore"]);

    let result = archive_items(&source, None).unwrap();
    assert!(
        result.ends_with("done.txt' is ignored by Git, so the archived items are not synced."),
        "{result}"
    );
    assert_eq!(
        git(dir, &["show", "--name-only", "--format=", "HEAD"]),
        "todo.txt"
    );
    assert_eq!(archive_lines(&source).len(), 2);
    remove(source);
}
