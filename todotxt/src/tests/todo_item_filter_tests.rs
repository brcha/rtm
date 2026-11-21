use super::*;

fn item(line: &str) -> TodoItem {
    line.parse().unwrap()
}

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 20).unwrap()
}

fn selects(filter: DateFilter, line: &str) -> bool {
    filter.matches(&item(line), today())
}

#[test]
fn test_today_and_week_select_a_due_or_threshold_date_in_range() {
    for line in ["Call due:2026-11-20", "Call t:2026-11-20 due:2026-11-30"] {
        assert!(selects(DateFilter::Today, line), "{line}");
        assert!(selects(DateFilter::Week, line), "{line}");
    }
    assert!(!selects(DateFilter::Today, "Plan t:2026-11-23"));
    assert!(selects(DateFilter::Week, "Plan t:2026-11-23"));
    assert!(selects(DateFilter::Week, "Plan due:2026-11-26"));
    assert!(!selects(DateFilter::Week, "Plan due:2026-11-27"));
    let passed_threshold = "Report t:2026-11-15 due:2026-12-20";
    assert!(!selects(DateFilter::Today, passed_threshold));
    assert!(!selects(DateFilter::Week, passed_threshold));
}

#[test]
fn test_overdue_reads_only_the_due_date() {
    assert!(selects(DateFilter::Overdue, "Rent due:2026-11-19"));
    assert!(!selects(DateFilter::Overdue, "Report t:2026-11-15"));
    assert!(!selects(DateFilter::Overdue, "Call due:2026-11-20"));
}

#[test]
fn test_future_selects_items_not_actionable_yet() {
    assert!(!selects(
        DateFilter::Future,
        "Report t:2026-11-15 due:2026-12-01"
    ));
    assert!(selects(DateFilter::Future, "Trip t:2026-12-05"));
    assert!(selects(DateFilter::Future, "Car due:2026-11-21"));
    assert!(!selects(DateFilter::Future, "Read a book"));
}

#[test]
fn test_all_selects_every_item() {
    for line in ["Read a book", "Rent due:2026-11-19", "Trip t:2026-12-05"] {
        assert!(selects(DateFilter::All, line), "{line}");
    }
}
