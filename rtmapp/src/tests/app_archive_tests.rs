use super::support_tests::*;
use super::*;

fn days_ago(days: u64) -> String {
    let date = Local::now().date_naive() - Days::new(days);
    date.format("%Y-%m-%d").to_string()
}

fn done_lines(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("done.txt"))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn todo_path(app: &App) -> PathBuf {
    app.active.clone().unwrap()
}

#[test]
fn archivable_agrees_with_the_library() {
    let dir = temp_dir("archive predicate");
    let cutoff = Local::now().date_naive() - Days::new(7);
    let lines = [
        "Open".to_owned(),
        format!("x {} Old", days_ago(30)),
        format!("x {} Recent", days_ago(1)),
        format!("x {} On the cutoff", days_ago(7)),
        "x Undated".to_owned(),
        format!("x {} {} With both dates", days_ago(8), days_ago(40)),
    ];
    let file = dir.join("todo.txt");
    std::fs::write(&file, lines.join("\n")).unwrap();
    let mut library = load_library(&file).unwrap();
    let expected: Vec<TodoItem> = library
        .items
        .iter()
        .filter(|item| archivable(item, cutoff))
        .cloned()
        .collect();
    assert_eq!(library.archive(Some(cutoff)).unwrap(), expected);
    assert_eq!(expected.len(), 3);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_commit_message_names_one_item_or_counts_them() {
    let item = |line: &str| line.parse::<TodoItem>().unwrap();
    let cutoff = Local::now().date_naive();
    let one = Change::Archive {
        cutoff,
        archived: vec![item("x 2026-01-02 Pay rent +Home")],
    };
    assert_eq!(one.describe(), "rtm: archive Pay rent");
    let two = Change::Archive {
        cutoff,
        archived: vec![item("x Pay rent"), item("x Call mom")],
    };
    assert_eq!(two.describe(), "rtm: archive 2 items");
}

#[test]
fn archiving_moves_only_items_completed_before_the_age() {
    let dir = temp_dir("archive age");
    let (old, recent) = (days_ago(30), days_ago(1));
    let mut app = app_with(
        &dir,
        &format!("Open\nx {old} Old\nx {recent} Recent\nx Undated\n"),
    );
    app.config.archive_older_than_days = 7;

    app.archive(&todo_path(&app), Instant::now());

    assert_eq!(file_lines(&dir), ["Open", &format!("x {recent} Recent")]);
    assert_eq!(
        done_lines(&dir),
        [format!("x {old} Old"), "x Undated".into()]
    );
    let library = app.library().unwrap();
    assert_eq!(library.items.len(), 2);
    assert!(app.file().unwrap().pending.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn nothing_to_archive_writes_nothing() {
    let dir = temp_dir("archive nothing");
    let contents = format!("Open\nx {} Done today\n", days_ago(0));
    let mut app = app_with(&dir, &contents);
    app.config.archive_older_than_days = 0;

    app.archive(&todo_path(&app), Instant::now());

    assert_eq!(
        std::fs::read_to_string(dir.join("todo.txt")).unwrap(),
        contents
    );
    assert!(!dir.join("done.txt").exists());
    assert!(app.file().unwrap().archived_at.is_some());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn automatic_archiving_runs_on_open_and_then_once_a_day() {
    let dir = temp_dir("archive daily");
    let old = days_ago(60);
    let mut app = app_with(&dir, &format!("Open\nx {old} First\n"));
    app.config.auto_archive = true;
    let start = Instant::now();

    app.archive_due(start);
    assert_eq!(file_lines(&dir), ["Open"]);
    assert_eq!(done_lines(&dir), [format!("x {old} First")]);

    // Another program completes an item; the app reads it as it would.
    std::fs::write(dir.join("todo.txt"), format!("Open\nx {old} Second\n")).unwrap();
    app.files[0].reload(None);
    app.archive_due(start + Duration::from_secs(60 * 60));
    assert_eq!(file_lines(&dir), ["Open", &format!("x {old} Second")]);

    app.archive_due(start + ARCHIVE_INTERVAL);
    assert_eq!(file_lines(&dir), ["Open"]);
    assert_eq!(
        done_lines(&dir),
        [format!("x {old} First"), format!("x {old} Second")]
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn automatic_archiving_is_off_by_default() {
    let dir = temp_dir("archive off");
    let contents = format!("x {} Old\n", days_ago(60));
    let mut app = app_with(&dir, &contents);

    app.archive_due(Instant::now());

    assert_eq!(
        std::fs::read_to_string(dir.join("todo.txt")).unwrap(),
        contents
    );
    assert!(app.file().unwrap().archived_at.is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn only_a_new_archive_rule_in_settings_archives_again() {
    let dir = temp_dir("archive settings");
    let mut app = app_with(&dir, "Open\n");
    app.files[0].archived_at = Some(Instant::now());

    let mut form = SettingsForm::new(&app.config);
    form.interval = 25;
    app.settings = Some(form);
    app.save_settings();
    assert!(app.files[0].archived_at.is_some());

    let mut form = SettingsForm::new(&app.config);
    form.archive_days = 7;
    app.settings = Some(form);
    app.save_settings();
    assert!(app.files[0].archived_at.is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_done_txt_open_in_another_tab_shows_the_archived_items() {
    let dir = temp_dir("archive done tab");
    std::fs::write(dir.join("done.txt"), "x 2025-12-01 Older\n").unwrap();
    let mut app = app_with(&dir, "Open\nx 2026-01-02 Pay rent\n");
    let done = load_library(&dir.join("done.txt")).unwrap();
    app.files.push(OpenFile::loaded(done, None));

    app.archive(&todo_path(&app), Instant::now());
    app.reload_changed();

    let lines: Vec<_> = app.files[1]
        .library
        .as_ref()
        .unwrap()
        .items
        .iter()
        .map(|item| item.description.clone())
        .collect();
    assert_eq!(lines, ["Older", "Pay rent"]);
    let _ = std::fs::remove_dir_all(dir);
}
