//! `Vcs::commit` with Pijul. This binary holds a single test: it sets process-wide environment
//! variables, which is only sound while no other test runs beside it.

use std::path::Path;
use std::process::Command;

use rtmsync::Vcs;

/// Runs pijul in `dir` and returns its stdout, panicking on failure.
fn pijul(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("pijul")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "pijul {args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

/// A scratch `ssh-agent` holding one generated key, stopped when dropped. Pijul signs every change
/// through the agent; this keeps it away from the user's own agent and keys.
#[cfg(unix)]
struct Agent(std::process::Child);

#[cfg(unix)]
impl Agent {
    /// Starts the agent with its socket in `dir`. Every ssh tool spawned here is pointed at that
    /// socket explicitly, so a failed start can never reach the user's agent.
    fn start(dir: &Path) -> Agent {
        use std::time::{Duration, Instant};
        let ssh = |program: &str| {
            let mut command = Command::new(program);
            command.current_dir(dir).env("SSH_AUTH_SOCK", "agent.sock");
            command
        };
        let agent = Agent(
            ssh("ssh-agent")
                .args(["-D", "-a", "agent.sock"])
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
        let started = Instant::now();
        while !dir.join("agent.sock").exists() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "ssh-agent did not start"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        let keygen = ssh("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f", "key"])
            .status()
            .unwrap();
        assert!(keygen.success());
        assert!(
            ssh("ssh-add")
                .args(["-q", "key"])
                .status()
                .unwrap()
                .success()
        );
        std::fs::remove_file(dir.join("key")).unwrap();
        std::fs::remove_file(dir.join("key.pub")).unwrap();
        agent
    }
}

#[cfg(unix)]
impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn pijul_records_only_the_file() {
    if Command::new("pijul").arg("--version").output().is_err() {
        assert!(
            std::env::var_os("RTM_REQUIRE_PIJUL").is_none(),
            "pijul is not in PATH but RTM_REQUIRE_PIJUL is set"
        );
        eprintln!("pijul is not in PATH; skipping the Pijul test");
        return;
    }

    let dir = std::env::temp_dir().join(format!("rtmsync pijul test {}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (home, repo, sub) = (dir.join("home"), dir.join("repo"), dir.join("repo/sub"));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&sub).unwrap();

    // SAFETY: this binary has a single test, so no other thread reads the environment.
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_CONFIG_HOME", home.join(".config"));
        // Pijul's own override: on Windows its default comes from the system, not from `APPDATA`.
        std::env::set_var("PIJUL_CONFIG_DIR", home.join("pijul"));
    }
    // Windows runs pijul 1.0.0-beta.12 (see rust.yml), which signs with a key file instead of the
    // agent; it needs an identity, created here without prompts or the system keyring.
    #[cfg(not(unix))]
    pijul(
        &home,
        &[
            "identity",
            "new",
            "--no-prompt",
            "--no-link",
            "--no-keyring",
            "--display-name",
            "rtm test",
            "rtm-test",
        ],
    );
    #[cfg(unix)]
    let _agent = {
        let agent = Agent::start(&dir);
        // Relative to `sub`, where every pijul below runs: the absolute scratch path can exceed
        // the length limit of a Unix socket path.
        // SAFETY: as above; the agent and its tools were given their socket explicitly.
        unsafe { std::env::set_var("SSH_AUTH_SOCK", "../../agent.sock") };
        agent
    };

    pijul(&repo, &["init"]);
    let todo = sub.join("todo.txt");
    std::fs::write(&todo, "Buy milk\n").unwrap();
    std::fs::write(repo.join("other.txt"), "other\n").unwrap();
    pijul(&sub, &["add", "../other.txt"]);

    Vcs::Pijul.commit(&todo, "first").unwrap();
    let log = pijul(&sub, &["log"]);
    assert!(log.contains("first"), "{log}");

    Vcs::Pijul.commit(&todo, "unchanged").unwrap();
    assert_eq!(
        pijul(&sub, &["log"]),
        log,
        "an unchanged file recorded a change"
    );

    std::fs::write(&todo, "Buy milk\nCall mom\n").unwrap();
    Vcs::Pijul.commit(&todo, "second").unwrap();
    assert!(pijul(&sub, &["log"]).contains("second"));

    let pending = pijul(&sub, &["diff", "--short"]);
    assert!(pending.contains("other.txt"), "{pending}");
    assert!(!pending.contains("todo.txt"), "{pending}");
}
