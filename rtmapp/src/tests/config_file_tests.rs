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
        vcs: None,
        sync_interval_minutes: 0,
        files: vec![FileEntry {
            path: "/home/user/todo.txt".into(),
            name: Some("Home".into()),
            vcs: Some("pijul".into()),
        }],
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
    assert!(config.files.is_empty(), "parsing alone does not migrate");
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

/// Loads `text` as the config file, through the migration.
fn load_text(name: &str, text: &str) -> AppConfig {
    let path = temp_path(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    let config = AppConfig::load(&path);
    cleanup(&path);
    config
}

fn entry(path: &str, vcs: Option<&str>) -> FileEntry {
    FileEntry {
        path: path.into(),
        name: None,
        vcs: vcs.map(Into::into),
    }
}

#[test]
fn a_config_from_before_the_file_list_gets_one_entry_with_its_vcs() {
    let config = load_text(
        "migrate",
        "file_name = \"/tmp/todo.txt\"\nvcs = \"git\"\nreverse_sort = true\n",
    );
    assert_eq!(config.file_name.as_deref(), Some("/tmp/todo.txt"));
    assert_eq!(config.vcs, None);
    assert_eq!(config.files, [entry("/tmp/todo.txt", Some("git"))]);
    assert!(config.reverse_sort);
}

#[test]
fn a_file_switched_to_by_an_older_version_is_added_with_its_vcs() {
    // An older version dropped `[[files]]` when it saved; this one was edited back in by hand.
    let config = load_text(
        "older",
        "file_name = \"/tmp/b.txt\"\nvcs = \"pijul\"\n\n[[files]]\npath = \"/tmp/a.txt\"\nvcs = \"git\"\n",
    );
    assert_eq!(
        config.files,
        [
            entry("/tmp/a.txt", Some("git")),
            entry("/tmp/b.txt", Some("pijul"))
        ]
    );
}

#[test]
fn a_top_level_vcs_replaces_the_choice_of_the_active_entry() {
    let config = load_text(
        "replace",
        "file_name = \"/tmp/a.txt\"\nvcs = \"pijul\"\n\n[[files]]\npath = \"/tmp/a.txt\"\nvcs = \"git\"\n",
    );
    assert_eq!(config.files, [entry("/tmp/a.txt", Some("pijul"))]);
}

#[test]
fn a_top_level_vcs_without_a_file_is_dropped() {
    let config = load_text("no file", "vcs = \"git\"\n");
    assert_eq!(config.vcs, None);
    assert!(config.files.is_empty());
}

#[test]
fn an_entry_without_a_path_is_dropped_and_the_rest_loads() {
    let config = load_text(
        "no path",
        "file_name = \"/tmp/a.txt\"\nreverse_sort = true\n\n[[files]]\nname = \"Work\"\n",
    );
    assert!(config.reverse_sort);
    assert_eq!(config.files, [entry("/tmp/a.txt", None)]);
}

#[test]
fn an_unknown_vcs_is_no_choice_and_is_kept() {
    let path = temp_path("unknown vcs");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "file_name = \"/tmp/a.txt\"\n\n[[files]]\npath = \"/tmp/a.txt\"\nvcs = \"svn\"\n",
    )
    .unwrap();
    let config = AppConfig::load(&path);
    assert_eq!(crate::sync::chosen(&config, Path::new("/tmp/a.txt")), None);
    config.save(&path).unwrap();
    assert_eq!(
        AppConfig::load(&path).files,
        [entry("/tmp/a.txt", Some("svn"))]
    );
    cleanup(&path);
}

#[test]
fn the_choice_is_read_from_the_file_entry() {
    let mut config = load_text("chosen", "file_name = \"/tmp/a.txt\"\n");
    assert_eq!(crate::sync::chosen(&config, Path::new("/tmp/a.txt")), None);
    config.set_vcs("/tmp/a.txt", "Pijul");
    assert_eq!(
        crate::sync::chosen(&config, Path::new("/tmp/a.txt")),
        Some(rtmsync::Vcs::Pijul)
    );
    assert_eq!(crate::sync::chosen(&config, Path::new("/tmp/b.txt")), None);
}

#[test]
fn a_saved_config_has_file_name_and_the_file_list_but_no_top_level_vcs() {
    let path = temp_path("saved text");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "file_name = \"/tmp/a.txt\"\nvcs = \"git\"\n").unwrap();
    AppConfig::load(&path).save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let table: toml::Table = text.parse().unwrap();
    assert_eq!(table["file_name"].as_str(), Some("/tmp/a.txt"));
    assert!(!table.contains_key("vcs"), "{text}");
    assert!(text.contains("[[files]]"), "{text}");
    assert_eq!(table["files"][0]["vcs"].as_str(), Some("git"));
    cleanup(&path);
}

#[test]
fn set_vcs_adds_an_entry_for_a_file_without_one() {
    let mut config = AppConfig::default();
    config.set_vcs("/tmp/a.txt", "git");
    assert_eq!(config.files, [entry("/tmp/a.txt", Some("git"))]);
}

#[test]
fn opening_the_active_file_again_changes_nothing() {
    let mut config = load_text("same", "file_name = \"/tmp/a.txt\"\nvcs = \"git\"\n");
    let before = config.clone();
    config.set_active("/tmp/a.txt");
    assert_eq!(config, before);
}

#[test]
fn opening_another_file_replaces_the_entry_and_its_choice() {
    let mut config = load_text("other", "file_name = \"/tmp/a.txt\"\nvcs = \"git\"\n");
    config.set_active("/tmp/b.txt");
    assert_eq!(config.file_name.as_deref(), Some("/tmp/b.txt"));
    assert_eq!(config.files, [entry("/tmp/b.txt", None)]);
}

#[test]
fn opening_a_listed_file_keeps_its_name_and_choice() {
    let mut config = load_text(
        "listed",
        "file_name = \"/tmp/a.txt\"\n\n[[files]]\npath = \"/tmp/b.txt\"\nname = \"Work\"\nvcs = \"pijul\"\n",
    );
    config.set_active("/tmp/b.txt");
    assert_eq!(
        config.files,
        [FileEntry {
            path: "/tmp/b.txt".into(),
            name: Some("Work".into()),
            vcs: Some("pijul".into()),
        }]
    );
}

#[test]
fn opening_with_no_file_before_adds_the_entry() {
    let mut config = AppConfig::default();
    config.set_active("/tmp/a.txt");
    assert_eq!(config.file_name.as_deref(), Some("/tmp/a.txt"));
    assert_eq!(config.files, [entry("/tmp/a.txt", None)]);
}
