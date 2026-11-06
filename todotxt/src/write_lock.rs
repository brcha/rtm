//! The lock every rtm process holds while it writes a todo.txt, a pull included, so two of them
//! (the app, the CLI, MCP servers started by different clients) never interleave a load, change
//! and save of the same file with another write and lose one of the changes.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

/// An exclusive lock on a todo.txt, or on the repository directory holding it, held until it is
/// dropped.
///
/// The lock is taken on a file in a per-user directory, never next to the todo.txt, where its
/// repository would see it. Its name is a hash of the locked path's canonical form, so every
/// spelling of the path, in every rtm program, takes the same lock.
#[derive(Debug)]
pub struct WriteLock {
    _file: File,
}

impl WriteLock {
    /// Waits for the lock on `path`: a todo.txt (which need not exist yet; its directory must), or
    /// the repository directory holding it.
    pub fn acquire(path: &Path) -> io::Result<WriteLock> {
        Self::acquire_in(&lock_dir()?, path)
    }

    /// Takes the lock on `path` if no one holds it, without waiting: `None` while another
    /// holder has it. For a thread that must not block, such as a user interface's.
    pub fn try_acquire(path: &Path) -> io::Result<Option<WriteLock>> {
        Self::try_acquire_in(&lock_dir()?, path)
    }

    fn acquire_in(dir: &Path, path: &Path) -> io::Result<WriteLock> {
        let file = open(dir, path)?;
        file.lock()?;
        Ok(WriteLock { _file: file })
    }

    fn try_acquire_in(dir: &Path, path: &Path) -> io::Result<Option<WriteLock>> {
        let file = open(dir, path)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(WriteLock { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(e)) => Err(e),
        }
    }
}

/// The lock file for `path` in `dir`, opened for locking.
fn open(dir: &Path, path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path(dir, path)?)
}

/// The lock file for `path` in `dir`.
fn lock_path(dir: &Path, todo_file: &Path) -> io::Result<PathBuf> {
    let canonical = match todo_file.canonicalize() {
        Ok(path) => path,
        // A file not created yet: its directory's canonical path, then its name.
        Err(_) => {
            let parent = match todo_file.parent() {
                Some(parent) if !parent.as_os_str().is_empty() => parent,
                _ => Path::new("."),
            };
            let name = todo_file
                .file_name()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a file path"))?;
            parent.canonicalize()?.join(name)
        }
    };
    let hash = fnv1a(canonical.to_string_lossy().as_bytes());
    Ok(dir.join(format!("{hash:016x}.lock")))
}

/// 64-bit FNV-1a. Written out rather than `DefaultHasher`, whose output may change between Rust
/// versions, so programs built with different compilers still agree on the lock's name.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The per-user directory holding the lock files, created when missing. Other rtm locks (the MCP
/// server's one-server-per-client check) live here too.
pub fn lock_dir() -> io::Result<PathBuf> {
    // The temporary directory is per user on macOS and Windows; on Linux it is shared, so the
    // runtime directory comes first and the fallback carries the user's name.
    // shortcut: two processes that disagree on XDG_RUNTIME_DIR use different locks; derive the
    // directory from the user ID instead if that ever shows up.
    let dir = match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(runtime) if !runtime.is_empty() => PathBuf::from(runtime).join("rtm"),
        _ => {
            let user = std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_default();
            std::env::temp_dir().join(format!("rtm-{user}"))
        }
    };
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&dir)?;
    if !std::fs::symlink_metadata(&dir)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("'{}' is not a directory", dir.display()),
        ));
    }
    Ok(dir)
}

#[cfg(test)]
#[path = "tests/write_lock_locking_tests.rs"]
mod locking_tests;
