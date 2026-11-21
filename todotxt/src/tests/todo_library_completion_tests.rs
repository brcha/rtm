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
    // Pushed directly, as a loaded line can lack the creation date `add_item` would stamp.
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
fn test_complete_item_reads_an_open_pri_tag_as_the_priority() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    let today = chrono::Local::now().date_naive();
    lib.items.push("Task pri:B +p".parse().unwrap());
    lib.complete_item(0);
    let line = lib.items[0].to_string();
    assert_eq!(
        line,
        format!("x {} Task pri:B +p", today.format("%Y-%m-%d"))
    );
    assert_eq!(lib.items[0], line.parse::<TodoItem>().unwrap());
    assert_eq!(lib.items[0].priority.priority, Some(1));
    assert_eq!(lib.items[0].description, "Task");
}

#[test]
fn test_complete_item_with_priority_and_pri_tag_equals_its_line() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.items.push("(A) Task pri:B".parse().unwrap());
    lib.complete_item(0);
    let read_back: TodoItem = lib.items[0].to_string().parse().unwrap();
    assert_eq!(lib.items[0], read_back);
    assert_eq!(lib.items[0].priority.priority, Some(0));
}

#[test]
fn test_complete_item_next_occurrence_keeps_the_pri_tag() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.items.push("Task pri:B rec:1w".parse().unwrap());
    assert_eq!(lib.complete_item(0), Some(true));
    let next = &lib.items[1];
    assert!(!next.done);
    assert_eq!(next.priority.priority, None);
    assert_eq!(next.description, "Task pri:B");
    assert!(next.to_string().contains(" Task pri:B rec:1w"), "{next}");
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

#[test]
fn the_next_occurrence_keeps_the_line_layout() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.items.push(
        "(B) 2026-01-05 Water +garden the plants due:2026-01-10 rec:+1w h:1"
            .parse()
            .unwrap(),
    );
    lib.complete_item(0);
    let today = chrono::Local::now().date_naive();
    assert_eq!(
        lib.items[1].to_string(),
        format!("(B) {today} Water +garden the plants due:2026-01-17 rec:+1w h:1")
    );
}

#[test]
fn business_days_skip_saturdays_and_sundays() {
    let date = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
    // 2026-01-09 is a Friday, 2026-01-05 a Monday.
    for (from, count, expected) in [
        ("2026-01-09", 1, "2026-01-12"),  // Friday + 1: Monday
        ("2026-01-05", 5, "2026-01-12"),  // Monday + 5: next Monday
        ("2026-01-10", 1, "2026-01-12"),  // Saturday + 1: Monday
        ("2026-01-07", 0, "2026-01-07"),  // Wednesday + 0: unchanged
        ("2026-01-08", 10, "2026-01-22"), // Thursday + 10: over two weekends
    ] {
        assert_eq!(
            add_business_days(date(from), count),
            date(expected),
            "{from} + {count}b"
        );
    }
}

#[test]
fn a_normal_recurrence_with_only_a_threshold_moves_it_from_today() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.items
        .push("Water the plants t:2026-01-05 rec:1w".parse().unwrap());
    assert_eq!(lib.complete_item(0), Some(true));
    let today = chrono::Local::now().date_naive();
    assert_eq!(
        lib.items[1].threshold,
        Some(today + chrono::Duration::weeks(1))
    );
    assert_eq!(lib.items[1].due, None);
}

#[test]
fn a_strict_recurrence_without_dates_is_due_from_today() {
    let mut lib = TodoLibrary::new("dummy.txt".to_string());
    lib.items.push("Water the plants rec:+3d".parse().unwrap());
    assert_eq!(lib.complete_item(0), Some(true));
    let today = chrono::Local::now().date_naive();
    assert_eq!(lib.items[1].due, Some(today + chrono::Duration::days(3)));
    assert_eq!(lib.items[1].threshold, None);
}
