use super::*;

#[test]
fn test_new() {
    let lib = TodoLibrary::new("test.txt".to_string());
    assert_eq!(lib.file_name, "test.txt");
    assert!(lib.items.is_empty());
}

#[test]
fn test_add_item() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let item = "Test item".parse().unwrap();
    lib.add_item(item);
    assert_eq!(lib.items.len(), 1);
    assert_eq!(lib.items[0].description, "Test item");
}

#[test]
fn test_add_item_stamps_creation_date() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let today = chrono::Local::now().date_naive();
    let item: TodoItem = "Test item".parse().unwrap();
    assert_eq!(item.creation_date, None);
    lib.add_item(item);
    assert_eq!(lib.items[0].creation_date, Some(today));
}

#[test]
fn test_add_item_preserves_existing_creation_date() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let creation = chrono::NaiveDate::from_ymd_opt(2023, 5, 20).unwrap();
    let mut item: TodoItem = "Test item".parse().unwrap();
    item.creation_date = Some(creation);
    lib.add_item(item);
    assert_eq!(lib.items[0].creation_date, Some(creation));
}

#[test]
fn test_remove_item() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.add_item("Item 1".parse().unwrap());
    lib.add_item("Item 2".parse().unwrap());
    let removed = lib.remove_item(0).unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(lib.items.len(), 1);
    assert_eq!(removed[0].description, "Item 1");
}

#[test]
fn test_remove_item_out_of_bounds() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let removed = lib.remove_item(0);
    assert!(removed.is_none());
}

#[test]
fn test_list_items() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.add_item("Item".parse().unwrap());
    let items = lib.list_items();
    assert_eq!(items.len(), 1);
}

#[test]
fn test_item_count() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.add_item("Item 1".parse().unwrap());
    lib.add_item("Item 2".parse().unwrap());
    assert_eq!(lib.item_count(), 2);
}

#[test]
fn test_clear_items() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.add_item("Item 1".parse().unwrap());
    lib.add_item("Item 2".parse().unwrap());
    assert_eq!(lib.item_count(), 2);
    lib.clear_items();
    assert_eq!(lib.item_count(), 0);
    assert!(lib.list_items().is_empty());
}

#[test]
fn test_item_count_empty() {
    let lib = TodoLibrary::new("dummy.txt".to_string());
    assert_eq!(lib.item_count(), 0);
}

#[test]
fn test_list_items_multiple() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.add_item("First".parse().unwrap());
    lib.add_item("Second".parse().unwrap());
    let items = lib.list_items();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].description, "First");
    assert_eq!(items[1].description, "Second");
}

#[test]
fn test_remove_item_first() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.add_item("First".parse().unwrap());
    lib.add_item("Second".parse().unwrap());
    lib.add_item("Third".parse().unwrap());
    let removed = lib.remove_item(0).unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].description, "First");
    assert_eq!(lib.item_count(), 2);
}

#[test]
fn test_remove_item_middle() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.add_item("First".parse().unwrap());
    lib.add_item("Second".parse().unwrap());
    lib.add_item("Third".parse().unwrap());
    let removed = lib.remove_item(1).unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].description, "Second");
    assert_eq!(lib.item_count(), 2);
}

/// Builds a library of items named by `description`, each with the given uuid and sub.
fn linked(items: &[(&str, Option<Uuid>, Option<Uuid>)]) -> TodoLibrary {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    for (description, uuid, sub) in items {
        let mut item: TodoItem = description.parse().unwrap();
        item.uuid = *uuid;
        item.sub = *sub;
        lib.add_item(item);
    }
    assert_eq!(lib.item_count(), items.len());
    lib
}

fn descriptions(items: &[TodoItem]) -> Vec<&str> {
    items.iter().map(|item| item.description.as_str()).collect()
}

#[test]
fn test_remove_item_without_uuid_removes_only_itself() {
    let other = Uuid::new_v4();
    let mut lib = linked(&[("Plain", None, None), ("Child", None, Some(other))]);
    let removed = lib.remove_item(0).unwrap();
    assert_eq!(descriptions(&removed), ["Plain"]);
    assert_eq!(descriptions(&lib.items), ["Child"]);
}

#[test]
fn test_remove_item_removes_sub_items_recursively() {
    let (parent, child) = (Uuid::new_v4(), Uuid::new_v4());
    let mut lib = linked(&[
        ("Grandchild", None, Some(child)),
        ("Unrelated", None, None),
        ("Parent", Some(parent), None),
        ("Child A", Some(child), Some(parent)),
        ("Child B", None, Some(parent)),
    ]);
    let removed = lib.remove_item(2).unwrap();
    assert_eq!(
        descriptions(&removed),
        ["Grandchild", "Parent", "Child A", "Child B"]
    );
    assert_eq!(descriptions(&lib.items), ["Unrelated"]);
}

#[test]
fn test_remove_sub_item_keeps_parent_and_siblings() {
    let (parent, child) = (Uuid::new_v4(), Uuid::new_v4());
    let mut lib = linked(&[
        ("Parent", Some(parent), None),
        ("Child A", Some(child), Some(parent)),
        ("Child B", None, Some(parent)),
        ("Grandchild", None, Some(child)),
    ]);
    let removed = lib.remove_item(1).unwrap();
    assert_eq!(descriptions(&removed), ["Child A", "Grandchild"]);
    assert_eq!(descriptions(&lib.items), ["Parent", "Child B"]);
}

#[test]
fn test_remove_item_with_cyclic_links_terminates() {
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let mut lib = linked(&[
        ("A", Some(a), Some(b)),
        ("B", Some(b), Some(a)),
        ("Other", None, None),
    ]);
    let removed = lib.remove_item(0).unwrap();
    assert_eq!(descriptions(&removed), ["A", "B"]);
    assert_eq!(descriptions(&lib.items), ["Other"]);
}

#[test]
fn test_library_equality() {
    let lib1 = TodoLibrary::new("test.txt".to_string());
    let lib2 = TodoLibrary::new("test.txt".to_string());
    assert_eq!(lib1, lib2);
}

#[test]
fn test_library_clone() {
    let mut lib1 = TodoLibrary::new("test.txt".to_string());
    lib1.add_item("Task".parse().unwrap());
    let lib2 = lib1.clone();
    assert_eq!(lib1.file_name, lib2.file_name);
    assert_eq!(lib1.item_count(), lib2.item_count());
}
