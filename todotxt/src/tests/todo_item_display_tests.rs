use super::*;
use chrono::NaiveDate;

#[test]
fn display_simple() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Buy milk".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    assert_eq!(item.to_string(), "Buy milk");
    assert_eq!(item.threshold, None);
    assert_eq!(item.uuid, None);
    assert_eq!(item.sub, None);
}

#[test]
fn display_with_extensions() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Buy groceries".to_string(),
        projects: vec![TodoProject {
            name: "Personal".to_string(),
        }],
        contexts: vec![TodoContext {
            name: "home".to_string(),
        }],
        due: Some(NaiveDate::from_ymd_opt(2023, 5, 30).unwrap()),
        recurrence: Some("1m".parse().unwrap()),
        threshold: Some(NaiveDate::from_ymd_opt(2023, 5, 25).unwrap()),
        uuid: None,
        sub: None,
    };
    assert_eq!(
        item.to_string(),
        "Buy groceries +Personal @home due:2023-05-30 rec:m t:2023-05-25"
    );
}

#[test]
fn display_completed_item_uses_pri_tag() {
    let item: TodoItem =
        "x 2024-10-07 2024-08-31 Стронгхолд pri:A +одржавање @фејнман @здравље due:2024-10-05 rec:5w"
            .parse()
            .unwrap();
    assert_eq!(
        item.to_string(),
        "x 2024-10-07 2024-08-31 Стронгхолд pri:A +одржавање @фејнман @здравље due:2024-10-05 rec:5w"
    );
}

#[test]
fn display_completed_item_round_trip_is_idempotent() {
    let item: TodoItem =
        "x 2024-10-07 2024-08-31 Стронгхолд pri:A +одржавање @фејнман @здравље due:2024-10-05 rec:5w"
            .parse()
            .unwrap();
    let once = item.to_string();
    let twice: TodoItem = once.parse().unwrap();
    assert_eq!(twice.to_string(), once);
}

#[test]
fn display_open_item_with_priority_uses_parens_not_pri_tag() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: Some(0) },
        completion_date: None,
        creation_date: None,
        description: "Call mom".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    assert_eq!(item.to_string(), "(A) Call mom");
}

#[test]
fn display_normalizes_pri_tag_on_open_item_to_parens() {
    let item: TodoItem = "Task pri:B".parse().unwrap();
    assert_eq!(item.to_string(), "(B) Task");
}

#[test]
fn display_normalizes_parens_on_completed_item_to_pri_tag() {
    let item: TodoItem = "x (C) 2024-01-02 2024-01-01 Task".parse().unwrap();
    assert_eq!(item.to_string(), "x 2024-01-02 2024-01-01 Task pri:C");
}

#[test]
fn display_legacy_bare_x_round_trip() {
    let item: TodoItem = "x Task".parse().unwrap();
    assert_eq!(item.to_string(), "x Task");
}

#[test]
fn display_done_item_with_creation_date_but_no_completion_date_omits_it() {
    // A done item can end up with a creation date and no completion date only via
    // direct construction (e.g. a pre-fix file, or manual struct construction) since
    // TodoLibrary::complete_item always sets a completion date. Display must never
    // emit that lone creation date on a done line: FromStr would read it back as the
    // completion date, silently turning it into a different fact on the next load.
    let item = TodoItem {
        done: true,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: Some(NaiveDate::from_ymd_opt(2023, 5, 20).unwrap()),
        description: "Review code".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    assert_eq!(item.to_string(), "x Review code");
}

#[test]
fn display_done_item_date_handling_is_a_parse_display_fixed_point() {
    // Guards the defect directly: constructing the corrupting shape by hand and
    // round-tripping it must converge to a stable, information-preserving line rather
    // than silently reinterpreting the lone date as something it is not.
    let corrupting_shape = TodoItem {
        done: true,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: Some(NaiveDate::from_ymd_opt(2023, 5, 20).unwrap()),
        description: "Review code".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    let once = corrupting_shape.to_string();
    let reparsed: TodoItem = once.parse().unwrap();
    let twice = reparsed.to_string();
    assert_eq!(once, twice);
    assert_eq!(reparsed.completion_date, None);
    assert_eq!(reparsed.creation_date, None);
}

#[test]
fn display_completed_item_with_empty_description_has_no_double_space() {
    let item = TodoItem {
        done: true,
        priority: TodoPriority { priority: Some(0) },
        completion_date: None,
        creation_date: None,
        description: "".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    assert_eq!(item.to_string(), "x pri:A");
}

#[test]
fn display_with_uuid_and_sub() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "Buy milk".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: Some(Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap()),
        sub: Some(Uuid::parse_str("87654321-4321-4321-4321-abc123456789").unwrap()),
    };
    assert_eq!(
        item.to_string(),
        "Buy milk uuid:12345678-1234-1234-1234-123456789abc sub:87654321-4321-4321-4321-abc123456789"
    );
}

#[test]
fn test_roundtrip_with_all_fields() {
    let original = "(A) 2023-05-20 Complete task +Work @office due:2023-05-30 rec:1w t:2023-05-25";
    let parsed: TodoItem = original.parse().unwrap();
    let output = parsed.to_string();
    assert!(output.contains("(A)"));
    assert!(output.contains("Complete task"));
    assert!(output.contains("+Work"));
    assert!(output.contains("@office"));
    assert!(output.contains("due:2023-05-30"));
    assert!(output.contains("rec:w"));
    assert!(output.contains("t:2023-05-25"));
}

#[test]
fn test_display_empty_item() {
    let item = TodoItem {
        done: false,
        priority: TodoPriority { priority: None },
        completion_date: None,
        creation_date: None,
        description: "".to_string(),
        projects: vec![],
        contexts: vec![],
        due: None,
        recurrence: None,
        threshold: None,
        uuid: None,
        sub: None,
    };
    assert_eq!(item.to_string(), "");
}
