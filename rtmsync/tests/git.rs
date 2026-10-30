//! `Vcs::commit` and `Vcs::sync` with Git.

use std::path::{Path, PathBuf};
use std::process::Command;

use rtmsync::{Error, Vcs};

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

/// An empty scratch directory for test `name`.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmsync git test {} {name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("no hooks")).unwrap();
    dir
}

/// Local config that keeps the user's global identity, signing, hooks and rerere out of the tests.
/// `scratch` is the directory holding the empty `no hooks` directory.
fn isolate(repo: &Path, scratch: &Path) {
    let hooks = scratch.join("no hooks");
    git(repo, &["config", "user.name", "rtm test"]);
    git(repo, &["config", "user.email", "rtm@example.com"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
    git(repo, &["config", "core.hooksPath", hooks.to_str().unwrap()]);
    git(repo, &["config", "rerere.enabled", "false"]);
}

/// A scratch git repository holding an empty, untracked `sub/todo.txt`.
fn repo(name: &str) -> PathBuf {
    let dir = scratch(name);
    std::fs::create_dir(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/todo.txt"), "").unwrap();
    git(&dir, &["init", "--quiet"]);
    isolate(&dir, &dir);
    dir
}

/// A scratch bare `remote` and two clones of it, `a` and `b`, sharing a committed and pushed
/// `sub/todo.txt` holding "one". Returns the scratch directory.
fn remote_and_clones(name: &str) -> PathBuf {
    let dir = scratch(name);
    git(&dir, &["init", "--quiet", "--bare", "remote"]);
    isolate(&dir.join("remote"), &dir);
    let hooks = format!("core.hooksPath={}", dir.join("no hooks").to_str().unwrap());
    for clone in ["a", "b"] {
        git(&dir, &["clone", "--quiet", "-c", &hooks, "remote", clone]);
        isolate(&dir.join(clone), &dir);
        if clone == "a" {
            std::fs::create_dir(dir.join("a/sub")).unwrap();
            std::fs::write(dir.join("a/sub/todo.txt"), "one\n").unwrap();
            Vcs::Git.commit(&dir.join("a/sub/todo.txt"), "one").unwrap();
            git(&dir.join("a"), &["push", "--quiet", "-u", "origin", "HEAD"]);
        }
    }
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

#[test]
fn sync_pushes_a_local_commit() {
    let dir = remote_and_clones("push");
    std::fs::write(dir.join("a/sub/todo.txt"), "one\ntwo\n").unwrap();
    Vcs::Git.commit(&dir.join("a/sub/todo.txt"), "two").unwrap();
    Vcs::Git.sync(&dir.join("a/sub/todo.txt")).unwrap();
    assert_eq!(
        git(&dir.join("remote"), &["log", "-1", "--format=%s"]),
        "two"
    );
}

#[test]
fn sync_rebases_local_commits_onto_the_remote() {
    let dir = remote_and_clones("rebase");
    std::fs::write(dir.join("b/other.txt"), "other\n").unwrap();
    git(&dir.join("b"), &["add", "other.txt"]);
    git(&dir.join("b"), &["commit", "--quiet", "-m", "other"]);
    git(&dir.join("b"), &["push", "--quiet"]);
    std::fs::write(dir.join("a/sub/todo.txt"), "one\ntwo\n").unwrap();
    Vcs::Git.commit(&dir.join("a/sub/todo.txt"), "two").unwrap();
    Vcs::Git.sync(&dir.join("a/sub/todo.txt")).unwrap();
    assert_eq!(
        git(&dir.join("remote"), &["log", "-3", "--format=%s"]),
        "two\nother\none"
    );
}

#[test]
fn sync_with_nothing_to_do_succeeds() {
    let dir = remote_and_clones("nothing");
    Vcs::Git.sync(&dir.join("a/sub/todo.txt")).unwrap();
}

#[test]
fn sync_stops_on_a_conflict_and_leaves_it_to_the_user() {
    let dir = remote_and_clones("conflict");
    git(&dir.join("b"), &["pull", "--quiet"]);
    std::fs::write(dir.join("b/sub/todo.txt"), "one\nfrom b\n").unwrap();
    Vcs::Git.commit(&dir.join("b/sub/todo.txt"), "b").unwrap();
    git(&dir.join("b"), &["push", "--quiet"]);
    std::fs::write(dir.join("a/sub/todo.txt"), "one\nfrom a\n").unwrap();
    Vcs::Git.commit(&dir.join("a/sub/todo.txt"), "a").unwrap();
    let result = Vcs::Git.sync(&dir.join("a/sub/todo.txt"));
    assert!(matches!(result, Err(Error::Conflict)), "{result:?}");
    // The stopped rebase stays for the user, and nothing was pushed.
    assert!(dir.join("a/.git/rebase-merge").exists());
    assert_eq!(git(&dir.join("remote"), &["log", "-1", "--format=%s"]), "b");
}

#[test]
fn sync_without_an_upstream_fails() {
    let dir = repo("no upstream");
    Vcs::Git.commit(&dir.join("sub/todo.txt"), "first").unwrap();
    let result = Vcs::Git.sync(&dir.join("sub/todo.txt"));
    assert!(matches!(result, Err(Error::Failed { .. })), "{result:?}");
}

#[test]
fn sync_treats_a_resolved_but_unfinished_rebase_as_a_conflict() {
    let dir = remote_and_clones("unfinished");
    git(&dir.join("b"), &["pull", "--quiet"]);
    std::fs::write(dir.join("b/sub/todo.txt"), "one\nfrom b\n").unwrap();
    Vcs::Git.commit(&dir.join("b/sub/todo.txt"), "b").unwrap();
    git(&dir.join("b"), &["push", "--quiet"]);
    std::fs::write(dir.join("a/sub/todo.txt"), "one\nfrom a\n").unwrap();
    Vcs::Git.commit(&dir.join("a/sub/todo.txt"), "a").unwrap();
    assert!(matches!(
        Vcs::Git.sync(&dir.join("a/sub/todo.txt")),
        Err(Error::Conflict)
    ));
    // The user resolves the file but does not run `git rebase --continue`.
    std::fs::write(dir.join("a/sub/todo.txt"), "one\nfrom b\nfrom a\n").unwrap();
    git(&dir.join("a"), &["add", "sub/todo.txt"]);
    let result = Vcs::Git.sync(&dir.join("a/sub/todo.txt"));
    assert!(matches!(result, Err(Error::Conflict)), "{result:?}");
    assert_eq!(git(&dir.join("remote"), &["log", "-1", "--format=%s"]), "b");
}
