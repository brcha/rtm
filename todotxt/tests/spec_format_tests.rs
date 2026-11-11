//! The todo.txt format rules, from the "todo.txt format rules" section of the specification:
//! https://github.com/todotxt/todo.txt (README.md). Each test names the rule it checks; the
//! negative cases come from the specification's own examples where it gives them.

use chrono::NaiveDate;
use todotxt::{TodoContext, TodoItem, TodoProject};

fn parse(line: &str) -> TodoItem {
    line.parse()
        .unwrap_or_else(|e| panic!("{line:?} does not parse: {e}"))
}

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn projects(item: &TodoItem) -> Vec<&str> {
    item.projects.iter().map(|p| p.name.as_str()).collect()
}

fn contexts(item: &TodoItem) -> Vec<&str> {
    item.contexts.iter().map(|c| c.name.as_str()).collect()
}

// Incomplete tasks, rule 1: if priority exists, it always appears first. The priority is an
// uppercase character from A-Z enclosed in parentheses and followed by a space.

/// Incomplete tasks, rule 1: `(A) Call Mom` has priority A.
#[test]
fn a_leading_uppercase_letter_in_parentheses_is_the_priority() {
    let item = parse("(A) Call Mom");
    assert_eq!(item.priority.priority, Some(0));
    assert_eq!(item.description, "Call Mom");
    let item = parse("(Z) Call Mom");
    assert_eq!(item.priority.priority, Some(25));
}

/// Incomplete tasks, rule 1, the specification's example: a priority that is not first is not a
/// priority (`Really gotta call Mom (A) @phone @someday`).
#[test]
fn a_priority_that_is_not_first_is_text() {
    let item = parse("Really gotta call Mom (A) @phone @someday");
    assert_eq!(item.priority.priority, None);
    assert!(item.description.contains("Mom (A)"));
    assert_eq!(contexts(&item), ["phone", "someday"]);
}

/// Incomplete tasks, rule 1, the specification's example: a lowercase letter is not a priority
/// (`(b) Get back to the boss`); the line is an ordinary task.
#[test]
#[ignore = "RTM-203: lines rtm cannot interpret fail to parse and are dropped"]
fn a_lowercase_letter_in_parentheses_is_text() {
    let item = parse("(b) Get back to the boss");
    assert_eq!(item.priority.priority, None);
    assert!(item.description.starts_with("(b) Get back"));
}

/// Incomplete tasks, rule 1: only a letter from A to Z is a priority; `(1)`, `(?)` and a
/// non-ASCII letter are ordinary text.
#[test]
#[ignore = "RTM-203: lines rtm cannot interpret fail to parse and are dropped"]
fn a_non_letter_in_parentheses_is_text() {
    for line in [
        "(1) Get back to the boss",
        "(?) Get back to the boss",
        "(Ж) Позвать маму",
    ] {
        let item = parse(line);
        assert_eq!(item.priority.priority, None, "{line}");
        assert_eq!(item.to_string(), line);
    }
}

/// Incomplete tasks, rule 1: the priority is a single letter; `(AB)` is text.
#[test]
fn two_letters_in_parentheses_are_text() {
    let item = parse("(AB) Get back to the boss");
    assert_eq!(item.priority.priority, None);
    assert_eq!(item.to_string(), "(AB) Get back to the boss");
}

/// Incomplete tasks, rule 1, the specification's example: the priority is followed by a space
/// (`(B)->Submit TPS report` has none); so is `(A)Call`.
#[test]
fn a_priority_not_followed_by_a_space_is_text() {
    for line in ["(B)->Submit TPS report", "(A)Call Mom"] {
        let item = parse(line);
        assert_eq!(item.priority.priority, None, "{line}");
        assert_eq!(item.to_string(), line);
    }
}

/// Incomplete tasks, rule 1: the priority appears first, so a line that starts with whitespace
/// has none.
#[test]
#[ignore = "RTM-212: leading whitespace and tabs are ignored"]
fn a_priority_after_leading_whitespace_is_text() {
    let item = parse(" (A) Call Mom");
    assert_eq!(item.priority.priority, None);
    assert_eq!(item.to_string(), " (A) Call Mom");
}

/// Incomplete tasks, rule 1: the priority is followed by a space; a tab is not one.
#[test]
#[ignore = "RTM-212: leading whitespace and tabs are ignored"]
fn a_priority_followed_by_a_tab_is_text() {
    let item = parse("(A)\tCall Mom");
    assert_eq!(item.priority.priority, None);
    assert_eq!(item.to_string(), "(A)\tCall Mom");
}

// Incomplete tasks, rule 2: a task's creation date may optionally appear directly after
// priority and a space. If there is no priority, the creation date appears first. If the
// creation date exists, it should be in the format YYYY-MM-DD.

/// Incomplete tasks, rule 2, the specification's examples: a creation date first, or directly
/// after the priority.
#[test]
fn a_date_first_or_after_the_priority_is_the_creation_date() {
    let item = parse("2011-03-02 Document +TodoTxt task format");
    assert_eq!(item.creation_date, Some(date("2011-03-02")));
    assert_eq!(item.priority.priority, None);
    let item = parse("(A) 2011-03-02 Call Mom");
    assert_eq!(item.creation_date, Some(date("2011-03-02")));
    assert_eq!(item.priority.priority, Some(0));
    assert_eq!(item.description, "Call Mom");
}

/// Incomplete tasks, rule 2, the specification's example: a date later in the line is not the
/// creation date (`(A) Call Mom 2011-03-02`).
#[test]
fn a_date_later_in_the_line_is_text() {
    let item = parse("(A) Call Mom 2011-03-02");
    assert_eq!(item.creation_date, None);
    assert!(item.description.ends_with("Mom 2011-03-02"));
}

/// Incomplete tasks, rule 2: the creation date is in the format YYYY-MM-DD; a date written
/// differently is text and stays as written.
#[test]
#[ignore = "RTM-205: dates not in YYYY-MM-DD are accepted"]
fn a_date_not_in_yyyy_mm_dd_is_text() {
    for line in [
        "2011-3-2 Call Mom",
        "2011-03-2 Call Mom",
        "11-03-02 Call Mom",
    ] {
        let item = parse(line);
        assert_eq!(item.creation_date, None, "{line}");
        assert_eq!(item.to_string(), line);
    }
}

/// Incomplete tasks, rule 2: a word that only starts with a date is not a date.
#[test]
fn a_word_that_starts_with_a_date_is_text() {
    let item = parse("2011-03-02x Call Mom");
    assert_eq!(item.creation_date, None);
    assert_eq!(item.to_string(), "2011-03-02x Call Mom");
}

/// Incomplete tasks, rule 2: the date after a priority and a creation date is text, not a second
/// date field.
#[test]
fn only_one_creation_date_is_read() {
    let item = parse("(A) 2011-03-02 2011-03-01 Call Mom");
    assert_eq!(item.creation_date, Some(date("2011-03-02")));
    assert_eq!(item.completion_date, None);
    assert!(item.description.starts_with("2011-03-01 Call"));
}

// Incomplete tasks, rule 3: contexts and projects may appear anywhere in the line after
// priority/prepended date. A context is preceded by a single space and an at-sign (@), a project
// by a single space and a plus-sign (+); each contains any non-whitespace character.

/// Incomplete tasks, rule 3, the specification's example: several projects and contexts.
#[test]
fn projects_and_contexts_are_read() {
    let item = parse("(A) Call Mom +Family +PeaceLoveAndHappiness @iphone @phone");
    assert_eq!(projects(&item), ["Family", "PeaceLoveAndHappiness"]);
    assert_eq!(contexts(&item), ["iphone", "phone"]);
}

/// Incomplete tasks, rule 3: projects and contexts may appear anywhere after the priority and
/// date, also between the words of the description.
#[test]
fn projects_and_contexts_in_the_middle_of_the_line_are_read() {
    let item = parse("Call +Mom about @home stuff");
    assert_eq!(projects(&item), ["Mom"]);
    assert_eq!(contexts(&item), ["home"]);
    let item = parse("+Garage @home Sell the bike");
    assert_eq!(projects(&item), ["Garage"]);
    assert_eq!(contexts(&item), ["home"]);
}

/// Incomplete tasks, rule 3: a project or context contains any non-whitespace character,
/// punctuation and non-ASCII letters included.
#[test]
fn projects_and_contexts_take_any_non_whitespace_character() {
    let item = parse("Sell +Garage-Sale, @home/office now");
    assert_eq!(projects(&item), ["Garage-Sale,"]);
    assert_eq!(contexts(&item), ["home/office"]);
    let item = parse("Инфостан +рачуни @кућа");
    assert_eq!(
        item.projects,
        [TodoProject {
            name: "рачуни".to_string()
        }]
    );
    assert_eq!(
        item.contexts,
        [TodoContext {
            name: "кућа".to_string()
        }]
    );
}

/// Incomplete tasks, rule 3, the specification's examples: an at-sign or plus-sign inside a word
/// is not a context or project (`soandso@example.com`, `2+2`).
#[test]
fn an_at_or_plus_sign_inside_a_word_is_text() {
    let item = parse("Email SoAndSo at soandso@example.com");
    assert!(item.contexts.is_empty());
    assert_eq!(item.description, "Email SoAndSo at soandso@example.com");
    let item = parse("Learn how to add 2+2");
    assert!(item.projects.is_empty());
    assert_eq!(item.description, "Learn how to add 2+2");
}

/// Incomplete tasks, rule 3: a project or context has at least one character; a lone `+` or `@`
/// is text.
#[test]
fn a_lone_plus_or_at_sign_is_text() {
    let item = parse("Add 2 + 2 @ home");
    assert!(item.projects.is_empty());
    assert!(item.contexts.is_empty());
    assert_eq!(item.to_string(), "Add 2 + 2 @ home");
}

// Complete tasks, rule 1: a completed task starts with a lowercase x followed directly by a
// space.

/// Complete tasks, rule 1, the specification's example: `x 2011-03-03 Call Mom` is complete.
#[test]
fn a_line_starting_with_x_and_a_space_is_complete() {
    let item = parse("x 2011-03-03 Call Mom");
    assert!(item.done);
    assert_eq!(item.completion_date, Some(date("2011-03-03")));
    assert_eq!(item.description, "Call Mom");
}

/// Complete tasks, rule 1, the specification's examples: these are not complete
/// (`xylophone lesson`, `X 2012-01-01 Make resolutions`, `(A) x Find ticket prices`).
#[test]
fn other_lines_starting_with_x_are_not_complete() {
    let item = parse("xylophone lesson");
    assert!(!item.done);
    assert_eq!(item.description, "xylophone lesson");

    let item = parse("X 2012-01-01 Make resolutions");
    assert!(!item.done);
    assert_eq!(item.completion_date, None);
    assert_eq!(item.creation_date, None);
    assert_eq!(item.to_string(), "X 2012-01-01 Make resolutions");

    let item = parse("(A) x Find ticket prices");
    assert!(!item.done);
    assert_eq!(item.priority.priority, Some(0));
    assert_eq!(item.description, "x Find ticket prices");
}

/// Complete tasks, rule 1: a completed task starts with the x, so a line that starts with
/// whitespace is not complete.
#[test]
#[ignore = "RTM-212: leading whitespace and tabs are ignored"]
fn a_line_starting_with_whitespace_is_not_complete() {
    let item = parse(" x 2011-03-03 Call Mom");
    assert!(!item.done);
    assert_eq!(item.completion_date, None);
    assert_eq!(item.to_string(), " x 2011-03-03 Call Mom");
}

/// Complete tasks, rule 1: the x is followed directly by a space; a tab is not one.
#[test]
#[ignore = "RTM-212: leading whitespace and tabs are ignored"]
fn x_followed_by_a_tab_is_not_complete() {
    let item = parse("x\t2011-03-03 Call Mom");
    assert!(!item.done);
    assert_eq!(item.to_string(), "x\t2011-03-03 Call Mom");
}

// Complete tasks, rule 2: the date of completion appears directly after the x, separated by a
// space. A creation date, if any, follows the completion date. Many clients discard the priority
// on completion; to preserve it, use the key:value format (pri:A).

/// Complete tasks, rule 2, the specification's example: completion date, then creation date.
#[test]
fn a_complete_task_has_its_completion_date_then_its_creation_date() {
    let item = parse("x 2011-03-02 2011-03-01 Review Tim's pull request +TodoTxtTouch @github");
    assert!(item.done);
    assert_eq!(item.completion_date, Some(date("2011-03-02")));
    assert_eq!(item.creation_date, Some(date("2011-03-01")));
    assert_eq!(projects(&item), ["TodoTxtTouch"]);
    assert_eq!(contexts(&item), ["github"]);
}

/// Complete tasks, rule 2: a single date after the x is the completion date.
#[test]
fn a_single_date_on_a_complete_task_is_the_completion_date() {
    let item = parse("x 2011-03-03 Call Mom");
    assert_eq!(item.completion_date, Some(date("2011-03-03")));
    assert_eq!(item.creation_date, None);
}

/// Complete tasks, rule 2: a complete task is written with its completion date first and its
/// creation date after it; without a completion date no date is written, as a lone date would
/// be read back as the completion date.
#[test]
fn a_complete_task_is_written_with_its_dates_in_order() {
    let item = parse("2011-03-01 Call Mom");
    let done = item
        .set_done(true)
        .set_completion_date(Some(date("2011-03-02")));
    assert_eq!(done.to_string(), "x 2011-03-02 2011-03-01 Call Mom");
    let undated = item.set_done(true);
    assert_eq!(undated.to_string(), "x Call Mom");
}

/// Complete tasks, rule 2: the priority of a complete task is preserved as `pri:A`.
#[test]
fn the_priority_of_a_complete_task_is_written_as_a_pri_tag() {
    let item = parse("(A) 2011-03-01 Call Mom");
    let done = item
        .set_done(true)
        .set_completion_date(Some(date("2011-03-02")));
    assert_eq!(done.to_string(), "x 2011-03-02 2011-03-01 Call Mom pri:A");
    assert_eq!(parse(&done.to_string()).priority.priority, Some(0));
}

/// Complete tasks, rules 1 and 2: some clients write a complete task's priority between the x
/// and the dates (`x (A) 2011-03-02 …`). rtm accepts that form, reading the priority and the
/// dates, and writes the task back in the specification's form, with the completion date
/// directly after the x and the priority as `pri:A`.
#[test]
fn a_priority_between_x_and_the_dates_is_accepted_and_written_in_the_spec_form() {
    let item = parse("x (A) 2011-03-02 2011-03-01 Call Mom");
    assert!(item.done);
    assert_eq!(item.priority.priority, Some(0));
    assert_eq!(item.completion_date, Some(date("2011-03-02")));
    assert_eq!(item.creation_date, Some(date("2011-03-01")));
    assert_eq!(item.to_string(), "x 2011-03-02 2011-03-01 Call Mom pri:A");
}

// Additional file format definitions: tool developers define extra metadata as key:value. Both
// key and value consist of non-whitespace characters which are not colons; one colon separates
// them.

/// Additional definitions: tags rtm does not know stay in the line, in their order.
#[test]
fn unknown_key_value_tags_are_kept_in_order() {
    let item = parse("Call Mom h:1 id:3 foo:bar");
    assert!(item.description.contains("h:1 id:3 foo:bar"));
    assert_eq!(item.to_string(), "Call Mom h:1 id:3 foo:bar");
}

/// Additional definitions: a word with two colons, or with an empty key or value, is not a
/// key:value tag and stays as written.
#[test]
fn words_that_are_not_key_value_tags_are_text() {
    let line = "Read http://example.com a:b:c key: :value";
    let item = parse(line);
    assert_eq!(item.description, line);
    assert_eq!(item.to_string(), line);
}
