use std::path::{Path, PathBuf};
use std::process::Command;

use rtmsync::{Checked, Error, Refusal, State, Vcs, check};

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

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
fn two_files_are_committed_together() {
    let dir = repo("two files");
    let (todo, done) = (dir.join("sub/todo.txt"), dir.join("sub/done.txt"));
    Vcs::Git.commit(&todo, "first").unwrap();
    std::fs::write(dir.join("other.txt"), "staged\n").unwrap();
    git(&dir, &["add", "other.txt"]);
    std::fs::write(&todo, "Open\n").unwrap();
    std::fs::write(&done, "x Done\n").unwrap();
    Vcs::Git
        .commit_files(&[&todo, &done], "rtm: archive")
        .unwrap();
    assert_eq!(git(&dir, &["log", "-1", "--format=%s"]), "rtm: archive");
    assert_eq!(
        git(&dir, &["show", "--name-only", "--format=", "HEAD"]),
        "sub/done.txt\nsub/todo.txt"
    );
    assert_eq!(git(&dir, &["diff", "--cached", "--name-only"]), "other.txt");
}

#[test]
fn an_ignored_file_is_left_out() {
    let dir = repo("ignored archive");
    let (todo, done) = (dir.join("sub/todo.txt"), dir.join("sub/done.txt"));
    std::fs::write(dir.join(".gitignore"), "done.txt\n").unwrap();
    std::fs::write(&done, "x Done\n").unwrap();
    Vcs::Git
        .commit_files(&[&todo, &done], "rtm: archive")
        .unwrap();
    assert_eq!(
        git(&dir, &["show", "--name-only", "--format=", "HEAD"]),
        "sub/todo.txt"
    );
    assert_eq!(Vcs::Git.state(&done).unwrap(), State::Ignored);
}

#[test]
fn unchanged_files_commit_nothing() {
    let dir = repo("unchanged files");
    let (todo, done) = (dir.join("sub/todo.txt"), dir.join("sub/done.txt"));
    std::fs::write(&done, "x Done\n").unwrap();
    Vcs::Git.commit_files(&[&todo, &done], "first").unwrap();
    Vcs::Git.commit_files(&[&todo, &done], "second").unwrap();
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
    // A detached HEAD has no upstream, so callers must not rely on has_remote to reach sync.
    assert!(Vcs::Git.is_conflicted(&dir.join("a/sub/todo.txt")).unwrap());
    let result = Vcs::Git.sync(&dir.join("a/sub/todo.txt"));
    assert!(matches!(result, Err(Error::Conflict)), "{result:?}");
    assert_eq!(git(&dir.join("remote"), &["log", "-1", "--format=%s"]), "b");
}

#[test]
fn a_file_is_tracked_once_committed() {
    let dir = repo("tracked");
    assert!(!Vcs::Git.is_conflicted(&dir.join("sub/todo.txt")).unwrap());
    assert!(!Vcs::Git.is_tracked(&dir.join("sub/todo.txt")).unwrap());
    Vcs::Git.commit(&dir.join("sub/todo.txt"), "first").unwrap();
    assert!(Vcs::Git.is_tracked(&dir.join("sub/todo.txt")).unwrap());
}

#[test]
fn only_a_clone_with_an_upstream_has_a_remote() {
    let plain = repo("no remote");
    Vcs::Git
        .commit(&plain.join("sub/todo.txt"), "first")
        .unwrap();
    assert!(!Vcs::Git.has_remote(&plain.join("sub/todo.txt")).unwrap());
    let dir = remote_and_clones("remote");
    assert!(Vcs::Git.has_remote(&dir.join("a/sub/todo.txt")).unwrap());
}

#[test]
fn commit_waits_out_a_briefly_held_index_lock() {
    let dir = repo("short lock");
    Vcs::Git.commit(&dir.join("sub/todo.txt"), "first").unwrap();
    std::fs::write(dir.join("sub/todo.txt"), "Buy milk\n").unwrap();
    // Another git process, such as rtmcli, holding the index for a moment.
    let lock = dir.join(".git/index.lock");
    std::fs::write(&lock, "").unwrap();
    let releaser = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::remove_file(lock).unwrap();
    });
    Vcs::Git
        .commit(&dir.join("sub/todo.txt"), "second")
        .unwrap();
    releaser.join().unwrap();
    assert_eq!(git(&dir, &["log", "-1", "--format=%s"]), "second");
}

#[test]
fn commit_gives_up_on_an_index_lock_that_stays() {
    let dir = repo("held lock");
    let lock = dir.join(".git/index.lock");
    std::fs::write(&lock, "").unwrap();
    let result = Vcs::Git.commit(&dir.join("sub/todo.txt"), "first");
    assert!(matches!(result, Err(Error::Failed { .. })), "{result:?}");
    // rtm never removes another process's lock.
    assert!(lock.exists());
}

#[test]
fn state_tells_untracked_clean_and_modified() {
    let dir = repo("state");
    let todo = dir.join("sub/todo.txt");
    assert_eq!(Vcs::Git.state(&todo).unwrap(), State::Untracked);

    git(&dir, &["add", "sub/todo.txt"]);
    assert_eq!(
        Vcs::Git.state(&todo).unwrap(),
        State::Modified,
        "added, never committed"
    );
    Vcs::Git.commit(&todo, "one").unwrap();
    assert_eq!(Vcs::Git.state(&todo).unwrap(), State::Clean);

    // Another dirty file leaves the todo.txt clean.
    std::fs::write(dir.join("other.txt"), "other\n").unwrap();
    git(&dir, &["add", "other.txt"]);
    assert_eq!(Vcs::Git.state(&todo).unwrap(), State::Clean);

    std::fs::write(&todo, "one\n").unwrap();
    assert_eq!(Vcs::Git.state(&todo).unwrap(), State::Modified, "unstaged");
    git(&dir, &["add", "sub/todo.txt"]);
    assert_eq!(Vcs::Git.state(&todo).unwrap(), State::Modified, "staged");
    std::fs::write(&todo, "one\ntwo\n").unwrap();
    assert_eq!(
        Vcs::Git.state(&todo).unwrap(),
        State::Modified,
        "partly staged"
    );
}

#[test]
fn state_tells_a_file_ignored_by_name_or_by_its_directory() {
    let dir = repo("ignored");
    let todo = dir.join("sub/todo.txt");
    std::fs::write(dir.join(".gitignore"), "todo.txt\n").unwrap();
    assert_eq!(Vcs::Git.state(&todo).unwrap(), State::Ignored);
    std::fs::write(dir.join(".gitignore"), "sub/\n").unwrap();
    assert_eq!(Vcs::Git.state(&todo).unwrap(), State::Ignored);
}

/// Clone `a` of `remote_and_clones`, stopped in a rebase whose conflict in `sub/todo.txt` the user
/// resolved and staged without running `git rebase --continue`. Returns the scratch directory.
fn stopped_rebase(name: &str) -> PathBuf {
    let dir = remote_and_clones(name);
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
    std::fs::write(dir.join("a/sub/todo.txt"), "one\nfrom b\nfrom a\n").unwrap();
    git(&dir.join("a"), &["add", "sub/todo.txt"]);
    dir
}

#[test]
fn check_leaves_a_file_outside_a_repository_unsynced() {
    let dir = scratch("check none");
    let todo = dir.join("todo.txt");
    std::fs::write(&todo, "").unwrap();
    let result = check(&todo, None, true);
    assert!(matches!(result, Ok(Checked::NotVersioned)), "{result:?}");
}

#[test]
fn check_leaves_an_ignored_file_unsynced() {
    let dir = repo("check ignored");
    std::fs::write(dir.join(".gitignore"), "todo.txt\n").unwrap();
    let result = check(&dir.join("sub/todo.txt"), None, true);
    assert!(matches!(result, Ok(Checked::NotVersioned)), "{result:?}");
}

#[test]
fn check_syncs_an_untracked_file_only_when_the_caller_adds_it() {
    let dir = repo("check untracked");
    let todo = dir.join("sub/todo.txt");
    let result = check(&todo, None, false);
    assert!(
        matches!(result, Ok(Checked::Untracked(Vcs::Git))),
        "{result:?}"
    );
    let result = check(&todo, None, true);
    assert!(
        matches!(
            result,
            Ok(Checked::Sync {
                vcs: Vcs::Git,
                state: State::Untracked,
                remote: false
            })
        ),
        "{result:?}"
    );
}

#[test]
fn check_syncs_a_missing_file_only_when_it_is_tracked() {
    let dir = repo("check missing");
    let result = check(&dir.join("sub/never.txt"), None, true);
    assert!(matches!(result, Ok(Checked::NotVersioned)), "{result:?}");
    let todo = dir.join("sub/todo.txt");
    Vcs::Git.commit(&todo, "one").unwrap();
    std::fs::remove_file(&todo).unwrap();
    // Not `Modified`: committing the deletion would remove every item.
    let result = check(&todo, None, true);
    assert!(
        matches!(
            result,
            Ok(Checked::Sync {
                state: State::Clean,
                ..
            })
        ),
        "{result:?}"
    );
}

#[test]
fn check_refuses_outside_changes_unless_the_caller_commits_them() {
    let dir = repo("check modified");
    let todo = dir.join("sub/todo.txt");
    Vcs::Git.commit(&todo, "one").unwrap();
    let result = check(&todo, None, false);
    assert!(
        matches!(
            result,
            Ok(Checked::Sync {
                vcs: Vcs::Git,
                state: State::Clean,
                remote: false
            })
        ),
        "{result:?}"
    );
    std::fs::write(&todo, "by hand\n").unwrap();
    assert_eq!(check(&todo, None, false).unwrap_err(), Refusal::Modified);
    let result = check(&todo, None, true);
    assert!(
        matches!(
            result,
            Ok(Checked::Sync {
                state: State::Modified,
                ..
            })
        ),
        "{result:?}"
    );
}

#[test]
fn check_finds_the_remote_of_a_clone() {
    let dir = remote_and_clones("check remote");
    let result = check(&dir.join("a/sub/todo.txt"), None, false);
    assert!(
        matches!(
            result,
            Ok(Checked::Sync {
                vcs: Vcs::Git,
                state: State::Clean,
                remote: true
            })
        ),
        "{result:?}"
    );
}

#[test]
fn check_finds_a_conflict_before_outside_changes() {
    let dir = stopped_rebase("check conflict");
    let todo = dir.join("a/sub/todo.txt");
    assert_eq!(Vcs::Git.state(&todo).unwrap(), State::Modified);
    for commit_changes in [false, true] {
        assert_eq!(
            check(&todo, None, commit_changes).unwrap_err(),
            Refusal::Conflicted(State::Modified),
            "commit_changes: {commit_changes}"
        );
    }
}

#[test]
fn check_tells_an_untracked_file_in_a_conflicted_repository() {
    let dir = stopped_rebase("check conflict untracked");
    let new = dir.join("a/sub/new.txt");
    std::fs::write(&new, "").unwrap();
    let result = check(&new, None, false);
    assert!(
        matches!(result, Ok(Checked::Untracked(Vcs::Git))),
        "{result:?}"
    );
    assert_eq!(
        check(&new, None, true).unwrap_err(),
        Refusal::Conflicted(State::Untracked)
    );
}

#[test]
fn check_needs_a_choice_the_directory_has() {
    let dir = repo("check choice");
    let todo = dir.join("sub/todo.txt");
    Vcs::Git.commit(&todo, "one").unwrap();
    assert_eq!(
        check(&todo, Some(Vcs::Pijul), false).unwrap_err(),
        Refusal::NotChosen {
            found: Vcs::Git,
            chosen: Vcs::Pijul
        }
    );
    // Detection looks at markers only, so an empty `.pijul` makes the directory ambiguous.
    std::fs::create_dir(dir.join(".pijul")).unwrap();
    assert_eq!(
        check(&todo, None, false).unwrap_err(),
        Refusal::Ambiguous(vec![Vcs::Git, Vcs::Pijul])
    );
    let result = check(&todo, Some(Vcs::Git), false);
    assert!(
        matches!(
            result,
            Ok(Checked::Sync {
                vcs: Vcs::Git,
                state: State::Clean,
                ..
            })
        ),
        "{result:?}"
    );
}

#[test]
fn check_does_not_refuse_a_file_when_a_check_fails() {
    let dir = scratch("check broken");
    std::fs::create_dir(dir.join("sub")).unwrap();
    // Passes detection, which looks at markers only, but every git call fails.
    let gitdir = format!("gitdir: {}\n", dir.join("missing").display());
    std::fs::write(dir.join(".git"), gitdir).unwrap();
    let todo = dir.join("sub/todo.txt");
    std::fs::write(&todo, "").unwrap();
    let result = check(&todo, None, false);
    assert!(
        matches!(result, Ok(Checked::Unavailable(Error::Failed { .. }))),
        "{result:?}"
    );
}
