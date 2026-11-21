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
