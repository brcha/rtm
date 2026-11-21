use super::*;

fn day(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

#[test]
fn test_no_age_archives_every_done_item() {
    assert_eq!(archive_cutoff(None, None, day("2026-11-21")), Ok(None));
}

#[test]
fn test_older_than_days_counts_back_from_today() {
    let today = day("2026-11-21");
    assert_eq!(
        archive_cutoff(Some(7), None, today),
        Ok(Some(day("2026-11-14")))
    );
    assert_eq!(archive_cutoff(Some(0), None, today), Ok(Some(today)));
    assert_eq!(
        archive_cutoff(Some(u32::MAX), None, today),
        Ok(Some(NaiveDate::MIN))
    );
}

#[test]
fn test_completed_before_is_a_date() {
    let today = day("2026-11-21");
    assert_eq!(
        archive_cutoff(None, Some("2026-10-01"), today),
        Ok(Some(day("2026-10-01")))
    );
    let error = archive_cutoff(None, Some("October"), today).unwrap_err();
    assert_eq!(
        error,
        "'October' is not a date in YYYY-MM-DD form. Nothing was changed."
    );
}

#[test]
fn test_both_ages_are_refused() {
    let error = archive_cutoff(Some(7), Some("2026-10-01"), day("2026-11-21")).unwrap_err();
    assert!(error.contains("not both"), "{error}");
    assert!(error.ends_with("Nothing was changed."), "{error}");
}
