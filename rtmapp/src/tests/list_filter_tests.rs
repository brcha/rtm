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

/// The Tauri app's filter, copied verbatim from `get_items` in `rtmapp/src-tauri/src/lib.rs` at
/// 5076649 (removed in RTM-39).
fn tauri_app_shows(item: &TodoItem, config: &AppConfig, today: NaiveDate) -> bool {
    !(item.due.map_or(false, |d| d > today) && !config.show_future_items)
        && !(item.done && !config.show_completed_items)
        && !(config.hide_no_date && item.due.is_none())
}

#[test]
fn filters_match_the_tauri_app_for_every_flag_combination() {
    let items = fixture();
    for bits in 0..16 {
        let config = config(bits);
        let mut shown = visible(&items, &config, today());
        shown.sort();
        let expected: Vec<usize> = (0..items.len())
            .filter(|&i| tauri_app_shows(&items[i], &config, today()))
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
