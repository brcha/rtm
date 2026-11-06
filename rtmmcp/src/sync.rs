//! Syncing a change through the repository holding the file, with rtmcli's rules (see
//! `rtmcli/AGENTS.md`), returning what the user must fix instead of exiting: the server cannot ask
//! the user, so an outside change or a conflict refuses the change, with a message for the client.
//!
//! A write is: `RepoWrite::find`, `pull`, load, change, save, `commit_and_push`. Problems that do not
//! stop the change (a failed pull or push, an untracked file) become notes in the tool's result.

use std::path::{Path, PathBuf};

use rtmsync::{State, Vcs};

use crate::Source;

/// How one write of a file goes through its repository.
pub(crate) struct RepoWrite {
    repo: Option<Repo>,
    /// What the client is told besides the result: why the file is not synced, a failed pull.
    pub(crate) notes: Vec<String>,
}

/// The repository a file syncs through.
struct Repo {
    vcs: Vcs,
    file: PathBuf,
    remote: bool,
}

impl RepoWrite {
    /// Checks the file's repository before a write. `Err` is the message refusing the change: a
    /// directory with both Git and Pijul and no choice, a conflicted repository, or changes made
    /// outside rtm.
    pub(crate) fn find(source: &Source) -> Result<RepoWrite, String> {
        let mut sync = RepoWrite {
            repo: None,
            notes: Vec::new(),
        };
        if !source.sync {
            return Ok(sync);
        }
        let file = &source.path;
        let found = rtmsync::detect(file);
        let vcs = match (found.as_slice(), source.vcs) {
            ([], _) => return Ok(sync),
            ([only], None) => *only,
            (found, Some(chosen)) if found.contains(&chosen) => chosen,
            ([only], Some(chosen)) => {
                return Err(format!(
                    "'{}' is in a {only:?} repository, not {chosen:?}.",
                    file.display()
                ));
            }
            (found, _) => {
                return Err(format!(
                    "'{}' is in a directory used by {found:?}; the user has to choose which one \
                     to sync through (in rtmapp, or with rtmcli's --vcs).",
                    file.display()
                ));
            }
        };
        let state = if file.exists() {
            vcs.state(file)
        } else {
            // A missing file's state cannot be told reliably: synced when tracked, as in rtmcli.
            vcs.is_tracked(file).map(|tracked| {
                if tracked {
                    State::Clean
                } else {
                    State::Ignored
                }
            })
        };
        let modified = match state {
            Ok(State::Clean) => false,
            Ok(State::Modified) => true,
            Ok(State::Ignored) => return Ok(sync),
            Ok(State::Untracked) => {
                sync.notes.push(format!(
                    "'{}' is not tracked by {vcs:?}, so this change is not synced. The user can \
                     add it when rtmapp asks, or with rtmcli's --commit-changes.",
                    file.display()
                ));
                return Ok(sync);
            }
            Err(e) => {
                sync.notes.push(format!("Not syncing: {e}"));
                return Ok(sync);
            }
        };
        // Before the outside changes: during a stopped rebase the file counts as modified, and a
        // commit then would land in the middle of the rebase.
        match vcs.is_conflicted(file) {
            Ok(false) => {}
            Ok(true) => return Err(conflicted(file)),
            Err(e) => {
                sync.notes.push(format!("Not syncing: {e}"));
                return Ok(sync);
            }
        }
        if modified {
            return Err(format!(
                "'{}' has changes not committed by rtm (made by hand, or with sync off). Nothing \
                 was changed. The user has to commit or revert them first (rtmapp offers to when \
                 it opens the file, rtmcli has --commit-changes).",
                file.display()
            ));
        }
        sync.repo = Some(Repo {
            vcs,
            file: file.clone(),
            remote: vcs.has_remote(file).unwrap_or(false),
        });
        Ok(sync)
    }

    /// Pulls (and pushes) when there is a remote, so the change applies to the latest file. A
    /// failure other than a conflict is a note; a conflict refuses the change.
    pub(crate) fn pull(&mut self) -> Result<(), String> {
        let Some(repo) = &self.repo else {
            return Ok(());
        };
        if !repo.remote {
            return Ok(());
        }
        match repo.vcs.sync(&repo.file) {
            Ok(()) => Ok(()),
            Err(rtmsync::Error::Conflict) => Err(conflicted(&repo.file)),
            Err(e) => {
                self.notes.push(format!("Not synced: {e}"));
                Ok(())
            }
        }
    }

    /// Commits the saved file with `message`, then pulls and pushes when there is a remote. The
    /// change is saved by then, so nothing here fails the tool (a client might repeat it): a
    /// failed commit or push, and a conflict on the pull, are notes.
    pub(crate) fn commit_and_push(&mut self, message: &str) {
        let Some(repo) = &self.repo else {
            return;
        };
        if let Err(e) = repo.vcs.commit(&repo.file, message) {
            self.notes.push(format!("Not committed: {e}"));
            return;
        }
        if let Err(e) = self.pull() {
            self.notes.push(format!(
                "The change was saved and committed, but not synced. {e}"
            ));
        }
    }
}

/// The message for a repository the user has to resolve by hand.
fn conflicted(file: &Path) -> String {
    format!(
        "The repository holding '{}' has conflicts or a stopped rebase or merge. Nothing more \
         can be synced until the user resolves them by hand.",
        file.display()
    )
}

#[cfg(test)]
#[path = "tests/sync_git_tests.rs"]
mod git_tests;
