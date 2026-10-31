use super::*;
use crate::todo_recurrence::TodoRecurrenceUnit;
use chrono::NaiveDate;

#[test]
fn add_subtask_new_uuid() {
    let parent = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Parent task".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let child = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Child task".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let (updated_parent, new_child) = parent.add_subtask(&child);
    assert!(updated_parent.is_some());
    let up = updated_parent.unwrap();
    assert_eq!(up.uuid, new_child.sub);
    assert!(new_child.sub.is_some());
    assert_eq!(up.description, "Parent task");
    assert_eq!(new_child.description, "Child task");
}

#[test]
fn add_subtask_existing_uuid() {
    let existing_uuid = Uuid::new_v4();
    let parent = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Parent task".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: Some(existing_uuid),
        sub: None,
    };
    let child = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Child task".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: Some(Uuid::new_v4()), // existing sub, should be overwritten
    };
    let (updated_parent, new_child) = parent.add_subtask(&child);
    assert!(updated_parent.is_none());
    assert_eq!(new_child.sub, Some(existing_uuid));
    assert_eq!(new_child.description, "Child task");
}

#[test]
fn test_set_done() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let new_item = item.set_done(true);
    assert!(new_item.done);
    assert!(!item.done); // original unchanged
}

#[test]
fn test_set_priority() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: Some(0) },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let new_item = item.set_priority(TodoPriority { priority: Some(1) });
    assert_eq!(new_item.priority.priority, Some(1));
    assert_eq!(item.priority.priority, Some(0));
}

#[test]
fn test_set_description() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Old".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let new_item = item.set_description("New".to_string());
    assert_eq!(new_item.description, "New");
    assert_eq!(item.description, "Old");
}

#[test]
fn test_add_project() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let proj = TodoProject {
        name: "Work".to_string(),
    };
    let new_item = item.add_project(proj.clone());
    assert_eq!(new_item.projects.len(), 1);
    assert_eq!(new_item.projects[0], proj);
    assert_eq!(item.projects.len(), 0); // original unchanged
}

#[test]
fn test_remove_project() {
    let proj = TodoProject {
        name: "Work".to_string(),
    };
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![proj.clone()],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let new_item = item.remove_project(&proj).unwrap();
    assert_eq!(new_item.projects.len(), 0);
    assert_eq!(item.projects.len(), 1);
}

#[test]
fn test_add_context() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let ctx = TodoContext {
        name: "Home".to_string(),
    };
    let new_item = item.add_context(ctx.clone());
    assert_eq!(new_item.contexts.len(), 1);
    assert_eq!(new_item.contexts[0], ctx);
    assert_eq!(item.contexts.len(), 0);
}

#[test]
fn test_remove_context() {
    let ctx = TodoContext {
        name: "Home".to_string(),
    };
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![],
        contexts: vec![ctx.clone()],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let new_item = item.remove_context(&ctx).unwrap();
    assert_eq!(new_item.contexts.len(), 0);
    assert_eq!(item.contexts.len(), 1);
}

#[test]
fn test_set_due() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let date = NaiveDate::from_ymd_opt(2023, 5, 30).unwrap();
    let new_item = item.set_due(Some(date));
    assert_eq!(new_item.due, Some(date));
    assert_eq!(item.due, None);
}

#[test]
fn test_set_recurrence() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let rec = TodoRecurrence::from_str("1m").unwrap();
    let new_item = item.set_recurrence(Some(rec.clone()));
    assert_eq!(new_item.recurrence, Some(rec));
    assert_eq!(item.recurrence, None);
}

#[test]
fn test_set_threshold() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let date = NaiveDate::from_ymd_opt(2023, 5, 25).unwrap();
    let new_item = item.set_threshold(Some(date));
    assert_eq!(new_item.threshold, Some(date));
    assert_eq!(item.threshold, None);
}

#[test]
fn test_set_uuid() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let uuid = Uuid::new_v4();
    let new_item = TodoItem {
        uuid: Some(uuid),
        ..item.clone()
    };
    assert_eq!(new_item.uuid, Some(uuid));
    assert_eq!(item.uuid, None);
}

#[test]
fn test_set_sub() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let sub = Uuid::new_v4();
    let new_item = TodoItem {
        sub: Some(sub),
        ..item.clone()
    };
    assert_eq!(new_item.sub, Some(sub));
    assert_eq!(item.sub, None);
}

#[test]
fn test_set_completion_date() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let date = NaiveDate::from_ymd_opt(2023, 5, 26).unwrap();
    let new_item = item.set_completion_date(Some(date));
    assert_eq!(new_item.completion_date, Some(date));
    assert_eq!(item.completion_date, None);
}

#[test]
fn test_set_creation_date() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let date = NaiveDate::from_ymd_opt(2023, 5, 20).unwrap();
    let new_item = item.set_creation_date(Some(date));
    assert_eq!(new_item.creation_date, Some(date));
    assert_eq!(item.creation_date, None);
}

#[test]
fn test_set_contexts() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let contexts = vec![TodoContext {
        name: "home".to_string(),
    }];
    let new_item = item.set_contexts(contexts.clone());
    assert_eq!(new_item.contexts, contexts);
    assert!(item.contexts.is_empty());
}

#[test]
fn test_item_equality() {
    let item1 = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Test".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let item2 = item1.clone();
    assert_eq!(item1, item2);
}

#[test]
fn test_item_clone() {
    let item1 = TodoItem {
        done: false,
        priority: TodoPriority { priority: Some(1) },
        completion_date: None,
        creation_date: Some(NaiveDate::from_ymd_opt(2023, 5, 20).unwrap()),
        description: "Test".to_string(),
        projects: vec![TodoProject {
            name: "Work".to_string(),
        }],
        contexts: vec![TodoContext {
            name: "home".to_string(),
        }],
        due: Some(NaiveDate::from_ymd_opt(2023, 5, 30).unwrap()),
        recurrence: Some(TodoRecurrence {
            strict: false,
            count: 1,
            unit: TodoRecurrenceUnit::Daily,
        }),
        threshold: Some(NaiveDate::from_ymd_opt(2023, 5, 25).unwrap()),
        uuid: Some(Uuid::new_v4()),
        sub: None,
    };
    let item2 = item1.clone();
    assert_eq!(item1, item2);
}
