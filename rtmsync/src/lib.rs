//! Synchronisation of a todo.txt file through the version control system it lives in.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// How long a network call (pull, push) may run before it is killed.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(60);

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
    /// The pull left the repository conflicted; the user resolves it by hand before syncing again.
    #[error("the repository has conflicts to resolve by hand")]
    Conflict,
    /// A network call ran too long and was killed.
    #[error("{program} timed out")]
    Timeout { program: &'static str },
}

/// The state of a file in its repository, which rtm checks when it opens the file (RTM-63).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    /// Tracked, with nothing uncommitted.
    Clean,
    /// Tracked, with uncommitted changes (staged or not), or added and never committed.
    Modified,
    /// Not tracked, and not ignored.
    Untracked,
    /// Excluded by the repository's ignore rules (`.gitignore`, Pijul's `.ignore`).
    Ignored,
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

    /// Whether `file` is tracked by its repository.
    pub fn is_tracked(self, file: &Path) -> Result<bool, Error> {
        let file = resolve(file);
        let dir = file.parent().unwrap_or(Path::new("."));
        match self {
            Vcs::Git => {
                let args = ["--literal-pathspecs", "ls-files", "--error-unmatch"];
                let name = file.file_name().unwrap_or_default();
                succeeded(run("git", dir, &args, name))
            }
            Vcs::Pijul => {
                let Some(path) = pijul_path(&file) else {
                    return Ok(false);
                };
                let listed = output("pijul", command("pijul", dir).arg("ls"))?;
                Ok(listed.lines().any(|line| line == path))
            }
        }
    }

    /// The state of `file` in its repository. A missing file is not classified reliably: Git
    /// reports a deleted tracked file as `Modified`, Pijul a missing untracked one as `Ignored`.
    pub fn state(self, file: &Path) -> Result<State, Error> {
        let tracked = self.is_tracked(file)?;
        let file = resolve(file);
        let dir = file.parent().unwrap_or(Path::new("."));
        let name = file.file_name().unwrap_or_default();
        match (self, tracked) {
            (Vcs::Git, true) => {
                let mut status = command("git", dir);
                status
                    .args(["--literal-pathspecs", "status", "--porcelain=v2", "--"])
                    .arg(name);
                let changed = output("git", &mut status)?
                    .lines()
                    .any(|line| ["1 ", "2 ", "u "].iter().any(|p| line.starts_with(p)));
                Ok(if changed {
                    State::Modified
                } else {
                    State::Clean
                })
            }
            // Status shows nothing for an ignored file; check-ignore exits 1 for one not ignored.
            // It takes path names, not pathspecs, and refuses `--literal-pathspecs`.
            (Vcs::Git, false) => {
                let args = ["check-ignore", "-q"];
                match run("git", dir, &args, name) {
                    Ok(()) => Ok(State::Ignored),
                    Err(Error::Failed { status, .. }) if status.code() == Some(1) => {
                        Ok(State::Untracked)
                    }
                    Err(e) => Err(e),
                }
            }
            (Vcs::Pijul, true) => {
                let mut diff = command("pijul", dir);
                diff.args(["diff", "--short", "--"]).arg(name);
                let changed = !output("pijul", &mut diff)?.trim().is_empty();
                Ok(if changed {
                    State::Modified
                } else {
                    State::Clean
                })
            }
            // Lists untracked files that are not ignored, and leaves ignored ones out; `pijul add`
            // cannot tell, it reports success for an ignored file. Such a file shows as `U <path>`
            // at the repository root and as `A <path>` below it (pijul 1.0.0-beta.18).
            (Vcs::Pijul, false) => {
                let path = pijul_path(&file).unwrap_or_default();
                let listed = output(
                    "pijul",
                    command("pijul", dir).args(["diff", "--short", "--untracked", "."]),
                )?;
                let untracked = listed.lines().any(|line| {
                    let listed = line.strip_prefix("A ").or_else(|| line.strip_prefix("U "));
                    listed.map(str::trim_start) == Some(&path)
                });
                Ok(if untracked {
                    State::Untracked
                } else {
                    State::Ignored
                })
            }
        }
    }

    /// Whether the repository holding `file` is in a state rtm must leave to the user: unmerged
    /// paths or a rebase or merge stopped half-way (Git), or conflict markers in `file` (Pijul,
    /// which writes them into the file on a conflicting pull). A conflicted todo.txt would be read
    /// with its markers as items.
    pub fn is_conflicted(self, file: &Path) -> Result<bool, Error> {
        let file = resolve(file);
        let dir = file.parent().unwrap_or(Path::new("."));
        match self {
            Vcs::Git => {
                let status = output(
                    "git",
                    command("git", dir).args(["status", "--porcelain=v2"]),
                )?;
                Ok(status.lines().any(|line| line.starts_with("u ")) || git_stopped(dir)?)
            }
            Vcs::Pijul => {
                let text = std::fs::read_to_string(&file).unwrap_or_default();
                let marker = |line: &str| {
                    [">>>>>>> ", "======= ", "<<<<<<< "]
                        .iter()
                        .any(|m| line.starts_with(m))
                };
                Ok(text.lines().any(marker))
            }
        }
    }

    /// Whether the repository holding `file` has a remote to sync with: an upstream branch for
    /// Git, any remote for Pijul.
    pub fn has_remote(self, file: &Path) -> Result<bool, Error> {
        let file = resolve(file);
        let dir = file.parent().unwrap_or(Path::new("."));
        match self {
            Vcs::Git => {
                let args = [
                    "rev-parse",
                    "--abbrev-ref",
                    "--symbolic-full-name",
                    "@{upstream}",
                ];
                succeeded(output("git", command("git", dir).args(args)).map(drop))
            }
            // Prints nothing in a repository without remotes.
            Vcs::Pijul => Ok(!output("pijul", command("pijul", dir).arg("remote"))?
                .trim()
                .is_empty()),
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

    /// Syncs the repository holding `file` with its remote: pulls, then pushes if the pull applied
    /// cleanly. Git rebases local commits onto the remote. A pull that leaves conflicts is
    /// `Conflict`, and so is a rebase or merge already stopped, even with every path resolved; rtm
    /// leaves them for the user.
    pub fn sync(self, file: &Path) -> Result<(), Error> {
        let file = resolve(file);
        let dir = file.parent().unwrap_or(Path::new("."));
        let network = |program, args: &[&str]| {
            let mut command = command(program, dir);
            // Credentials come from a helper or ssh-agent; git never asks on the terminal.
            command.args(args).env("GIT_TERMINAL_PROMPT", "0");
            output_within(program, &mut command, NETWORK_TIMEOUT)
        };
        match self {
            Vcs::Git => {
                let pulled = network("git", &["pull", "--rebase", "--autostash"]);
                // Also catches an autostash that did not reapply cleanly.
                if self.is_conflicted(&file)? {
                    return Err(Error::Conflict);
                }
                pulled?;
                network("git", &["push"])
            }
            Vcs::Pijul => {
                // Pijul applies conflicting changes, exits 0 and writes markers into the file. A
                // pull cut short may have written them too, so they are checked either way.
                let pulled = network("pijul", &["pull", "--all"]);
                if self.is_conflicted(&file)? {
                    return Err(Error::Conflict);
                }
                pulled?;
                network("pijul", &["push", "--all"])
            }
        }
    }
}

/// `true` for success, `false` for a program that ran and said no; other errors pass through.
fn succeeded(result: Result<(), Error>) -> Result<bool, Error> {
    match result {
        Ok(()) => Ok(true),
        Err(Error::Failed { .. }) => Ok(false),
        Err(e) => Err(e),
    }
}

/// `file` relative to the root of its Pijul repository, joined with `/`, as pijul lists paths.
fn pijul_path(file: &Path) -> Option<String> {
    let root = file
        .ancestors()
        .skip(1)
        .find(|d| d.join(".pijul").exists())?;
    let relative = file.strip_prefix(root).unwrap_or(file);
    let relative: Vec<_> = relative.iter().map(|c| c.to_string_lossy()).collect();
    Some(relative.join("/"))
}

/// Whether the git repository at `dir` has a rebase or merge stopped half-way. Its paths can all
/// be resolved (no unmerged entries) while the rebase still waits for `git rebase --continue`.
fn git_stopped(dir: &Path) -> Result<bool, Error> {
    let paths = output(
        "git",
        command("git", dir).args([
            "rev-parse",
            "--git-path",
            "rebase-merge",
            "--git-path",
            "rebase-apply",
            "--git-path",
            "MERGE_HEAD",
        ]),
    )?;
    Ok(paths.lines().any(|path| dir.join(path).exists()))
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
    let mut command = command(program, dir);
    command.args(args).arg("--").arg(file);
    output(program, &mut command).map(drop)
}

/// A command for `program` in `dir`.
fn command(program: &str, dir: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .current_dir(dir)
        // A prompt that reads stdin fails instead of waiting; prompts on the terminal still can.
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Runs `command` to completion and returns its stdout.
/// A run blocked by another git process's `index.lock` (the app and the CLI at once) is retried
/// for about 3 s; the lock is never removed.
fn output(program: &'static str, command: &mut Command) -> Result<String, Error> {
    let mut wait = Duration::from_millis(100);
    let mut waited = Duration::ZERO;
    loop {
        let output = command
            .output()
            .map_err(|source| Error::Spawn { program, source })?;
        // The lock's file name in git's message is never translated, so this is locale-free.
        let locked = !output.status.success()
            && String::from_utf8_lossy(&output.stderr).contains("index.lock");
        if locked && waited < Duration::from_secs(3) {
            std::thread::sleep(wait);
            waited += wait;
            wait *= 2;
            continue;
        }
        check(program, output.status, &output.stderr)?;
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
}

/// Runs `command`, killing it after `timeout`. Its stdout is discarded and its stderr goes to a
/// temporary file rather than a pipe: children such as `ssh` can outlive the program and hold a
/// pipe open, so reading one to its end could block long after the program exited or was killed.
fn output_within(
    program: &'static str,
    command: &mut Command,
    timeout: Duration,
) -> Result<(), Error> {
    let spawn = |source| Error::Spawn { program, source };
    let (stderr_path, stderr) = private_temp_file().map_err(spawn)?;
    let result = (|| {
        let mut child = command
            .stdout(Stdio::null())
            .stderr(stderr)
            .spawn()
            .map_err(spawn)?;
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().map_err(spawn)? {
                break status;
            }
            if started.elapsed() > timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::Timeout { program });
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        check(
            program,
            status,
            &std::fs::read(&stderr_path).unwrap_or_default(),
        )
    })();
    let _ = std::fs::remove_file(&stderr_path);
    result
}

/// A new file in the temporary directory, readable only by this user. It is never an existing
/// file or symlink: the directory is shared, and its names are predictable.
fn private_temp_file() -> std::io::Result<(PathBuf, std::fs::File)> {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut tries = 0;
    loop {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("rtmsync-{}-{n}.stderr", std::process::id()));
        match options.open(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && tries < 100 => tries += 1,
            opened => return opened.map(|file| (path, file)),
        }
    }
}

/// `Failed` with `stderr` unless `status` is a success.
fn check(program: &'static str, status: ExitStatus, stderr: &[u8]) -> Result<(), Error> {
    if status.success() {
        Ok(())
    } else {
        Err(Error::Failed {
            program,
            status,
            stderr: String::from_utf8_lossy(stderr).trim().to_owned(),
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
}
