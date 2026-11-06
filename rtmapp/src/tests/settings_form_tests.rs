use super::*;

#[test]
fn form_round_trips_the_sync_settings() {
    let config = AppConfig {
        sync: false,
        sync_interval_minutes: 25,
        ..AppConfig::default()
    };
    let form = SettingsForm::new(&config);
    assert!(!form.sync);
    assert_eq!(form.interval, 25);
    let mut written = AppConfig::default();
    form.apply(&mut written);
    assert!(!written.sync);
    assert_eq!(written.sync_interval_minutes, 25);
}

#[test]
fn apply_leaves_other_keys_alone() {
    let mut config = AppConfig {
        file_name: Some("/tmp/todo.txt".to_owned()),
        show_completed_items: true,
        reverse_sort: true,
        ..AppConfig::default()
    };
    let before = config.clone();
    let form = SettingsForm {
        sync: false,
        interval: 0,
        check_for_updates: true,
        beta: false,
    };
    form.apply(&mut config);
    assert_eq!(
        config,
        AppConfig {
            sync: false,
            sync_interval_minutes: 0,
            ..before
        }
    );
}

#[test]
fn form_round_trips_the_update_settings() {
    let config = AppConfig {
        check_for_updates: false,
        update_channel: "beta".to_owned(),
        ..AppConfig::default()
    };
    let form = SettingsForm::new(&config);
    assert!(!form.check_for_updates);
    assert!(form.beta);
    let mut written = AppConfig::default();
    form.apply(&mut written);
    assert!(!written.check_for_updates);
    assert_eq!(written.update_channel, "beta");
}

#[test]
fn an_unknown_channel_is_saved_as_stable() {
    let config = AppConfig {
        update_channel: "nightly".to_owned(),
        ..AppConfig::default()
    };
    let mut written = config.clone();
    SettingsForm::new(&config).apply(&mut written);
    assert_eq!(written.update_channel, "stable");
}
