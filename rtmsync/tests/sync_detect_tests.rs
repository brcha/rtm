//! `detect` on files in repositories made by `git init` and `pijul init`, and outside any
//! repository.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rtmsync::{Vcs, detect};

/// An empty scratch directory for test `name`.
fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("rtmsync detect test {} {name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A new Git repository in `dir`.
fn git_init(dir: &Path) {
    let output = Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "git init: {output:?}");
}

/// A new Pijul repository in `dir`, with pijul's config kept in `home`. Returns false when pijul
/// is not in `PATH`, which fails the test instead when `RTM_REQUIRE_PIJUL` is set.
fn pijul_init(dir: &Path, home: &Path) -> bool {
    if Command::new("pijul").arg("--version").output().is_err() {
        assert!(
            std::env::var_os("RTM_REQUIRE_PIJUL").is_none(),
            "pijul is not in PATH but RTM_REQUIRE_PIJUL is set"
        );
        eprintln!("pijul is not in PATH; skipping the Pijul part");
        return false;
    }
    // Set on the command rather than the process, so these tests can run in parallel.
    let output = Command::new("pijul")
        .arg("init")
        .current_dir(dir)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("PIJUL_CONFIG_DIR", home.join("pijul"))
        .output()
        .unwrap();
    assert!(output.status.success(), "pijul init: {output:?}");
    true
}

#[test]
fn files_in_a_git_repository_are_detected_from_any_depth() {
    let repo = scratch("repository");
    git_init(&repo);
    assert!(repo.join(Vcs::Git.marker()).exists());

    let nested = repo.join("lists").join("home");
    fs::create_dir_all(&nested).unwrap();
    fs::write(repo.join("todo.txt"), "").unwrap();
    fs::write(nested.join("todo.txt"), "").unwrap();

    assert_eq!(detect(&repo.join("todo.txt")), [Vcs::Git]);
    assert_eq!(detect(&nested.join("todo.txt")), [Vcs::Git]);
    // A file not created yet is judged by the directory it would be in.
    assert_eq!(detect(&repo.join("new.txt")), [Vcs::Git]);
    fs::remove_dir_all(&repo).unwrap();
}

#[test]
fn files_in_a_pijul_repository_are_detected_from_any_depth() {
    let dir = scratch("pijul");
    let (home, repo) = (dir.join("home"), dir.join("repo"));
    let nested = repo.join("lists").join("home");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&nested).unwrap();
    if !pijul_init(&repo, &home) {
        return;
    }
    assert!(repo.join(Vcs::Pijul.marker()).exists());

    fs::write(repo.join("todo.txt"), "").unwrap();
    fs::write(nested.join("todo.txt"), "").unwrap();

    assert_eq!(detect(&repo.join("todo.txt")), [Vcs::Pijul]);
    assert_eq!(detect(&nested.join("todo.txt")), [Vcs::Pijul]);
    assert_eq!(detect(&repo.join("new.txt")), [Vcs::Pijul]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_directory_with_git_and_pijul_repositories_returns_both() {
    let dir = scratch("both");
    let (home, repo) = (dir.join("home"), dir.join("repo"));
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&repo).unwrap();
    git_init(&repo);
    if !pijul_init(&repo, &home) {
        return;
    }

    assert_eq!(detect(&repo.join("todo.txt")), [Vcs::Git, Vcs::Pijul]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_file_outside_any_repository_is_not_versioned() {
    let dir = scratch("plain");
    fs::write(dir.join("todo.txt"), "").unwrap();

    assert_eq!(detect(&dir.join("todo.txt")), Vec::<Vcs>::new());
    fs::remove_dir_all(&dir).unwrap();
}
