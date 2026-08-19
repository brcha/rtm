use std::fs;
use std::process::Command;

fn today() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

#[test]
fn test_cli_add_single_task() {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("test_cli_add_single.txt");
    let file_path = temp_file.to_str().unwrap();

    // Clean up any existing file
    fs::remove_file(&temp_file).ok();

    // Run add command
    let output = Command::new("cargo")
        .args(&[
            "run", "--bin", "rtmcli", "--", "-f", file_path, "add", "Buy milk",
        ])
        .output()
        .expect("Failed to run add command");

    assert!(output.status.success());

    // Check file content — add_item stamps today's date as the creation date
    let content = fs::read_to_string(&temp_file).unwrap();
    assert_eq!(content.trim(), format!("{} Buy milk", today()));

    // Clean up
    fs::remove_file(&temp_file).unwrap();
}

#[test]
fn test_cli_add_multiple_tasks() {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("test_cli_add_multiple.txt");
    let file_path = temp_file.to_str().unwrap();

    fs::remove_file(&temp_file).ok();

    // Add first task
    Command::new("cargo")
        .args(&[
            "run", "--bin", "rtmcli", "--", "-f", file_path, "add", "Buy milk",
        ])
        .output()
        .expect("Failed to add first task");

    // Add second task
    Command::new("cargo")
        .args(&[
            "run", "--bin", "rtmcli", "--", "-f", file_path, "add", "Call mom",
        ])
        .output()
        .expect("Failed to add second task");

    // Check file has both — add_item stamps today's date as the creation date
    let content = fs::read_to_string(&temp_file).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2);
    let today = today();
    assert!(lines.contains(&format!("{} Buy milk", today).as_str()));
    assert!(lines.contains(&format!("{} Call mom", today).as_str()));

    fs::remove_file(&temp_file).unwrap();
}

#[test]
fn test_cli_list_tasks() {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("test_cli_list.txt");
    let file_path = temp_file.to_str().unwrap();

    fs::remove_file(&temp_file).ok();

    // Add tasks
    Command::new("cargo")
        .args(&[
            "run", "--bin", "rtmcli", "--", "-f", file_path, "add", "Task 1",
        ])
        .output()
        .unwrap();
    Command::new("cargo")
        .args(&[
            "run", "--bin", "rtmcli", "--", "-f", file_path, "add", "Task 2",
        ])
        .output()
        .unwrap();

    // List tasks
    let output = Command::new("cargo")
        .args(&["run", "--bin", "rtmcli", "--", "-f", file_path, "list"])
        .output()
        .expect("Failed to run list");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let today = today();
    assert!(stdout.contains(&format!("1. {} Task 1", today)));
    assert!(stdout.contains(&format!("2. {} Task 2", today)));

    fs::remove_file(&temp_file).unwrap();
}

#[test]
fn test_cli_complete_task_by_index() {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("test_cli_complete.txt");
    let file_path = temp_file.to_str().unwrap();

    fs::remove_file(&temp_file).ok();

    // Add task
    Command::new("cargo")
        .args(&[
            "run",
            "--bin",
            "rtmcli",
            "--",
            "-f",
            file_path,
            "add",
            "Incomplete task",
        ])
        .output()
        .unwrap();

    // Complete task by index
    let output = Command::new("cargo")
        .args(&[
            "run", "--bin", "rtmcli", "--", "-f", file_path, "complete", "1",
        ])
        .output()
        .expect("Failed to complete");

    assert!(output.status.success());

    // Check file has 'x', today's completion date, and today's creation date (stamped by
    // add_item when the task was first added, moments before completion)
    let content = fs::read_to_string(&temp_file).unwrap();
    let today = today();
    assert!(content.starts_with(&format!("x {} {} Incomplete task", today, today)));

    fs::remove_file(&temp_file).unwrap();
}

#[test]
fn test_cli_list_completed_tasks() {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("test_cli_completed.txt");
    let file_path = temp_file.to_str().unwrap();

    fs::remove_file(&temp_file).ok();

    // Add and complete task
    Command::new("cargo")
        .args(&[
            "run",
            "--bin",
            "rtmcli",
            "--",
            "-f",
            file_path,
            "add",
            "Complete this",
        ])
        .output()
        .unwrap();
    Command::new("cargo")
        .args(&[
            "run", "--bin", "rtmcli", "--", "-f", file_path, "complete", "1",
        ])
        .output()
        .unwrap();

    // Add another uncompleted
    Command::new("cargo")
        .args(&[
            "run",
            "--bin",
            "rtmcli",
            "--",
            "-f",
            file_path,
            "add",
            "Keep uncompleted",
        ])
        .output()
        .unwrap();

    // List completed
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--bin",
            "rtmcli",
            "--",
            "-f",
            file_path,
            "list",
            "--completed",
        ])
        .output()
        .expect("Failed to list completed");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let today = today();
    assert!(stdout.contains(&format!("x {} {} Complete this", today, today)));
    assert!(!stdout.contains("Keep uncompleted"));

    fs::remove_file(&temp_file).unwrap();
}
