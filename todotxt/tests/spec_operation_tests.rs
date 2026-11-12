//! The operations every frontend performs through `TodoLibrary`: complete, uncomplete, edit and
//! recurrence. Completion follows the specification's complete-task rules
//! (https://github.com/todotxt/todo.txt); recurrence is an extension, and where the
//! specification is silent the cases follow topydo and SimpleTask: strict recurrence (`rec:+…`)
//! counts from the due date, normal recurrence from the day of completion, months and years are
//! calendar months (the day clamped to the month's end), and `b` counts business days.

use chrono::{Days, Local, NaiveDate};
use todotxt::{TodoItem, TodoLibrary, TodoPriority};

fn parse(line: &str) -> TodoItem {
    line.parse()
        .unwrap_or_else(|e| panic!("{line:?} does not parse: {e}"))
}

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// A library holding the item of `line`, not saved anywhere.
fn library(line: &str) -> TodoLibrary {
    let mut library = TodoLibrary::new(String::new());
    library.items.push(parse(line));
    library
}

/// Completes the only item of a library holding `line` and returns the library.
fn complete(line: &str) -> TodoLibrary {
    let mut library = library(line);
    library.complete_item(0).unwrap();
    library
}

/// Completes the recurring item of `line` and returns its next occurrence.
fn next_occurrence(line: &str) -> TodoItem {
    let library = complete(line);
    assert_eq!(library.items.len(), 2, "{line} got no next occurrence");
    library.items[1].clone()
}

// Complete and uncomplete

/// Complete tasks, rules 1 and 2: completing marks the task with `x`, today's completion date and
/// the creation date after it, and keeps the priority as `pri:A`.
#[test]
fn completing_writes_x_the_dates_and_the_priority_tag() {
    // Where the new pri: tag goes is not part of the rule, only that it is there.
    let line = complete("(A) 2026-01-05 Call Mom +Family @phone").items[0].to_string();
    assert!(
        line.starts_with(&format!("x {} 2026-01-05 Call Mom ", today())),
        "{line}"
    );
    assert!(line.contains("+Family @phone"), "{line}");
    assert_eq!(line.matches(" pri:A").count(), 1, "{line}");
    assert!(!line.contains("(A)"), "{line}");
}

/// Complete tasks, rule 2: a task without a creation date gets only its completion date.
#[test]
fn completing_a_task_without_a_creation_date_writes_only_the_completion_date() {
    let library = complete("Call Mom");
    assert_eq!(
        library.items[0].to_string(),
        format!("x {} Call Mom", today())
    );
}

/// Complete tasks, rule 1: completing changes only the start of the line; projects, contexts and
/// tags between the words stay in place.
#[test]
fn completing_keeps_the_rest_of_the_line_in_place() {
    let line = complete("2026-01-05 Call +Mom about @home stuff h:1").items[0].to_string();
    assert!(
        line.starts_with(&format!("x {} 2026-01-05 ", today())),
        "{line}"
    );
    assert!(line.contains("Call +Mom about @home stuff h:1"), "{line}");
}

/// Complete tasks, rule 2: completing a task that is already complete keeps its completion date.
#[test]
fn completing_a_complete_task_keeps_its_completion_date() {
    let library = complete("x 2026-01-06 2026-01-05 Call Mom");
    assert_eq!(library.items[0].completion_date, Some(date("2026-01-06")));
}

/// Complete tasks, rules 1 and 2, reversed: uncompleting removes the `x` and the completion date
/// and turns `pri:A` back into the leading `(A)` (incomplete tasks, rules 1 and 2).
#[test]
fn uncompleting_restores_the_open_form() {
    for (line, open) in [
        (
            "x 2026-01-06 2026-01-05 Call Mom pri:A",
            "(A) 2026-01-05 Call Mom",
        ),
        (
            "x 2026-01-06 2026-01-05 Call Mom +Family",
            "2026-01-05 Call Mom +Family",
        ),
    ] {
        let mut lib = library(line);
        lib.uncomplete_item(0).unwrap();
        assert_eq!(lib.items[0].to_string(), open);
    }
}

// Edit

/// Editing one field of a line in rtm's order changes only that field.
#[test]
fn editing_a_field_changes_only_that_field() {
    let item = parse("2026-01-05 Call Mom +Family due:2026-01-10");
    assert_eq!(
        item.set_due(Some(date("2026-01-12"))).to_string(),
        "2026-01-05 Call Mom +Family due:2026-01-12"
    );
    assert_eq!(
        item.set_priority(TodoPriority { priority: Some(1) })
            .to_string(),
        "(B) 2026-01-05 Call Mom +Family due:2026-01-10"
    );
}

/// Incomplete tasks, rule 3: editing one field keeps the projects, contexts and tags placed
/// between the words where they are.
#[test]
fn editing_a_field_keeps_the_rest_of_the_line_in_place() {
    let item = parse("2026-01-05 Call +Mom about @home stuff due:2026-01-10 h:1");
    assert_eq!(
        item.set_due(Some(date("2026-01-12"))).to_string(),
        "2026-01-05 Call +Mom about @home stuff due:2026-01-12 h:1"
    );
    assert_eq!(
        item.set_priority(TodoPriority { priority: Some(1) })
            .to_string(),
        "(B) 2026-01-05 Call +Mom about @home stuff due:2026-01-10 h:1"
    );
    assert_eq!(
        item.set_threshold(Some(date("2026-01-08"))).to_string(),
        "2026-01-05 Call +Mom about @home stuff due:2026-01-10 h:1 t:2026-01-08"
    );
}

// Recurrence (extension)

/// Extension `rec:`, strict: the next due date counts from the old due date.
#[test]
fn strict_recurrence_counts_from_the_due_date() {
    for (rec, due, next) in [
        ("+1d", "2026-01-05", "2026-01-06"),
        ("+2d", "2026-01-05", "2026-01-07"),
        ("+1w", "2026-01-05", "2026-01-12"),
        ("+3w", "2026-12-20", "2027-01-10"),
    ] {
        let next_item = next_occurrence(&format!("Water the plants due:{due} rec:{rec}"));
        assert_eq!(next_item.due, Some(date(next)), "rec:{rec} from {due}");
    }
}

/// Extension `rec:`, normal: the next due date counts from the day of completion.
#[test]
fn normal_recurrence_counts_from_today() {
    let next = next_occurrence("Water the plants due:2020-01-05 rec:1w");
    assert_eq!(next.due, Some(today() + Days::new(7)));
    let next = next_occurrence("Water the plants due:2020-01-05 rec:3d");
    assert_eq!(next.due, Some(today() + Days::new(3)));
}

/// Extension `rec:` with `m`: a month is a calendar month, as in topydo and SimpleTask.
#[test]
fn monthly_recurrence_adds_calendar_months() {
    for (rec, due, next) in [
        ("+1m", "2026-03-15", "2026-04-15"),
        ("+m", "2026-07-10", "2026-08-10"),
        ("+3m", "2026-10-28", "2027-01-28"),
    ] {
        let next_item = next_occurrence(&format!("Pay the rent due:{due} rec:{rec}"));
        assert_eq!(next_item.due, Some(date(next)), "rec:{rec} from {due}");
    }
}

/// Extension `rec:` with `m`: a day that the next month lacks is clamped to its last day.
#[test]
fn monthly_recurrence_clamps_to_the_end_of_the_month() {
    let next = next_occurrence("Pay the rent due:2026-01-31 rec:+1m");
    assert_eq!(next.due, Some(date("2026-02-28")));
    let next = next_occurrence("Pay the rent due:2028-01-31 rec:+1m");
    assert_eq!(next.due, Some(date("2028-02-29")));
}

/// Extension `rec:` with `y`: a year is a calendar year, leap years included.
#[test]
fn yearly_recurrence_adds_calendar_years() {
    let next = next_occurrence("Renew the passport due:2027-03-01 rec:+1y");
    assert_eq!(next.due, Some(date("2028-03-01")));
    let next = next_occurrence("Renew the passport due:2028-02-29 rec:+1y");
    assert_eq!(next.due, Some(date("2029-02-28")));
}

/// Extension `rec:` with `b`: business days skip Saturdays and Sundays, as in topydo and
/// SimpleTask.
#[test]
fn business_day_recurrence_skips_weekends() {
    // 2026-01-09 is a Friday, 2026-01-05 a Monday.
    let next = next_occurrence("File the report due:2026-01-09 rec:+1b");
    assert_eq!(next.due, Some(date("2026-01-12")));
    let next = next_occurrence("File the report due:2026-01-05 rec:+5b");
    assert_eq!(next.due, Some(date("2026-01-12")));
}

/// Extension `rec:`: the next occurrence is an open copy of the task with today's creation date,
/// the same priority, projects, contexts, text and unknown tags, and an identity of its own (no
/// `uuid:`, which would duplicate the completed task's).
#[test]
fn the_next_occurrence_copies_the_task() {
    let line = format!(
        "(B) 2026-01-01 Pay +bills @home the rent h:1 due:2026-01-05 rec:+1w uuid:{}",
        "12345678-1234-1234-1234-123456789abc"
    );
    let library = complete(&line);
    let done = &library.items[0];
    let next = &library.items[1];
    assert!(done.done);
    assert!(!next.done);
    assert_eq!(next.completion_date, None);
    assert_eq!(next.creation_date, Some(today()));
    assert_eq!(next.priority, done.priority);
    assert_eq!(next.projects, done.projects);
    assert_eq!(next.contexts, done.contexts);
    assert_eq!(next.description, done.description);
    assert!(next.description.contains("h:1"));
    assert_eq!(next.recurrence, done.recurrence);
    assert_eq!(next.uuid, None);
}

/// Extension `rec:` and `t:`: the threshold date moves by the same interval as the due date, for
/// strict recurrence.
#[test]
fn strict_recurrence_moves_the_threshold_date() {
    let next = next_occurrence("Water the plants due:2026-01-10 t:2026-01-05 rec:+1w");
    assert_eq!(next.due, Some(date("2026-01-17")));
    assert_eq!(next.threshold, Some(date("2026-01-12")));
}

/// Extension `rec:` and `t:`: for normal recurrence the threshold date keeps its distance to the
/// new due date.
#[test]
fn normal_recurrence_moves_the_threshold_date() {
    let next = next_occurrence("Water the plants due:2020-01-10 t:2020-01-05 rec:3d");
    assert_eq!(next.due, Some(today() + Days::new(3)));
    assert_eq!(next.threshold, Some(today() + Days::new(3) - Days::new(5)));
}

/// Extension `rec:` and `t:`: a monthly recurrence moves the threshold date by a calendar month
/// too. A bill that can be paid from the 5th and must be paid by the 15th comes back on the same
/// days of the next month.
#[test]
fn monthly_recurrence_moves_the_threshold_date_by_a_month() {
    let next =
        next_occurrence("(C) 2026-10-11 Инфостан +рачуни @кућа t:2026-11-05 due:2026-11-15 rec:+m");
    assert_eq!(next.due, Some(date("2026-12-15")));
    assert_eq!(next.threshold, Some(date("2026-12-05")));
}

/// Extension `rec:` and `t:`: when the due date is clamped to the end of a shorter month, the
/// threshold date keeps its own day of the month (topydo would keep its distance to the due date
/// instead, giving 2026-02-18).
#[test]
fn monthly_recurrence_keeps_the_threshold_day_at_the_end_of_the_month() {
    let next = next_occurrence("Pay the rent t:2026-01-21 due:2026-01-31 rec:+m");
    assert_eq!(next.due, Some(date("2026-02-28")));
    assert_eq!(next.threshold, Some(date("2026-02-21")));
}

/// Extension `rec:`: a recurring task without a due date still recurs; the next due date counts
/// from today, as in topydo.
#[test]
#[ignore = "RTM-209: no next occurrence without a due date"]
fn a_recurring_task_without_a_due_date_recurs() {
    let mut library = library("Water the plants rec:1w");
    assert_eq!(library.complete_item(0), Some(true));
    assert_eq!(library.items.len(), 2);
    assert_eq!(library.items[1].due, Some(today() + Days::new(7)));
}

/// Extension `rec:` and `t:`: a recurring task with only a threshold date moves that date, as in
/// topydo.
#[test]
#[ignore = "RTM-209: no next occurrence without a due date"]
fn a_recurring_task_with_only_a_threshold_date_moves_it() {
    let next = next_occurrence("Water the plants t:2026-01-05 rec:+1w");
    assert_eq!(next.threshold, Some(date("2026-01-12")));
    assert_eq!(next.due, None);
}
