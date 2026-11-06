//! One rtm server per client: the first server a client starts claims a lock named after the
//! client's process ID; a second one finds it held and offers no tools.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::time::Duration;

/// The outcome of the claim; `_file` holds it until dropped.
pub(crate) struct Claim {
    /// Whether this is the first rtm server of its client, or the check could not be made.
    pub(crate) first: bool,
    _file: Option<File>,
}

/// Claims the client that started this process. When the check cannot be made (no parent ID, no
/// lock directory) the server counts as the first: a missing check must not hide the tools. A
/// wrapper between the client and the server gives each server a different parent, which turns
/// the check off the same way.
pub(crate) fn claim_client() -> Claim {
    match (parent_id(), todotxt::write_lock::lock_dir()) {
        (Some(parent), Ok(dir)) => claim_in(&dir, parent),
        _ => Claim::unchecked(),
    }
}

impl Claim {
    fn unchecked() -> Claim {
        Claim {
            first: true,
            _file: None,
        }
    }
}

/// Claims the client with process ID `parent` through a lock file in `dir`.
fn claim_in(dir: &Path, parent: u32) -> Claim {
    // shortcut: one empty file per client process is left in the lock directory; remove them if
    // they ever pile up (the runtime directory is cleared at logout).
    let Ok(file) = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(format!("mcp-client-{parent}.lock")))
    else {
        return Claim::unchecked();
    };
    // A client that restarts its server can start the new one before the old one has exited, so
    // a held claim is tried again for a while before this server counts as a second one.
    let mut waited = Duration::ZERO;
    loop {
        match file.try_lock() {
            Ok(()) => {
                return Claim {
                    first: true,
                    _file: Some(file),
                };
            }
            Err(TryLockError::WouldBlock) if waited < RESTART_WAIT => {
                std::thread::sleep(RETRY);
                waited += RETRY;
            }
            Err(TryLockError::WouldBlock) => {
                return Claim {
                    first: false,
                    _file: None,
                };
            }
            Err(TryLockError::Error(_)) => return Claim::unchecked(),
        }
    }
}

/// How long a held claim is tried again, and how often.
const RESTART_WAIT: Duration = Duration::from_secs(2);
const RETRY: Duration = Duration::from_millis(100);

#[cfg(unix)]
fn parent_id() -> Option<u32> {
    Some(std::os::unix::process::parent_id())
}

/// The parent's process ID from a snapshot of the process list (Windows has no direct call).
#[cfg(windows)]
fn parent_id() -> Option<u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    let me = std::process::id();
    // SAFETY: the snapshot handle is checked and closed; the entry is a plain struct whose size
    // field is set before the first call, as the API requires.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut parent = None;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            if entry.th32ProcessID == me {
                parent = Some(entry.th32ParentProcessID);
                break;
            }
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        parent
    }
}

#[cfg(not(any(unix, windows)))]
fn parent_id() -> Option<u32> {
    None
}

#[cfg(test)]
#[path = "tests/claim_client_tests.rs"]
mod client_tests;
