//! The due-date filters of `list` and `complete`, run on a scratch todo.txt.

use chrono::{Duration, Local};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const UUID: &str = "12345678-1234-1234-1234-123456789abc";

/// The date `days` from today.
fn day(days: i64) -> String {
    (Local::now().date_naive() + Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

/// Open items due yesterday, today, in three days and in ten days, one without a due date, and a
/// completed one due today.
fn fixture() -> Vec<String> {
    vec![
        format!("Pay the rent due:{}", day(-1)),
        format!("Call the bank due:{}", day(0)),
        format!("Renew the passport due:{}", day(3)),
        format!("Service the car due:{} uuid:{UUID}", day(10)),
        "Read a book".to_owned(),
        format!("x {} Water the plants due:{}", day(0), day(0)),
    ]
}

/// A todo.txt holding the fixture, in an empty scratch directory for test `name`.
fn todo_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmcli-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("todo.txt");
    fs::write(&file, fixture().join("\n") + "\n").unwrap();
    file
}

fn cleanup(file: &Path) {
    fs::remove_dir_all(file.parent().unwrap()).unwrap();
}

/// Runs rtmcli on `file` with `args`, with the environment that would change the outcome cleared.
fn rtmcli(file: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rtmcli"))
        .arg("-f")
        .arg(file)
        .args(args)
        .env_remove("RTM_NO_SYNC")
        .env_remove("RTM_VCS")
        .env_remove("TODOTXT")
        .output()
        .unwrap()
}

/// The numbered rows printed by a successful `list`.
fn rows(output: &Output) -> Vec<String> {
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout.clone())
        .unwrap()
        .lines()
        .skip(1)
        .filter(|line| *line != "No items found.")
        .map(str::to_owned)
        .collect()
}

/// `fixture()` rows `indices`, numbered as `list` numbers them.
fn numbered(indices: &[usize]) -> Vec<String> {
    let items = fixture();
    indices
        .iter()
        .enumerate()
        .map(|(n, &i)| format!("{}. {}", n + 1, items[i]))
        .collect()
}

/// The fixture after `complete` today on the open items `indices`.
fn completed(indices: &[usize]) -> Vec<String> {
    let mut items = fixture();
    for &i in indices {
        items[i] = format!("x {} {}", day(0), items[i]);
    }
    items
}

fn file_lines(file: &Path) -> Vec<String> {
    fs::read_to_string(file)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn list_filters_open_items_by_due_date() {
    let file = todo_file("list");

    assert_eq!(rows(&rtmcli(&file, &["list", "today"])), numbered(&[1]));
    assert_eq!(rows(&rtmcli(&file, &["list", "week"])), numbered(&[1, 2]));
    assert_eq!(rows(&rtmcli(&file, &["list", "overdue"])), numbered(&[0]));
    assert_eq!(rows(&rtmcli(&file, &["list", "future"])), numbered(&[2, 3]));
    assert_eq!(
        rows(&rtmcli(&file, &["list", "all"])),
        numbered(&[0, 1, 2, 3, 4])
    );
    assert_eq!(
        rows(&rtmcli(&file, &["list", "-c", "today"])),
        numbered(&[5])
    );
    cleanup(&file);
}

#[test]
fn complete_with_a_filter_and_a_number_completes_that_row() {
    let file = todo_file("filter-number");

    let output = rtmcli(&file, &["complete", "week", "2"]);
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("Completed 1 item(s)"));
    assert_eq!(file_lines(&file), completed(&[2]));
    cleanup(&file);
}

#[test]
fn complete_with_a_filter_and_a_uuid_completes_that_item() {
    let file = todo_file("filter-uuid");

    let output = rtmcli(&file, &["complete", "future", UUID]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(file_lines(&file), completed(&[3]));
    cleanup(&file);
}

#[test]
fn complete_with_only_a_filter_completes_every_match() {
    let file = todo_file("filter-all");

    let output = rtmcli(&file, &["complete", "week"]);
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("Completed 2 item(s)"));
    assert_eq!(file_lines(&file), completed(&[1, 2]));
    cleanup(&file);
}

#[test]
fn complete_with_a_uuid_completes_that_item() {
    let file = todo_file("uuid");

    let output = rtmcli(&file, &["complete", UUID]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(file_lines(&file), completed(&[3]));
    cleanup(&file);
}

#[test]
fn a_number_past_the_filtered_rows_changes_nothing() {
    let file = todo_file("past-rows");

    let output = rtmcli(&file, &["complete", "today", "2"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Invalid index for filter: 2"));
    assert_eq!(file_lines(&file), fixture());
    cleanup(&file);
}
