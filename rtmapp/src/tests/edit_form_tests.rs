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
fn fields_are_filled_like_the_tauri_app() {
    let f = form("(B) Task +Work +Home @office due:2026-11-01 rec:1w t:2026-10-30");
    assert_eq!(f.description, "Task");
    assert_eq!(f.priority, Some(1));
    assert_eq!(f.due, "2026-11-01");
    assert_eq!(f.threshold, "2026-10-30");
    assert_eq!(f.recurrence, "w");
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
    let cases: [InvalidCase; 9] = [
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
