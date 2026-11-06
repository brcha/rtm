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

/// The file's lines now.
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
