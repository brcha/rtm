use super::*;

fn item(line: &str) -> TodoItem {
    line.parse().unwrap()
}

fn form(line: &str) -> EditForm {
    EditForm::new(0, &item(line))
}

const FULL: &str =
    "x 2026-10-25 2026-10-20 Pay bill pri:D +Home @online due:2026-11-01 rec:+2w t:2026-10-30";

#[test]
fn saving_an_unchanged_form_keeps_the_item() {
    let mut target = item(FULL);
    form(FULL).apply(&mut target).unwrap();
    assert_eq!(target, item(FULL));
    assert_eq!(target.priority.priority, Some(3));
}

#[test]
fn fields_are_filled_from_the_line() {
    let f = form("(B) Task +Work +Home @office due:2026-11-01 rec:1w t:2026-10-30");
    assert_eq!(f.description, "Task");
    assert_eq!(f.priority, Some(1));
    assert_eq!(f.due, "2026-11-01");
    assert_eq!(f.threshold, "2026-10-30");
    assert_eq!(f.recurrence, "1w");
    assert_eq!(f.projects, "Work, Home");
    assert_eq!(f.contexts, "office");
}

#[test]
fn every_field_is_written_and_the_rest_kept() {
    let mut target = item(FULL);
    let mut f = form(FULL);
    f.description = "  New text ".into();
    f.priority = Some(0);
    f.due = "2026-12-24".into();
    f.threshold = String::new();
    f.recurrence = " 1m ".into();
    f.projects = "+Work, , Garden ".into();
    f.contexts = "@home,phone".into();

    f.apply(&mut target).unwrap();

    assert_eq!(target.description, "New text");
    assert_eq!(target.priority.priority, Some(0));
    assert_eq!(target.due, NaiveDate::from_ymd_opt(2026, 12, 24));
    assert_eq!(target.threshold, None);
    assert_eq!(target.recurrence, "1m".parse().ok());
    let projects: Vec<_> = target.projects.iter().map(|p| p.name.as_str()).collect();
    let contexts: Vec<_> = target.contexts.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(projects, ["Work", "Garden"]);
    assert_eq!(contexts, ["home", "phone"]);
    // Not in the dialog: kept.
    let original = item(FULL);
    assert!(target.done);
    assert_eq!(target.completion_date, original.completion_date);
    assert_eq!(target.creation_date, original.creation_date);
}

#[test]
fn empty_fields_clear() {
    let mut target = item(FULL);
    let mut f = form(FULL);
    f.priority = None;
    (f.due, f.threshold, f.recurrence) = (String::new(), " ".into(), String::new());
    (f.projects, f.contexts) = (String::new(), " , ".into());
    f.apply(&mut target).unwrap();
    assert_eq!(target.priority.priority, None);
    assert_eq!((target.due, target.threshold), (None, None));
    assert!(target.recurrence.is_none());
    assert!(target.projects.is_empty() && target.contexts.is_empty());
}

type InvalidCase = (fn(&mut EditForm), &'static str);

/// Each invalid field is refused with its own message and leaves the item untouched.
#[test]
fn invalid_input_is_refused() {
    let cases: [InvalidCase; 10] = [
        (|f| f.description = "  ".into(), "Description is required"),
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
        assert_eq!(target, item(FULL), "{message}");
    }
}

/// Saves `description` on the item read from `line`; the item as saved, or the error.
fn save_description(line: &str, description: &str) -> Result<TodoItem, String> {
    let mut target = item(line);
    let mut f = form(line);
    f.description = description.into();
    f.apply(&mut target).map(|()| target)
}

/// A description the file would read back as a different item is refused, with the field to
/// use when there is one, and the item is left untouched.
#[test]
fn a_description_that_reads_back_differently_is_refused() {
    const OTHER: &str = "Description: reads back as a different item";
    let cases = [
        ("x marks the spot", OTHER),
        ("(A) Call mom", OTHER),
        ("2026-11-01 Call mom", OTHER),
        ("Call uuid:6ba7b810-9dad-11d1-80b4-00c04fd430c8", OTHER),
        (
            "Call +mom",
            "Description: put projects in the Projects field",
        ),
        (
            "Call @phone",
            "Description: put contexts in the Contexts field",
        ),
        (
            "Call due:2027-01-01",
            "Description: put the due date in the Due Date field",
        ),
        (
            "Call t:2027-01-01",
            "Description: put the threshold in the Threshold field",
        ),
        (
            "Call rec:1w",
            "Description: put the recurrence in the Recurrence field",
        ),
        // Projects are named first when several fields differ.
        (
            "Call +mom due:2027-01-01",
            "Description: put projects in the Projects field",
        ),
    ];
    for (description, message) in cases {
        let mut target = item("Call mom");
        let mut f = form("Call mom");
        f.description = description.into();
        let error = f.apply(&mut target).unwrap_err();
        assert!(error.starts_with(message), "{description}: {error}");
        assert_eq!(target, item("Call mom"), "{description}");
    }
}

#[test]
fn a_description_that_reads_back_the_same_is_saved() {
    // After a creation date, a leading `x` is text.
    let saved = save_description("2026-10-20 Call mom", "x marks the spot").unwrap();
    assert!(!saved.done);
    assert_eq!(saved.description, "x marks the spot");

    let saved = save_description("Call mom", "  New   text\there ").unwrap();
    assert_eq!(saved.description, "New text here");

    // A changed description on a line with tags among its words.
    let line = "(B) Call +mom about @phone the bill due:2026-12-01";
    let saved = save_description(line, "Ring about the gas bill").unwrap();
    assert_eq!(saved.to_string().parse::<TodoItem>().unwrap(), saved);
    assert_eq!(saved.description, "Ring about the gas bill");
    assert_eq!(saved.projects, item(line).projects);
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
    let mut target = item(FULL);
    let mut f = form(FULL);
    f.threshold = "2026-11-01".into();
    f.apply(&mut target).unwrap();
    assert_eq!(target.threshold, target.due);
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
