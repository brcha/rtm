use super::*;

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir()
        .join(format!("rtmapp-test-{}-{name}", std::process::id()))
        .join("rtm")
        .join("config.toml")
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_dir_all(path.parent().and_then(Path::parent).unwrap());
}

#[test]
fn round_trip() {
    let path = temp_path("round-trip");
    let config = AppConfig {
        file_name: Some("/home/user/todo.txt".into()),
        show_completed_items: true,
        show_future_items: false,
        hide_no_date: true,
        reverse_sort: true,
        sync: false,
        vcs: Some("pijul".into()),
        sync_interval_minutes: 0,
    };
    config.save(&path).unwrap();
    assert_eq!(AppConfig::load(&path), config);
    cleanup(&path);
}

#[test]
fn file_with_only_file_name() {
    let config: AppConfig = toml::from_str("file_name = \"/tmp/todo.txt\"").unwrap();
    assert_eq!(config.file_name.as_deref(), Some("/tmp/todo.txt"));
    assert!(!config.show_completed_items && !config.hide_no_date && !config.reverse_sort);
    // Files written before sync existed keep syncing on.
    assert!(config.sync);
    assert_eq!(config.vcs, None);
    assert_eq!(config.sync_interval_minutes, 10);
}

#[test]
fn unknown_keys_are_ignored() {
    let config: AppConfig = toml::from_str("future_option = 3\nreverse_sort = true").unwrap();
    assert!(config.reverse_sort);
}

#[test]
fn missing_or_invalid_file_gives_defaults() {
    let path = temp_path("invalid");
    assert_eq!(AppConfig::load(&path), AppConfig::default());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "this is = = not toml").unwrap();
    assert_eq!(AppConfig::load(&path), AppConfig::default());
    cleanup(&path);
}
