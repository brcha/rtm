//! The update check (RTM-138): a few seconds after start, then daily, on a thread of its own.
//! A newer release becomes the notice under the header; a failure is only logged. Without the
//! `update-check` feature (the Nix package) the check is never enabled.

use std::sync::mpsc::{self, TryRecvError};
use std::time::{Duration, Instant};

use eframe::egui;

/// How long after start the first check waits, so it never slows the start.
const FIRST_DELAY: Duration = Duration::from_secs(3);
const INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// A newer release the notice offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub version: String,
    /// The release's news post, which Download opens.
    pub url: String,
}

type Answer = Result<Option<Notice>, String>;

/// The release list's URL for the app; empty without the feature, which never checks.
pub fn url() -> String {
    #[cfg(feature = "update-check")]
    return rtmupdate::url();
    #[cfg(not(feature = "update-check"))]
    String::new()
}

pub struct UpdateCheck {
    /// The release list's URL: `rtmupdate::url()`, or a local server in the tests.
    url: String,
    /// When the next check is due; none while the check is off.
    due: Option<Instant>,
    running: Option<mpsc::Receiver<Answer>>,
    pub notice: Option<Notice>,
    /// The notice was closed: it stays away for the rest of the run.
    pub dismissed: bool,
}

impl UpdateCheck {
    pub fn new(url: String, enabled: bool, now: Instant) -> Self {
        UpdateCheck {
            url,
            due: (enabled && cfg!(feature = "update-check")).then(|| now + FIRST_DELAY),
            running: None,
            notice: None,
            dismissed: false,
        }
    }

    /// The notice to show, unless it was closed.
    pub fn shown(&self) -> Option<&Notice> {
        self.notice.as_ref().filter(|_| !self.dismissed)
    }

    /// Starts the check when it is due, and asks for a repaint when the next one is, so the timer
    /// runs in a hidden or minimized window too. The thread asks for a repaint when it is done.
    pub fn tick(&mut self, now: Instant, beta: bool, ctx: &egui::Context) {
        let Some(due) = self.due else {
            return;
        };
        if self.running.is_some() {
            return;
        }
        if due <= now {
            let (send, receive) = mpsc::channel();
            let url = self.url.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                // The receiver is gone when Settings changed in the meantime.
                let _ = send.send(fetch(&url, beta));
                ctx.request_repaint();
            });
            self.running = Some(receive);
            self.due = Some(now + INTERVAL);
        } else {
            ctx.request_repaint_after(due - now);
        }
    }

    /// Takes the finished check's answer: a newer release becomes the notice, none removes it, a
    /// failure (offline, an HTTP error, an unknown file) is logged and changes nothing.
    pub fn results(&mut self) {
        let Some(running) = &self.running else {
            return;
        };
        match running.try_recv() {
            Ok(Ok(notice)) => self.notice = notice,
            Ok(Err(e)) => log::warn!("update check failed: {e}"),
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => log::warn!("update check failed: no answer"),
        }
        self.running = None;
    }

    /// After Settings turned the check on or off or changed the channel: a running check is
    /// dropped, so its answer for the old channel never shows; on, the next check runs at once.
    pub fn set(&mut self, enabled: bool, now: Instant) {
        self.running = None;
        self.notice = None;
        self.due = (enabled && cfg!(feature = "update-check")).then_some(now);
    }
}

#[cfg(feature = "update-check")]
fn fetch(url: &str, beta: bool) -> Answer {
    let channel = if beta {
        rtmupdate::Channel::Beta
    } else {
        rtmupdate::Channel::Stable
    };
    let release =
        rtmupdate::check_at(url, env!("CARGO_PKG_VERSION"), channel).map_err(|e| e.to_string())?;
    match release {
        // Download hands the URL to the OS, which would open a file or another app as readily.
        Some(release) if !release.url.starts_with("https://") => Err(format!(
            "release {} has a page that is not https: {}",
            release.version, release.url
        )),
        release => Ok(release.map(|release| Notice {
            version: release.version,
            url: release.url,
        })),
    }
}

/// Never called: without the feature the check is never enabled.
#[cfg(not(feature = "update-check"))]
fn fetch(_url: &str, _beta: bool) -> Answer {
    Ok(None)
}

#[cfg(test)]
#[path = "tests/update_check_tests.rs"]
mod check_tests;
