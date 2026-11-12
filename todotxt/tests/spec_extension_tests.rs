//! The todo.txt extensions rtm supports, all `key:value` tags in the sense of the specification's
//! "Additional File Format Definitions" (https://github.com/todotxt/todo.txt):
//!
//! - `due:YYYY-MM-DD` (due date) and `t:YYYY-MM-DD` (threshold date), as used by SimpleTask and
//!   topydo;
//! - `rec:[+][N]U` (recurrence, `U` one of `d`, `b`, `w`, `m`, `y`; `+` for strict), as used by
//!   SimpleTask and topydo;
//! - `pri:X`, the specification's own suggestion for the priority of a complete task;
//! - `uuid:` and `sub:`, rtm's id tags for subtasks.
//!
//! Tags rtm does not know, such as SimpleTask's hidden tag `h:1`, are ordinary text.

use chrono::NaiveDate;
use todotxt::TodoItem;
use todotxt::todo_recurrence::{TodoRecurrence, TodoRecurrenceUnit};
use uuid::Uuid;

const PARENT: &str = "12345678-1234-1234-1234-123456789abc";
const CHILD: &str = "87654321-4321-4321-4321-abc123456789";

fn parse(line: &str) -> TodoItem {
    line.parse()
        .unwrap_or_else(|e| panic!("{line:?} does not parse: {e}"))
}

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// Extension `due:` and `t:`: the dates are read and written back as `YYYY-MM-DD`.
#[test]
fn due_and_threshold_dates_are_read_and_written() {
    let line = "Pay rent due:2026-12-01 t:2026-11-25";
    let item = parse(line);
    assert_eq!(item.due, Some(date("2026-12-01")));
    assert_eq!(item.threshold, Some(date("2026-11-25")));
    assert_eq!(item.description, "Pay rent");
    assert_eq!(item.to_string(), line);
}

/// Extension `rec:`: every unit, strict and normal, with and without a count, is read.
#[test]
fn every_recurrence_form_is_read() {
    use TodoRecurrenceUnit::*;
    for (value, strict, count, unit) in [
        ("1d", false, 1, Daily),
        ("2b", false, 2, BusinessDay),
        ("+1w", true, 1, Weekly),
        ("+m", true, 1, Monthly),
        ("3m", false, 3, Monthly),
        ("+10y", true, 10, Yearly),
        ("w", false, 1, Weekly),
    ] {
        let item = parse(&format!("Water the plants rec:{value}"));
        assert_eq!(
            item.recurrence,
            Some(TodoRecurrence {
                strict,
                count,
                unit
            }),
            "rec:{value}"
        );
    }
}

/// Extension `rec:`: the value is written back as it was spelled. topydo reads a recurrence only
/// with a count (`rec:1w`), so dropping a count of 1 makes the line unreadable there; and a
/// canonical rewrite of `rec:+m` would change every synced file on its next save.
#[test]
fn a_recurrence_is_written_back_as_spelled() {
    for value in ["1d", "+1w", "1m", "+1y", "2b", "+m", "w"] {
        let line = format!("Water the plants rec:{value}");
        assert_eq!(parse(&line).to_string(), line);
    }
}

/// Extension `pri:`: the specification names `pri:A` only as the way to preserve the priority of
/// a complete task. On an open task the priority is the leading `(A)` (incomplete tasks, rule 1),
/// and `pri:A` is an ordinary key:value tag that stays as written.
#[test]
#[ignore = "RTM-211: pri: on an open line is read as the priority"]
fn a_pri_tag_on_an_open_task_is_an_ordinary_tag() {
    let item = parse("Call Mom pri:A");
    assert_eq!(item.priority.priority, None);
    assert_eq!(item.to_string(), "Call Mom pri:A");
    let item = parse("(C) Call Mom pri:A");
    assert_eq!(item.priority.priority, Some(2));
    assert_eq!(item.to_string(), "(C) Call Mom pri:A");
}

/// Extension `pri:`: on a complete task, `pri:B` is the priority and is written back unchanged
/// (complete tasks, rule 2).
#[test]
fn a_pri_tag_on_a_complete_task_is_the_priority() {
    let item = parse("x 2026-01-06 2026-01-05 Call Mom pri:B");
    assert_eq!(item.priority.priority, Some(1));
    assert_eq!(item.to_string(), "x 2026-01-06 2026-01-05 Call Mom pri:B");
}

/// Extension `pri:`: a value that is not a single letter A-Z is text.
#[test]
fn a_malformed_pri_tag_is_text() {
    let item = parse("Call Mom pri:xyz");
    assert_eq!(item.priority.priority, None);
    assert_eq!(item.to_string(), "Call Mom pri:xyz");
}

/// Extension `uuid:` and `sub:`: rtm's id tags are read and written back.
#[test]
fn id_tags_are_read_and_written() {
    let line = format!("Buy paint uuid:{CHILD} sub:{PARENT}");
    let item = parse(&line);
    assert_eq!(item.uuid, Some(Uuid::parse_str(CHILD).unwrap()));
    assert_eq!(item.sub, Some(Uuid::parse_str(PARENT).unwrap()));
    assert_eq!(item.description, "Buy paint");
    assert_eq!(item.to_string(), line);
}

/// Additional definitions: SimpleTask's hidden tag `h:1` is a tag rtm does not know; it stays in
/// the line unchanged.
#[test]
fn the_hidden_tag_is_kept() {
    let item = parse("Template for the weekly review h:1");
    assert!(item.description.ends_with("h:1"));
    assert_eq!(item.to_string(), "Template for the weekly review h:1");
}

/// Additional definitions: a key:value is free-form metadata, so a known key with a value rtm
/// cannot read is ordinary text. The line still parses, the field stays empty and the word is
/// kept as written.
#[test]
fn a_known_tag_with_a_malformed_value_is_text() {
    for line in [
        "Pay rent due:soon",
        "Pay rent due:2026-02-30",
        "Pay rent t:never",
        "Water the plants rec:often",
        "Water the plants rec:1x",
        "Buy paint uuid:nope",
        "Buy paint sub:nope",
    ] {
        let item = line
            .parse::<TodoItem>()
            .unwrap_or_else(|e| panic!("{line:?} does not parse: {e}"));
        assert_eq!(item.due, None, "{line}");
        assert_eq!(item.threshold, None, "{line}");
        assert_eq!(item.recurrence, None, "{line}");
        assert_eq!(item.uuid, None, "{line}");
        assert_eq!(item.sub, None, "{line}");
        assert_eq!(item.to_string(), line);
    }
}
