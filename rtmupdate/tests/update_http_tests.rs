//! `check_at` against a local HTTP server; no test reaches the network.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, ToSocketAddrs};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rtmupdate::{Channel, Error, check_at};

const FEED: &str = r#"{"schema": 1, "releases": [
    {"version": "26.3.0", "channel": "stable", "date": "2026-11-05", "url": "https://rtm.brcha.com/news/26.3.0/"}
]}"#;

fn response(status: &str, headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
        body.len()
    )
}

/// Answers one request on `listener` with `response`, and returns the request's head.
fn serve(listener: TcpListener, response: String) -> JoinHandle<String> {
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut head = String::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                break;
            }
            head.push_str(&line);
        }
        stream.write_all(response.as_bytes()).unwrap();
        head
    })
}

fn local() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
}

#[test]
fn newer_release_found() {
    let (listener, addr) = local();
    let server = serve(listener, response("200 OK", "", FEED));
    let release = check_at(
        &format!("http://{addr}/releases.json"),
        "26.2.0",
        Channel::Stable,
    )
    .unwrap();
    assert_eq!(release.map(|r| r.version).as_deref(), Some("26.3.0"));

    let head = server.join().unwrap().to_lowercase();
    assert!(
        head.starts_with("get /releases.json http/1.1\r\n"),
        "{head}"
    );
    assert!(
        head.contains("\r\nuser-agent: rtm-update-check\r\n"),
        "{head}"
    );
}

#[test]
fn redirect_to_another_host_followed() {
    // The target binds to whatever `localhost` resolves to first (often `::1` on macOS and
    // Windows), so the redirect goes to a different host name and address family alike.
    let target_addr = ("localhost", 0).to_socket_addrs().unwrap().next().unwrap();
    let target = TcpListener::bind(target_addr).unwrap();
    let port = target.local_addr().unwrap().port();
    let target = serve(target, response("200 OK", "", FEED));

    let (listener, addr) = local();
    let location = format!("Location: http://localhost:{port}/moved/releases.json\r\n");
    let origin = serve(listener, response("301 Moved Permanently", &location, ""));

    let release = check_at(
        &format!("http://{addr}/releases.json"),
        "26.2.0",
        Channel::Stable,
    )
    .unwrap();
    assert_eq!(release.map(|r| r.version).as_deref(), Some("26.3.0"));
    origin.join().unwrap();
    let head = target.join().unwrap().to_lowercase();
    assert!(head.starts_with("get /moved/releases.json "), "{head}");
}

#[test]
fn not_found_is_http_error() {
    let (listener, addr) = local();
    let server = serve(listener, response("404 Not Found", "", "missing"));
    let result = check_at(
        &format!("http://{addr}/releases.json"),
        "26.2.0",
        Channel::Stable,
    );
    assert!(matches!(result, Err(Error::Http(_))), "{result:?}");
    server.join().unwrap();
}

#[test]
fn silent_server_times_out() {
    let (listener, addr) = local();
    // Accepts the connection and never answers; the thread is left behind when the test ends.
    thread::spawn(move || {
        let _stream = listener.accept().unwrap();
        thread::sleep(Duration::from_secs(30));
    });
    let start = Instant::now();
    let result = check_at(
        &format!("http://{addr}/releases.json"),
        "26.2.0",
        Channel::Stable,
    );
    assert!(matches!(result, Err(Error::Http(_))), "{result:?}");
    assert!(
        start.elapsed() < Duration::from_secs(15),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn closed_port_is_http_error() {
    let (listener, addr) = local();
    drop(listener);
    let result = check_at(
        &format!("http://{addr}/releases.json"),
        "26.2.0",
        Channel::Stable,
    );
    assert!(matches!(result, Err(Error::Http(_))), "{result:?}");
}
