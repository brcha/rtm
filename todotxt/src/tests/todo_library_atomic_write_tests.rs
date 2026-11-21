use super::*;
use std::fs;

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtm-atomic-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    dir
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn replaces_the_content_and_leaves_no_other_file() {
    let dir = scratch();
    let path = dir.join("todo.txt");
    fs::write(&path, "old line that is longer than the new one\n").unwrap();
    write_atomic(&path, "new\n").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "new\n");
    assert_eq!(names(&dir), ["todo.txt"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn creates_a_missing_file() {
    let dir = scratch();
    let path = dir.join("todo.txt");
    write_atomic(&path, "first\n").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "first\n");
    assert_eq!(names(&dir), ["todo.txt"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn keeps_the_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch();
    let path = dir.join("todo.txt");
    fs::write(&path, "old\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    write_atomic(&path, "new\n").unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn refuses_a_read_only_file() {
    let dir = scratch();
    let path = dir.join("todo.txt");
    fs::write(&path, "old\n").unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions.clone()).unwrap();
    let error = write_atomic(&path, "new\n").unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(fs::read_to_string(&path).unwrap(), "old\n");
    assert_eq!(names(&dir), ["todo.txt"]);
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    fs::set_permissions(&path, permissions).unwrap();
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_failed_rename_leaves_no_temporary_file() {
    let dir = scratch();
    // A non-empty directory cannot be replaced by a file on any platform.
    let path = dir.join("todo.txt");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("inner.txt"), "kept\n").unwrap();
    assert!(write_atomic(&path, "new\n").is_err());
    assert_eq!(names(&dir), ["todo.txt"]);
    assert_eq!(
        fs::read_to_string(path.join("inner.txt")).unwrap(),
        "kept\n"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn writes_through_a_symbolic_link_and_keeps_it() {
    let dir = scratch();
    let real = dir.join("real.toml");
    let link = dir.join("config.toml");
    fs::write(&real, "old\n").unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    write_atomic(&link, "new\n").unwrap();
    assert!(link.is_symlink());
    assert_eq!(fs::read_to_string(&real).unwrap(), "new\n");
    assert_eq!(names(&dir), ["config.toml", "real.toml"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn creates_the_file_a_dangling_link_names() {
    let dir = scratch();
    let link = dir.join("config.toml");
    std::os::unix::fs::symlink("real.toml", &link).unwrap();
    write_atomic(&link, "new\n").unwrap();
    assert!(link.is_symlink());
    assert_eq!(fs::read_to_string(dir.join("real.toml")).unwrap(), "new\n");
    assert_eq!(names(&dir), ["config.toml", "real.toml"]);
    fs::remove_dir_all(&dir).unwrap();
}
