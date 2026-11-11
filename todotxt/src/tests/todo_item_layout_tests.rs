use super::*;
use chrono::NaiveDate;

fn item(line: &str) -> TodoItem {
    line.parse().unwrap()
}

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

#[test]
fn a_removed_tag_takes_the_whitespace_before_it() {
    let line = "Call +Mom about @home stuff due:2026-01-10 h:1";
    assert_eq!(
        item(line).set_due(None).to_string(),
        "Call +Mom about @home stuff h:1"
    );
    let mom = TodoProject {
        name: "Mom".to_owned(),
    };
    assert_eq!(
        item(line).remove_project(&mom).unwrap().to_string(),
        "Call about @home stuff due:2026-01-10 h:1"
    );
}

#[test]
fn a_tag_removed_from_the_start_leaves_no_leading_space() {
    assert_eq!(
        item("due:2026-01-10 Pay the rent")
            .set_due(None)
            .to_string(),
        "Pay the rent"
    );
}

#[test]
fn a_new_tag_goes_after_the_last_word_and_before_trailing_whitespace() {
    assert_eq!(
        item("Pay the rent ")
            .set_threshold(Some(date("2026-01-08")))
            .to_string(),
        "Pay the rent t:2026-01-08 "
    );
}

#[test]
fn new_tags_are_appended_in_the_usual_order() {
    let phone = TodoContext {
        name: "phone".to_owned(),
    };
    assert_eq!(
        item("Call Mom")
            .set_due(Some(date("2026-01-10")))
            .add_context(phone)
            .to_string(),
        "Call Mom @phone due:2026-01-10"
    );
}

#[test]
fn a_description_edit_keeps_the_tags_in_place() {
    let line = "Call +Mom about @home stuf due:2026-01-10 h:1";
    for (description, expected) in [
        (
            "Call about stuff h:1",
            "Call +Mom about @home stuff due:2026-01-10 h:1",
        ),
        (
            "Call about stuf h:1 tonight",
            "Call +Mom about @home stuf due:2026-01-10 h:1 tonight",
        ),
        (
            "Please call about stuf h:1",
            "Please call +Mom about @home stuf due:2026-01-10 h:1",
        ),
        ("Call about h:1", "Call +Mom about @home due:2026-01-10 h:1"),
    ] {
        assert_eq!(
            item(line)
                .set_description(description.to_owned())
                .to_string(),
            expected
        );
    }
}

#[test]
fn a_rewritten_description_takes_the_place_of_the_old_one() {
    assert_eq!(
        item("Call +Mom about @home stuf due:2026-01-10 h:1")
            .set_description("Ask about things".to_owned())
            .to_string(),
        "Ask about things +Mom @home due:2026-01-10"
    );
}

#[test]
fn a_repeated_project_removed_once_stays_once() {
    let home = TodoProject {
        name: "home".to_owned(),
    };
    assert_eq!(
        item("Paint +home the fence +home")
            .remove_project(&home)
            .unwrap()
            .to_string(),
        "Paint +home the fence"
    );
}

#[test]
fn completing_and_uncompleting_change_only_the_head_and_pri() {
    let mut done = item("(A) 2026-01-05 Call Mom +Family about the trip");
    done.done = true;
    done.completion_date = Some(date("2026-01-06"));
    let done = done.to_string();
    assert_eq!(
        done,
        "x 2026-01-06 2026-01-05 Call Mom +Family about the trip pri:A"
    );

    let mut open = item(&done);
    open.done = false;
    open.completion_date = None;
    assert_eq!(
        open.to_string(),
        "(A) 2026-01-05 Call Mom +Family about the trip"
    );
}

#[test]
fn a_head_is_added_to_a_line_without_one() {
    assert_eq!(
        item("Call  Mom +Family")
            .set_creation_date(Some(date("2026-01-05")))
            .to_string(),
        "2026-01-05 Call  Mom +Family"
    );
}

#[test]
fn a_removed_head_leaves_no_leading_space() {
    assert_eq!(
        item("(A) Call Mom")
            .set_priority(TodoPriority { priority: None })
            .to_string(),
        "Call Mom"
    );
}

#[test]
fn an_item_built_in_code_is_written_in_the_fixed_order() {
    let mut item = item("Call +Mom about stuff");
    item.layout = None;
    assert_eq!(item.to_string(), "Call about stuff +Mom");
}

#[test]
fn the_layout_does_not_take_part_in_equality() {
    assert_eq!(item("Pay  the rent"), item("Pay the rent"));
}
