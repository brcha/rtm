use super::*;
use std::fs;

#[test]
fn test_load_leaves_dateless_lines_dateless() {
    let temp_dir = std::env::temp_dir();
    let file_name = format!("{}.txt", uuid::Uuid::new_v4());
    let path = temp_dir.join(file_name);
    fs::write(&path, "Buy milk\n").unwrap();
    let mut lib = TodoLibrary::new(path.to_str().unwrap().to_string());
    lib.load().unwrap();
    assert_eq!(lib.items[0].creation_date, None);
    fs::remove_file(&path).unwrap();
}

#[test]
fn test_load_and_save() {
    let temp_dir = std::env::temp_dir();
    let file_name = format!("{}.txt", uuid::Uuid::new_v4());
    let path = temp_dir.join(file_name);
    let content = "Buy milk\nx Complete task\n";
    fs::write(&path, content).unwrap();
    let mut lib = TodoLibrary::new(path.to_str().unwrap().to_string());
    lib.load().unwrap();
    assert_eq!(lib.items.len(), 2);
    assert_eq!(lib.items[0].description, "Buy milk");
    assert_eq!(lib.items[1].description, "Complete task");
    assert!(lib.items[1].done);
    lib.save().unwrap();
    let saved_content = fs::read_to_string(&path).unwrap();
    assert_eq!(saved_content.trim(), content.trim());
    fs::remove_file(&path).unwrap();
}

#[test]
fn test_save_empty() {
    let lib = TodoLibrary::new("test_empty.txt".to_string());
    lib.save().unwrap();
    let content = fs::read_to_string("test_empty.txt").unwrap();
    assert_eq!(content.trim(), "");
    fs::remove_file("test_empty.txt").unwrap();
}

#[test]
fn test_load_invalid_file() {
    let mut lib = TodoLibrary::new("nonexistent_file_12345.txt".to_string());
    let result = lib.load();
    assert!(result.is_err());
}

#[test]
fn test_save_to_new_file() {
    let temp_dir = std::env::temp_dir();
    let file_name = format!("{}.txt", uuid::Uuid::new_v4());
    let path = temp_dir.join(&file_name);

    let mut lib = TodoLibrary::new(path.to_str().unwrap().to_string());
    lib.add_item("Task 1".parse().unwrap());
    lib.add_item("Task 2".parse().unwrap());
    lib.save().unwrap();

    let mut lib2 = TodoLibrary::new(path.to_str().unwrap().to_string());
    lib2.load().unwrap();
    assert_eq!(lib2.item_count(), 2);

    std::fs::remove_file(&path).unwrap();
}

#[test]
fn a_new_library_is_saved_with_a_final_newline() {
    let path = std::env::temp_dir().join(format!("{}.txt", uuid::Uuid::new_v4()));
    let mut lib = TodoLibrary::new(path.to_str().unwrap().to_string());
    lib.items.push("Buy milk".parse().unwrap());
    lib.save().unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "Buy milk\n");
    fs::remove_file(&path).unwrap();
}

#[test]
fn an_empty_file_gets_a_final_newline_once_it_has_an_item() {
    let path = std::env::temp_dir().join(format!("{}.txt", uuid::Uuid::new_v4()));
    fs::write(&path, "").unwrap();
    let mut lib = TodoLibrary::new(path.to_str().unwrap().to_string());
    lib.load().unwrap();
    lib.items.push("Buy milk".parse().unwrap());
    lib.save().unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "Buy milk\n");
    fs::remove_file(&path).unwrap();
}

/// A new empty directory under the system temp directory.
#[cfg(unix)]
fn scratch_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rtm-symlink-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    dir
}

/// The `SymlinkError` inside a `load` error.
#[cfg(unix)]
fn symlink_error(error: &std::io::Error) -> &SymlinkError {
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    error.get_ref().unwrap().downcast_ref().unwrap()
}

#[cfg(unix)]
#[test]
fn a_symlinked_file_is_refused_and_named_with_its_target() {
    let dir = scratch_dir();
    let target = dir.join("real.txt");
    fs::write(&target, "Buy milk\n").unwrap();
    let link = dir.join("todo.txt");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let mut lib = TodoLibrary::new(link.to_str().unwrap().to_string());
    let error = lib.load().unwrap_err();
    let refused = symlink_error(&error);
    // canonicalize: the temp directory is reached through a symlink on macOS.
    let canonical = target.canonicalize().unwrap();
    assert_eq!(refused.link, link);
    assert_eq!(refused.target, canonical);
    assert_eq!(
        error.to_string(),
        format!(
            "'{}' is a symbolic link to '{}'; open '{}' instead",
            link.display(),
            canonical.display(),
            canonical.display()
        )
    );
    assert!(lib.items.is_empty());
    fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn a_chain_of_links_names_the_file_at_its_end() {
    let dir = scratch_dir();
    let target = dir.join("real.txt");
    fs::write(&target, "").unwrap();
    std::os::unix::fs::symlink(&target, dir.join("middle.txt")).unwrap();
    let link = dir.join("todo.txt");
    std::os::unix::fs::symlink("middle.txt", &link).unwrap();
    let error = TodoLibrary::new(link.to_str().unwrap().to_string())
        .load()
        .unwrap_err();
    assert_eq!(symlink_error(&error).target, target.canonicalize().unwrap());
    fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn a_dangling_link_is_refused_and_nothing_is_created() {
    let dir = scratch_dir();
    let link = dir.join("todo.txt");
    std::os::unix::fs::symlink("missing.txt", &link).unwrap();
    let error = TodoLibrary::new(link.to_str().unwrap().to_string())
        .load()
        .unwrap_err();
    assert_eq!(symlink_error(&error).target, dir.join("missing.txt"));
    assert!(!dir.join("missing.txt").exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn a_file_in_a_symlinked_directory_loads() {
    let dir = scratch_dir();
    fs::create_dir(dir.join("real")).unwrap();
    fs::write(dir.join("real/todo.txt"), "Buy milk\n").unwrap();
    std::os::unix::fs::symlink(dir.join("real"), dir.join("linked")).unwrap();
    let mut lib = TodoLibrary::new(dir.join("linked/todo.txt").to_str().unwrap().to_string());
    lib.load().unwrap();
    assert_eq!(lib.items.len(), 1);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_missing_path_is_not_refused() {
    let path = std::env::temp_dir().join(format!("{}.txt", uuid::Uuid::new_v4()));
    refuse_symlink(&path).unwrap();
}
