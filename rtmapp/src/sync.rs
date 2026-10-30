//! Sync of the open todo.txt through its Git or Pijul repository (RTM-64), with the same rules as
//! rtmcli: only a tracked file syncs, a conflicted repository is not opened, a repository without a
//! remote is committed to without pulling or pushing. Every git or pijul call runs on one worker
//! thread, so calls never overlap each other; the app disables changes while the worker is busy,
//! so they never overlap a save either.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui;
use rtmsync::Vcs;

use crate::config::AppConfig;

/// The repository a todo.txt syncs through.
pub struct Repo {
    vcs: Vcs,
    file: PathBuf,
    remote: bool,
}

/// What opening a file means for sync.
pub enum Found {
    /// Sync is off, the file is not in a repository, or it is not tracked.
    NoSync,
    Repo(Repo),
    /// The directory belongs to several VCSs and the config names none of them.
    Ambiguous(Vec<Vcs>),
    /// Unmerged paths, a stopped rebase or merge, or Pijul conflict markers.
    Conflicted,
    /// A check failed, for example because `git` is not installed.
    Unavailable(String),
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
    match vcs.is_tracked(file) {
        Ok(true) => {}
        Ok(false) => return Found::NoSync,
        Err(e) => return Found::Unavailable(e.to_string()),
    }
    match vcs.is_conflicted(file) {
        Ok(false) => {}
        Ok(true) => return Found::Conflicted,
        Err(e) => return Found::Unavailable(e.to_string()),
    }
    let remote = vcs.has_remote(file).unwrap_or(false);
    Found::Repo(Repo {
        vcs,
        file: file.to_path_buf(),
        remote,
    })
}

enum Job {
    Pull,
    CommitAndSync(String),
}

/// The result of one job.
pub enum Done {
    /// Synced (or committed without a remote). `changed`: the file differs from before the job.
    Synced { changed: bool },
    /// The pull or push failed (offline, timeout, rejected push); the change is committed.
    NotSynced(String),
    /// The repository now has conflicts to resolve by hand.
    Conflict,
    /// The commit failed; the file is saved.
    NotCommitted(String),
}

/// The worker thread and its queues.
pub struct Worker {
    jobs: Sender<Job>,
    results: Receiver<(bool, Done)>,
    /// Jobs sent and not yet reported back.
    pending: usize,
    pulls_pending: usize,
    /// Whether the repository has a remote; without one there is nothing to pull.
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
                let before = std::fs::read(&repo.file).ok();
                let pull = matches!(job, Job::Pull);
                let done = match job {
                    Job::Pull => repo.sync(),
                    Job::CommitAndSync(message) => match repo.vcs.commit(&repo.file, &message) {
                        Ok(()) => repo.sync(),
                        Err(e) => Done::NotCommitted(e.to_string()),
                    },
                };
                let done = match done {
                    Done::Synced { .. } => Done::Synced {
                        changed: std::fs::read(&repo.file).ok() != before,
                    },
                    other => other,
                };
                if result_tx.send((pull, done)).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Worker {
            jobs,
            results,
            pending: 0,
            pulls_pending: 0,
            remote,
        }
    }

    /// Queues a pull, unless one is already waiting or there is no remote.
    pub fn pull(&mut self) {
        if self.remote && self.pulls_pending == 0 && self.jobs.send(Job::Pull).is_ok() {
            self.pending += 1;
            self.pulls_pending += 1;
        }
    }

    /// Queues a commit of the saved file with `message`, then a sync.
    pub fn commit_and_sync(&mut self, message: String) {
        if self.jobs.send(Job::CommitAndSync(message)).is_ok() {
            self.pending += 1;
        }
    }

    pub fn busy(&self) -> bool {
        self.pending > 0
    }

    /// The results of the jobs finished since the last call.
    pub fn poll(&mut self) -> Vec<Done> {
        let mut done = Vec::new();
        while let Ok((pull, result)) = self.results.try_recv() {
            self.pending -= 1;
            if pull {
                self.pulls_pending -= 1;
            }
            done.push(result);
        }
        done
    }

    /// Waits, at most `within`, for the queued jobs to finish, so a change saved just before the
    /// window closes is still committed.
    pub fn finish(&mut self, within: Duration) {
        let deadline = Instant::now() + within;
        while self.busy() {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.results.recv_timeout(left) {
                Ok((pull, _)) => {
                    self.pending -= 1;
                    if pull {
                        self.pulls_pending -= 1;
                    }
                }
                Err(_) => break,
            }
        }
    }
}

impl Repo {
    /// Pulls and pushes when there is a remote; otherwise there is nothing to do.
    fn sync(&self) -> Done {
        if !self.remote {
            return Done::Synced { changed: false };
        }
        match self.vcs.sync(&self.file) {
            Ok(()) => Done::Synced { changed: false },
            Err(rtmsync::Error::Conflict) => Done::Conflict,
            Err(e) => Done::NotSynced(e.to_string()),
        }
    }
}
