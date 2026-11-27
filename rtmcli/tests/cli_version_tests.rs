//! `rtmcli --version` and `rtmcli -V`.

use std::path::PathBuf;
use std::process::{Command, Output};

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("rtmcli version test {} {name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs rtmcli with `flag` in an empty scratch directory.
fn rtmcli(name: &str, flag: &str) -> Output {
    let dir = scratch(name);
    let output = Command::new(env!("CARGO_BIN_EXE_rtmcli"))
        .current_dir(&dir)
        .arg(flag)
        .env_remove("TODOTXT")
        .env_remove("RTM_NO_SYNC")
        .env_remove("RTM_VCS")
        .env_remove("RTM_UPDATE_URL")
        .env_remove("RTM_ARCHIVE_DAYS")
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    output
}

fn assert_prints_version(output: &Output) {
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("rtmcli {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn long_flag_prints_the_version() {
    assert_prints_version(&rtmcli("long", "--version"));
}

#[test]
fn short_flag_prints_the_version() {
    assert_prints_version(&rtmcli("short", "-V"));
}
