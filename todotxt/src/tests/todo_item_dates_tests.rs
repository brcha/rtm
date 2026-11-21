use super::*;

fn item(line: &str) -> TodoItem {
    line.parse().unwrap()
}

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

#[test]
fn a_threshold_after_the_due_date_is_refused() {
    let error = item("Pay rent due:2026-01-05 t:2026-01-10")
        .check_dates()
        .unwrap_err();
    assert_eq!(
        error,
        TodoItemDateError::ThresholdAfterDue {
            threshold: date("2026-01-10"),
            due: date("2026-01-05"),
        }
    );
    assert_eq!(
        error.to_string(),
        "threshold date 2026-01-10 is after due date 2026-01-05"
    );
}

#[test]
fn a_threshold_on_the_due_date_is_allowed() {
    assert_eq!(
        item("Pay rent due:2026-01-05 t:2026-01-05").check_dates(),
        Ok(())
    );
}

#[test]
fn an_item_with_one_date_or_none_is_allowed() {
    for line in [
        "Pay rent t:2026-01-10",
        "Pay rent due:2026-01-05",
        "Pay rent",
    ] {
        assert_eq!(item(line).check_dates(), Ok(()), "{line}");
    }
}
