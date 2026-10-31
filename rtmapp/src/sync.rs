//! Sync of the open todo.txt through its Git or Pijul repository (RTM-64), with the same rules as
//! rtmcli: outside changes and an untracked file are asked about first (RTM-70), an ignored file
//! never syncs, a conflicted repository is not opened, a repository without a remote is committed
//! to without pulling or pushing. Every git or pijul call after opening runs on one worker thread,
//! so calls never overlap each other; the app writes the file only while the worker is idle
//! (RTM-20), so they never overlap a save either.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use eframe::egui;
use rtmsync::{State, Vcs};

use crate::config::AppConfig;

/// The repository a todo.txt syncs through.
pub struct Repo {
    vcs: Vcs,
    file: PathBuf,
    remote: bool,
}

/// What opening a file means for sync.
pub enum Found {
    /// Sync is off, the file is not in a repository, or it is ignored (or untracked in a
    /// conflicted repository).
    NoSync,
    Repo(Repo),
    /// The file has changes rtm did not commit, or is not tracked yet: ask before committing.
    Ask(Repo, Question),
    /// The directory belongs to several VCSs and the config names none of them.
    Ambiguous(Vec<Vcs>),
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

/// The VCS named in the config (`"git"`, `"pijul"`, any case). Anything else is no choice.
pub fn chosen(config: &AppConfig) -> Option<Vcs> {
    match config.vcs.as_deref()?.to_ascii_lowercase().as_str() {
        "git" => Some(Vcs::Git),
        "pijul" => Some(Vcs::Pijul),
        _ => None,
    }
}

/// How `file` syncs under `config`.
pub fn find(file: &Path, config: &AppConfig) -> Found {
    if !config.sync {
        return Found::NoSync;
    }
    let found = rtmsync::detect(file);
    let vcs = match found.as_slice() {
        [] => return Found::NoSync,
        [only] => *only,
        several => match chosen(config).filter(|vcs| several.contains(vcs)) {
            Some(vcs) => vcs,
            None => return Found::Ambiguous(found),
        },
    };
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
        file: file.to_path_buf(),
        remote: vcs.has_remote(file).unwrap_or(false),
    };
    match question {
        None => Found::Repo(repo),
        Some(question) => Found::Ask(repo, question),
    }
}

enum Job {
    Commit(String),
    Sync,
}

/// The result of one job.
pub enum Done {
    Committed,
    /// Synced (nothing to do without a remote). `changed`: the file differs from before the job.
    Synced {
        changed: bool,
    },
    /// The pull or push failed (offline, timeout, rejected push); local commits wait for the next.
    /// `changed`: the pull got through and changed the file before the push failed.
    NotSynced {
        error: String,
        changed: bool,
    },
    /// The repository now has conflicts to resolve by hand.
    Conflict,
    /// The commit failed; the file is saved.
    NotCommitted(String),
}

/// The worker thread and its queues.
pub struct Worker {
    jobs: Sender<Job>,
    results: Receiver<(bool, Done)>,
    /// Jobs sent and not yet reported back through `poll` or `next`: a finished job whose result
    /// was not handled still counts, so the app never writes the file over an unhandled pull.
    pending: usize,
    syncs_pending: usize,
    /// Whether the repository has a remote; without one there is nothing to sync.
    remote: bool,
}

impl Worker {
    /// Starts the worker for `repo`. It asks `ctx` to repaint after each job, so the app picks
    /// the result up without waiting for input.
    pub fn start(repo: Repo, ctx: egui::Context) -> Worker {
        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (result_tx, results) = mpsc::channel();
        let remote = repo.remote;
        std::thread::spawn(move || {
            for job in job_rx {
                let sync = matches!(job, Job::Sync);
                let done = match job {
                    Job::Commit(message) => match repo.vcs.commit(&repo.file, &message) {
                        Ok(()) => Done::Committed,
                        Err(e) => Done::NotCommitted(e.to_string()),
                    },
                    Job::Sync => repo.sync(),
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
            remote,
        }
    }

    /// Queues a commit of the saved file with `message`. Local only: the next sync pushes it.
    pub fn commit(&mut self, message: String) {
        if self.jobs.send(Job::Commit(message)).is_ok() {
            self.pending += 1;
        }
    }

    /// Queues a pull and push, unless one is already waiting or there is no remote.
    pub fn sync(&mut self) {
        if self.remote && self.syncs_pending == 0 && self.jobs.send(Job::Sync).is_ok() {
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

    /// Pulls and pushes when there is a remote; otherwise there is nothing to do.
    fn sync(&self) -> Done {
        if !self.remote {
            return Done::Synced { changed: false };
        }
        let before = std::fs::read(&self.file).ok();
        let result = self.vcs.sync(&self.file);
        let changed = std::fs::read(&self.file).ok() != before;
        match result {
            Ok(()) => Done::Synced { changed },
            Err(rtmsync::Error::Conflict) => Done::Conflict,
            Err(e) => Done::NotSynced {
                error: e.to_string(),
                changed,
            },
        }
    }
}
