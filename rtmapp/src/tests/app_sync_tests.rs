use super::support_tests::*;
use super::*;

#[test]
fn a_commit_message_names_only_the_description() {
    let item: TodoItem = "(A) 2026-10-30 Pay bill +Home @online due:2026-11-01 rec:w"
        .parse()
        .unwrap();
    assert_eq!(Change::Toggle(item).describe(), "rtm: complete Pay bill");
}

// Sync (RTM-64, RTM-20): scratch git remotes and clones on local paths, no network.

fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// Local config that keeps the user's global identity, signing, hooks and rerere out.
fn isolate(repo: &Path, scratch: &Path) {
    let hooks = scratch.join("no hooks");
    git(repo, &["config", "user.name", "rtm test"]);
    git(repo, &["config", "user.email", "rtm@example.com"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
    git(repo, &["config", "core.hooksPath", hooks.to_str().unwrap()]);
    git(repo, &["config", "rerere.enabled", "false"]);
}

/// A bare `remote` and clones `a` and `b` sharing a tracked `todo.txt` holding `lines`.
fn remote_and_clones(name: &str, lines: &str) -> PathBuf {
    let dir = temp_dir(name);
    std::fs::create_dir_all(dir.join("no hooks")).unwrap();
    git(&dir, &["init", "--quiet", "--bare", "remote"]);
    isolate(&dir.join("remote"), &dir);
    let hooks = format!("core.hooksPath={}", dir.join("no hooks").to_str().unwrap());
    for clone in ["a", "b"] {
        git(&dir, &["clone", "--quiet", "-c", &hooks, "remote", clone]);
        isolate(&dir.join(clone), &dir);
        if clone == "a" {
            std::fs::write(dir.join("a/todo.txt"), lines).unwrap();
            git(&dir.join("a"), &["add", "todo.txt"]);
            git(&dir.join("a"), &["commit", "--quiet", "-m", "start"]);
            git(&dir.join("a"), &["push", "--quiet", "-u", "origin", "HEAD"]);
        }
    }
    git(&dir.join("b"), &["pull", "--quiet"]);
    dir
}

fn push_from_b(dir: &Path, lines: &str) {
    std::fs::write(dir.join("b/todo.txt"), lines).unwrap();
    git(&dir.join("b"), &["commit", "--quiet", "-am", "from b"]);
    git(&dir.join("b"), &["push", "--quiet"]);
}

/// Lets the worker finish its jobs and applies their results, as the frames would.
fn wait(app: &mut App) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        app.sync_results();
        if !app.busy() {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the worker never finished"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// An app on `file`, connected as at startup, with its config in `dir`.
fn synced_app(dir: &Path, file: &Path) -> App {
    let mut app = bare_app(dir, Some(load_library(file).unwrap()));
    app.connect();
    wait(&mut app);
    app
}

/// The worker of the shown file's repository.
fn worker(app: &App) -> Option<&Worker> {
    app.repo_of(app.file()?).map(|repo| &repo.worker)
}

/// The sync line of the shown file's repository.
fn status(app: &App) -> Option<&str> {
    app.repo_of(app.file()?)
        .and_then(|repo| repo.status.as_deref())
}

/// Queues a sync, as the timer or Sync now does.
fn sync(app: &mut App) {
    app.sync_now(Instant::now());
}

fn subjects(repo: &Path, count: usize) -> String {
    git(repo, &["log", &format!("-{count}"), "--format=%s"])
}

fn descriptions(app: &App) -> Vec<String> {
    let library = app.file().unwrap().library.as_ref().unwrap();
    library
        .items
        .iter()
        .map(|item| item.description.clone())
        .collect()
}

#[test]
fn opening_syncs_and_a_change_is_committed_at_once_and_pushed_by_the_next_sync() {
    let dir = remote_and_clones("sync push", "Alpha\n");
    push_from_b(&dir, "Alpha\nFrom b\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    assert_eq!(descriptions(&app), ["Alpha", "From b"]);

    app.new_item = "Call mom".to_owned();
    app.add_item();
    // Saved in the same frame, committed by the worker, not pushed.
    assert!(
        file_lines(&dir.join("a"))
            .iter()
            .any(|l| l.ends_with("Call mom"))
    );
    wait(&mut app);
    assert_eq!(subjects(&dir.join("a"), 1), "rtm: add Call mom");
    assert_eq!(subjects(&dir.join("remote"), 1), "from b");

    sync(&mut app);
    wait(&mut app);
    assert_eq!(subjects(&dir.join("remote"), 1), "rtm: add Call mom");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_change_made_while_a_job_runs_shows_at_once_and_is_written_after_it() {
    let dir = remote_and_clones("sync busy", "Alpha\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    sync(&mut app);
    // Outstanding until its result is picked up, however fast the job itself is.
    app.toggle(0);
    app.new_item = "Call mom".to_owned();
    app.add_item();
    assert_eq!(descriptions(&app), ["Alpha", "Call mom"]);
    assert!(app.file().unwrap().library.as_ref().unwrap().items[0].done);
    assert_eq!(
        std::fs::read_to_string(dir.join("a/todo.txt")).unwrap(),
        "Alpha\n"
    );
    wait(&mut app);
    let lines = file_lines(&dir.join("a"));
    assert!(
        lines[0].starts_with("x ") && lines[1].ends_with("Call mom"),
        "{lines:?}"
    );
    assert_eq!(
        git(&dir.join("a"), &["log", "-1", "--format=%B"]),
        "rtm: 2 changes\n\nrtm: complete Alpha\nrtm: add Call mom"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_pending_change_is_redone_on_what_the_pull_brought() {
    let dir = remote_and_clones("sync replay", "Alpha\nBravo\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    push_from_b(&dir, "First\nAlpha\nBravo\n");
    sync(&mut app);
    app.toggle(1);
    wait(&mut app);
    assert_eq!(descriptions(&app), ["First", "Alpha", "Bravo"]);
    let lines = file_lines(&dir.join("a"));
    assert!(
        lines[2].starts_with("x ") && lines[2].ends_with("Bravo"),
        "{lines:?}"
    );
    assert_eq!(subjects(&dir.join("a"), 1), "rtm: complete Bravo");
    assert_eq!(errors(&app), Vec::<&str>::new());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_pending_change_whose_item_the_pull_changed_is_dropped_with_a_note() {
    let dir = remote_and_clones("sync dropped", "Alpha\nBravo\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    push_from_b(&dir, "Alpha\nBravo, renamed\n");
    sync(&mut app);
    app.toggle(1);
    wait(&mut app);
    assert_eq!(descriptions(&app), ["Alpha", "Bravo, renamed"]);
    assert!(!app.file().unwrap().library.as_ref().unwrap().items[1].done);
    assert!(
        app.file()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("rtm: complete Bravo")
    );
    assert_eq!(subjects(&dir.join("a"), 1), "from b");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_pull_whose_push_fails_is_still_taken_in() {
    let dir = remote_and_clones("sync push fails", "Alpha\nBravo\nCharlie\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    let missing = dir.join("missing");
    git(
        &dir.join("a"),
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            missing.to_str().unwrap(),
        ],
    );
    push_from_b(&dir, "From b\nAlpha\nBravo\nCharlie\n");
    app.new_item = "Call mom".to_owned();
    app.add_item();
    wait(&mut app);
    sync(&mut app);
    wait(&mut app);
    assert!(
        status(&app).is_some_and(|s| s.starts_with("Not synced")),
        "{:?}",
        status(&app)
    );
    assert_eq!(
        descriptions(&app),
        ["From b", "Alpha", "Bravo", "Charlie", "Call mom"]
    );

    app.new_item = "Again".to_owned();
    app.add_item();
    wait(&mut app);
    assert_eq!(file_lines(&dir.join("a"))[0], "From b");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_edit_is_refused_when_a_pull_changed_its_item() {
    let dir = remote_and_clones("sync stale", "Alpha\nBravo\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    open_editor(&mut app, 1).description = "Bravo, edited here".to_owned();

    push_from_b(&dir, "Alpha\nBravo, edited there\n");
    sync(&mut app);
    wait(&mut app);
    app.save_edit();

    let form = app.editing.as_ref().expect("the dialog stays open");
    assert!(form.error.as_deref().unwrap().contains("changed"));
    assert_eq!(descriptions(&app), ["Alpha", "Bravo, edited there"]);
    assert_eq!(subjects(&dir.join("a"), 1), "from b");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_edit_follows_its_item_when_a_pull_renumbers() {
    let dir = remote_and_clones("sync renumber", "Alpha\nBravo\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    open_editor(&mut app, 1).description = "Bravo, edited".to_owned();

    push_from_b(&dir, "First\nAlpha\nBravo\n");
    sync(&mut app);
    wait(&mut app);
    app.save_edit();
    wait(&mut app);

    assert!(app.editing.is_none());
    assert_eq!(descriptions(&app), ["First", "Alpha", "Bravo, edited"]);
    assert_eq!(subjects(&dir.join("a"), 1), "rtm: edit Bravo");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_picked_file_opens_while_a_sync_runs_and_the_first_still_commits() {
    let dir = remote_and_clones("sync dialog", "Alpha\n");
    std::fs::write(dir.join("other.txt"), "Other item\n").unwrap();
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    sync(&mut app);
    app.new_item = "Call mom".to_owned();
    app.add_item();

    dialog_returned(&mut app, Some(dir.join("other.txt")), App::open_file);
    app.dialog_result();
    assert!(app.dialog.is_none());
    assert_eq!(descriptions(&app), ["Other item"]);

    wait(&mut app);
    assert!(file_lines(&dir.join("a"))[1].ends_with(" Call mom"));
    assert_eq!(subjects(&dir.join("a"), 1), "rtm: add Call mom");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn exit_commits_and_pushes() {
    let dir = remote_and_clones("sync exit", "Alpha\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    app.new_item = "Call mom".to_owned();
    app.add_item();
    eframe::App::on_exit(&mut app);
    assert!(!app.busy());
    assert_eq!(subjects(&dir.join("remote"), 1), "rtm: add Call mom");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn exit_during_a_pull_redoes_pending_changes_on_it() {
    let dir = remote_and_clones("sync exit pull", "Alpha\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    push_from_b(&dir, "Alpha\nFrom b\n");
    sync(&mut app);
    app.new_item = "Call mom".to_owned();
    app.add_item();
    eframe::App::on_exit(&mut app);
    assert!(!app.busy());
    let lines = file_lines(&dir.join("a"));
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(lines[1], "From b");
    assert_eq!(
        subjects(&dir.join("remote"), 2),
        "rtm: add Call mom\nfrom b"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_conflict_found_by_a_sync_closes_the_file() {
    let dir = remote_and_clones("sync job conflict", "Alpha\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    // b changes the line before the app has pulled; the app then changes it too.
    push_from_b(&dir, "Alpha from b\n");
    open_editor(&mut app, 0).description = "Alpha from a".to_owned();
    app.save_edit();
    wait(&mut app);
    sync(&mut app);
    app.new_item = "Lost".to_owned();
    app.add_item();
    wait(&mut app);

    assert!(app.file().unwrap().library.is_none());
    let error = app.file().unwrap().error.as_deref().unwrap();
    assert!(error.contains("resolve them by hand"), "{error}");
    assert!(error.contains("One change"), "{error}");
    assert!(worker(&app).is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_sync_is_due_once_the_interval_has_passed_and_never_with_0() {
    let dir = remote_and_clones("sync timer", "Alpha\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    let opened = app.repos.first().unwrap().last_sync;
    let minutes = |n: u64| Duration::from_secs(n * 60);

    app.tick(opened + minutes(9));
    assert!(!worker(&app).unwrap().syncing());
    app.tick(opened + minutes(10));
    assert!(worker(&app).unwrap().syncing());
    assert_eq!(app.repos.first().unwrap().last_sync, opened + minutes(10));
    wait(&mut app);

    app.config.sync_interval_minutes = 0;
    app.tick(opened + minutes(60 * 24));
    assert!(!worker(&app).unwrap().syncing());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sync_off_neither_commits_nor_pulls() {
    let dir = remote_and_clones("sync off", "Alpha\n");
    let mut app = bare_app(&dir, Some(load_library(&dir.join("a/todo.txt")).unwrap()));
    app.config.sync = false;
    app.connect();
    assert!(worker(&app).is_none());
    app.new_item = "Call mom".to_owned();
    app.add_item();
    assert_eq!(git(&dir.join("a"), &["log", "-1", "--format=%s"]), "start");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn outside_changes_are_committed_and_pushed_when_the_user_says_yes() {
    let dir = remote_and_clones("ask modified yes", "Alpha\n");
    std::fs::write(dir.join("a/todo.txt"), "Alpha\nBy hand\n").unwrap();
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    // Nothing is committed before the answer.
    assert_eq!(subjects(&dir.join("a"), 1), "start");
    assert!(matches!(
        app.file().unwrap().asking,
        Some((_, Question::Modified))
    ));

    app.answer(true);
    wait(&mut app);
    assert!(app.file().unwrap().asking.is_none());
    assert_eq!(subjects(&dir.join("a"), 1), "rtm: commit outside changes");
    assert_eq!(
        subjects(&dir.join("remote"), 1),
        "rtm: commit outside changes"
    );
    assert_eq!(descriptions(&app), ["Alpha", "By hand"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn outside_changes_close_the_file_when_the_user_says_no() {
    let dir = remote_and_clones("ask modified no", "Alpha\n");
    std::fs::write(dir.join("a/todo.txt"), "Alpha\nBy hand\n").unwrap();
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    app.answer(false);
    assert!(app.file().unwrap().library.is_none() && worker(&app).is_none());
    assert!(
        app.file()
            .unwrap()
            .error
            .as_deref()
            .is_some_and(|e| e.contains("commit or revert them by hand"))
    );
    assert_eq!(subjects(&dir.join("a"), 1), "start");
    assert_eq!(
        std::fs::read_to_string(dir.join("a/todo.txt")).unwrap(),
        "Alpha\nBy hand\n"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_untracked_file_is_added_and_pushed_when_the_user_says_yes() {
    let dir = remote_and_clones("ask untracked yes", "Alpha\n");
    std::fs::write(dir.join("a/other.txt"), "Other\n").unwrap();
    let mut app = synced_app(&dir, &dir.join("a/other.txt"));
    assert!(matches!(
        app.file().unwrap().asking,
        Some((_, Question::Untracked))
    ));

    app.answer(true);
    wait(&mut app);
    assert_eq!(subjects(&dir.join("a"), 1), "rtm: track other.txt");
    assert_eq!(subjects(&dir.join("remote"), 1), "rtm: track other.txt");
    assert!(worker(&app).is_some());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_untracked_file_opens_without_sync_when_the_user_says_no() {
    let dir = remote_and_clones("ask untracked no", "Alpha\n");
    std::fs::write(dir.join("a/other.txt"), "Other\n").unwrap();
    let mut app = synced_app(&dir, &dir.join("a/other.txt"));
    app.answer(false);
    assert!(
        app.file().unwrap().library.is_some()
            && worker(&app).is_none()
            && app.file().unwrap().asking.is_none()
    );
    app.new_item = "Call mom".to_owned();
    app.add_item();
    assert_eq!(subjects(&dir.join("a"), 1), "start");

    // Asked again the next time the file opens.
    app.connect();
    wait(&mut app);
    assert!(matches!(
        app.file().unwrap().asking,
        Some((_, Question::Untracked))
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_ignored_file_opens_without_sync_or_a_question() {
    let dir = remote_and_clones("ask ignored", "Alpha\n");
    std::fs::write(dir.join("a/.gitignore"), "other.txt\n").unwrap();
    std::fs::write(dir.join("a/other.txt"), "Other\n").unwrap();
    let app = synced_app(&dir, &dir.join("a/other.txt"));
    assert!(
        app.file().unwrap().library.is_some()
            && worker(&app).is_none()
            && app.file().unwrap().asking.is_none()
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_untracked_file_in_a_conflicted_repository_opens_without_sync_or_a_question() {
    let dir = remote_and_clones("ask untracked conflict", "Alpha\n");
    push_from_b(&dir, "Alpha from b\n");
    std::fs::write(dir.join("a/todo.txt"), "Alpha from a\n").unwrap();
    git(&dir.join("a"), &["commit", "--quiet", "-am", "from a"]);
    let pull = std::process::Command::new("git")
        .current_dir(dir.join("a"))
        .args(["pull", "--quiet", "--rebase"])
        .output()
        .unwrap();
    assert!(!pull.status.success());
    std::fs::write(dir.join("a/other.txt"), "Other\n").unwrap();

    let app = synced_app(&dir, &dir.join("a/other.txt"));
    assert!(
        app.file().unwrap().library.is_some()
            && worker(&app).is_none()
            && app.file().unwrap().asking.is_none()
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_new_file_in_a_repository_is_asked_about_and_added() {
    let dir = remote_and_clones("new file untracked", "Alpha\n");
    let mut app = bare_app(&dir, None);

    app.create_file(&dir.join("a/new.txt"));
    wait(&mut app);

    assert!(app.file().unwrap().library.is_some());
    assert_eq!(subjects(&dir.join("a"), 1), "start");
    assert!(matches!(
        app.file().unwrap().asking,
        Some((_, Question::Untracked))
    ));
    app.answer(true);
    wait(&mut app);
    assert_eq!(subjects(&dir.join("a"), 1), "rtm: track new.txt");
    assert_eq!(status(&app), None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_missing_tracked_file_is_never_asked_about() {
    let dir = remote_and_clones("ask missing", "Alpha\n");
    std::fs::remove_file(dir.join("a/todo.txt")).unwrap();
    let file = dir.join("a/todo.txt");
    let Place::Repo(key) = sync::place(&file, &AppConfig::default()) else {
        panic!("the file is in a repository");
    };
    assert!(matches!(
        sync::check(key.vcs, key.root, &file),
        Found::Repo(_)
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sync_off_asks_nothing_about_outside_changes() {
    let dir = remote_and_clones("ask sync off", "Alpha\n");
    std::fs::write(dir.join("a/todo.txt"), "Alpha\nBy hand\n").unwrap();
    let mut app = bare_app(&dir, Some(load_library(&dir.join("a/todo.txt")).unwrap()));
    app.config.sync = false;
    app.connect();
    assert!(app.file().unwrap().asking.is_none() && worker(&app).is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_conflicted_repository_is_not_opened() {
    let dir = remote_and_clones("sync conflict", "Alpha\n");
    push_from_b(&dir, "Alpha from b\n");
    std::fs::write(dir.join("a/todo.txt"), "Alpha from a\n").unwrap();
    git(&dir.join("a"), &["commit", "--quiet", "-am", "from a"]);
    let pull = std::process::Command::new("git")
        .current_dir(dir.join("a"))
        .args(["pull", "--quiet", "--rebase"])
        .output()
        .unwrap();
    assert!(!pull.status.success());

    let app = synced_app(&dir, &dir.join("a/todo.txt"));
    assert!(app.file().unwrap().library.is_none());
    assert!(
        app.file()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("resolve them by hand")
    );
    let _ = std::fs::remove_dir_all(dir);
}

// Several open files (RTM-126): one worker per repository, shared by its files. These tests open
// the files directly, as the session at startup does.

/// An app with `files` open, each connected as at startup, the first one shown.
fn app_on(dir: &Path, files: &[PathBuf]) -> App {
    let mut app = bare_app(dir, None);
    app.files = files
        .iter()
        .map(|file| OpenFile::loaded(load_library(file).unwrap()))
        .collect();
    app.connect_all();
    app.active = app.files.first().map(|file| file.path.clone());
    wait(&mut app);
    app
}

/// Adds `text` to the open file at `file`, as the quick-add does for the shown file.
fn add_to(app: &mut App, file: &Path, text: &str) {
    let shown = app.active.replace(file.canonicalize().unwrap());
    app.new_item = text.to_owned();
    app.add_item();
    app.active = shown;
}

/// A second tracked file, `a/work.txt`, pushed and pulled into `b`.
fn add_work_file(dir: &Path, lines: &str) {
    std::fs::write(dir.join("a/work.txt"), lines).unwrap();
    git(&dir.join("a"), &["add", "work.txt"]);
    git(&dir.join("a"), &["commit", "--quiet", "-m", "work"]);
    git(&dir.join("a"), &["push", "--quiet"]);
    git(&dir.join("b"), &["pull", "--quiet"]);
}

#[test]
fn two_files_in_one_repository_share_a_worker_commit_apart_and_sync_once() {
    let dir = remote_and_clones("sync two files", "Alpha\n");
    add_work_file(&dir, "Report\n");
    let mut app = app_on(&dir, &[dir.join("a/todo.txt"), dir.join("a/work.txt")]);
    assert_eq!(app.repos.len(), 1);

    add_to(&mut app, &dir.join("a/todo.txt"), "Call mom");
    add_to(&mut app, &dir.join("a/work.txt"), "Send invoice");
    wait(&mut app);
    assert_eq!(
        subjects(&dir.join("a"), 2),
        "rtm: add Send invoice\nrtm: add Call mom"
    );

    sync(&mut app);
    wait(&mut app);
    assert_eq!(
        subjects(&dir.join("remote"), 2),
        "rtm: add Send invoice\nrtm: add Call mom"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_pull_reloads_only_the_files_it_changed() {
    let dir = remote_and_clones("sync reload one", "Alpha\n");
    add_work_file(&dir, "Report\n");
    let mut app = app_on(&dir, &[dir.join("a/todo.txt"), dir.join("a/work.txt")]);
    // Seen only in memory: a reload of this file would bring "Alpha" back.
    let first = app.files.first_mut().unwrap().library.as_mut().unwrap();
    first.items.first_mut().unwrap().description = "Not reloaded".to_owned();
    std::fs::write(dir.join("b/work.txt"), "Report\nFrom b\n").unwrap();
    git(&dir.join("b"), &["commit", "--quiet", "-am", "work from b"]);
    git(&dir.join("b"), &["push", "--quiet"]);

    sync(&mut app);
    wait(&mut app);

    let items: Vec<Vec<String>> = app
        .files
        .iter()
        .map(|file| {
            let library = file.library.as_ref().unwrap();
            library
                .items
                .iter()
                .map(|i| i.description.clone())
                .collect()
        })
        .collect();
    assert_eq!(items, [vec!["Not reloaded"], vec!["Report", "From b"]]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn files_in_two_repositories_sync_independently() {
    let one = remote_and_clones("sync repo one", "Alpha\n");
    let two = remote_and_clones("sync repo two", "Bravo\n");
    let mut app = app_on(&one, &[one.join("a/todo.txt"), two.join("a/todo.txt")]);
    assert_eq!(app.repos.len(), 2);

    add_to(&mut app, &one.join("a/todo.txt"), "In one");
    add_to(&mut app, &two.join("a/todo.txt"), "In two");
    wait(&mut app);
    sync(&mut app);
    wait(&mut app);

    assert_eq!(subjects(&one.join("remote"), 1), "rtm: add In one");
    assert_eq!(subjects(&two.join("remote"), 1), "rtm: add In two");
    let _ = std::fs::remove_dir_all(one);
    let _ = std::fs::remove_dir_all(two);
}

#[test]
fn a_conflict_closes_only_the_files_of_its_repository() {
    let one = remote_and_clones("sync conflict one", "Alpha\n");
    let two = remote_and_clones("sync conflict two", "Bravo\n");
    let mut app = app_on(&one, &[one.join("a/todo.txt"), two.join("a/todo.txt")]);
    push_from_b(&one, "Alpha from b\n");
    open_editor(&mut app, 0).description = "Alpha from a".to_owned();
    app.save_edit();
    wait(&mut app);

    sync(&mut app);
    wait(&mut app);

    let [closed, other] = app.files.as_slice() else {
        panic!("two files are open");
    };
    assert!(closed.library.is_none() && closed.repo.is_none());
    let error = closed.error.as_deref().unwrap();
    assert!(error.contains("resolve them by hand"), "{error}");
    assert!(other.library.is_some() && other.repo.is_some());
    assert_eq!(app.repos.len(), 1);
    assert_eq!(
        app.files.len(),
        2,
        "the closed file stays open, with its message"
    );
    let _ = std::fs::remove_dir_all(one);
    let _ = std::fs::remove_dir_all(two);
}

// The check on open runs on the repository's worker (RTM-126).

#[test]
fn a_change_made_during_the_check_on_open_waits_for_it() {
    let dir = remote_and_clones("check holds changes", "Alpha\n");
    let mut app = bare_app(&dir, Some(load_library(&dir.join("a/todo.txt")).unwrap()));
    app.connect();
    assert!(app.file().unwrap().checking);

    // The check's result is handled only by `sync_results`, so it is still running here.
    app.new_item = "Call mom".to_owned();
    app.add_item();
    assert_eq!(file_lines(&dir.join("a")), ["Alpha"]);

    wait(&mut app);
    assert!(!app.file().unwrap().checking);
    let lines = file_lines(&dir.join("a"));
    assert!(
        lines.last().is_some_and(|l| l.ends_with("Call mom")),
        "{lines:?}"
    );
    assert_eq!(subjects(&dir.join("a"), 1), "rtm: add Call mom");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_file_joining_after_a_sync_was_queued_reads_what_it_pulled() {
    let dir = remote_and_clones("check rereads", "Alpha\n");
    add_work_file(&dir, "Report\n");
    let mut app = app_on(&dir, &[dir.join("a/todo.txt")]);
    std::fs::write(dir.join("b/work.txt"), "Report\nFrom b\n").unwrap();
    git(&dir.join("b"), &["commit", "--quiet", "-am", "work from b"]);
    git(&dir.join("b"), &["push", "--quiet"]);

    // A sync listing only todo.txt, then work.txt opens: its check waits behind that sync.
    sync(&mut app);
    let work = OpenFile::loaded(load_library(&dir.join("a/work.txt")).unwrap());
    app.active = Some(work.path.clone());
    app.files.push(work);
    app.connect();
    app.new_item = "Mine".to_owned();
    app.add_item();
    wait(&mut app);

    let work = app.file().unwrap().library.as_ref().unwrap();
    let items: Vec<_> = work.items.iter().map(|i| i.description.as_str()).collect();
    assert_eq!(items, ["Report", "From b", "Mine"]);
    let on_disk = std::fs::read_to_string(dir.join("a/work.txt")).unwrap();
    assert!(on_disk.starts_with("Report\nFrom b\n"), "{on_disk}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_change_during_the_check_of_an_untracked_file_is_written_but_not_committed() {
    let dir = remote_and_clones("check untracked change", "Alpha\n");
    std::fs::write(dir.join("a/other.txt"), "Other\n").unwrap();
    let mut app = bare_app(&dir, Some(load_library(&dir.join("a/other.txt")).unwrap()));
    app.connect();
    app.new_item = "Call mom".to_owned();
    app.add_item();
    wait(&mut app);

    assert!(matches!(
        app.file().unwrap().asking,
        Some((_, Question::Untracked))
    ));
    let on_disk = std::fs::read_to_string(dir.join("a/other.txt")).unwrap();
    assert!(on_disk.contains("Call mom"), "{on_disk}");
    assert_eq!(subjects(&dir.join("a"), 1), "start");
    assert_eq!(git(&dir.join("a"), &["ls-files", "other.txt"]), "");
    let _ = std::fs::remove_dir_all(dir);
}

// Turning sync on or off in Settings (RTM-134).

/// Saves the Settings dialog with `sync` changed to `on`.
fn set_sync(app: &mut App, on: bool) {
    app.settings = Some(SettingsForm::new(&app.config));
    app.settings.as_mut().unwrap().sync = on;
    app.save_settings();
}

#[test]
fn turning_sync_off_stops_the_worker_once_it_is_idle() {
    let dir = remote_and_clones("settings sync off", "Alpha\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    set_sync(&mut app, false);
    app.reconnect_when_idle();
    assert!(worker(&app).is_none());
    assert!(!app.reconnect);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn turning_sync_off_waits_for_the_running_job() {
    let dir = remote_and_clones("settings sync busy", "Alpha\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    sync(&mut app);
    set_sync(&mut app, false);
    // Outstanding until its result is picked up, however fast the job itself is.
    app.reconnect_when_idle();
    assert!(worker(&app).is_some());
    wait(&mut app);
    app.reconnect_when_idle();
    assert!(worker(&app).is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn turning_sync_on_starts_the_worker() {
    let dir = remote_and_clones("settings sync on", "Alpha\n");
    let mut app = bare_app(&dir, Some(load_library(&dir.join("a/todo.txt")).unwrap()));
    app.config.sync = false;
    app.connect();
    assert!(worker(&app).is_none());
    set_sync(&mut app, true);
    app.reconnect_when_idle();
    assert!(worker(&app).is_some());
    // The remote is known once the check on open has run on the worker.
    wait(&mut app);
    assert!(worker(&app).is_some_and(Worker::has_remote));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_new_interval_keeps_the_worker() {
    let dir = remote_and_clones("settings interval", "Alpha\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    let last_sync = app.repos.first().unwrap().last_sync;
    app.settings = Some(SettingsForm::new(&app.config));
    app.settings.as_mut().unwrap().interval = 1;
    app.save_settings();
    assert!(!app.reconnect);
    app.reconnect_when_idle();
    assert!(worker(&app).is_some());
    assert_eq!(app.repos.first().unwrap().last_sync, last_sync);
    assert_eq!(app.config.sync_interval_minutes, 1);
    let _ = std::fs::remove_dir_all(dir);
}

// Closing tabs and questions in background tabs (RTM-127).

#[test]
fn closing_waits_for_the_repository_and_its_last_tab_pushes_once_more() {
    let dir = remote_and_clones("close pushes", "Alpha\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    let path = app.file().unwrap().path.clone();
    app.new_item = "Call mom".to_owned();
    app.add_item();

    app.close(&path);
    assert_eq!(app.files.len(), 1, "a commit is outstanding");
    wait(&mut app);
    app.close(&path);

    assert!(app.files.is_empty() && app.repos.is_empty());
    eframe::App::on_exit(&mut app);
    assert!(app.closing.iter().all(|worker| !worker.busy()));
    assert_eq!(subjects(&dir.join("remote"), 1), "rtm: add Call mom");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_closed_repository_is_let_go_once_its_last_sync_is_done() {
    let dir = remote_and_clones("close lets go", "Alpha\n");
    let mut app = synced_app(&dir, &dir.join("a/todo.txt"));
    let path = app.file().unwrap().path.clone();

    app.close(&path);
    assert_eq!(app.closing.len(), 1);
    let deadline = Instant::now() + Duration::from_secs(60);
    while !app.closing.is_empty() {
        assert!(Instant::now() < deadline, "the last sync never finished");
        std::thread::sleep(Duration::from_millis(20));
        app.sync_results();
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_question_in_a_background_tab_brings_it_to_front_without_writing_the_config() {
    let dir = remote_and_clones("question tab", "Alpha\n");
    std::fs::write(dir.join("a/other.txt"), "Other\n").unwrap();
    let mut app = app_on(&dir, &[dir.join("a/todo.txt"), dir.join("a/other.txt")]);
    assert!(app.file().unwrap().asking.is_none());

    app.show_question();

    assert!(matches!(
        app.file().unwrap().asking,
        Some((_, Question::Untracked))
    ));
    assert!(app.file().unwrap().path.ends_with("other.txt"));
    assert!(!dir.join("config.toml").exists());
    let _ = std::fs::remove_dir_all(dir);
}
