use super::*;
use std::fs;

/// A scratch directory holding `todo.txt` with `content`, loaded into a library.
fn scratch(content: &str) -> (PathBuf, TodoLibrary) {
    let dir = std::env::temp_dir().join(format!("todotxt-archive-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let file = dir.join("todo.txt");
    fs::write(&file, content).unwrap();
    let mut lib = TodoLibrary::new(file.to_str().unwrap().to_string());
    lib.load().unwrap();
    (dir, lib)
}

fn date(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn lines(lib: &TodoLibrary) -> Vec<String> {
    lib.items.iter().map(|item| item.to_string()).collect()
}

#[test]
fn archive_path_of_todo_txt_is_done_txt() {
    let lib = TodoLibrary::new("dir/todo.txt".to_string());
    assert_eq!(lib.archive_path(), Path::new("dir/done.txt"));
}

#[test]
fn archive_path_of_another_file_inserts_done_before_the_extension() {
    let lib = TodoLibrary::new("dir/work.txt".to_string());
    assert_eq!(lib.archive_path(), Path::new("dir/work.done.txt"));
    let lib = TodoLibrary::new("dir/notes".to_string());
    assert_eq!(lib.archive_path(), Path::new("dir/notes.done"));
    let lib = TodoLibrary::new("Todo.txt".to_string());
    assert_eq!(lib.archive_path(), Path::new("Todo.done.txt"));
}

#[cfg(unix)]
#[test]
fn archive_path_of_a_symlinked_todo_file_is_next_to_its_target() {
    let (dir, _) = scratch("");
    let real = dir.join("real");
    fs::create_dir(&real).unwrap();
    fs::rename(dir.join("todo.txt"), real.join("todo.txt")).unwrap();
    let link = dir.join("link.txt");
    std::os::unix::fs::symlink(real.join("todo.txt"), &link).unwrap();
    let lib = TodoLibrary::new(link.to_str().unwrap().to_string());
    // Canonical, since macOS resolves the temp directory itself (/tmp to /private/tmp).
    let expected = fs::canonicalize(&real).unwrap().join("done.txt");
    assert_eq!(lib.archive_path(), expected);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn archive_moves_every_done_item_and_keeps_the_open_ones() {
    let (dir, mut lib) = scratch(
        "(A) Open task +p\n\
         x 2026-11-01 2026-10-01 Done  with  spaces +p pri:B\n\
         Another open\n\
         x Undated done unknown:tag\n",
    );
    let moved = lib.archive(None).unwrap();
    let [first, second] = moved.as_slice() else {
        panic!("two archived items: {moved:?}");
    };
    assert_eq!(first.description, "Done with spaces");
    assert_eq!(second.description, "Undated done unknown:tag");
    assert_eq!(lines(&lib), ["(A) Open task +p", "Another open"]);
    assert_eq!(
        fs::read_to_string(dir.join("done.txt")).unwrap(),
        "x 2026-11-01 2026-10-01 Done  with  spaces +p pri:B\nx Undated done unknown:tag\n"
    );
    lib.save().unwrap();
    assert_eq!(
        fs::read_to_string(dir.join("todo.txt")).unwrap(),
        "(A) Open task +p\nAnother open\n"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn archive_appends_to_an_existing_archive() {
    let (dir, mut lib) = scratch("x New done\nOpen\n");
    fs::write(dir.join("done.txt"), "x Old done\n").unwrap();
    lib.archive(None).unwrap();
    assert_eq!(
        fs::read_to_string(dir.join("done.txt")).unwrap(),
        "x Old done\nx New done\n"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn archive_ends_an_archive_without_a_final_newline_first() {
    let (dir, mut lib) = scratch("x New done\n");
    fs::write(dir.join("done.txt"), "x Old done").unwrap();
    lib.archive(None).unwrap();
    assert_eq!(
        fs::read_to_string(dir.join("done.txt")).unwrap(),
        "x Old done\nx New done\n"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn archive_with_nothing_done_creates_no_archive() {
    let (dir, mut lib) = scratch("Open\n(B) Another\n");
    assert!(lib.archive(None).unwrap().is_empty());
    assert_eq!(lines(&lib), ["Open", "(B) Another"]);
    assert!(!dir.join("done.txt").exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn archive_before_a_date_keeps_items_completed_on_or_after_it() {
    let (dir, mut lib) = scratch(
        "x 2026-11-09 Older\n\
         x 2026-11-10 On the day\n\
         x 2026-11-15 Newer\n\
         x Undated\n\
         Open\n",
    );
    let moved = lib.archive(Some(date("2026-11-10"))).unwrap();
    let descriptions: Vec<&str> = moved.iter().map(|i| i.description.as_str()).collect();
    assert_eq!(descriptions, ["Older", "Undated"]);
    assert_eq!(
        lines(&lib),
        ["x 2026-11-10 On the day", "x 2026-11-15 Newer", "Open"]
    );
    assert_eq!(
        fs::read_to_string(dir.join("done.txt")).unwrap(),
        "x 2026-11-09 Older\nx Undated\n"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn archive_that_cannot_write_leaves_the_library_unchanged() {
    let (dir, mut lib) = scratch("x Done\nOpen\n");
    fs::create_dir(dir.join("done.txt")).unwrap();
    assert!(lib.archive(None).is_err());
    assert_eq!(lines(&lib), ["x Done", "Open"]);
    fs::remove_dir_all(&dir).unwrap();
}
