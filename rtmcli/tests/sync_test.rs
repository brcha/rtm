//! rtmcli syncing a todo.txt through a git repository: scratch bare remotes and clones on local
//! paths, no network.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Runs git in `dir` and returns its trimmed stdout, panicking on failure.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// Runs rtmcli on `file` with `args`, with sync settings from the environment cleared.
fn rtmcli(file: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rtmcli"))
        .arg("-f")
        .arg(file)
        .args(args)
        .env_remove("RTM_NO_SYNC")
        .env_remove("RTM_VCS")
        .output()
        .unwrap()
}

/// An empty scratch directory for test `name`.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmcli sync test {} {name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("no hooks")).unwrap();
    dir
}

/// Local config that keeps the user's global identity, signing, hooks and rerere out.
fn isolate(repo: &Path, scratch: &Path) {
    let hooks = scratch.join("no hooks");
    git(repo, &["config", "user.name", "rtm test"]);
    git(repo, &["config", "user.email", "rtm@example.com"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
    git(repo, &["config", "core.hooksPath", hooks.to_str().unwrap()]);
    git(repo, &["config", "rerere.enabled", "false"]);
}

/// A bare `remote` and clones `a` and `b`, sharing a tracked `todo.txt` holding `lines`.
fn remote_and_clones(name: &str, lines: &str) -> PathBuf {
    let dir = scratch(name);
    git(&dir, &["init", "--quiet", "--bare", "remote"]);
    isolate(&dir.join("remote"), &dir);
    let hooks = format!("core.hooksPath={}", dir.join("no hooks").to_str().unwrap());
    for clone in ["a", "b"] {
        git(&dir, &["clone", "--quiet", "-c", &hooks, "remote", clone]);
        isolate(&dir.join(clone), &dir);
        if clone == "a" {
            std::fs::write(dir.join("a/todo.txt"), lines).unwrap();
            git(&dir.join("a"), &["add", "todo.txt"]);
            git(&dir.join("a"), &["commit", "--quiet", "-m", "start"]);
            git(&dir.join("a"), &["push", "--quiet", "-u", "origin", "HEAD"]);
        }
    }
    git(&dir.join("b"), &["pull", "--quiet"]);
    dir
}

/// Changes `b`'s todo.txt to `lines`, commits and pushes it.
fn push_from_b(dir: &Path, lines: &str) {
    std::fs::write(dir.join("b/todo.txt"), lines).unwrap();
    git(&dir.join("b"), &["commit", "--quiet", "-am", "from b"]);
    git(&dir.join("b"), &["push", "--quiet"]);
}

fn last_subject(repo: &Path) -> String {
    git(repo, &["log", "-1", "--format=%s"])
}

#[test]
fn add_commits_and_pushes() {
    let dir = remote_and_clones("add", "Buy milk\n");
    let output = rtmcli(&dir.join("a/todo.txt"), &["add", "Call mom"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(last_subject(&dir.join("remote")), "rtm: add Call mom");
    let listed = rtmcli(&dir.join("b/todo.txt"), &["list"]);
    assert!(String::from_utf8_lossy(&listed.stdout).contains("Call mom"));
}

#[test]
fn complete_follows_its_item_when_the_pull_renumbers() {
    let dir = remote_and_clones("renumber", "Alpha\nBravo\n");
    push_from_b(&dir, "First\nAlpha\nBravo\n");
    // Number 2 is Bravo in the list `a` saw before the pull.
    let output = rtmcli(&dir.join("a/todo.txt"), &["complete", "2"]);
    assert!(output.status.success(), "{output:?}");
    let text = std::fs::read_to_string(dir.join("a/todo.txt")).unwrap();
    let completed: Vec<_> = text.lines().filter(|l| l.starts_with("x ")).collect();
    assert_eq!(completed.len(), 1, "{text}");
    assert!(completed[0].ends_with("Bravo"), "{text}");
    assert_eq!(last_subject(&dir.join("remote")), "rtm: complete Bravo");
}

#[test]
fn complete_is_rejected_when_the_pull_changed_its_item() {
    let dir = remote_and_clones("changed", "Alpha\nBravo\n");
    push_from_b(&dir, "Alpha\nBravo, renamed\n");
    let output = rtmcli(&dir.join("a/todo.txt"), &["complete", "2"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let text = std::fs::read_to_string(dir.join("a/todo.txt")).unwrap();
    assert!(!text.lines().any(|l| l.starts_with("x ")), "{text}");
    assert_eq!(last_subject(&dir.join("remote")), "from b");
}

#[test]
fn delete_is_rejected_when_the_pull_removed_its_item() {
    let dir = remote_and_clones("removed", "Alpha\nBravo\n");
    push_from_b(&dir, "Alpha\n");
    let output = rtmcli(&dir.join("a/todo.txt"), &["delete", "2", "--yes"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(last_subject(&dir.join("remote")), "from b");
}

#[test]
fn no_sync_neither_commits_nor_pushes() {
    let dir = remote_and_clones("no sync", "Buy milk\n");
    let output = rtmcli(&dir.join("a/todo.txt"), &["--no-sync", "add", "Call mom"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(last_subject(&dir.join("a")), "start");
    assert!(git(&dir.join("a"), &["status", "--porcelain"]).contains("todo.txt"));
}

#[test]
fn rtm_no_sync_reads_falsey_values_as_off() {
    let dir = remote_and_clones("env", "Buy milk\n");
    let run = |value: &str, item: &str| {
        Command::new(env!("CARGO_BIN_EXE_rtmcli"))
            .arg("-f")
            .arg(dir.join("a/todo.txt"))
            .args(["add", item])
            .env("RTM_NO_SYNC", value)
            .env_remove("RTM_VCS")
            .output()
            .unwrap()
    };
    assert!(run("false", "Synced item").status.success());
    assert_eq!(last_subject(&dir.join("remote")), "rtm: add Synced item");
    assert!(run("1", "Local item").status.success());
    assert_eq!(last_subject(&dir.join("a")), "rtm: add Synced item");
}

#[test]
fn a_repository_without_a_remote_commits_quietly() {
    let dir = scratch("no remote");
    git(&dir, &["init", "--quiet"]);
    isolate(&dir, &dir);
    std::fs::write(dir.join("todo.txt"), "Buy milk\n").unwrap();
    git(&dir, &["add", "todo.txt"]);
    git(&dir, &["commit", "--quiet", "-m", "start"]);
    let output = rtmcli(&dir.join("todo.txt"), &["add", "Call mom"]);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(last_subject(&dir), "rtm: add Call mom");
}

#[test]
fn an_untracked_todo_txt_is_left_alone() {
    let dir = remote_and_clones("untracked", "Buy milk\n");
    let other = dir.join("a/other.txt");
    let output = rtmcli(&other, &["add", "Call mom"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(last_subject(&dir.join("a")), "start");
    assert_eq!(last_subject(&dir.join("remote")), "start");
}

#[test]
fn a_conflicted_repository_stops_rtmcli() {
    let dir = remote_and_clones("conflict", "Alpha\n");
    push_from_b(&dir, "Alpha from b\n");
    std::fs::write(dir.join("a/todo.txt"), "Alpha from a\n").unwrap();
    git(&dir.join("a"), &["commit", "--quiet", "-am", "from a"]);
    let pull = Command::new("git")
        .current_dir(dir.join("a"))
        .args(["pull", "--quiet", "--rebase"])
        .output()
        .unwrap();
    assert!(!pull.status.success());
    let output = rtmcli(&dir.join("a/todo.txt"), &["list"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("resolve them by hand"));
}

#[test]
fn offline_changes_are_still_saved_and_committed() {
    let dir = remote_and_clones("offline", "Buy milk\n");
    let missing = dir.join("missing");
    git(
        &dir.join("a"),
        &["remote", "set-url", "origin", missing.to_str().unwrap()],
    );
    let output = rtmcli(&dir.join("a/todo.txt"), &["add", "Call mom"]);
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("Not synced"));
    assert_eq!(last_subject(&dir.join("a")), "rtm: add Call mom");
}

#[test]
fn the_sync_command_fails_when_it_cannot_sync() {
    let dir = remote_and_clones("sync command", "Buy milk\n");
    let synced = rtmcli(&dir.join("a/todo.txt"), &["sync"]);
    assert!(synced.status.success(), "{synced:?}");
    assert!(String::from_utf8_lossy(&synced.stdout).contains("Synced"));
    let missing = dir.join("missing");
    git(
        &dir.join("a"),
        &["remote", "set-url", "origin", missing.to_str().unwrap()],
    );
    let offline = rtmcli(&dir.join("a/todo.txt"), &["sync"]);
    assert_eq!(offline.status.code(), Some(1), "{offline:?}");
}

#[test]
fn a_directory_used_by_two_vcss_needs_a_choice() {
    let dir = remote_and_clones("ambiguous", "Buy milk\n");
    std::fs::create_dir(dir.join("a/.pijul")).unwrap();
    let output = rtmcli(&dir.join("a/todo.txt"), &["list"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let chosen = rtmcli(&dir.join("a/todo.txt"), &["--vcs", "git", "list"]);
    assert!(chosen.status.success(), "{chosen:?}");
}
