//! `rtmcli mcp`: the MCP server on the file from `-f` or `$TODOTXT`, over stdin and stdout.
//!
//! Only one test here completes the handshake: every server started by this test binary has the
//! same parent, and a second one running at the same time would offer no tools (one server per
//! client).

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// An empty scratch directory for test `name`.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmcli-mcp-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // The spelling the child's working directory has (/private/var rather than /var on macOS),
    // without the verbatim prefix (\\?\) that Windows adds to a canonical path.
    let dir = dir.canonicalize().unwrap();
    PathBuf::from(dir.to_str().unwrap().trim_start_matches(r"\\?\"))
}

/// rtmcli started in `dir`, with the environment that would change the outcome cleared.
fn rtmcli(dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rtmcli"));
    command
        .current_dir(dir)
        .env_remove("RTM_NO_SYNC")
        .env_remove("RTM_VCS")
        .env_remove("RTM_UPDATE_URL")
        .env_remove("TODOTXT");
    command
}

#[test]
fn mcp_without_a_file_exits_2() {
    let dir = scratch("no file");
    fs::write(dir.join("todo.txt"), "Buy milk\n").unwrap();
    let output = rtmcli(&dir).arg("mcp").output().unwrap();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("-f FILE or set TODOTXT"),
        "{output:?}"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn mcp_serves_the_file_by_its_absolute_path() {
    let dir = scratch("serve");
    fs::write(dir.join("todo.txt"), "Buy milk\n").unwrap();
    // A relative -f after the subcommand: the server must report the absolute path.
    let mut child = rtmcli(&dir)
        .args(["--no-sync", "mcp", "-f", "todo.txt"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let (lines, replies) = mpsc::channel();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    std::thread::spawn(move || {
        for line in stdout.lines() {
            if lines.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    // The reply to request `id`; a hung server fails the test instead of hanging it.
    let reply = |id: u32| loop {
        let line = replies
            .recv_timeout(Duration::from_secs(10))
            .expect("no reply from the server");
        if line.contains(&format!(r#""id":{id}"#)) {
            break line;
        }
    };

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"test","version":"0"}}}}}}"#
    )
    .unwrap();
    let initialized = reply(1);
    assert!(initialized.contains(r#""name":"rtm""#), "{initialized}");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
    )
    .unwrap();
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list"}}"#).unwrap();
    let tools = reply(2);
    assert!(tools.contains(r#""list_items""#), "{tools}");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"list_files","arguments":{{}}}}}}"#
    )
    .unwrap();
    let files = reply(3);
    let path = dir.join("todo.txt").display().to_string();
    // The path inside a JSON string: backslashes (Windows) are escaped.
    assert!(files.contains(&path.replace('\\', "\\\\")), "{files}");

    drop(stdin);
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            panic!("the server did not exit after stdin closed");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success());
    fs::remove_dir_all(&dir).unwrap();
}
