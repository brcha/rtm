use super::*;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 28).unwrap()
}

/// Past, today, future and undated items, done and not, with and without priority.
fn fixture() -> Vec<TodoItem> {
    [
        "Undated",                                            // 0
        "Past due:2026-10-01",                                // 1
        "Today due:2026-10-28",                               // 2
        "Future due:2026-11-15",                              // 3
        "x 2026-10-20 2026-10-19 Done past due:2026-10-02",   // 4
        "x 2026-10-20 2026-10-19 Done undated",               // 5
        "(A) Today high due:2026-10-28",                      // 6
        "x 2026-10-20 2026-10-19 Done future due:2026-12-01", // 7
        "(B) Undated with priority",                          // 8
    ]
    .iter()
    .map(|line| line.parse().unwrap())
    .collect()
}

fn config(bits: u8) -> AppConfig {
    AppConfig {
        file_name: None,
        show_completed_items: bits & 1 != 0,
        show_future_items: bits & 2 != 0,
        hide_no_date: bits & 4 != 0,
        reverse_sort: bits & 8 != 0,
        ..AppConfig::default()
    }
}

const ALL: u8 = 1 | 2;

/// The filter rule for items without a threshold date, written out independently of `visible`.
fn shown_by_the_rule(item: &TodoItem, config: &AppConfig, today: NaiveDate) -> bool {
    (config.show_future_items || item.due.is_none_or(|d| d <= today))
        && (config.show_completed_items || !item.done)
        && !(config.hide_no_date && item.due.is_none())
}

#[test]
fn filters_match_the_rule_for_every_flag_combination() {
    let items = fixture();
    for bits in 0..16 {
        let config = config(bits);
        let mut shown = visible(&items, &config, today());
        shown.sort();
        let expected: Vec<usize> = (0..items.len())
            .filter(|&i| shown_by_the_rule(&items[i], &config, today()))
            .collect();
        assert_eq!(shown, expected, "flags {bits:04b}");
    }
}

#[test]
fn default_filters_hide_done_and_future_but_keep_today() {
    assert_eq!(visible(&fixture(), &config(0), today()), [8, 0, 1, 6, 2]);
}

#[test]
fn hide_no_date_hides_undated_items() {
    assert_eq!(visible(&fixture(), &config(4), today()), [1, 6, 2]);
}

#[test]
fn sorts_undated_first_then_earliest_due_then_priority() {
    // Undated: 8 (B) first, then 0 and 5 in file order. 6 (A) comes before 2 on the same date.
    assert_eq!(
        visible(&fixture(), &config(ALL), today()),
        [8, 0, 5, 1, 4, 6, 2, 3, 7]
    );
}

#[test]
fn reverse_sort_puts_latest_due_first() {
    assert_eq!(
        visible(&fixture(), &config(ALL | 8), today()),
        [8, 0, 5, 7, 3, 6, 2, 4, 1]
    );
}

/// Items with a threshold date, around `today()`.
fn threshold_fixture() -> Vec<TodoItem> {
    [
        "Passed t:2026-10-20 due:2026-11-15", // 0
        "Today t:2026-10-28 due:2026-11-15",  // 1
        "Later t:2026-11-05",                 // 2
    ]
    .iter()
    .map(|line| line.parse().unwrap())
    .collect()
}

#[test]
fn a_passed_or_todays_threshold_shows_an_item_due_later() {
    assert_eq!(visible(&threshold_fixture(), &config(0), today()), [0, 1]);
}

#[test]
fn a_future_threshold_without_a_due_date_is_a_future_item() {
    let items = threshold_fixture();
    assert!(!visible(&items, &config(0), today()).contains(&2));
    assert_eq!(visible(&items, &config(2), today()), [2, 0, 1]);
}
