//! Sync through git: scratch bare remotes and clones on local paths, no network.

use super::*;
use std::process::Command;

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

/// An empty scratch directory under the system temp directory.
fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmmcp sync {}", uuid::Uuid::new_v4()));
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

/// A bare `remote` and clones `a` and `b`, sharing a tracked `todo.txt` holding "Buy milk".
fn remote_and_clones() -> PathBuf {
    let dir = scratch();
    git(&dir, &["init", "--quiet", "--bare", "remote"]);
    isolate(&dir.join("remote"), &dir);
    let hooks = format!("core.hooksPath={}", dir.join("no hooks").to_str().unwrap());
    for clone in ["a", "b"] {
        git(&dir, &["clone", "--quiet", "-c", &hooks, "remote", clone]);
        isolate(&dir.join(clone), &dir);
        if clone == "a" {
            std::fs::write(dir.join("a/todo.txt"), "Buy milk\n").unwrap();
            git(&dir.join("a"), &["add", "todo.txt"]);
            git(&dir.join("a"), &["commit", "--quiet", "-m", "start"]);
            git(&dir.join("a"), &["push", "--quiet", "-u", "origin", "HEAD"]);
        }
    }
    git(&dir.join("b"), &["pull", "--quiet"]);
    dir
}

/// A repository without a remote, holding a committed `todo.txt`.
fn local_repo() -> PathBuf {
    let dir = scratch();
    git(&dir, &["init", "--quiet", "repo"]);
    isolate(&dir.join("repo"), &dir);
    std::fs::write(dir.join("repo/todo.txt"), "Buy milk\n").unwrap();
    git(&dir.join("repo"), &["add", "todo.txt"]);
    git(&dir.join("repo"), &["commit", "--quiet", "-m", "start"]);
    dir
}

fn source(path: PathBuf) -> Source {
    Source {
        path,
        name: None,
        sync: true,
        vcs: None,
    }
}

fn last_subject(repo: &Path) -> String {
    git(repo, &["log", "-1", "--format=%s"])
}

#[test]
fn test_a_change_is_pulled_before_and_pushed_after() {
    let dir = remote_and_clones();
    std::fs::write(dir.join("b/todo.txt"), "Buy milk\nCall mom\n").unwrap();
    git(&dir.join("b"), &["commit", "--quiet", "-am", "from b"]);
    git(&dir.join("b"), &["push", "--quiet"]);

    let file = dir.join("a/todo.txt");
    let mut sync = RepoWrite::find(&source(file.clone())).unwrap();
    sync.pull().unwrap();
    // By lines: git may check the file out with CRLF line ends (core.autocrlf on Windows).
    assert_eq!(
        std::fs::read_to_string(&file)
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        ["Buy milk", "Call mom"]
    );
    std::fs::write(&file, "Buy milk\nCall mom\nPay rent").unwrap();
    sync.commit_and_push("rtm: add Pay rent");
    assert!(sync.notes.is_empty(), "{:?}", sync.notes);
    assert_eq!(last_subject(&dir.join("remote")), "rtm: add Pay rent");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_without_a_remote_a_change_is_only_committed() {
    let dir = local_repo();
    let file = dir.join("repo/todo.txt");
    let mut sync = RepoWrite::find(&source(file.clone())).unwrap();
    sync.pull().unwrap();
    std::fs::write(&file, "Buy milk\nPay rent").unwrap();
    sync.commit_and_push("rtm: add Pay rent");
    assert!(sync.notes.is_empty(), "{:?}", sync.notes);
    assert_eq!(last_subject(&dir.join("repo")), "rtm: add Pay rent");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_outside_changes_refuse_the_change() {
    let dir = local_repo();
    let file = dir.join("repo/todo.txt");
    std::fs::write(&file, "Buy milk\nEdited by hand\n").unwrap();
    let error = RepoWrite::find(&source(file)).err().unwrap();
    assert!(error.contains("not committed by rtm"), "{error}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_an_untracked_file_is_changed_without_sync() {
    let dir = local_repo();
    let file = dir.join("repo/work.txt");
    std::fs::write(&file, "Write report\n").unwrap();
    let mut sync = RepoWrite::find(&source(file.clone())).unwrap();
    assert!(sync.notes[0].contains("not tracked"), "{:?}", sync.notes);
    std::fs::write(&file, "Write report\nSend report").unwrap();
    sync.commit_and_push("rtm: add Send report");
    assert_eq!(last_subject(&dir.join("repo")), "start");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_sync_off_leaves_the_repository_alone() {
    let dir = local_repo();
    let file = dir.join("repo/todo.txt");
    let mut sync = RepoWrite::find(&Source {
        sync: false,
        ..source(file.clone())
    })
    .unwrap();
    std::fs::write(&file, "Buy milk\nPay rent").unwrap();
    sync.commit_and_push("rtm: add Pay rent");
    assert!(sync.notes.is_empty());
    assert_eq!(last_subject(&dir.join("repo")), "start");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_a_file_outside_any_repository_is_not_synced() {
    let dir = scratch();
    let file = dir.join("todo.txt");
    std::fs::write(&file, "Buy milk\n").unwrap();
    let sync = RepoWrite::find(&source(file)).unwrap();
    assert!(sync.repo.is_none());
    assert!(sync.notes.is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_a_conflicted_repository_refuses_the_change() {
    let dir = remote_and_clones();
    std::fs::write(dir.join("b/todo.txt"), "Buy milk\nFrom b\n").unwrap();
    git(&dir.join("b"), &["commit", "--quiet", "-am", "from b"]);
    git(&dir.join("b"), &["push", "--quiet"]);
    std::fs::write(dir.join("a/todo.txt"), "Buy milk\nFrom a\n").unwrap();
    git(&dir.join("a"), &["commit", "--quiet", "-am", "from a"]);
    let pull = Command::new("git")
        .current_dir(dir.join("a"))
        .args(["pull", "--quiet", "--rebase"])
        .output()
        .unwrap();
    assert!(!pull.status.success(), "the rebase did not stop: {pull:?}");
    let error = RepoWrite::find(&source(dir.join("a/todo.txt")))
        .err()
        .unwrap();
    assert!(error.contains("conflicts"), "{error}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_both_git_and_pijul_without_a_choice_refuse_the_change() {
    let dir = local_repo();
    // Detection looks at the markers only, so an empty .pijul is enough.
    std::fs::create_dir(dir.join("repo/.pijul")).unwrap();
    let error = RepoWrite::find(&source(dir.join("repo/todo.txt")))
        .err()
        .unwrap();
    assert!(error.contains("choose"), "{error}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_a_choice_the_directory_lacks_refuses_the_change() {
    let dir = local_repo();
    let error = RepoWrite::find(&Source {
        vcs: Some(Vcs::Pijul),
        ..source(dir.join("repo/todo.txt"))
    })
    .err()
    .unwrap();
    assert!(error.contains("not Pijul"), "{error}");
    std::fs::remove_dir_all(&dir).unwrap();
}
