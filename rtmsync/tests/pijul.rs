//! `Vcs::commit`, `Vcs::state` and `Vcs::sync` with Pijul. This binary holds a single test: it sets
//! process-wide environment variables, which is only sound while no other test runs beside it.

use std::path::Path;
use std::process::Command;

use rtmsync::{Error, State, Vcs};

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
fn pijul_records_only_the_file_and_syncs() {
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
    // Windows runs pijul 1.0.0-beta.11 (see rust.yml), which signs with an unencrypted key file
    // instead of the agent. It needs an identity, and on a fresh config it would first prompt for
    // one (failing without a terminal, even under `identity new --no-prompt`), so a throwaway
    // test identity is written directly. It signs nothing but these scratch changes.
    #[cfg(not(unix))]
    {
        let identity = home.join("pijul/identities/rtm-test");
        std::fs::create_dir_all(&identity).unwrap();
        std::fs::write(
            identity.join("identity.toml"),
            r#"display_name = "rtm test"
last_modified = "2026-10-30T10:36:41.494964317Z"

[public_key]
version = 0
algorithm = "Ed25519"
signature = "3d826vxwEZsqKL8AusPJfQB1mZ4RTx9zjYgQTHviA6WtvgHuyEYnDCTGjo5b4s1xFcJzcPsCG6b4E4rafYZo2493"
key = "EzvZqHUT69TW3kak4QKUNPUToh3pCYg7gfLtkZ1doeMB"
"#,
        )
        .unwrap();
        std::fs::write(
            identity.join("secret_key.json"),
            r#"{
  "version": 0,
  "algorithm": "Ed25519",
  "key": "V5NBHDdTNKpvsycNoHeomfZD72qEMf4xC4AQwMooTpjH9jowhQcbkowtBNkg15CpiZfdyxmubB1oJCgNyq24Xim"
}"#,
        )
        .unwrap();
    }
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

    assert!(!Vcs::Pijul.is_tracked(&todo).unwrap());
    assert_eq!(Vcs::Pijul.state(&todo).unwrap(), State::Untracked);
    let ignored = sub.join("ignored.txt");
    std::fs::write(&ignored, "ignored\n").unwrap();
    std::fs::write(repo.join(".ignore"), "ignored.txt\n").unwrap();
    assert_eq!(Vcs::Pijul.state(&ignored).unwrap(), State::Ignored);
    // At the repository root, where a todo.txt usually is: added and never recorded, untracked.
    assert_eq!(
        Vcs::Pijul.state(&repo.join("other.txt")).unwrap(),
        State::Modified
    );
    std::fs::write(repo.join("top.txt"), "top\n").unwrap();
    assert_eq!(
        Vcs::Pijul.state(&repo.join("top.txt")).unwrap(),
        State::Untracked
    );
    Vcs::Pijul.commit(&todo, "first").unwrap();
    assert!(Vcs::Pijul.is_tracked(&todo).unwrap());
    assert_eq!(Vcs::Pijul.state(&todo).unwrap(), State::Clean);
    assert!(!Vcs::Pijul.has_remote(&todo).unwrap());
    let log = pijul(&sub, &["log"]);
    assert!(log.contains("first"), "{log}");

    Vcs::Pijul.commit(&todo, "unchanged").unwrap();
    assert_eq!(
        pijul(&sub, &["log"]),
        log,
        "an unchanged file recorded a change"
    );

    std::fs::write(&todo, "Buy milk\nCall mom\n").unwrap();
    assert_eq!(Vcs::Pijul.state(&todo).unwrap(), State::Modified);
    Vcs::Pijul.commit(&todo, "second").unwrap();
    assert!(pijul(&sub, &["log"]).contains("second"));

    let pending = pijul(&sub, &["diff", "--short"]);
    assert!(pending.contains("other.txt"), "{pending}");
    assert!(!pending.contains("todo.txt"), "{pending}");

    // Sync between two clones of `repo`, their remote. Each todo.txt sits at the same depth as
    // `repo/sub/todo.txt`, so the relative agent socket path still resolves.
    pijul(&dir, &["clone", "repo", "a"]);
    pijul(&dir, &["clone", "repo", "b"]);
    let (a, b) = (dir.join("a/sub/todo.txt"), dir.join("b/sub/todo.txt"));
    assert!(Vcs::Pijul.has_remote(&a).unwrap());

    std::fs::write(&a, "Buy milk\nCall mom\nFrom a\n").unwrap();
    Vcs::Pijul.commit(&a, "from a").unwrap();
    Vcs::Pijul.sync(&a).unwrap();
    assert!(pijul(&repo, &["log"]).contains("from a"));
    Vcs::Pijul.sync(&b).unwrap();
    assert!(std::fs::read_to_string(&b).unwrap().contains("From a"));

    std::fs::write(&a, "Buy milk\nCall mom\nFrom a\nAgain a\n").unwrap();
    Vcs::Pijul.commit(&a, "again a").unwrap();
    Vcs::Pijul.sync(&a).unwrap();
    std::fs::write(&b, "Buy milk\nCall mom\nFrom a\nThen b\n").unwrap();
    Vcs::Pijul.commit(&b, "then b").unwrap();
    assert!(!Vcs::Pijul.is_conflicted(&b).unwrap());
    let result = Vcs::Pijul.sync(&b);
    assert!(matches!(result, Err(Error::Conflict)), "{result:?}");
    assert!(Vcs::Pijul.is_conflicted(&b).unwrap());
    assert!(
        !pijul(&repo, &["log"]).contains("then b"),
        "a conflicted sync pushed"
    );
}
