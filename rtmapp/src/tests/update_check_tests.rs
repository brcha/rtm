use super::*;

use std::io::ErrorKind;
#[cfg(feature = "update-check")]
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
#[cfg(feature = "update-check")]
use std::thread::JoinHandle;

/// A release list with one stable release.
#[cfg(feature = "update-check")]
fn feed(version: &str) -> String {
    format!(
        r#"{{"schema": 1, "releases": [{{"version": "{version}", "channel": "stable", "date": "2026-11-06", "url": "https://rtm.example/news/{version}/"}}]}}"#
    )
}

/// Answers one request with `body`, once `gate` lets it (immediately when it is none).
#[cfg(feature = "update-check")]
fn serve(body: String, gate: Option<mpsc::Receiver<()>>) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/releases.json", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                break;
            }
        }
        if let Some(gate) = gate {
            let _ = gate.recv();
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
    });
    (url, server)
}

/// A URL nothing listens on.
fn closed_url() -> String {
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    format!("http://127.0.0.1:{port}/releases.json")
}

/// Takes results until the running check is done, for 5 s at most.
#[cfg(feature = "update-check")]
fn wait(check: &mut UpdateCheck) {
    let start = Instant::now();
    while check.running.is_some() {
        assert!(start.elapsed() < Duration::from_secs(5), "no answer");
        std::thread::sleep(Duration::from_millis(10));
        check.results();
    }
}

fn notice(version: &str) -> Notice {
    Notice {
        version: version.to_owned(),
        url: format!("https://rtm.example/news/{version}/"),
    }
}

#[cfg(feature = "update-check")]
#[test]
fn the_first_check_waits_and_the_next_comes_a_day_later() {
    let ctx = egui::Context::default();
    let start = Instant::now();
    let mut check = UpdateCheck::new(closed_url(), true, start);
    check.tick(start + Duration::from_secs(1), false, &ctx);
    assert!(check.running.is_none());
    let first = start + FIRST_DELAY;
    check.tick(first, false, &ctx);
    assert!(check.running.is_some());
    assert_eq!(check.due, Some(first + INTERVAL));
    wait(&mut check);
    check.tick(first + INTERVAL - Duration::from_secs(1), false, &ctx);
    assert!(check.running.is_none());
    check.tick(first + INTERVAL, false, &ctx);
    assert!(check.running.is_some());
}

#[cfg(feature = "update-check")]
#[test]
fn a_newer_release_becomes_the_notice() {
    let (url, server) = serve(feed("999.0.0"), None);
    let mut check = UpdateCheck::new(url, true, Instant::now());
    check.set(true, Instant::now());
    check.tick(Instant::now(), false, &egui::Context::default());
    wait(&mut check);
    server.join().unwrap();
    assert_eq!(check.shown(), Some(&notice("999.0.0")));
}

#[cfg(feature = "update-check")]
#[test]
fn a_release_page_that_is_not_https_is_refused() {
    let body = feed("999.0.0").replace("https://rtm.example/news/999.0.0/", "file:///etc/passwd");
    let (url, server) = serve(body, None);
    let mut check = UpdateCheck::new(url, true, Instant::now());
    check.due = Some(Instant::now());
    check.tick(Instant::now(), false, &egui::Context::default());
    wait(&mut check);
    server.join().unwrap();
    assert_eq!(check.notice, None);
}

#[cfg(feature = "update-check")]
#[test]
fn an_up_to_date_answer_removes_the_notice() {
    let (url, server) = serve(feed("0.1.0"), None);
    let mut check = UpdateCheck::new(url, true, Instant::now());
    check.due = Some(Instant::now());
    check.notice = Some(notice("999.0.0"));
    check.tick(Instant::now(), false, &egui::Context::default());
    wait(&mut check);
    server.join().unwrap();
    assert_eq!(check.notice, None);
}

#[cfg(feature = "update-check")]
#[test]
fn a_failed_check_keeps_the_notice() {
    let mut check = UpdateCheck::new(closed_url(), true, Instant::now());
    check.due = Some(Instant::now());
    check.notice = Some(notice("999.0.0"));
    check.tick(Instant::now(), false, &egui::Context::default());
    wait(&mut check);
    assert_eq!(check.notice, Some(notice("999.0.0")));
}

#[cfg(feature = "update-check")]
#[test]
fn a_check_started_before_settings_changed_is_ignored() {
    let (open, gate) = mpsc::channel();
    let (url, server) = serve(feed("999.0.0"), Some(gate));
    let mut check = UpdateCheck::new(url, true, Instant::now());
    check.due = Some(Instant::now());
    check.tick(Instant::now(), true, &egui::Context::default());
    check.set(true, Instant::now());
    open.send(()).unwrap();
    server.join().unwrap();
    std::thread::sleep(Duration::from_millis(50));
    check.results();
    assert_eq!(check.notice, None);
}

#[cfg(feature = "update-check")]
#[test]
fn turning_the_check_on_makes_it_due_at_once() {
    let now = Instant::now();
    let mut check = UpdateCheck::new(closed_url(), false, now);
    assert_eq!(check.due, None);
    check.set(true, now);
    assert_eq!(check.due, Some(now));
}

#[test]
fn turning_the_check_off_removes_the_notice_and_stops_it() {
    let now = Instant::now();
    let mut check = UpdateCheck::new(closed_url(), true, now);
    check.notice = Some(notice("999.0.0"));
    check.set(false, now);
    assert_eq!(check.notice, None);
    assert_eq!(check.due, None);
}

#[test]
fn a_closed_notice_is_not_shown() {
    let mut check = UpdateCheck::new(closed_url(), false, Instant::now());
    check.notice = Some(notice("999.0.0"));
    check.dismissed = true;
    assert_eq!(check.shown(), None);
}

/// Off, or built without the feature, the check never connects.
#[test]
fn a_disabled_check_never_connects() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/releases.json", listener.local_addr().unwrap());
    let ctx = egui::Context::default();
    let start = Instant::now();
    let enabled = !cfg!(feature = "update-check");
    let mut check = UpdateCheck::new(url, enabled, start);
    for seconds in [0, 5, 60, 25 * 60 * 60] {
        check.tick(start + Duration::from_secs(seconds), false, &ctx);
        check.results();
    }
    assert!(check.running.is_none());
    std::thread::sleep(Duration::from_millis(50));
    listener.set_nonblocking(true).unwrap();
    let accepted = listener.accept();
    assert!(
        matches!(&accepted, Err(e) if e.kind() == ErrorKind::WouldBlock),
        "{accepted:?}"
    );
}
