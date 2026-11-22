//! The write lock shared with other rtm processes.

use super::support_tests::*;
use super::*;

/// The lock another rtm process would hold while it writes `dir`'s todo.txt.
fn hold(dir: &Path) -> WriteLock {
    WriteLock::acquire(&rtmsync::lock_target(
        &dir.join("todo.txt").canonicalize().unwrap(),
    ))
    .unwrap()
}

#[test]
fn a_change_waits_while_another_process_holds_the_lock() {
    let dir = temp_dir("lock held");
    let mut app = app_with(&dir, "Alpha\n");
    let held = hold(&dir);

    app.new_item = "Bravo".to_owned();
    app.add_item();

    // Shown at once, not written, and kept for the next frames.
    let file = app.file().unwrap();
    assert_eq!(file.library.as_ref().unwrap().items.len(), 2);
    assert!(file.locked_out.is_some() && file.pending.len() == 1);
    assert_eq!(file_lines(&dir), ["Alpha"]);

    drop(held);
    std::thread::sleep(LOCK_RETRY);
    app.flush();
    assert!(app.file().unwrap().locked_out.is_none());
    let lines = file_lines(&dir);
    assert!(lines.len() == 2 && lines[1].ends_with("Bravo"), "{lines:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_held_lock_is_tried_again_only_after_lock_retry() {
    let dir = temp_dir("lock retry");
    let mut app = app_with(&dir, "Alpha\n");
    let held = hold(&dir);
    app.new_item = "Bravo".to_owned();
    app.add_item();
    assert!(app.file().unwrap().locked_out.is_some());

    // Free now, but the next frames do not try it before `LOCK_RETRY` has passed.
    drop(held);
    app.flush();
    let file = app.file().unwrap();
    assert!(file.locked_out.is_some() && file.pending.len() == 1);
    assert_eq!(file_lines(&dir), ["Alpha"]);

    std::thread::sleep(LOCK_RETRY);
    app.flush();
    assert!(app.file().unwrap().locked_out.is_none());
    let lines = file_lines(&dir);
    let [_, added] = lines.as_slice() else {
        panic!("two lines: {lines:?}")
    };
    assert!(added.ends_with("Bravo"), "{lines:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn exit_writes_after_waiting_for_the_lock() {
    let dir = temp_dir("lock exit");
    let mut app = app_with(&dir, "Alpha\n");
    let _held = hold(&dir);
    app.new_item = "Bravo".to_owned();
    app.add_item();
    assert_eq!(file_lines(&dir), ["Alpha"]);

    // The lock is never released: after the wait the change is written anyway.
    app.write_through_lock(Instant::now() + Duration::from_millis(300));

    let lines = file_lines(&dir);
    assert!(lines.len() == 2 && lines[1].ends_with("Bravo"), "{lines:?}");
    assert!(!app.write_unlocked);
    let _ = std::fs::remove_dir_all(dir);
}
