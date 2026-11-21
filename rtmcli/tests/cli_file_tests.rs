//! How rtmcli finds its todo.txt: `-f`, then `$TODOTXT`, then `todo.txt` in the working directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
        .env_remove("RTM_UPDATE_URL")
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

/// The message refusing `link`, a symbolic link to `target`.
#[cfg(unix)]
fn refusal(link: &Path, target: &Path) -> String {
    // canonicalize: the temp directory is reached through a symlink on macOS.
    let target = target.canonicalize().unwrap();
    format!(
        "Error: '{}' is a symbolic link to '{}'; open '{}' instead",
        link.display(),
        target.display(),
        target.display()
    )
}

#[cfg(unix)]
#[test]
fn adding_to_a_symlinked_file_is_refused_and_writes_nothing() {
    let dir = scratch("symlink-add");
    let target = dir.join("real.txt");
    fs::write(&target, "Buy milk\n").unwrap();
    let link = dir.join("todo.txt");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let add = rtmcli(&dir)
        .arg("-f")
        .arg(&link)
        .args(["add", "Call mom"])
        .output()
        .unwrap();
    assert_eq!(add.status.code(), Some(1), "{add:?}");
    assert_eq!(
        String::from_utf8_lossy(&add.stderr).trim(),
        refusal(&link, &target)
    );
    assert_eq!(file_lines(&target), ["Buy milk"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn listing_a_symlinked_todotxt_is_refused() {
    let dir = scratch("symlink-list");
    let target = dir.join("real.txt");
    fs::write(&target, "Buy milk\n").unwrap();
    let link = dir.join("todo.txt");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let list = rtmcli(&dir)
        .env("TODOTXT", &link)
        .arg("list")
        .output()
        .unwrap();
    assert_eq!(list.status.code(), Some(1), "{list:?}");
    assert_eq!(
        String::from_utf8_lossy(&list.stderr).trim(),
        refusal(&link, &target)
    );
    assert!(list.stdout.is_empty());
    fs::remove_dir_all(&dir).unwrap();
}
