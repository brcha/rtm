use super::*;

fn config_with(file: &str, rule: Option<(bool, u64)>) -> AppConfig {
    let mut config = AppConfig {
        auto_archive: true,
        archive_older_than_days: 7,
        ..AppConfig::default()
    };
    config.add_file(file);
    config.set_archive(file, rule);
    config
}

#[test]
fn a_file_without_its_own_settings_shows_the_app_wide_ones() {
    let config = config_with("/tmp/a.txt", None);
    let form = ArchiveForm::new(
        &config,
        Path::new("/tmp/a.txt"),
        "a",
        "/tmp/a.done.txt".into(),
    );
    assert!(form.use_app_wide);
    assert!(form.auto_archive);
    assert_eq!(form.days, 7);
}

#[test]
fn a_file_with_its_own_settings_shows_them() {
    let config = config_with("/tmp/a.txt", Some((false, 2)));
    let form = ArchiveForm::new(
        &config,
        Path::new("/tmp/a.txt"),
        "a",
        "/tmp/a.done.txt".into(),
    );
    assert!(!form.use_app_wide);
    assert!(!form.auto_archive);
    assert_eq!(form.days, 2);
}

#[test]
fn apply_stores_or_clears_the_override() {
    let mut config = config_with("/tmp/a.txt", None);
    let mut form = ArchiveForm::new(
        &config,
        Path::new("/tmp/a.txt"),
        "a",
        "/tmp/a.done.txt".into(),
    );
    form.use_app_wide = false;
    form.auto_archive = false;
    form.days = 3;
    form.apply(&mut config);
    assert_eq!(config.archive_rule(Path::new("/tmp/a.txt")), (false, 3));

    form.use_app_wide = true;
    form.apply(&mut config);
    let entry = config.entry(Path::new("/tmp/a.txt")).unwrap();
    assert_eq!(
        (entry.auto_archive, entry.archive_older_than_days),
        (None, None)
    );
    assert_eq!(config.archive_rule(Path::new("/tmp/a.txt")), (true, 7));
}
