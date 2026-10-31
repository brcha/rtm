use super::*;

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, DATE_FORMAT).unwrap()
}

#[test]
fn first_weekday_follows_the_locale() {
    for (locale, day) in [
        ("en_US.UTF-8", Weekday::Sun),
        ("en-US", Weekday::Sun),
        ("sr_RS.UTF-8", Weekday::Mon),
        ("sr-Latn-RS", Weekday::Mon),
        ("de_DE@euro", Weekday::Mon),
        ("ar_EG.UTF-8", Weekday::Sat),
        ("en-US-u-fw-mon", Weekday::Mon),
        ("C", Weekday::Sun),
        ("C.UTF-8", Weekday::Sun),
        ("POSIX", Weekday::Sun),
        ("", Weekday::Sun),
        ("not a locale!", Weekday::Sun),
    ] {
        assert_eq!(first_weekday(locale), day, "{locale:?}");
    }
}

#[test]
fn grid_starts_on_the_first_weekday() {
    // 2026-11-01 is a Sunday: no leading days when the week starts on Sunday.
    let november = grid(date("2026-11-15"), Weekday::Sun);
    assert_eq!(november[0], date("2026-11-01"));
    assert_eq!(november[41], date("2026-12-12"));
    // With Monday first, the grid starts on the last Monday of October.
    assert_eq!(
        grid(date("2026-11-01"), Weekday::Mon)[0],
        date("2026-10-26")
    );
    // February 2028 (leap year) starts on a Tuesday.
    let february = grid(date("2028-02-29"), Weekday::Mon);
    assert_eq!(february[0], date("2028-01-31"));
    assert!(february.contains(&date("2028-02-29")));
}

#[test]
fn stepping_crosses_years() {
    assert_eq!(step(date("2026-01-01"), -1), date("2025-12-01"));
    assert_eq!(step(date("2026-12-01"), 1), date("2027-01-01"));
    assert_eq!(step(date("2026-03-01"), -12), date("2025-03-01"));
}
