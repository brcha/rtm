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
