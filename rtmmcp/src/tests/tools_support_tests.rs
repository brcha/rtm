use super::*;
use std::path::PathBuf;

/// A not-synced source for a todo.txt holding `lines`, in a fresh scratch directory under the
/// system temp directory.
pub(super) fn source_with(lines: &str) -> Source {
    let dir = std::env::temp_dir().join(format!("rtmmcp-tools-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("todo.txt");
    std::fs::write(&path, lines).unwrap();
    Source {
        path,
        name: None,
        sync: false,
        vcs: None,
    }
}

pub(super) fn lines(source: &Source) -> Vec<String> {
    std::fs::read_to_string(&source.path)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

pub(super) fn remove(source: Source) {
    let dir: PathBuf = source.path.parent().unwrap().to_owned();
    std::fs::remove_dir_all(dir).unwrap();
}

pub(super) fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// A synced source whose todo.txt holds "Buy milk", committed in a repository without a remote,
/// with local config keeping the user's identity, signing, hooks and rerere out.
pub(super) fn synced_source() -> Source {
    synced_source_with("Buy milk\n")
}

/// `synced_source` with a todo.txt holding `lines`.
pub(super) fn synced_source_with(lines: &str) -> Source {
    let source = Source {
        sync: true,
        ..source_with(lines)
    };
    let dir = source.path.parent().unwrap();
    std::fs::create_dir(dir.join("no hooks")).unwrap();
    git(dir, &["init", "--quiet"]);
    for (key, value) in [
        ("user.name", "rtm test"),
        ("user.email", "rtm@example.com"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", dir.join("no hooks").to_str().unwrap()),
        ("rerere.enabled", "false"),
    ] {
        git(dir, &["config", key, value]);
    }
    git(dir, &["add", "todo.txt"]);
    git(dir, &["commit", "--quiet", "-m", "start"]);
    source
}

/// A not-synced source whose todo.txt is a symbolic link to `real.txt` holding `lines`, and the
/// message refusing it.
#[cfg(unix)]
pub(super) fn symlinked_source_with(lines: &str) -> (Source, String) {
    let source = source_with(lines);
    let target = source.path.with_file_name("real.txt");
    std::fs::rename(&source.path, &target).unwrap();
    std::os::unix::fs::symlink(&target, &source.path).unwrap();
    // canonicalize: the temp directory is reached through a symlink on macOS.
    let target = target.canonicalize().unwrap();
    let refusal = format!(
        "'{}' is a symbolic link to '{}'; open '{}' instead.",
        source.path.display(),
        target.display(),
        target.display()
    );
    (source, refusal)
}
