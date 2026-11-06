use super::*;

fn library(lines: &[&str]) -> TodoLibrary {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.items = lines.iter().map(|line| line.parse().unwrap()).collect();
    lib
}

fn project(name: &str) -> TodoProject {
    TodoProject {
        name: name.to_string(),
    }
}

fn context(name: &str) -> TodoContext {
    TodoContext {
        name: name.to_string(),
    }
}

#[test]
fn test_empty_library_has_no_tags() {
    let lib = library(&[]);
    assert!(lib.projects().is_empty());
    assert!(lib.contexts().is_empty());
}

#[test]
fn test_projects_count_open_items() {
    let lib = library(&["One +work", "Two +work +home", "x Three +work"]);
    assert_eq!(
        lib.projects(),
        vec![(project("home"), 1), (project("work"), 2)]
    );
}

#[test]
fn test_contexts_count_open_items() {
    let lib = library(&["One @phone", "x Two @phone @office", "Three"]);
    assert_eq!(
        lib.contexts(),
        vec![(context("office"), 0), (context("phone"), 1)]
    );
}

#[test]
fn test_done_only_tag_listed_with_zero() {
    let lib = library(&["x Done +archive @old", "Open"]);
    assert_eq!(lib.projects(), vec![(project("archive"), 0)]);
    assert_eq!(lib.contexts(), vec![(context("old"), 0)]);
}

#[test]
fn test_tags_merge_case_with_first_spelling() {
    let lib = library(&["One +Work", "Two +work", "Three @Home", "Four @HOME"]);
    assert_eq!(lib.projects(), vec![(project("Work"), 2)]);
    assert_eq!(lib.contexts(), vec![(context("Home"), 2)]);
}

#[test]
fn test_duplicate_tag_on_one_item_counts_once() {
    let lib = library(&["One +work +Work @a @a"]);
    assert_eq!(lib.projects(), vec![(project("work"), 1)]);
    assert_eq!(lib.contexts(), vec![(context("a"), 1)]);
}

#[test]
fn test_tags_sorted_ignoring_case() {
    let lib = library(&["One +beta +Alpha +gamma"]);
    let names: Vec<String> = lib.projects().into_iter().map(|(p, _)| p.name).collect();
    assert_eq!(names, vec!["Alpha", "beta", "gamma"]);
}

#[test]
fn test_filter_items_returns_file_indices() {
    let lib = library(&[
        "One +work",
        "Two +home",
        "x Three +work",
        "Four +Work @office",
    ]);
    assert_eq!(lib.filter_items(&[project("work")], &[]), vec![0, 2, 3]);
}

#[test]
fn test_filter_items_without_filter_returns_all() {
    let lib = library(&["One", "x Two +work", "Three @home"]);
    assert_eq!(lib.filter_items(&[], &[]), vec![0, 1, 2]);
}

#[test]
fn test_filter_items_combines_project_and_context() {
    let lib = library(&[
        "One +work @office",
        "Two +work @home",
        "Three +home @office",
    ]);
    assert_eq!(
        lib.filter_items(&[project("work")], &[context("office")]),
        vec![0]
    );
}
