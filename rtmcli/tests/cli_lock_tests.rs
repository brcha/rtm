//! rtmcli and the write lock it shares with rtmapp and the MCP server (RTM-158): held while a
//! command reads, pulls and changes the file, released while `delete` asks. The lock directory does
//! not depend on the environment, so the test and the rtmcli it starts share it.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use todotxt::WriteLock;

/// An empty scratch directory for test `name`.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmcli-lock-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// rtmcli on `file` with `args`, without sync, the environment that would change the outcome
/// cleared.
fn rtmcli(file: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rtmcli"));
    command
        .arg("-f")
        .arg(file)
        .arg("--no-sync")
        .args(args)
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

/// The lock another rtm process would hold while it writes `file`.
fn hold(file: &Path) -> WriteLock {
    WriteLock::acquire(&rtmsync::lock_target(file)).unwrap()
}

/// Whether `child` is still running after `wait`.
fn still_running(child: &mut Child, wait: Duration) -> bool {
    std::thread::sleep(wait);
    child.try_wait().unwrap().is_none()
}

#[test]
fn a_change_waits_while_another_process_holds_the_lock() {
    let dir = scratch("held");
    let file = dir.join("todo.txt");
    fs::write(&file, "2026-01-01 Alpha\n").unwrap();
    let held = hold(&file);

    let mut child = rtmcli(&file, &["add", "2026-01-01 Bravo"])
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    assert!(
        still_running(&mut child, Duration::from_millis(500)),
        "rtmcli did not wait for the lock"
    );
    assert_eq!(file_lines(&file), ["2026-01-01 Alpha"]);

    drop(held);
    assert!(child.wait().unwrap().success());
    assert_eq!(file_lines(&file), ["2026-01-01 Alpha", "2026-01-01 Bravo"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_file_waits_for_its_repositorys_lock() {
    let dir = scratch("repository");
    // `lock_target` reads markers only, so an empty .git is enough; --no-sync keeps git out.
    fs::create_dir(dir.join(".git")).unwrap();
    let (home, work) = (dir.join("home.txt"), dir.join("work.txt"));
    fs::write(&home, "2026-01-01 Alpha\n").unwrap();
    fs::write(&work, "2026-01-01 Report\n").unwrap();
    let held = hold(&home);

    let mut child = rtmcli(&work, &["list"])
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    assert!(
        still_running(&mut child, Duration::from_millis(500)),
        "rtmcli did not wait for the repository's lock"
    );

    drop(held);
    assert!(child.wait().unwrap().success());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn changes_at_once_all_land() {
    // Process start-up makes a collision rare even without the lock: a smoke test.
    let dir = scratch("at once");
    let file = dir.join("todo.txt");
    fs::write(&file, "").unwrap();
    let children: Vec<Child> = (0..10)
        .map(|n| {
            rtmcli(&file, &["add", &format!("2026-01-01 Item {n}")])
                .stdout(Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let mut lines = file_lines(&file);
    lines.sort();
    let expected: Vec<String> = (0..10).map(|n| format!("2026-01-01 Item {n}")).collect();
    assert_eq!(lines, expected);
    fs::remove_dir_all(&dir).unwrap();
}

/// `delete 1` on `file`, stopped at its question: the child and its stdin.
fn delete_asking(file: &Path) -> Child {
    let mut child = rtmcli(file, &["delete", "1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.as_mut().unwrap();
    let mut shown = Vec::new();
    let mut byte = [0];
    while !shown.ends_with(b"Delete? [y/N] ") {
        assert_eq!(stdout.read(&mut byte).unwrap(), 1, "no question: {shown:?}");
        shown.push(byte[0]);
    }
    child
}

/// Whether the lock on `file` can be taken within about a second.
fn lock_is_free(file: &Path) -> bool {
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        if WriteLock::try_acquire(&rtmsync::lock_target(file))
            .unwrap()
            .is_some()
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn delete_holds_no_lock_while_it_asks() {
    let dir = scratch("delete asks");
    let file = dir.join("todo.txt");
    fs::write(&file, "2026-01-01 Alpha\n2026-01-01 Bravo\n").unwrap();
    let mut child = delete_asking(&file);

    assert!(lock_is_free(&file), "the lock is held during the question");
    // Another program adds a line meanwhile; the deletion still applies to what was shown.
    fs::write(
        &file,
        "2026-01-01 Alpha\n2026-01-01 Bravo\n2026-01-01 Charlie\n",
    )
    .unwrap();
    child.stdin.take().unwrap().write_all(b"y\n").unwrap();

    assert!(child.wait().unwrap().success());
    assert_eq!(
        file_lines(&file),
        ["2026-01-01 Bravo", "2026-01-01 Charlie"]
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn delete_changes_nothing_when_its_item_changed_during_the_question() {
    let dir = scratch("delete changed");
    let file = dir.join("todo.txt");
    fs::write(&file, "2026-01-01 Alpha\n2026-01-01 Bravo\n").unwrap();
    let mut child = delete_asking(&file);

    fs::write(&file, "2026-01-01 Alpha, edited\n2026-01-01 Bravo\n").unwrap();
    child.stdin.take().unwrap().write_all(b"y\n").unwrap();

    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("changed or removed meanwhile"),
        "{output:?}"
    );
    assert_eq!(
        file_lines(&file),
        ["2026-01-01 Alpha, edited", "2026-01-01 Bravo"]
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// Runs git in `dir`, panicking on failure.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn delete_checks_the_repository_again_after_its_question() {
    let dir = scratch("delete recheck");
    fs::create_dir(dir.join("no hooks")).unwrap();
    let repo = dir.join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "--quiet"]);
    // Local config keeping the user's identity, signing, hooks and rerere out.
    let hooks = dir.join("no hooks");
    for (key, value) in [
        ("user.name", "rtm test"),
        ("user.email", "rtm@example.com"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", hooks.to_str().unwrap()),
        ("rerere.enabled", "false"),
    ] {
        git(&repo, &["config", key, value]);
    }
    let file = repo.join("todo.txt");
    fs::write(&file, "2026-01-01 Alpha\n2026-01-01 Bravo\n").unwrap();
    git(&repo, &["add", "todo.txt"]);
    git(&repo, &["commit", "--quiet", "-m", "start"]);

    // Synced this time: the question comes after the repository check.
    let mut child = Command::new(env!("CARGO_BIN_EXE_rtmcli"))
        .arg("-f")
        .arg(&file)
        .args(["delete", "1"])
        .env_remove("RTM_NO_SYNC")
        .env_remove("RTM_VCS")
        .env_remove("RTM_UPDATE_URL")
        .env_remove("TODOTXT")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.as_mut().unwrap();
    let mut shown = Vec::new();
    let mut byte = [0];
    while !shown.ends_with(b"Delete? [y/N] ") {
        assert_eq!(stdout.read(&mut byte).unwrap(), 1, "no question: {shown:?}");
        shown.push(byte[0]);
    }
    // An editor changes the file meanwhile and commits nothing.
    fs::write(
        &file,
        "2026-01-01 Alpha\n2026-01-01 Bravo\n2026-01-01 By hand\n",
    )
    .unwrap();
    child.stdin.take().unwrap().write_all(b"y\n").unwrap();

    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(
        file_lines(&file),
        ["2026-01-01 Alpha", "2026-01-01 Bravo", "2026-01-01 By hand"]
    );
    assert_eq!(git(&repo, &["log", "-1", "--format=%s"]), "start");
    fs::remove_dir_all(&dir).unwrap();
}
