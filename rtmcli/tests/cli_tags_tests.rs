//! Projects and contexts: the `projects` and `contexts` commands and filtering `list` by them, run
//! on a scratch todo.txt.

use chrono::{Duration, Local};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Open items on `+work` (twice, once spelled `+Work` first), `+home` and `@phone`, and a completed
/// item that is the only one on `+garden` and `@yard`.
const FIXTURE: &str = "\
Write the report +Work @office
Call the client +work @phone
Fix the fence +home @phone @PHONE
x 2026-11-01 2026-10-30 Plant the bulbs +garden @yard
";

/// A todo.txt holding `content`, in an empty scratch directory for test `name`.
fn todo_file(name: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmcli-tags-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("todo.txt");
    fs::write(&file, content).unwrap();
    file
}

fn cleanup(file: &Path) {
    fs::remove_dir_all(file.parent().unwrap()).unwrap();
}

/// Runs rtmcli on `file` with `args`, with the environment that would change the outcome cleared.
fn rtmcli(file: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rtmcli"))
        .arg("-f")
        .arg(file)
        .args(args)
        .env_remove("RTM_NO_SYNC")
        .env_remove("RTM_VCS")
        .env_remove("RTM_UPDATE_URL")
        .env_remove("TODOTXT")
        .output()
        .unwrap()
}

/// The lines printed by a successful command.
fn lines(output: &Output) -> Vec<String> {
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout.clone())
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn projects_lists_open_counts() {
    let file = todo_file("projects", FIXTURE);
    let header = format!("Projects in '{}':", file.display());
    assert_eq!(
        lines(&rtmcli(&file, &["projects"])),
        [header.as_str(), "+garden (0)", "+home (1)", "+Work (2)"]
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), FIXTURE);
    cleanup(&file);
}

#[test]
fn contexts_lists_open_counts() {
    let file = todo_file("contexts", FIXTURE);
    let header = format!("Contexts in '{}':", file.display());
    assert_eq!(
        lines(&rtmcli(&file, &["contexts"])),
        [header.as_str(), "@office (1)", "@phone (2)", "@yard (0)"]
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), FIXTURE);
    cleanup(&file);
}

#[test]
fn empty_file_has_no_tags() {
    let file = todo_file("empty", "Plain task\n");
    assert_eq!(
        lines(&rtmcli(&file, &["projects"]))[1..],
        ["No projects found."]
    );
    assert_eq!(
        lines(&rtmcli(&file, &["contexts"]))[1..],
        ["No contexts found."]
    );
    cleanup(&file);
}

#[test]
fn missing_file_is_an_error() {
    let file = todo_file("missing", "");
    fs::remove_file(&file).unwrap();
    for command in ["projects", "contexts"] {
        let output = rtmcli(&file, &[command]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Error loading file"),
            "{output:?}"
        );
    }
    cleanup(&file);
}

/// The date `days` from today.
fn day(days: i64) -> String {
    (Local::now().date_naive() + Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

/// The rows a successful `list` prints after its header.
fn rows(output: &Output) -> Vec<String> {
    lines(output).into_iter().skip(1).collect()
}

/// The exit code and stderr of a failed command.
fn failure(output: &Output) -> (Option<i32>, String) {
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn list_filters_by_project() {
    let file = todo_file("list-project", FIXTURE);
    assert_eq!(
        rows(&rtmcli(&file, &["list", "+work"])),
        [
            "1. Write the report +Work @office",
            "2. Call the client +work @phone"
        ]
    );
    cleanup(&file);
}

#[test]
fn list_needs_every_tag_in_any_order() {
    let file = todo_file("list-every", FIXTURE);
    let expected = ["1. Call the client +work @phone"];
    assert_eq!(rows(&rtmcli(&file, &["list", "+work", "@phone"])), expected);
    assert_eq!(rows(&rtmcli(&file, &["list", "@phone", "+work"])), expected);
    assert_eq!(
        rows(&rtmcli(&file, &["list", "+work", "@yard"])),
        ["No items found."]
    );
    cleanup(&file);
}

#[test]
fn list_completed_by_tag() {
    let file = todo_file("list-completed", FIXTURE);
    assert_eq!(
        rows(&rtmcli(&file, &["list", "-c", "+garden"])),
        ["1. x 2026-11-01 2026-10-30 Plant the bulbs +garden @yard"]
    );
    cleanup(&file);
}

#[test]
fn list_combines_tags_with_a_due_date_filter() {
    let content = format!(
        "Send the invoice +work due:{}\nBook the trip +work due:{}\nWater the plants +home due:{}\n",
        day(0),
        day(3),
        day(0)
    );
    let file = todo_file("list-due", &content);
    assert_eq!(
        rows(&rtmcli(&file, &["list", "today", "+work"])),
        [format!("1. Send the invoice +work due:{}", day(0))]
    );
    assert_eq!(rows(&rtmcli(&file, &["list", "+work", "all"])).len(), 2);
    cleanup(&file);
}

#[test]
fn list_rejects_unknown_filters() {
    let file = todo_file("list-unknown", FIXTURE);
    for term in ["work", "+", "@"] {
        let (code, stderr) = failure(&rtmcli(&file, &["list", term]));
        assert_eq!(code, Some(2), "{term}: {stderr}");
        assert!(stderr.contains("unknown filter"), "{term}: {stderr}");
    }
    let (code, stderr) = failure(&rtmcli(&file, &["list", "today", "+work", "week"]));
    assert_eq!(code, Some(2), "{stderr}");
    assert!(stderr.contains("only one due date filter"), "{stderr}");
    cleanup(&file);
}
