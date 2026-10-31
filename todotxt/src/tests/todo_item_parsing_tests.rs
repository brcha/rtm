use super::*;
use chrono::NaiveDate;

#[test]
fn parse_incomplete_simple() {
    let item: TodoItem = "Buy groceries".parse().unwrap();
    assert!(!item.done);
    assert_eq!(item.priority, TodoPriority { priority: None });
    assert_eq!(item.completion_date, None);
    assert_eq!(item.creation_date, None);
    assert_eq!(item.description, "Buy groceries");
    assert!(item.projects.is_empty());
    assert!(item.contexts.is_empty());
    assert_eq!(item.due, None);
    assert_eq!(item.recurrence, None);
    assert_eq!(item.threshold, None);
    assert_eq!(item.uuid, None);
    assert_eq!(item.sub, None);
}

#[test]
fn parse_with_priority() {
    let item: TodoItem = "(A) Call mom".parse().unwrap();
    assert!(!item.done);
    assert_eq!(item.priority.priority, Some(0));
    assert_eq!(item.description, "Call mom");
}

#[test]
fn parse_with_project_and_context() {
    let item: TodoItem = "Buy milk +Personal @home".parse().unwrap();
    assert!(!item.done);
    assert_eq!(
        item.projects,
        vec![TodoProject {
            name: "Personal".to_string()
        }]
    );
    assert_eq!(
        item.contexts,
        vec![TodoContext {
            name: "home".to_string()
        }]
    );
    assert_eq!(item.description, "Buy milk");
    assert_eq!(item.threshold, None);
    assert_eq!(item.uuid, None);
    assert_eq!(item.sub, None);
}

#[test]
fn parse_completed_with_dates() {
    let item: TodoItem = "x (A) 2023-05-26 2023-05-20 Review code".parse().unwrap();
    assert!(item.done);
    assert_eq!(item.priority.priority, Some(0));
    assert_eq!(
        item.completion_date,
        Some(NaiveDate::from_ymd_opt(2023, 5, 26).unwrap())
    );
    assert_eq!(
        item.creation_date,
        Some(NaiveDate::from_ymd_opt(2023, 5, 20).unwrap())
    );
    assert_eq!(item.description, "Review code");
}

#[test]
fn parse_pri_tag_on_completed_item() {
    let item: TodoItem = "x 2024-10-07 2024-08-31 Task pri:A +p @c due:2024-10-05 rec:5w"
        .parse()
        .unwrap();
    assert!(item.done);
    assert_eq!(item.priority.priority, Some(0));
    assert_eq!(
        item.completion_date,
        Some(NaiveDate::from_ymd_opt(2024, 10, 7).unwrap())
    );
    assert_eq!(
        item.creation_date,
        Some(NaiveDate::from_ymd_opt(2024, 8, 31).unwrap())
    );
    assert_eq!(item.description, "Task");
}

#[test]
fn parse_pri_tag_on_open_item() {
    let item: TodoItem = "Task pri:B".parse().unwrap();
    assert!(!item.done);
    assert_eq!(item.priority.priority, Some(1));
    assert_eq!(item.description, "Task");
}

#[test]
fn parse_pri_tag_agreeing_with_parens() {
    let item: TodoItem = "x (C) 2024-01-02 2024-01-01 Task pri:C".parse().unwrap();
    assert_eq!(item.priority.priority, Some(2));
    assert_eq!(item.description, "Task");
}

#[test]
fn parse_pri_tag_conflicting_with_parens() {
    let result: Result<TodoItem, _> = "x (C) 2024-01-02 2024-01-01 Task pri:A".parse();
    assert_eq!(
        result,
        Err(TodoItemParseError::ConflictingPriority('C', 'A'))
    );
}

#[test]
fn parse_pri_tag_malformed_falls_through_to_description() {
    let item: TodoItem = "Task pri:xyz".parse().unwrap();
    assert_eq!(item.priority.priority, None);
    assert_eq!(item.description, "Task pri:xyz");
}

#[test]
fn parse_legacy_bare_x_no_dates() {
    let item: TodoItem = "x Task".parse().unwrap();
    assert!(item.done);
    assert_eq!(item.completion_date, None);
    assert_eq!(item.creation_date, None);
    assert_eq!(item.description, "Task");
}

#[test]
fn parse_legacy_bare_x_with_priority_no_dates() {
    let item: TodoItem = "x (A) Task".parse().unwrap();
    assert!(item.done);
    assert_eq!(item.priority.priority, Some(0));
    assert_eq!(item.completion_date, None);
    assert_eq!(item.creation_date, None);
    assert_eq!(item.description, "Task");
}

#[test]
fn parse_reported_issue_line() {
    let item: TodoItem =
        "x (C) 2026-05-23 Инфостан pri:C +рачуни @кућа due:2026-05-09 rec:+m t:2026-03-15"
            .parse()
            .unwrap();
    assert!(item.done);
    assert_eq!(item.priority.priority, Some(2));
    assert_eq!(item.description, "Инфостан");
}

#[test]
fn parse_malformed_recurrence_is_error() {
    for line in ["Task rec:+", "Task rec:é", "Task rec:1é"] {
        assert!(
            matches!(
                line.parse::<TodoItem>(),
                Err(TodoItemParseError::Recurrence(_))
            ),
            "{line:?}"
        );
    }
}

#[test]
fn parse_with_extensions() {
    let item: TodoItem = "Buy groceries +Personal @home due:2023-05-30 rec:1m t:2023-05-25"
        .parse()
        .unwrap();
    assert!(!item.done);
    assert_eq!(item.priority, TodoPriority { priority: None });
    assert_eq!(item.description, "Buy groceries");
    assert_eq!(
        item.projects,
        vec![TodoProject {
            name: "Personal".to_string()
        }]
    );
    assert_eq!(
        item.contexts,
        vec![TodoContext {
            name: "home".to_string()
        }]
    );
    assert_eq!(
        item.due,
        Some(NaiveDate::from_ymd_opt(2023, 5, 30).unwrap())
    );
    assert_eq!(item.recurrence, Some("1m".parse().unwrap()));
    assert_eq!(
        item.threshold,
        Some(NaiveDate::from_ymd_opt(2023, 5, 25).unwrap())
    );
}

#[test]
fn parse_with_uuid_and_sub() {
    let item: TodoItem = "Buy milk uuid:12345678-1234-1234-1234-123456789abc sub:87654321-4321-4321-4321-abc123456789".parse().unwrap();
    assert_eq!(item.description, "Buy milk");
    assert_eq!(
        item.uuid,
        Some(Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap())
    );
    assert_eq!(
        item.sub,
        Some(Uuid::parse_str("87654321-4321-4321-4321-abc123456789").unwrap())
    );
}

#[test]
fn test_parse_only_description() {
    let item: TodoItem = "Just a simple task".parse().unwrap();
    assert_eq!(item.description, "Just a simple task");
    assert!(!item.done);
    assert_eq!(item.priority.priority, None);
    assert!(item.projects.is_empty());
    assert!(item.contexts.is_empty());
    assert_eq!(item.due, None);
}

#[test]
fn test_parse_multiple_projects_and_contexts() {
    let item: TodoItem = "Task +Project1 +Project2 @context1 @context2"
        .parse()
        .unwrap();
    assert_eq!(item.projects.len(), 2);
    assert_eq!(item.contexts.len(), 2);
    assert_eq!(item.projects[0].name, "Project1");
    assert_eq!(item.projects[1].name, "Project2");
    assert_eq!(item.contexts[0].name, "context1");
    assert_eq!(item.contexts[1].name, "context2");
}
