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
        check_for_updates: false,
        update_channel: "beta".into(),
        projects_collapsed: true,
        contexts_collapsed: true,
        auto_archive: true,
        archive_older_than_days: 7,
        files: vec![FileEntry {
            path: "/home/user/todo.txt".into(),
            name: Some("Home".into()),
            vcs: Some("pijul".into()),
            auto_archive: Some(false),
            archive_older_than_days: Some(14),
        }],
    };
    config.save(&path).unwrap();
    assert_eq!(AppConfig::load(&path).unwrap(), config);
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
    // Files written before the update check existed check, on the stable channel.
    assert!(config.check_for_updates);
    assert_eq!(config.update_channel, "stable");
    assert!(!config.beta());
    assert!(config.files.is_empty(), "parsing alone does not migrate");
}

#[test]
fn only_beta_is_the_beta_channel() {
    for (channel, beta) in [
        ("beta", true),
        ("stable", false),
        ("Beta", false),
        ("nightly", false),
    ] {
        let config = AppConfig {
            update_channel: channel.into(),
            ..AppConfig::default()
        };
        assert_eq!(config.beta(), beta, "{channel}");
    }
}

#[test]
fn sidebar_sections_are_open_without_their_keys() {
    let config: AppConfig = toml::from_str("reverse_sort = true").unwrap();
    assert!(!config.projects_collapsed);
    assert!(!config.contexts_collapsed);
}

#[test]
fn archiving_is_off_with_30_days_without_its_keys() {
    let config: AppConfig = toml::from_str("reverse_sort = true").unwrap();
    assert!(!config.auto_archive);
    assert_eq!(config.archive_older_than_days, 30);
}

#[test]
fn unknown_keys_are_ignored() {
    let config: AppConfig = toml::from_str("future_option = 3\nreverse_sort = true").unwrap();
    assert!(config.reverse_sort);
}

#[test]
fn a_missing_file_gives_defaults() {
    let path = temp_path("missing");
    assert_eq!(AppConfig::load(&path).unwrap(), AppConfig::default());
}

#[test]
fn a_file_that_does_not_parse_is_an_error_naming_it() {
    let path = temp_path("invalid");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    for text in [
        "[[files]]\npath = \"/home/user/todo.txt\"\nthis is = = not toml\n",
        "sync_interval_minutes = -5\n",
    ] {
        std::fs::write(&path, text).unwrap();
        let error = AppConfig::load(&path).unwrap_err();
        assert!(error.starts_with(&path.display().to_string()), "{error}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }
    cleanup(&path);
}

#[test]
fn a_config_that_cannot_be_read_is_an_error() {
    let path = temp_path("unreadable");
    // Reading a directory fails on every system, without changing permissions.
    std::fs::create_dir_all(&path).unwrap();
    let error = AppConfig::load(&path).unwrap_err();
    assert!(error.starts_with(&path.display().to_string()), "{error}");
    cleanup(&path);
}

/// Loads `text` as the config file, through the migration.
fn load_text(name: &str, text: &str) -> AppConfig {
    let path = temp_path(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    let config = AppConfig::load(&path).unwrap();
    cleanup(&path);
    config
}

fn entry(path: &str, vcs: Option<&str>) -> FileEntry {
    FileEntry {
        path: path.into(),
        name: None,
        vcs: vcs.map(Into::into),
        ..FileEntry::default()
    }
}

#[cfg(windows)]
#[test]
fn verbatim_paths_saved_by_an_earlier_version_load_plain_and_once() {
    let config = load_text(
        "verbatim",
        r#"file_name = '\\?\C:\todo\a.txt'

[[files]]
path = '\\?\C:\todo\a.txt'
name = "Home"
vcs = "git"

[[files]]
path = 'C:\todo\a.txt'
vcs = "pijul"

[[files]]
path = '\\?\UNC\server\share\b.txt'
"#,
    );
    assert_eq!(config.file_name.as_deref(), Some(r"C:\todo\a.txt"));
    assert_eq!(
        config.files,
        [
            FileEntry {
                name: Some("Home".into()),
                ..entry(r"C:\todo\a.txt", Some("git"))
            },
            entry(r"\\server\share\b.txt", None),
        ]
    );
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
    let config = AppConfig::load(&path).unwrap();
    assert_eq!(crate::sync::chosen(&config, Path::new("/tmp/a.txt")), None);
    config.save(&path).unwrap();
    assert_eq!(
        AppConfig::load(&path).unwrap().files,
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
    AppConfig::load(&path).unwrap().save(&path).unwrap();
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
fn add_file_appends_an_entry_once() {
    let mut config = load_text("add", "file_name = \"/tmp/a.txt\"\nvcs = \"git\"\n");
    config.add_file("/tmp/b.txt");
    config.add_file("/tmp/a.txt");
    config.add_file("/tmp/b.txt");
    assert_eq!(
        config.files,
        [entry("/tmp/a.txt", Some("git")), entry("/tmp/b.txt", None)]
    );
    assert_eq!(config.file_name.as_deref(), Some("/tmp/a.txt"));
}

#[test]
fn set_active_only_names_the_file() {
    let mut config = load_text("active", "file_name = \"/tmp/a.txt\"\nvcs = \"git\"\n");
    config.add_file("/tmp/b.txt");
    config.set_active("/tmp/b.txt");
    assert_eq!(config.file_name.as_deref(), Some("/tmp/b.txt"));
    assert_eq!(
        config.files,
        [entry("/tmp/a.txt", Some("git")), entry("/tmp/b.txt", None)]
    );
}

#[test]
fn removing_a_file_drops_its_entry_and_clears_file_name_when_it_was_active() {
    let mut config = load_text("remove", "file_name = \"/tmp/a.txt\"\n");
    config.add_file("/tmp/b.txt");
    config.remove_file("/tmp/b.txt");
    assert_eq!(config.files, [entry("/tmp/a.txt", None)]);
    assert_eq!(config.file_name.as_deref(), Some("/tmp/a.txt"));
    config.remove_file("/tmp/a.txt");
    assert_eq!(config.files, []);
    assert_eq!(config.file_name, None);
}

#[test]
fn renaming_an_entry_keeps_its_name_and_choice() {
    let mut config = load_text(
        "rename",
        "file_name = \"/tmp/a.txt\"\n\n[[files]]\npath = \"/tmp/b.txt\"\nname = \"Work\"\nvcs = \"pijul\"\n",
    );
    config.rename("/tmp/b.txt", "/real/b.txt");
    assert_eq!(config.file_name.as_deref(), Some("/tmp/a.txt"));
    assert_eq!(
        config.files,
        [
            FileEntry {
                path: "/real/b.txt".into(),
                name: Some("Work".into()),
                vcs: Some("pijul".into()),
                ..FileEntry::default()
            },
            entry("/tmp/a.txt", None),
        ]
    );
}

#[test]
fn renaming_the_active_entry_moves_file_name() {
    let mut config = load_text(
        "rename active",
        "file_name = \"/tmp/a.txt\"\nvcs = \"git\"\n",
    );
    config.rename("/tmp/a.txt", "/real/a.txt");
    assert_eq!(config.file_name.as_deref(), Some("/real/a.txt"));
    assert_eq!(config.files, [entry("/real/a.txt", Some("git"))]);
}

#[test]
fn renaming_onto_a_listed_spelling_keeps_one_entry_with_the_newer_choice() {
    let mut config = load_text(
        "rename duplicate",
        "file_name = \"/tmp/a.txt\"\nvcs = \"git\"\n\n[[files]]\npath = \"/real/a.txt\"\nvcs = \"pijul\"\n",
    );
    config.rename("/tmp/a.txt", "/real/a.txt");
    assert_eq!(config.file_name.as_deref(), Some("/real/a.txt"));
    assert_eq!(config.files, [entry("/real/a.txt", Some("git"))]);
}

#[test]
fn set_name_sets_and_clears_the_tab_label_of_a_listed_file_only() {
    let mut config = AppConfig::default();
    config.add_file("/tmp/a.txt");

    config.set_name("/tmp/a.txt", Some("Work"));
    assert_eq!(config.files[0].name.as_deref(), Some("Work"));
    config.set_name("/tmp/a.txt", None);
    assert_eq!(config.files[0].name, None);
    config.set_name("/tmp/b.txt", Some("Home"));
    assert_eq!(config.files, [entry("/tmp/a.txt", None)]);
}

#[test]
fn a_file_without_archive_settings_writes_none_and_follows_the_app() {
    let path = temp_path("archive override");
    let mut config = AppConfig {
        auto_archive: true,
        archive_older_than_days: 7,
        ..AppConfig::default()
    };
    config.add_file("/tmp/a.txt");
    config.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.matches("auto_archive").count(), 1, "{text}");
    assert_eq!(config.archive_rule(Path::new("/tmp/a.txt")), (true, 7));
    cleanup(&path);
}

#[test]
fn a_file_override_replaces_the_app_wide_rule_until_cleared() {
    let mut config = AppConfig::default();
    config.add_file("/tmp/a.txt");
    config.set_archive("/tmp/a.txt", Some((true, 3)));
    assert_eq!(config.archive_rule(Path::new("/tmp/a.txt")), (true, 3));
    assert_eq!(config.archive_rule(Path::new("/tmp/b.txt")), (false, 30));
    config.set_archive("/tmp/a.txt", None);
    assert_eq!(config.archive_rule(Path::new("/tmp/a.txt")), (false, 30));
    assert_eq!(
        config.entry(Path::new("/tmp/a.txt")).unwrap().auto_archive,
        None
    );
}

#[test]
fn each_archive_key_falls_back_on_its_own() {
    let config = load_text(
        "archive fallback",
        "auto_archive = true\narchive_older_than_days = 9\n\n[[files]]\npath = \"/tmp/a.txt\"\n\
         archive_older_than_days = 2\n",
    );
    assert_eq!(config.archive_rule(Path::new("/tmp/a.txt")), (true, 2));
}
