//! rtmcli archive outside a repository: which done items move to the file's archive.

use chrono::{Days, Local};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("rtmcli archive test {} {name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// rtmcli on `file` with `args`, with the settings from the environment cleared.
fn rtmcli(file: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rtmcli"));
    command
        .arg("-f")
        .arg(file)
        .args(args)
        .env_remove("TODOTXT")
        .env_remove("RTM_NO_SYNC")
        .env_remove("RTM_VCS")
        .env_remove("RTM_UPDATE_URL")
        .env_remove("RTM_ARCHIVE_DAYS");
    command
}

fn run(command: &mut Command) -> Output {
    let output = command.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    output
}

fn days_ago(days: u64) -> String {
    let date = Local::now().date_naive() - Days::new(days);
    date.format("%Y-%m-%d").to_string()
}

#[test]
fn every_done_item_moves_to_done_txt() {
    let dir = scratch("all");
    let todo = dir.join("todo.txt");
    fs::write(
        &todo,
        "Buy milk\nx 2026-01-02 Pay rent\nCall mom\nx Water plants\n",
    )
    .unwrap();

    let output = run(&mut rtmcli(&todo, &["archive"]));

    assert_eq!(fs::read_to_string(&todo).unwrap(), "Buy milk\nCall mom\n");
    assert_eq!(
        fs::read_to_string(dir.join("done.txt")).unwrap(),
        "x 2026-01-02 Pay rent\nx Water plants\n"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Archived 2 item(s)"), "{stdout}");
    assert!(stdout.contains("  x 2026-01-02 Pay rent\n"), "{stdout}");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn older_than_keeps_recently_completed_items() {
    let dir = scratch("older than");
    let todo = dir.join("todo.txt");
    let (recent, old) = (days_ago(1), days_ago(30));
    fs::write(
        &todo,
        format!("x {recent} Recent\nx {old} Old\nx Undated\nOpen\n"),
    )
    .unwrap();

    run(&mut rtmcli(&todo, &["archive", "--older-than", "7"]));

    assert_eq!(
        fs::read_to_string(&todo).unwrap(),
        format!("x {recent} Recent\nOpen\n")
    );
    assert_eq!(
        fs::read_to_string(dir.join("done.txt")).unwrap(),
        format!("x {old} Old\nx Undated\n")
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn rtm_archive_days_sets_the_age_and_the_option_wins() {
    let dir = scratch("env");
    let todo = dir.join("todo.txt");
    let (recent, old) = (days_ago(1), days_ago(30));
    fs::write(&todo, format!("x {recent} Recent\nx {old} Old\n")).unwrap();

    run(rtmcli(&todo, &["archive"]).env("RTM_ARCHIVE_DAYS", "7"));
    assert_eq!(
        fs::read_to_string(&todo).unwrap(),
        format!("x {recent} Recent\n")
    );

    run(rtmcli(&todo, &["archive", "--older-than", "0"]).env("RTM_ARCHIVE_DAYS", "7"));
    assert_eq!(fs::read_to_string(&todo).unwrap(), "");
    assert_eq!(
        fs::read_to_string(dir.join("done.txt")).unwrap(),
        format!("x {old} Old\nx {recent} Recent\n")
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn nothing_to_archive_writes_nothing() {
    let dir = scratch("nothing");
    let todo = dir.join("todo.txt");
    let content = format!("Open\nx {} Done today\n", days_ago(0));
    fs::write(&todo, &content).unwrap();

    let output = run(&mut rtmcli(&todo, &["archive", "--older-than", "0"]));

    assert!(String::from_utf8_lossy(&output.stdout).contains("No items to archive"));
    assert_eq!(fs::read_to_string(&todo).unwrap(), content);
    assert!(!dir.join("done.txt").exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn another_file_archives_to_its_own_done_file() {
    let dir = scratch("other name");
    let notes = dir.join("notes.txt");
    fs::write(&notes, "Open\nx Done\n").unwrap();

    run(&mut rtmcli(&notes, &["archive"]));

    assert_eq!(fs::read_to_string(&notes).unwrap(), "Open\n");
    assert_eq!(
        fs::read_to_string(dir.join("notes.done.txt")).unwrap(),
        "x Done\n"
    );
    assert!(!dir.join("done.txt").exists());
    fs::remove_dir_all(&dir).unwrap();
}
