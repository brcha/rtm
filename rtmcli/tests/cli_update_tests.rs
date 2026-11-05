//! `rtmcli check-update` against a local HTTP server; no test reaches the network.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[cfg(feature = "update-check")]
use std::io::{BufRead, BufReader, Write};
#[cfg(feature = "update-check")]
use std::net::TcpListener;
#[cfg(feature = "update-check")]
use std::thread::{self, JoinHandle};

/// An empty scratch directory for test `name`.
fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("rtmcli update test {} {name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs rtmcli in `dir` with `args`, checking for updates at `url`.
fn rtmcli(dir: &Path, url: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rtmcli"))
        .current_dir(dir)
        .args(args)
        .env("RTM_UPDATE_URL", url)
        .env_remove("RTM_NO_SYNC")
        .env_remove("RTM_VCS")
        .env_remove("TODOTXT")
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A URL nothing listens on, so any request through it fails.
fn closed_url() -> String {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    format!("http://127.0.0.1:{port}/releases.json")
}

/// Serves one request with `status` and `body`; returns the URL and the server thread.
#[cfg(feature = "update-check")]
fn serve(status: &str, body: &str) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/releases.json", listener.local_addr().unwrap());
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                break;
            }
        }
        stream.write_all(response.as_bytes()).unwrap();
    });
    (url, server)
}

/// A release list with `(version, channel)` entries; the versions are far from any real one.
#[cfg(feature = "update-check")]
fn feed(entries: &[(&str, &str)]) -> String {
    let releases: Vec<String> = entries
        .iter()
        .map(|(v, c)| {
            format!(
                r#"{{"version": "{v}", "channel": "{c}", "date": "2026-11-06", "url": "https://rtm.example/news/{v}/"}}"#
            )
        })
        .collect();
    format!(r#"{{"schema": 1, "releases": [{}]}}"#, releases.join(", "))
}

#[cfg(feature = "update-check")]
#[test]
fn newer_release_reported() {
    let dir = scratch("newer");
    let (url, server) = serve(
        "200 OK",
        &feed(&[("999.0.0", "stable"), ("0.1.0", "stable")]),
    );
    let output = rtmcli(&dir, &url, &["check-update"]);
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        stdout(&output),
        "rtm 999.0.0 is available: https://rtm.example/news/999.0.0/\n"
    );
}

#[cfg(feature = "update-check")]
#[test]
fn up_to_date_reported() {
    let dir = scratch("up to date");
    let (url, server) = serve("200 OK", &feed(&[("0.1.0", "stable")]));
    let output = rtmcli(&dir, &url, &["check-update"]);
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        stdout(&output),
        format!("rtmcli {} is up to date\n", env!("CARGO_PKG_VERSION"))
    );
}

#[cfg(feature = "update-check")]
#[test]
fn beta_offered_only_on_beta_channel() {
    let dir = scratch("beta");
    let releases = feed(&[("999.0.0-beta.1", "beta"), ("0.1.0", "stable")]);

    let (url, server) = serve("200 OK", &releases);
    let stable = rtmcli(&dir, &url, &["check-update"]);
    server.join().unwrap();
    assert!(stdout(&stable).ends_with(" is up to date\n"), "{stable:?}");

    let (url, server) = serve("200 OK", &releases);
    let beta = rtmcli(&dir, &url, &["check-update", "--channel", "beta"]);
    server.join().unwrap();
    assert!(
        stdout(&beta).starts_with("rtm 999.0.0-beta.1 is available: "),
        "{beta:?}"
    );
}

#[cfg(feature = "update-check")]
#[test]
fn failed_fetch_exits_1() {
    let dir = scratch("404");
    let (url, server) = serve("404 Not Found", "missing");
    let output = rtmcli(&dir, &url, &["check-update"]);
    server.join().unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        stderr(&output).starts_with("Error: cannot fetch the release list"),
        "{output:?}"
    );
}

#[cfg(feature = "update-check")]
#[test]
fn unknown_schema_exits_1() {
    let dir = scratch("schema");
    let (url, server) = serve("200 OK", r#"{"schema": 2, "releases": {}}"#);
    let output = rtmcli(&dir, &url, &["check-update"]);
    server.join().unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(stderr(&output).contains("schema 2"), "{output:?}");
}

/// In a repository with an untracked todo.txt, every file command reminds the user of it;
/// check-update runs before the file and the repository are looked at, so it does not.
#[test]
fn file_and_repository_not_touched() {
    let dir = scratch("untouched");
    let git = Command::new("git")
        .current_dir(&dir)
        .args(["init", "-q"])
        .output()
        .unwrap();
    assert!(git.status.success(), "{git:?}");
    std::fs::write(dir.join("todo.txt"), "Buy milk\n").unwrap();

    let list = rtmcli(&dir, &closed_url(), &["list"]);
    assert!(stderr(&list).contains("is not tracked by"), "{list:?}");

    // Without the feature no request is made; with it the closed port makes the check fail.
    let check = rtmcli(&dir, &closed_url(), &["check-update"]);
    assert!(!stderr(&check).contains("is not tracked by"), "{check:?}");
    assert_eq!(
        std::fs::read_to_string(dir.join("todo.txt")).unwrap(),
        "Buy milk\n"
    );
}

#[cfg(not(feature = "update-check"))]
#[test]
fn build_without_check_says_how_to_update() {
    let dir = scratch("no check");
    let output = rtmcli(&dir, &closed_url(), &["check-update", "--channel", "beta"]);
    assert!(output.status.success(), "{output:?}");
    assert!(
        stdout(&output).starts_with("This build of rtmcli has no update check"),
        "{output:?}"
    );
}
