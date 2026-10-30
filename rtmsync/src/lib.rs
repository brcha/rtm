//! Synchronisation of a todo.txt file through the version control system it lives in.

use std::path::Path;

/// A version control system, recognised by the marker its repositories carry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Vcs {
    Git,
    Pijul,
}

impl Vcs {
    pub const ALL: [Vcs; 2] = [Vcs::Git, Vcs::Pijul];

    /// The entry that marks a repository.
    pub fn marker(self) -> &'static str {
        match self {
            Vcs::Git => ".git",
            Vcs::Pijul => ".pijul",
        }
    }
}

/// The version control systems managing `file`: the markers found in the nearest directory, from
/// the file's own up to the filesystem root, that has any. Empty when the file is not versioned.
/// More than one means the directory is ambiguous and the caller has to choose. Symlinks are
/// resolved first, so a link into a repository counts as versioned.
pub fn detect(file: &Path) -> Vec<Vcs> {
    let file = std::fs::canonicalize(file)
        .or_else(|_| std::path::absolute(file))
        .unwrap_or_else(|_| file.to_path_buf());
    let Some(dir) = file.parent() else {
        return Vec::new();
    };
    dir.ancestors()
        .map(|d| {
            // `exists`, not `is_dir`: `.git` is a file in linked worktrees and submodules.
            Vcs::ALL
                .into_iter()
                .filter(|vcs| d.join(vcs.marker()).exists())
                .collect::<Vec<_>>()
        })
        .find(|found| !found.is_empty())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rtmsync test {} {name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/todo.txt"), "").unwrap();
        dir
    }

    #[test]
    fn a_file_without_markers_is_not_versioned() {
        let dir = scratch("none");
        assert_eq!(detect(&dir.join("sub/todo.txt")), []);
    }

    #[test]
    fn every_marker_is_recognised_in_the_files_directory() {
        for vcs in Vcs::ALL {
            let dir = scratch(vcs.marker());
            std::fs::create_dir(dir.join("sub").join(vcs.marker())).unwrap();
            assert_eq!(detect(&dir.join("sub/todo.txt")), [vcs]);
        }
    }

    #[test]
    fn a_marker_above_the_file_is_found() {
        let dir = scratch("above");
        std::fs::create_dir(dir.join(".pijul")).unwrap();
        assert_eq!(detect(&dir.join("sub/todo.txt")), [Vcs::Pijul]);
    }

    #[test]
    fn a_git_file_marks_a_linked_worktree() {
        let dir = scratch("worktree");
        std::fs::write(dir.join(".git"), "gitdir: elsewhere\n").unwrap();
        assert_eq!(detect(&dir.join("sub/todo.txt")), [Vcs::Git]);
    }

    #[test]
    fn the_nearest_directory_with_a_marker_wins() {
        let dir = scratch("nearest");
        std::fs::create_dir(dir.join(".git")).unwrap();
        std::fs::create_dir(dir.join("sub/.pijul")).unwrap();
        assert_eq!(detect(&dir.join("sub/todo.txt")), [Vcs::Pijul]);
    }

    #[test]
    fn several_markers_in_one_directory_are_all_returned() {
        let dir = scratch("ambiguous");
        std::fs::create_dir(dir.join(".git")).unwrap();
        std::fs::create_dir(dir.join(".pijul")).unwrap();
        assert_eq!(detect(&dir.join("sub/todo.txt")), [Vcs::Git, Vcs::Pijul]);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_judged_by_its_target() {
        let dir = scratch("symlink");
        std::fs::create_dir(dir.join("sub/.git")).unwrap();
        let outside = scratch("symlink outside");
        std::os::unix::fs::symlink(dir.join("sub/todo.txt"), outside.join("todo.txt")).unwrap();
        assert_eq!(detect(&outside.join("todo.txt")), [Vcs::Git]);
    }
}
