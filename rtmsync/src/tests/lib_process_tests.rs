use super::*;

#[cfg(unix)]
#[test]
fn a_timeout_returns_even_when_a_child_holds_the_output_open() {
    let started = Instant::now();
    let mut command = command("sh", Path::new("."));
    // The backgrounded sleep outlives the killed shell, as ssh can outlive git.
    command.args(["-c", "sleep 3 & sleep 3"]);
    let result = output_within("sh", &mut command, Duration::from_millis(200));
    assert!(
        matches!(result, Err(Error::Timeout { program: "sh" })),
        "{result:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn a_temporary_file_never_reuses_an_existing_path() {
    let (path, _file) = private_temp_file().unwrap();
    // The name a second call would try first, taken by a file that must survive.
    let n: u32 = path
        .to_str()
        .unwrap()
        .rsplit('-')
        .next()
        .unwrap()
        .trim_end_matches(".stderr")
        .parse()
        .unwrap();
    let taken = path.with_file_name(format!("rtmsync-{}-{}.stderr", std::process::id(), n + 1));
    // Claimed exclusively: a test running alongside may already use that name for its own
    // stderr, and must not have it overwritten or deleted.
    let Ok(mut claimed) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&taken)
    else {
        let _ = std::fs::remove_file(path);
        return;
    };
    std::io::Write::write_all(&mut claimed, b"keep").unwrap();
    let (other, _) = private_temp_file().unwrap();
    assert_ne!(other, taken);
    assert_eq!(std::fs::read_to_string(&taken).unwrap(), "keep");
    for p in [path, taken, other] {
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn a_failing_timed_call_reports_its_stderr() {
    let mut command = command("git", Path::new("."));
    command.arg("no-such-command");
    let result = output_within("git", &mut command, Duration::from_secs(10));
    assert!(
        matches!(&result, Err(Error::Failed { stderr, .. }) if stderr.contains("no-such-command")),
        "{result:?}"
    );
}
