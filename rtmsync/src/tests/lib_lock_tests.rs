use super::*;

/// A fresh scratch directory under the system temp directory, holding `sub/todo.txt`.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmsync lock test {} {name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/todo.txt"), "").unwrap();
    dir
}

#[test]
fn a_file_in_a_repository_locks_the_repository() {
    let dir = scratch("repository");
    std::fs::create_dir(dir.join(".git")).unwrap();
    let target = lock_target(&dir.join("sub/todo.txt"));
    assert_eq!(target, root(&dir.join("sub/todo.txt")).unwrap());
    // Every file of the repository, an existing one or not, shares it.
    assert_eq!(lock_target(&dir.join("other.txt")), target);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_file_outside_any_repository_locks_itself() {
    let dir = scratch("outside");
    let file = dir.join("sub/todo.txt");
    assert_eq!(lock_target(&file), file);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn a_link_into_a_repository_locks_the_repository() {
    // rtmapp locks by the file's canonical path, rtmcli and the MCP server by the path as given.
    let dir = scratch("link");
    std::fs::create_dir(dir.join(".git")).unwrap();
    let outside = scratch("link outside");
    std::os::unix::fs::symlink(dir.join("sub/todo.txt"), outside.join("todo.txt")).unwrap();
    assert_eq!(
        lock_target(&outside.join("todo.txt")),
        lock_target(&dir.join("sub/todo.txt").canonicalize().unwrap())
    );
    std::fs::remove_dir_all(&dir).unwrap();
    std::fs::remove_dir_all(&outside).unwrap();
}
