//! Synchronisation of a todo.txt file through the version control system it lives in.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

/// A failure to run a version control system's executable.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The executable could not be started; `NotFound` means it is not in `PATH`.
    #[error("cannot run {program}")]
    Spawn {
        program: &'static str,
        #[source]
        source: std::io::Error,
    },
    /// The executable ran and failed.
    #[error("{program} failed ({status}): {stderr}")]
    Failed {
        program: &'static str,
        status: ExitStatus,
        stderr: String,
    },
}

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

    /// Commits the current content of `file`, and nothing else in its repository, with `message`.
    /// An untracked file is added first; an unchanged file commits nothing.
    pub fn commit(self, file: &Path, message: &str) -> Result<(), Error> {
        let file = resolve(file);
        let dir = file.parent().unwrap_or(Path::new("."));
        let name = file.file_name().unwrap_or_default();
        let message = format!("--message={message}");
        match self {
            Vcs::Git => {
                run("git", dir, &["--literal-pathspecs", "add"], name)?;
                let staged = ["--literal-pathspecs", "diff", "--cached", "--quiet"];
                if run("git", dir, &staged, name).is_ok() {
                    return Ok(());
                }
                // A commit with a pathspec takes only that path, not what else is staged.
                run(
                    "git",
                    dir,
                    &["--literal-pathspecs", "commit", &message],
                    name,
                )
            }
            Vcs::Pijul => {
                run("pijul", dir, &["add"], name)?;
                // Records nothing, successfully, when the file is unchanged.
                run("pijul", dir, &["record", "--all", &message], name)
            }
        }
    }
}

/// `file` with its symlinks resolved, or made absolute when it does not exist.
fn resolve(file: &Path) -> PathBuf {
    let Ok(resolved) = std::fs::canonicalize(file) else {
        return std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
    };
    // Windows canonicalizes to a `\\?\C:\` path, which git does not accept as a working directory.
    match resolved.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(plain) if plain.as_bytes().get(1) == Some(&b':') => PathBuf::from(plain),
        _ => resolved,
    }
}

/// Runs `program` with `args`, then `--` and `file`, in `dir`.
fn run(program: &'static str, dir: &Path, args: &[&str], file: &OsStr) -> Result<(), Error> {
    let mut command = Command::new(program);
    command
        .current_dir(dir)
        .args(args)
        .arg("--")
        .arg(file)
        // A prompt that reads stdin fails instead of waiting; prompts on the terminal still can.
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command
        .output()
        .map_err(|source| Error::Spawn { program, source })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(Error::Failed {
            program,
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}

/// The version control systems managing `file`: the markers found in the nearest directory, from
/// the file's own up to the filesystem root, that has any. Empty when the file is not versioned.
/// More than one means the directory is ambiguous and the caller has to choose. Symlinks are
/// resolved first, so a link into a repository counts as versioned.
pub fn detect(file: &Path) -> Vec<Vcs> {
    let file = resolve(file);
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
