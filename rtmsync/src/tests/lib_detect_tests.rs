use super::*;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmsync test {} {name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/todo.txt"), "").unwrap();
    dir
}

#[test]
fn a_file_without_markers_is_not_versioned() {
    let dir = scratch("none");
    assert_eq!(detect(&dir.join("sub/todo.txt")), []);
}

#[test]
fn every_marker_is_recognised_in_the_files_directory() {
    for vcs in Vcs::ALL {
        let dir = scratch(vcs.marker());
        std::fs::create_dir(dir.join("sub").join(vcs.marker())).unwrap();
        assert_eq!(detect(&dir.join("sub/todo.txt")), [vcs]);
    }
}

#[test]
fn a_marker_above_the_file_is_found() {
    let dir = scratch("above");
    std::fs::create_dir(dir.join(".pijul")).unwrap();
    assert_eq!(detect(&dir.join("sub/todo.txt")), [Vcs::Pijul]);
}

#[test]
fn a_git_file_marks_a_linked_worktree() {
    let dir = scratch("worktree");
    std::fs::write(dir.join(".git"), "gitdir: elsewhere\n").unwrap();
    assert_eq!(detect(&dir.join("sub/todo.txt")), [Vcs::Git]);
}

#[test]
fn the_nearest_directory_with_a_marker_wins() {
    let dir = scratch("nearest");
    std::fs::create_dir(dir.join(".git")).unwrap();
    std::fs::create_dir(dir.join("sub/.pijul")).unwrap();
    assert_eq!(detect(&dir.join("sub/todo.txt")), [Vcs::Pijul]);
}

#[test]
fn several_markers_in_one_directory_are_all_returned() {
    let dir = scratch("ambiguous");
    std::fs::create_dir(dir.join(".git")).unwrap();
    std::fs::create_dir(dir.join(".pijul")).unwrap();
    assert_eq!(detect(&dir.join("sub/todo.txt")), [Vcs::Git, Vcs::Pijul]);
}

#[cfg(unix)]
#[test]
fn a_symlink_is_judged_by_its_target() {
    let dir = scratch("symlink");
    std::fs::create_dir(dir.join("sub/.git")).unwrap();
    let outside = scratch("symlink outside");
    std::os::unix::fs::symlink(dir.join("sub/todo.txt"), outside.join("todo.txt")).unwrap();
    assert_eq!(detect(&outside.join("todo.txt")), [Vcs::Git]);
}
