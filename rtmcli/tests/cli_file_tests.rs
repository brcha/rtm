//! How rtmcli finds its todo.txt: `-f`, then `$TODOTXT`, then `todo.txt` in the working directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// An empty scratch directory for test `name`.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmcli-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// rtmcli started in `dir`, with the environment that would change the outcome cleared.
fn rtmcli(dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rtmcli"));
    command
        .current_dir(dir)
        .env_remove("RTM_NO_SYNC")
        .env_remove("RTM_VCS")
        .env_remove("TODOTXT");
    command
}

fn file_lines(file: &Path) -> Vec<String> {
    fs::read_to_string(file)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn today() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

#[test]
fn todotxt_names_the_file() {
    let dir = scratch("todotxt");
    let file = dir.join("named.txt");
    fs::write(&file, "Buy milk\n").unwrap();

    let list = rtmcli(&dir)
        .env("TODOTXT", &file)
        .arg("list")
        .output()
        .unwrap();
    assert!(list.status.success(), "{list:?}");
    assert!(String::from_utf8_lossy(&list.stdout).contains("1. Buy milk"));

    let add = rtmcli(&dir)
        .env("TODOTXT", &file)
        .args(["add", "Call mom"])
        .output()
        .unwrap();
    assert!(add.status.success(), "{add:?}");
    assert_eq!(
        file_lines(&file),
        ["Buy milk".to_owned(), format!("{} Call mom", today())]
    );
    assert!(!dir.join("todo.txt").exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn without_f_or_todotxt_the_file_is_todo_txt_in_the_working_directory() {
    let dir = scratch("default");

    let add = rtmcli(&dir).args(["add", "Buy milk"]).output().unwrap();
    assert!(add.status.success(), "{add:?}");
    assert_eq!(
        file_lines(&dir.join("todo.txt")),
        [format!("{} Buy milk", today())]
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn f_takes_precedence_over_todotxt() {
    let dir = scratch("precedence");
    let file = dir.join("chosen.txt");
    let other = dir.join("other.txt");

    let add = rtmcli(&dir)
        .env("TODOTXT", &other)
        .arg("-f")
        .arg(&file)
        .args(["add", "Buy milk"])
        .output()
        .unwrap();
    assert!(add.status.success(), "{add:?}");
    assert_eq!(file_lines(&file), [format!("{} Buy milk", today())]);
    assert!(!other.exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn listing_a_missing_file_is_an_error_that_names_it() {
    let dir = scratch("missing");
    let file = dir.join("missing.txt");

    let list = rtmcli(&dir)
        .arg("-f")
        .arg(&file)
        .arg("list")
        .output()
        .unwrap();
    assert_eq!(list.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&list.stderr).contains(&file.display().to_string()));
    assert!(!file.exists());
    fs::remove_dir_all(&dir).unwrap();
}
