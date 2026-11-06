use super::*;

fn item(line: &str) -> TodoItem {
    line.parse().unwrap()
}

fn projects(names: &[&str]) -> Vec<TodoProject> {
    names
        .iter()
        .map(|n| TodoProject {
            name: n.to_string(),
        })
        .collect()
}

fn contexts(names: &[&str]) -> Vec<TodoContext> {
    names
        .iter()
        .map(|n| TodoContext {
            name: n.to_string(),
        })
        .collect()
}

#[test]
fn test_matches_without_filter() {
    assert!(item("Plain task").matches(&[], &[]));
    assert!(item("Task +work @home").matches(&[], &[]));
}

#[test]
fn test_matches_one_project() {
    let i = item("Task +work +home");
    assert!(i.matches(&projects(&["work"]), &[]));
    assert!(!i.matches(&projects(&["garden"]), &[]));
}

#[test]
fn test_matches_one_context() {
    let i = item("Task @phone");
    assert!(i.matches(&[], &contexts(&["phone"])));
    assert!(!i.matches(&[], &contexts(&["office"])));
}

#[test]
fn test_matches_needs_every_tag() {
    let i = item("Task +work +release @office @phone");
    assert!(i.matches(
        &projects(&["work", "release"]),
        &contexts(&["office", "phone"])
    ));
    assert!(!i.matches(&projects(&["work", "garden"]), &[]));
    assert!(!i.matches(&projects(&["work"]), &contexts(&["home"])));
}

#[test]
fn test_matches_ignores_case() {
    let i = item("Task +Work @Home +Über");
    assert!(i.matches(&projects(&["work"]), &contexts(&["HOME"])));
    assert!(i.matches(&projects(&["über"]), &[]));
}
