//! `Vcs::commit` with Git.

use std::path::{Path, PathBuf};
use std::process::Command;

use rtmsync::Vcs;

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

/// A scratch git repository holding an empty, untracked `sub/todo.txt`. Its local config keeps the
/// user's global identity, signing and hooks out of the tests.
fn repo(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmsync git test {} {name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::create_dir(dir.join("no hooks")).unwrap();
    std::fs::write(dir.join("sub/todo.txt"), "").unwrap();
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["config", "user.name", "rtm test"]);
    git(&dir, &["config", "user.email", "rtm@example.com"]);
    git(&dir, &["config", "commit.gpgsign", "false"]);
    git(&dir, &["config", "core.hooksPath", "no hooks"]);
    dir
}

#[test]
fn an_untracked_file_is_added_and_committed() {
    let dir = repo("untracked");
    Vcs::Git
        .commit(&dir.join("sub/todo.txt"), "rtm: add")
        .unwrap();
    assert_eq!(git(&dir, &["log", "-1", "--format=%s"]), "rtm: add");
    assert_eq!(
        git(&dir, &["show", "--name-only", "--format=", "HEAD"]),
        "sub/todo.txt"
    );
}

#[test]
fn only_the_file_is_committed() {
    let dir = repo("only");
    Vcs::Git.commit(&dir.join("sub/todo.txt"), "first").unwrap();
    std::fs::write(dir.join("other.txt"), "staged\n").unwrap();
    git(&dir, &["add", "other.txt"]);
    std::fs::write(dir.join("sub/todo.txt"), "Buy milk\n").unwrap();
    Vcs::Git
        .commit(&dir.join("sub/todo.txt"), "second")
        .unwrap();
    assert_eq!(
        git(&dir, &["show", "--name-only", "--format=", "HEAD"]),
        "sub/todo.txt"
    );
    assert_eq!(git(&dir, &["diff", "--cached", "--name-only"]), "other.txt");
}

#[test]
fn an_unchanged_file_commits_nothing() {
    let dir = repo("unchanged");
    Vcs::Git.commit(&dir.join("sub/todo.txt"), "first").unwrap();
    Vcs::Git
        .commit(&dir.join("sub/todo.txt"), "second")
        .unwrap();
    assert_eq!(git(&dir, &["rev-list", "--count", "HEAD"]), "1");
}
