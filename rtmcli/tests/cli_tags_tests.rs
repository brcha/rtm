//! The `projects` and `contexts` commands, run on a scratch todo.txt.

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
