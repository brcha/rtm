//! Round trips: a line that is read and written again without changes comes out byte for byte
//! the same, and so does a file loaded and saved through `TodoLibrary`. The specification
//! (https://github.com/todotxt/todo.txt) lets projects, contexts and key:value tags appear
//! anywhere after the priority and dates (incomplete tasks, rule 3), and a todo.txt is shared
//! with other tools and plain text editors, so rtm must not rewrite what it did not change.

use std::fs;
use std::path::{Path, PathBuf};
use todotxt::{TodoItem, TodoLibrary};

fn round_trip(line: &str) -> String {
    line.parse::<TodoItem>()
        .unwrap_or_else(|e| panic!("{line:?} does not parse: {e}"))
        .to_string()
}

fn assert_round_trip(line: &str) {
    assert_eq!(round_trip(line), line);
}

/// A todo.txt with exactly `content`, in an empty scratch directory for test `name`.
fn todo_file(name: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("todotxt-spec-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("todo.txt");
    fs::write(&file, content).unwrap();
    file
}

/// The file's content after a load and a save with no change in between.
fn load_and_save(file: &Path) -> String {
    let mut library = TodoLibrary::new(file.to_str().unwrap().to_owned());
    library.load().unwrap();
    library.save().unwrap();
    let content = fs::read_to_string(file).unwrap();
    fs::remove_dir_all(file.parent().unwrap()).unwrap();
    content
}

// Lines

/// Every field in the order rtm writes them comes back unchanged.
#[test]
fn a_line_with_every_field_in_order_comes_back_unchanged() {
    assert_round_trip(
        "(A) 2026-01-05 Call the plumber +home @phone due:2026-01-10 rec:+w t:2026-01-08",
    );
    assert_round_trip("x 2026-01-06 2026-01-05 Call the plumber pri:A +home @phone");
}

/// Incomplete tasks, rule 3: projects and contexts between the words of the description stay
/// where they are.
#[test]
#[ignore = "RTM-204: tags move to the end of the line and spacing collapses"]
fn projects_and_contexts_stay_in_place() {
    assert_round_trip("Call +Mom about @home stuff");
    assert_round_trip("(B) 2026-01-05 @phone Call Mom +Family about the trip");
}

/// Additional definitions: key:value tags stay where they are, known and unknown alike, in their
/// order.
#[test]
#[ignore = "RTM-204: tags move to the end of the line and spacing collapses"]
fn key_value_tags_stay_in_place_and_in_order() {
    assert_round_trip("due:2026-01-10 Pay the rent");
    assert_round_trip("Pay the rent h:1 due:2026-01-10 id:7");
    assert_round_trip("Pay the rent t:2026-01-08 due:2026-01-10");
}

/// Unknown tags that rtm leaves in the description come back in their order.
#[test]
fn unknown_tags_come_back_in_order() {
    assert_round_trip("Pay the rent id:7 h:1 foo:bar");
}

/// A tag that appears twice is kept twice.
#[test]
#[ignore = "RTM-204: tags move to the end of the line and spacing collapses"]
fn repeated_tags_are_kept() {
    assert_round_trip("Pay the rent due:2026-01-10 due:2026-01-11");
    assert_round_trip("Paint the fence +home +home");
}

/// The spacing of the line is kept: rtm did not change it.
#[test]
#[ignore = "RTM-204: tags move to the end of the line and spacing collapses"]
fn spacing_is_kept() {
    assert_round_trip("Pay  the rent");
    assert_round_trip("Pay the rent ");
    assert_round_trip("Pay\tthe rent");
}

/// Non-ASCII text, projects and contexts come back unchanged.
#[test]
#[ignore = "RTM-204: tags move to the end of the line and spacing collapses"]
fn non_ascii_text_comes_back_unchanged() {
    assert_round_trip("(C) 2026-10-11 Инфостан +рачуни @кућа due:2026-11-05 rec:+m t:2026-03-15");
    assert_round_trip("Платити +рачуни гаражу @кућа на време");
    assert_round_trip("買い物 +家 牛乳 @店");
}

// Files

/// A file loaded and saved with no change is byte for byte the same, its final newline
/// included: another tool appending a line to a file without one would join it to the last
/// task.
#[test]
#[ignore = "RTM-210: save drops the final newline"]
fn an_unchanged_file_is_saved_byte_for_byte() {
    let content = "(A) 2026-01-05 Call the plumber +home @phone due:2026-01-10\n\
                   x 2026-01-04 2026-01-02 File the taxes pri:B +admin\n\
                   2026-01-03 Water the plants rec:+w t:2026-01-06\n";
    let file = todo_file("unchanged", content);
    assert_eq!(load_and_save(&file), content);
}

/// A file without a final newline stays without one.
#[test]
fn a_file_without_a_final_newline_stays_so() {
    let content = "Call Mom\nPay the rent";
    let file = todo_file("no-final-newline", content);
    assert_eq!(load_and_save(&file), content);
}

/// A file with CRLF line endings, as Windows editors write it, is read without the carriage
/// returns and saved with LF line endings, the only exception to the byte-for-byte rule.
#[test]
#[ignore = "RTM-210: save drops the final newline"]
fn crlf_line_endings_are_saved_as_lf() {
    let file = todo_file("crlf", "Call Mom\r\nPay the rent\r\n");
    assert_eq!(load_and_save(&file), "Call Mom\nPay the rent\n");
}

/// A line rtm cannot interpret is never lost: loading and saving keeps every line of the file.
/// The lines are a negative example of incomplete tasks rule 1, known tags with values rtm cannot
/// read (additional definitions: key:value is free-form) and a conflicting priority.
#[test]
#[ignore = "RTM-203: lines rtm cannot interpret fail to parse and are dropped"]
fn lines_rtm_cannot_interpret_are_kept() {
    let content = "Call Mom\n\
                   (b) Get back to the boss\n\
                   Pay the rent due:soon\n\
                   Water the plants rec:often\n\
                   (C) Call the plumber pri:A\n\
                   Pay the rent\n";
    let file = todo_file("uninterpretable", content);
    assert_eq!(load_and_save(&file), content);
}
