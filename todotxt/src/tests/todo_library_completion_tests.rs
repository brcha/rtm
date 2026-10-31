use super::*;

#[test]
fn test_complete_item() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.add_item("Incomplete".parse().unwrap());
    let result = lib.complete_item(0);
    assert_eq!(result, Some(false));
    assert!(lib.items[0].done);
}

#[test]
fn test_complete_item_sets_completion_date() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let today = chrono::Local::now().date_naive();
    lib.add_item("Incomplete".parse().unwrap());
    lib.complete_item(0);
    assert_eq!(lib.items[0].completion_date, Some(today));
}

#[test]
fn test_complete_item_preserves_existing_creation_date() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let today = chrono::Local::now().date_naive();
    let creation = chrono::NaiveDate::from_ymd_opt(2023, 5, 20).unwrap();
    let mut item: TodoItem = "Task".parse().unwrap();
    item.creation_date = Some(creation);
    lib.add_item(item);
    lib.complete_item(0);
    assert_eq!(lib.items[0].completion_date, Some(today));
    assert_eq!(lib.items[0].creation_date, Some(creation));
    assert_eq!(
        lib.items[0].to_string(),
        format!(
            "x {} {} Task",
            today.format("%Y-%m-%d"),
            creation.format("%Y-%m-%d")
        )
    );
}

#[test]
fn test_complete_item_with_no_creation_date_emits_one_date() {
    // add_item always stamps a creation date (see test_add_item_stamps_creation_date),
    // so the only way an item lacks one at completion time is if it entered the library
    // some other way — e.g. loaded from a pre-existing file. Push directly to bypass
    // add_item and exercise that path.
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let today = chrono::Local::now().date_naive();
    let item: TodoItem = "Task".parse().unwrap();
    assert_eq!(item.creation_date, None);
    lib.items.push(item);
    lib.complete_item(0);
    assert_eq!(lib.items[0].creation_date, None);
    assert_eq!(
        lib.items[0].to_string(),
        format!("x {} Task", today.format("%Y-%m-%d"))
    );
}

#[test]
fn test_complete_item_with_priority_writes_pri_tag() {
    // add_item stamps today as the creation date, and completing it the same instant
    // stamps today as the completion date too, so both dates appear.
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let today = chrono::Local::now().date_naive();
    let item: TodoItem = "(A) Task".parse().unwrap();
    lib.add_item(item);
    lib.complete_item(0);
    assert_eq!(
        lib.items[0].to_string(),
        format!(
            "x {} {} Task pri:A",
            today.format("%Y-%m-%d"),
            today.format("%Y-%m-%d")
        )
    );
}

#[test]
fn test_complete_item_is_idempotent_on_completion_date() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.add_item("Task".parse().unwrap());
    lib.complete_item(0);
    let first_completion = lib.items[0].completion_date;
    lib.complete_item(0);
    assert_eq!(lib.items[0].completion_date, first_completion);
}

#[test]
fn test_complete_item_out_of_bounds() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let result = lib.complete_item(0);
    assert_eq!(result, None);
    assert_eq!(lib.items.len(), 0);
}

#[test]
fn test_uncomplete_item_clears_done_and_completion_date() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let item: TodoItem = "(C) Task".parse().unwrap();
    lib.add_item(item);
    lib.complete_item(0);
    assert!(lib.items[0].done);
    assert!(lib.items[0].completion_date.is_some());

    let result = lib.uncomplete_item(0);
    assert_eq!(result, Some(()));
    assert!(!lib.items[0].done);
    assert_eq!(lib.items[0].completion_date, None);
}

#[test]
fn test_uncomplete_item_out_of_bounds() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let result = lib.uncomplete_item(0);
    assert_eq!(result, None);
}

#[test]
fn test_complete_then_uncomplete_round_trips_to_original_line() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let original = "(C) Task +p @c";
    lib.add_item(original.parse().unwrap());
    // Neutralize the creation date add_item just stamped, so the line is directly
    // comparable to the un-decorated original after the round trip.
    lib.items[0].creation_date = None;

    lib.complete_item(0);
    assert_ne!(lib.items[0].to_string(), original);

    lib.uncomplete_item(0);
    assert_eq!(lib.items[0].to_string(), original);
}

#[test]
fn test_complete_item_success() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.add_item("Uncompleted".parse().unwrap());
    assert!(!lib.items[0].done);
    let result = lib.complete_item(0);
    assert_eq!(result, Some(false));
    assert!(lib.items[0].done);
}

#[test]
fn test_complete_recurring_daily() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let today = chrono::Local::now().date_naive();
    let mut item = "Test rec daily".parse::<TodoItem>().unwrap();
    item.due = Some(today - chrono::Duration::days(2)); // Set past due for non-strict test
    item.recurrence = Some(crate::todo_recurrence::TodoRecurrence {
        count: 1,
        unit: crate::todo_recurrence::TodoRecurrenceUnit::Daily,
        strict: false,
    });
    lib.add_item(item);
    assert_eq!(lib.item_count(), 1);
    let result = lib.complete_item(0);
    assert_eq!(result, Some(true));
    assert_eq!(lib.item_count(), 2);
    assert!(lib.items[0].done);
    assert_eq!(lib.items[0].completion_date, Some(today));
    assert!(!lib.items[1].done);
    assert_eq!(lib.items[1].completion_date, None);
    assert_eq!(lib.items[1].creation_date, Some(today));
    // For non-strict, next due is today + 1 day, not original_due + 1 day
    assert_eq!(lib.items[1].due, Some(today + chrono::Duration::days(1)));
    assert_eq!(lib.items[1].description, "Test rec daily");
}

#[test]
fn test_complete_recurring_weekly() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let today = chrono::Local::now().date_naive();
    let mut item = "Test rec weekly".parse::<TodoItem>().unwrap();
    item.due = Some(today);
    item.recurrence = Some(crate::todo_recurrence::TodoRecurrence {
        count: 2,
        unit: crate::todo_recurrence::TodoRecurrenceUnit::Weekly,
        strict: false,
    });
    lib.add_item(item);
    let result = lib.complete_item(0);
    assert_eq!(result, Some(true));
    assert_eq!(lib.item_count(), 2);
    assert_eq!(lib.items[1].due, Some(today + chrono::Duration::weeks(2)));
}

#[test]
fn test_complete_non_recurring() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let item = "Test non rec".parse().unwrap();
    lib.add_item(item);
    let result = lib.complete_item(0);
    assert_eq!(result, Some(false));
    assert_eq!(lib.item_count(), 1);
    assert!(lib.items[0].done);
}

#[test]
fn test_complete_item_invalid_index() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.add_item("Task 1".parse().unwrap());
    lib.add_item("Task 2".parse().unwrap());
    let result = lib.complete_item(5);
    assert_eq!(result, None);
}

#[test]
fn test_complete_item_at_last_index() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.add_item("Task 1".parse().unwrap());
    lib.add_item("Task 2".parse().unwrap());
    let result = lib.complete_item(1);
    assert_eq!(result, Some(false));
    assert!(lib.items[1].done);
}

#[test]
fn test_complete_item_with_threshold() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let mut item: TodoItem = "Task with threshold".parse().unwrap();
    item.threshold = Some(chrono::NaiveDate::from_ymd_opt(2023, 1, 1).unwrap());
    lib.add_item(item);

    let result = lib.complete_item(0);
    assert_eq!(result, Some(false));
    assert!(lib.items[0].done);
}
