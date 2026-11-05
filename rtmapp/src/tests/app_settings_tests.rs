use super::support_tests::*;
use super::*;

#[test]
fn saving_settings_writes_the_config_and_closes_the_dialog() {
    let dir = temp_dir("settings save");
    let mut app = app_with(&dir, "Alpha\n");
    app.settings = Some(SettingsForm {
        sync: false,
        interval: 30,
    });
    app.save_settings();
    assert!(app.settings.is_none());
    let saved = AppConfig::load(&dir.join("config.toml"));
    assert!(!saved.sync);
    assert_eq!(saved.sync_interval_minutes, 30);
    // `sync` changed, so sync restarts once the worker is idle.
    assert!(app.reconnect);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn saving_unchanged_sync_does_not_reconnect() {
    let dir = temp_dir("settings same sync");
    let mut app = app_with(&dir, "Alpha\n");
    app.settings = Some(SettingsForm::new(&app.config));
    app.save_settings();
    assert!(!app.reconnect);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn shortcuts_are_inactive_while_a_modal_is_open() {
    let dir = temp_dir("settings modal");
    let mut app = app_with(&dir, "Alpha\n");
    assert!(!app.modal_open());
    app.open_editor(0);
    assert!(app.modal_open());
    app.editing = None;
    app.settings = Some(SettingsForm::new(&app.config));
    assert!(app.modal_open());
    let _ = std::fs::remove_dir_all(dir);
}
