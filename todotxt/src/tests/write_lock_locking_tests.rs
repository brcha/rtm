use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// A fresh scratch directory under the system temp directory.
fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtm-lock-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    dir
}

#[test]
fn test_second_acquire_waits_for_the_first() {
    let dir = scratch();
    let todo = dir.join("todo.txt");
    let first = WriteLock::acquire_in(&dir, &todo).unwrap();
    let released = Arc::new(AtomicBool::new(false));
    let waiter = {
        let (dir, todo, released) = (dir.clone(), todo.clone(), released.clone());
        std::thread::spawn(move || {
            let _second = WriteLock::acquire_in(&dir, &todo).unwrap();
            released.load(Ordering::SeqCst)
        })
    };
    std::thread::sleep(Duration::from_millis(200));
    released.store(true, Ordering::SeqCst);
    drop(first);
    assert!(
        waiter.join().unwrap(),
        "the second lock was taken while the first was held"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_spellings_of_one_path_share_a_lock() {
    let dir = scratch();
    std::fs::create_dir(dir.join("sub")).unwrap();
    let plain = dir.join("todo.txt");
    let roundabout = dir.join("sub").join("..").join(".").join("todo.txt");
    // Before the file exists, and after.
    assert_eq!(
        lock_path(&dir, &plain).unwrap(),
        lock_path(&dir, &roundabout).unwrap()
    );
    std::fs::write(&plain, "Buy milk\n").unwrap();
    assert_eq!(
        lock_path(&dir, &plain).unwrap(),
        lock_path(&dir, &roundabout).unwrap()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_different_files_have_different_locks() {
    let dir = scratch();
    assert_ne!(
        lock_path(&dir, &dir.join("todo.txt")).unwrap(),
        lock_path(&dir, &dir.join("work.txt")).unwrap()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_lock_name_is_stable() {
    // FNV-1a's published test vectors: the name must not change between builds.
    assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(fnv1a(b"foobar"), 0x8594_4171_f739_67e8);
}

#[test]
fn test_try_acquire_does_not_wait() {
    let dir = scratch();
    let todo = dir.join("todo.txt");
    let held = WriteLock::acquire_in(&dir, &todo).unwrap();
    assert!(WriteLock::try_acquire_in(&dir, &todo).unwrap().is_none());
    drop(held);
    let taken = WriteLock::try_acquire_in(&dir, &todo).unwrap();
    assert!(taken.is_some());
    // And it excludes the next one in turn.
    assert!(WriteLock::try_acquire_in(&dir, &todo).unwrap().is_none());
    drop(taken);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_a_directory_can_be_locked() {
    // Writers lock a file's repository directory.
    let dir = scratch();
    let repo = dir.join("repo");
    std::fs::create_dir(&repo).unwrap();
    let held = WriteLock::acquire_in(&dir, &repo).unwrap();
    assert!(WriteLock::try_acquire_in(&dir, &repo).unwrap().is_none());
    drop(held);
    std::fs::remove_dir_all(&dir).unwrap();
}
