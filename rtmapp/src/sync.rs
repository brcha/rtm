//! Sync of the open todo.txt through its Git or Pijul repository (RTM-64), with the same rules as
//! rtmcli: outside changes and an untracked file are asked about first (RTM-70), an ignored file
//! never syncs, a conflicted repository is not opened, a repository without a remote is committed
//! to without pulling or pushing. Every git or pijul call, the check on open included, runs on its
//! repository's worker thread, one per repository and VCS (RTM-126), so calls in one repository never overlap
//! each other; the app writes a file only while its repository's worker is idle (RTM-20), so they
//! never overlap a save either.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use eframe::egui;
use rtmsync::{State, Vcs};

use crate::config::AppConfig;

/// The repository a todo.txt syncs through.
pub struct Repo {
    vcs: Vcs,
    /// The repository's directory (`rtmsync::root`).
    root: PathBuf,
    file: PathBuf,
    remote: bool,
}

/// A repository's directory and VCS: one worker each, as two files in a directory holding both
/// may choose differently (the choice is per file).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoKey {
    pub root: PathBuf,
    pub vcs: Vcs,
}

/// Where a file syncs, from the config and the repository markers alone: no executables run.
pub enum Place {
    /// Sync is off, or the file is not in a repository.
    NoSync,
    /// The directory belongs to several VCSs and the config names none of them for the file.
    Ambiguous(Vec<Vcs>),
    /// The repository; its worker checks the file (`check`).
    Repo(RepoKey),
}

/// What the check on open found.
pub enum Found {
    /// The file is ignored (or untracked in a conflicted repository).
    NoSync,
    Repo(Repo),
    /// The file has changes rtm did not commit, or is not tracked yet: ask before committing.
    Ask(Repo, Question),
    /// Unmerged paths, a stopped rebase or merge, or Pijul conflict markers.
    Conflicted,
    /// A check failed, for example because `git` is not installed.
    Unavailable(String),
}

/// What to ask about a file before it syncs (RTM-57, RTM-70).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Question {
    Modified,
    Untracked,
}

/// The VCS the config names for `file` (`"git"`, `"pijul"`, any case). Anything else is no
/// choice.
pub fn chosen(config: &AppConfig, file: &Path) -> Option<Vcs> {
    match config
        .entry(file)?
        .vcs
        .as_deref()?
        .to_ascii_lowercase()
        .as_str()
    {
        "git" => Some(Vcs::Git),
        "pijul" => Some(Vcs::Pijul),
        _ => None,
    }
}

/// Where `file` syncs under `config`. Runs no executables, so it is quick enough for the UI
/// thread; the git or pijul part is `check`, which runs on the repository's worker.
pub fn place(file: &Path, config: &AppConfig) -> Place {
    if !config.sync {
        return Place::NoSync;
    }
    let Some(root) = rtmsync::root(file) else {
        return Place::NoSync;
    };
    let found = rtmsync::detect(file);
    match found.as_slice() {
        [] => Place::NoSync,
        [only] => Place::Repo(RepoKey { root, vcs: *only }),
        several => match chosen(config, file).filter(|vcs| several.contains(vcs)) {
            Some(vcs) => Place::Repo(RepoKey { root, vcs }),
            None => Place::Ambiguous(found),
        },
    }
}

/// The check on open of `file` in the repository at `root` (RTM-57, RTM-70): its state, the
/// repository's conflicts and whether it has a remote.
pub fn check(vcs: Vcs, root: PathBuf, file: &Path) -> Found {
    let state = if file.exists() {
        vcs.state(file)
    } else {
        // A missing file's state cannot be told reliably (a deleted tracked file is modified for
        // Git), so it keeps the rule from before the check, as in rtmcli: tracked files sync.
        vcs.is_tracked(file).map(|tracked| {
            if tracked {
                State::Clean
            } else {
                State::Ignored
            }
        })
    };
    let question = match state {
        Ok(State::Clean) => None,
        Ok(State::Modified) => Some(Question::Modified),
        Ok(State::Untracked) => Some(Question::Untracked),
        Ok(State::Ignored) => return Found::NoSync,
        Err(e) => return Found::Unavailable(e.to_string()),
    };
    match vcs.is_conflicted(file) {
        Ok(false) => {}
        // Nothing can be committed into the repository, so an untracked file opens as before.
        Ok(true) if question == Some(Question::Untracked) => return Found::NoSync,
        Ok(true) => return Found::Conflicted,
        Err(e) => return Found::Unavailable(e.to_string()),
    }
    let repo = Repo {
        vcs,
        root,
        file: file.to_path_buf(),
        remote: vcs.has_remote(file).unwrap_or(false),
    };
    match question {
        None => Found::Repo(repo),
        Some(question) => Found::Ask(repo, question),
    }
}

enum Job {
    /// The check on open, run here so it never meets a pull of the same repository half-way.
    Check { root: PathBuf, file: PathBuf },
    /// `lock`: the write lock the app took for the write being committed; the worker takes it
    /// itself when there is none.
    Commit {
        file: PathBuf,
        message: String,
        lock: Option<todotxt::WriteLock>,
    },
    /// Pull and push; the files are the repository's open ones, whose changes are reported.
    Sync(Vec<PathBuf>),
}

/// The result of one job.
pub enum Done {
    /// The check on open of `file`.
    Checked {
        file: PathBuf,
        found: Found,
    },
    Committed,
    /// Synced (nothing to do without a remote). `changed`: the files that differ from before the
    /// job.
    Synced {
        changed: Vec<PathBuf>,
    },
    /// The pull or push failed (offline, timeout, rejected push); local commits wait for the next.
    /// `changed`: the files the pull changed before the push failed.
    NotSynced {
        error: String,
        changed: Vec<PathBuf>,
    },
    /// The repository now has conflicts to resolve by hand.
    Conflict,
    /// The commit of `file` failed; the file is saved.
    NotCommitted {
        file: PathBuf,
        error: String,
    },
}

/// The worker thread of one repository and its queues.
pub struct Worker {
    jobs: Sender<Job>,
    results: Receiver<(bool, Done)>,
    /// Jobs sent and not yet reported back through `poll` or `next`: a finished job whose result
    /// was not handled still counts, so the app never writes the file over an unhandled pull.
    pending: usize,
    syncs_pending: usize,
    /// Whether the repository has a remote, as a check on open found; without one there is
    /// nothing to sync.
    remote: bool,
}

impl Worker {
    /// Starts the worker for a repository of `vcs`. It asks `ctx` to repaint after each job, so
    /// the app picks the result up without waiting for input. It has no remote until a check on
    /// open finds one (`found_remote`).
    pub fn start(vcs: Vcs, ctx: egui::Context) -> Worker {
        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (result_tx, results) = mpsc::channel();
        std::thread::spawn(move || {
            for job in job_rx {
                let sync = matches!(job, Job::Sync(_));
                let done = match job {
                    Job::Check { root, file } => {
                        // Another rtm process between its save and its commit would look like
                        // an outside change; its lock is held until it has committed (RTM-142).
                        let _lock = todotxt::WriteLock::acquire(&rtmsync::lock_target(&file));
                        Done::Checked {
                            found: check(vcs, root, &file),
                            file,
                        }
                    }
                    Job::Commit {
                        file,
                        message,
                        lock,
                    } => {
                        // Held until the commit is done, then released.
                        let lock = lock.or_else(|| {
                            todotxt::WriteLock::acquire(&rtmsync::lock_target(&file)).ok()
                        });
                        let committed = vcs.commit(&file, &message);
                        drop(lock);
                        match committed {
                            Ok(()) => Done::Committed,
                            Err(e) => Done::NotCommitted {
                                file,
                                error: e.to_string(),
                            },
                        }
                    }
                    Job::Sync(files) => sync_files(vcs, &files),
                };
                if result_tx.send((sync, done)).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Worker {
            jobs,
            results,
            pending: 0,
            syncs_pending: 0,
            remote: false,
        }
    }

    /// Queues the check on open of `file` in the repository at `root`.
    pub fn check(&mut self, root: PathBuf, file: PathBuf) {
        if self.jobs.send(Job::Check { root, file }).is_ok() {
            self.pending += 1;
        }
    }

    /// Records what a check on open found about the repository's remote.
    pub fn found_remote(&mut self, remote: bool) {
        self.remote |= remote;
    }

    /// Queues a commit of the saved `file` with `message`. Local only: the next sync pushes it.
    /// `lock` is the write lock taken for the write, released once the commit is done; without
    /// one the worker takes it.
    pub fn commit(&mut self, file: PathBuf, message: String, lock: Option<todotxt::WriteLock>) {
        if self
            .jobs
            .send(Job::Commit {
                file,
                message,
                lock,
            })
            .is_ok()
        {
            self.pending += 1;
        }
    }

    /// Queues a pull and push that reports which of `files` it changed, unless one is already
    /// waiting or there is no remote.
    pub fn sync(&mut self, files: Vec<PathBuf>) {
        if self.remote && self.syncs_pending == 0 && self.jobs.send(Job::Sync(files)).is_ok() {
            self.pending += 1;
            self.syncs_pending += 1;
        }
    }

    pub fn busy(&self) -> bool {
        self.pending > 0
    }

    pub fn has_remote(&self) -> bool {
        self.remote
    }

    pub fn syncing(&self) -> bool {
        self.syncs_pending > 0
    }

    /// The results of the jobs finished since the last call.
    pub fn poll(&mut self) -> Vec<Done> {
        let mut done = Vec::new();
        while let Ok(result) = self.results.try_recv() {
            done.push(self.received(result));
        }
        done
    }

    /// The next result, waiting at most `within` for it.
    pub fn next(&mut self, within: Duration) -> Option<Done> {
        let result = self.results.recv_timeout(within).ok()?;
        Some(self.received(result))
    }

    fn received(&mut self, (sync, done): (bool, Done)) -> Done {
        self.pending -= 1;
        if sync {
            self.syncs_pending -= 1;
        }
        done
    }
}

impl Repo {
    pub fn vcs(&self) -> Vcs {
        self.vcs
    }

    pub fn remote(&self) -> bool {
        self.remote
    }

    /// What identifies the repository's worker.
    pub fn key(&self) -> RepoKey {
        RepoKey {
            root: self.root.clone(),
            vcs: self.vcs,
        }
    }

    /// rtmcli's message for committing what `question` asked about (RTM-63).
    pub fn commit_message(&self, question: Question) -> String {
        match question {
            Question::Modified => "rtm: commit outside changes".to_owned(),
            Question::Untracked => format!(
                "rtm: track {}",
                self.file.file_name().unwrap_or_default().to_string_lossy()
            ),
        }
    }
}

/// Pulls and pushes (queued only when the repository has a remote). The pull is for the whole
/// repository; each of `files` is then checked for conflicts (Pijul writes its markers into the
/// files) and compared with what it held before.
fn sync_files(vcs: Vcs, files: &[PathBuf]) -> Done {
    let Some(first) = files.first() else {
        return Done::Synced {
            changed: Vec::new(),
        };
    };
    // A pull rewrites the repository's files: no other rtm process may be writing one meanwhile.
    let _lock = todotxt::WriteLock::acquire(&rtmsync::lock_target(first));
    let before: Vec<_> = files.iter().map(|file| std::fs::read(file).ok()).collect();
    let result = vcs.sync(first);
    if matches!(result, Err(rtmsync::Error::Conflict))
        || files
            .iter()
            .any(|file| vcs.is_conflicted(file).unwrap_or(false))
    {
        return Done::Conflict;
    }
    let changed = files
        .iter()
        .zip(before)
        .filter(|(file, before)| std::fs::read(file).ok() != *before)
        .map(|(file, _)| file.clone())
        .collect();
    match result {
        Ok(()) => Done::Synced { changed },
        Err(e) => Done::NotSynced {
            error: e.to_string(),
            changed,
        },
    }
}
