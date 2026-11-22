use super::*;

fn item(line: &str) -> TodoItem {
    line.parse().unwrap()
}

fn form(line: &str) -> EditForm {
    EditForm::new(0, &item(line))
}

const FULL: &str = "(D) 2026-10-20 Pay bill +Home @online due:2026-11-01 rec:+2w t:2026-10-30";

const UUID: &str = "123e4567-e89b-12d3-a456-426614174000";

/// Saves `f` on the item read from `line`; the item as saved, or the error.
fn save(line: &str, change: impl FnOnce(&mut EditForm)) -> Result<TodoItem, String> {
    let mut target = item(line);
    let mut f = form(line);
    change(&mut f);
    f.apply(&mut target).map(|()| target)
}

#[test]
fn saving_an_unchanged_form_keeps_the_line() {
    for line in [FULL, "Pay  bill +Home   @online", "Call uuid:{UUID} mom"] {
        let line = line.replace("{UUID}", UUID);
        let saved = save(&line, |_| {}).unwrap();
        assert_eq!(saved.to_string(), line);
    }
}

#[test]
fn the_form_is_filled_from_the_line() {
    let f = form("(B) Task +Work +Home @office due:2026-11-01 rec:1w t:2026-10-30");
    assert_eq!(
        f.line,
        "(B) Task +Work +Home @office due:2026-11-01 rec:1w t:2026-10-30"
    );
    assert_eq!(f.priority, Some(1));
    assert_eq!(f.due, "2026-11-01");
    assert_eq!(f.threshold, "2026-10-30");
    assert_eq!(f.recurrence, "1w");
    assert_eq!(f.projects, "Work, Home");
    assert_eq!(f.contexts, "office");
    assert!(!f.read_only);
}

#[test]
fn the_line_hides_the_sub_item_links_and_save_puts_them_back() {
    let line = format!("Plan uuid:{UUID} the trip sub:{UUID} +Travel");
    let mut f = form(&line);
    assert_eq!(f.line, "Plan the trip +Travel");
    f.line = "Plan the long trip +Travel".into();
    let mut target = item(&line);
    f.apply(&mut target).unwrap();
    assert_eq!(
        target.to_string(),
        format!("Plan the long trip +Travel uuid:{UUID} sub:{UUID}")
    );
}

#[test]
fn tags_typed_in_the_line_show_in_the_fields() {
    let mut f = form("Prepare presentation");
    f.line = "(A) Prepare presentation on +Horizon and @DDI-O for @SerbianWeekly \
              due:2026-12-01 t:2026-11-20 rec:1m"
        .into();
    f.sync();
    assert_eq!(f.priority, Some(0));
    assert_eq!(f.projects, "Horizon");
    assert_eq!(f.contexts, "DDI-O, SerbianWeekly");
    assert_eq!(f.due, "2026-12-01");
    assert_eq!(f.threshold, "2026-11-20");
    assert_eq!(f.recurrence, "1m");

    f.line = "Prepare presentation on Horizon for @SerbianWeekly".into();
    f.sync();
    assert_eq!(f.priority, None);
    assert_eq!(f.projects, "");
    assert_eq!(f.contexts, "SerbianWeekly");
    assert_eq!((f.due.as_str(), f.threshold.as_str()), ("", ""));
    assert_eq!(f.recurrence, "");
}

#[test]
fn a_date_reaches_its_field_once_it_reads_as_one() {
    let mut f = form("Ship it");
    f.line = "Ship it due:2026-12-0".into();
    f.sync();
    assert_eq!(f.due, "");
    f.line = "Ship it due:2026-12-01".into();
    f.sync();
    assert_eq!(f.due, "2026-12-01");
}

#[test]
fn field_changes_are_written_into_the_line_in_place() {
    let mut f = form("Prepare +Horizon slides for @work due:2026-12-01 soon");

    f.contexts = "work, phone".into();
    f.sync();
    assert_eq!(
        f.line,
        "Prepare +Horizon slides for @work due:2026-12-01 soon @phone"
    );

    f.projects = String::new();
    f.sync();
    assert_eq!(
        f.line,
        "Prepare slides for @work due:2026-12-01 soon @phone"
    );

    f.due = "2026-12-24".into();
    f.priority = Some(1);
    f.threshold = "2026-12-20".into();
    f.recurrence = "2w".into();
    f.sync();
    assert_eq!(
        f.line,
        "(B) Prepare slides for @work due:2026-12-24 soon @phone rec:2w t:2026-12-20"
    );

    f.due = String::new();
    f.priority = None;
    f.sync();
    assert_eq!(
        f.line,
        "Prepare slides for @work soon @phone rec:2w t:2026-12-20"
    );
}

#[test]
fn a_field_still_being_typed_leaves_the_line_alone() {
    let line = "Ship it due:2026-12-01";
    let mut f = form(line);
    f.due = "2026-1".into();
    f.sync();
    assert_eq!(f.line, line);
    // Another field changed meanwhile keeps the half-typed date.
    f.contexts = "desk".into();
    f.sync();
    assert_eq!(f.line, "Ship it due:2026-12-01 @desk");
    assert_eq!(f.due, "2026-1");
    assert_eq!(
        f.apply(&mut item(line)).unwrap_err(),
        "Due date: expected YYYY-MM-DD"
    );
}

#[test]
fn a_line_and_a_field_changed_together_are_both_saved() {
    let saved = save("(D) Second +Home", |f| {
        f.line = "(D) Second, edited +Home".into();
        f.due = "2026-11-02".into();
    })
    .unwrap();
    assert_eq!(saved.to_string(), "(D) Second, edited +Home due:2026-11-02");
}

#[test]
fn the_issue_example_is_saved_as_typed() {
    let typed = "Prepare presentation on +Horizon and @DDI-O for @SerbianWeekly";
    let saved = save("Prepare presentation", |f| f.line = format!("  {typed} ")).unwrap();
    assert_eq!(saved.to_string(), typed);
    let names: Vec<_> = saved.contexts.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["DDI-O", "SerbianWeekly"]);
}

#[test]
fn the_creation_date_is_saved_as_typed() {
    let saved = save("2026-10-20 Call mom", |f| f.line = "Call mom".into()).unwrap();
    assert_eq!(saved.creation_date, None);
    // After a creation date, a leading `x` is text.
    let saved = save("Call mom", |f| {
        f.line = "2026-10-21 x marks the spot".into()
    })
    .unwrap();
    assert!(!saved.done);
    assert_eq!(saved.description, "x marks the spot");
}

type InvalidCase = (fn(&mut EditForm), &'static str);

/// Each invalid line or field is refused with its own message and leaves the item untouched.
#[test]
fn invalid_input_is_refused() {
    let cases: [InvalidCase; 14] = [
        (|f| f.line = "  ".into(), "Line: a description is required"),
        (
            |f| f.line = "+Home @online".into(),
            "Line: a description is required",
        ),
        (
            |f| f.line = "x Pay bill".into(),
            "Line: complete the item with its checkbox",
        ),
        (
            |f| f.line = format!("Pay bill uuid:{UUID}"),
            "Line: uuid: and sub: tags",
        ),
        (
            |f| f.line = "Pay\nbill".into(),
            "Line: give exactly one todo.txt line",
        ),
        (
            |f| f.due = "2026-13-01".into(),
            "Due date: expected YYYY-MM-DD",
        ),
        (
            |f| f.threshold = "soon".into(),
            "Threshold: expected YYYY-MM-DD",
        ),
        (|f| f.recurrence = "weekly".into(), "Recurrence:"),
        (|f| f.recurrence = "+".into(), "Recurrence:"),
        (|f| f.recurrence = "1é".into(), "Recurrence:"),
        (|f| f.recurrence = "é".into(), "Recurrence:"),
        (
            |f| f.projects = "Work, My Project".into(),
            "Projects: names cannot contain spaces",
        ),
        (
            |f| f.contexts = "@at home".into(),
            "Contexts: names cannot contain spaces",
        ),
        (
            |f| f.threshold = "2026-11-05".into(),
            "Dates: threshold date 2026-11-05 is after due date 2026-11-01",
        ),
    ];
    for (change, message) in cases {
        let mut target = item(FULL);
        let mut f = form(FULL);
        change(&mut f);
        let error = f.apply(&mut target).unwrap_err();
        assert!(error.starts_with(message), "{error}");
        assert_eq!(target.to_string(), FULL, "{message}");
    }
}

#[test]
fn empty_fields_clear() {
    let saved = save(FULL, |f| {
        f.priority = None;
        (f.due, f.threshold, f.recurrence) = (String::new(), " ".into(), String::new());
        (f.projects, f.contexts) = (String::new(), " , ".into());
    })
    .unwrap();
    assert_eq!(saved.to_string(), "2026-10-20 Pay bill");
}

#[test]
fn a_done_item_is_shown_but_not_edited() {
    let line = format!("x 2026-10-25 2026-10-20 Pay bill pri:D +Home due:2026-11-01 uuid:{UUID}");
    let mut f = form(&line);
    assert!(f.read_only);
    assert_eq!(
        f.line,
        "x 2026-10-25 2026-10-20 Pay bill pri:D +Home due:2026-11-01"
    );
    f.line = "Something else".into();
    let mut target = item(&line);
    f.apply(&mut target).unwrap();
    assert_eq!(target.to_string(), line);
}

/// Only the exact `YYYY-MM-DD` form is a date: the file reads anything else as text.
#[test]
fn dates_must_be_exactly_yyyy_mm_dd() {
    for text in ["+10000-01-01", "2026-1-2", "26-01-02", "2026-01-02x"] {
        let mut f = form("Call mom");
        f.due = text.into();
        assert_eq!(
            f.apply(&mut item("Call mom")).unwrap_err(),
            "Due date: expected YYYY-MM-DD",
            "{text}"
        );
        let mut f = form("Call mom");
        f.threshold = text.into();
        assert_eq!(
            f.apply(&mut item("Call mom")).unwrap_err(),
            "Threshold: expected YYYY-MM-DD",
            "{text}"
        );
    }
}

#[test]
fn a_threshold_on_the_due_date_is_accepted() {
    let saved = save(FULL, |f| f.threshold = "2026-11-01".into()).unwrap();
    assert_eq!(saved.threshold, saved.due);
}

#[test]
fn delete_questions() {
    assert_eq!(delete_question(0), "Delete this item?");
    assert_eq!(delete_question(1), "Delete this item and 1 sub-item?");
    assert_eq!(delete_question(2), "Delete this item and 2 sub-items?");
}

#[test]
fn priority_labels() {
    assert_eq!(priority_label(None), "None");
    assert_eq!(priority_label(Some(0)), "(A) High");
    assert_eq!(priority_label(Some(2)), "(C) Low");
    assert_eq!(priority_label(Some(3)), "(D)");
    assert_eq!(priority_label(Some(25)), "(Z)");
}
