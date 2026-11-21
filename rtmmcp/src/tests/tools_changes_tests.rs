use super::support_tests::*;
use super::*;

#[test]
fn test_add_stamps_the_creation_date_and_returns_the_number() {
    let source = source_with("Buy milk\n");
    let today = Local::now().date_naive();
    let result = add_item(&source, "(B) Call the bank +money").unwrap();
    assert_eq!(result, format!("Added 2 (B) {today} Call the bank +money"));
    assert_eq!(
        lines(&source)[1],
        format!("(B) {today} Call the bank +money")
    );
    remove(source);
}

#[test]
fn test_add_refuses_an_invalid_line() {
    let source = source_with("Buy milk\n");
    assert!(add_item(&source, "   ").is_err());
    assert!(add_item(&source, "Pay rent\nCall Mom").is_err());
    assert!(add_item(&source, "Pay rent\rCall Mom").is_err());
    assert_eq!(lines(&source), ["Buy milk"]);
    remove(source);
}

#[test]
fn test_add_refuses_a_threshold_after_the_due_date() {
    let source = source_with("Buy milk\n");
    let error = add_item(&source, "Pay rent due:2026-01-05 t:2026-01-10").unwrap_err();
    assert_eq!(
        error,
        "'Pay rent due:2026-01-05 t:2026-01-10' cannot be saved: \
         threshold date 2026-01-10 is after due date 2026-01-05."
    );
    assert_eq!(lines(&source), ["Buy milk"]);
    remove(source);
}

#[test]
fn test_edit_refuses_a_threshold_after_the_due_date() {
    let source = source_with("Pay rent due:2026-01-05\n");
    let error = edit_item(
        &source,
        1,
        "Pay rent due:2026-01-05",
        "Pay rent due:2026-01-05 t:2026-01-10",
    )
    .unwrap_err();
    assert!(
        error.contains("threshold date 2026-01-10 is after due date 2026-01-05"),
        "{error}"
    );
    assert_eq!(lines(&source), ["Pay rent due:2026-01-05"]);
    remove(source);
}

#[test]
fn test_add_creates_a_missing_file() {
    let source = source_with("");
    std::fs::remove_file(&source.path).unwrap();
    add_item(&source, "2026-01-01 Buy milk").unwrap();
    assert_eq!(lines(&source), ["2026-01-01 Buy milk"]);
    remove(source);
}

#[test]
fn test_complete_and_uncomplete() {
    let source = source_with("2026-01-01 Buy milk\n");
    let today = Local::now().date_naive();
    let result = complete_item(&source, 1, "2026-01-01 Buy milk").unwrap();
    assert_eq!(result, format!("Completed 1 x {today} 2026-01-01 Buy milk"));
    let error = complete_item(&source, 1, &format!("x {today} 2026-01-01 Buy milk")).unwrap_err();
    assert!(error.contains("already completed"), "{error}");
    uncomplete_item(&source, 1, &format!("x {today} 2026-01-01 Buy milk")).unwrap();
    assert_eq!(lines(&source), ["2026-01-01 Buy milk"]);
    remove(source);
}

#[test]
fn test_completing_a_recurring_item_reports_the_next_one() {
    let source = source_with("2026-01-01 Water plants due:2026-01-05 rec:1w\n");
    // The line as listed, which is how a client names the item.
    let listed = list_items(&source, &Filter::default()).unwrap();
    let line = listed.strip_prefix("1 ").unwrap();
    let result = complete_item(&source, 1, line).unwrap();
    assert!(result.contains("\nNext: 2 "), "{result}");
    assert!(lines(&source)[1].contains("rec:"), "{:?}", lines(&source));
    remove(source);
}

#[test]
fn test_a_moved_item_is_found_by_its_line() {
    // The file changed since the client listed it: "Pay rent" was 2, now 3.
    let source = source_with("Buy milk\nCall mom\nPay rent\n");
    complete_item(&source, 2, "Pay rent").unwrap();
    assert!(lines(&source)[2].starts_with("x "));
    assert_eq!(lines(&source)[1], "Call mom");
    remove(source);
}

#[test]
fn test_a_changed_item_is_not_touched() {
    let source = source_with("Buy oat milk\n");
    let error = delete_item(&source, 1, "Buy milk").unwrap_err();
    assert!(error.contains("list the items again"), "{error}");
    assert!(error.ends_with("Nothing was changed."), "{error}");
    assert_eq!(lines(&source), ["Buy oat milk"]);
    remove(source);
}

#[test]
fn test_a_changed_item_with_a_uuid_shows_its_new_line() {
    let uuid = "uuid:6f3c8f0e-2b1a-4c5d-9e8f-7a6b5c4d3e2f";
    let source = source_with(&format!("Buy oat milk {uuid}\n"));
    let error = edit_item(&source, 1, &format!("Buy milk {uuid}"), "Buy soy milk").unwrap_err();
    assert!(
        error.contains(&format!("It now reads: 1 Buy oat milk {uuid}")),
        "{error}"
    );
    remove(source);
}

#[test]
fn test_edit_keeps_the_creation_date_and_links() {
    let uuid = "uuid:6f3c8f0e-2b1a-4c5d-9e8f-7a6b5c4d3e2f";
    let source = source_with(&format!("2026-01-01 Buy milk {uuid}\nOther\n"));
    edit_item(
        &source,
        1,
        &format!("2026-01-01 Buy milk {uuid}"),
        "(A) Buy oat milk due:2026-02-01",
    )
    .unwrap();
    assert_eq!(
        lines(&source),
        [
            format!("(A) 2026-01-01 Buy oat milk due:2026-02-01 {uuid}"),
            "Other".to_owned()
        ]
    );
    remove(source);
}

#[test]
fn test_delete_removes_sub_items_and_lists_them() {
    let parent = "uuid:6f3c8f0e-2b1a-4c5d-9e8f-7a6b5c4d3e2f";
    let source = source_with(&format!(
        "Plan trip {parent}\nBook hotel sub:6f3c8f0e-2b1a-4c5d-9e8f-7a6b5c4d3e2f\nBuy milk\n"
    ));
    let result = delete_item(&source, 1, &format!("Plan trip {parent}")).unwrap();
    assert!(result.starts_with("Deleted 2 items"), "{result}");
    assert!(result.contains("Book hotel"), "{result}");
    assert_eq!(lines(&source), ["Buy milk"]);
    remove(source);
}

#[test]
fn test_writes_at_once_all_land() {
    let source = source_with("");
    let writers: Vec<_> = (0..8)
        .map(|n| {
            let source = source.clone();
            std::thread::spawn(move || add_item(&source, &format!("2026-01-01 Item {n}")).unwrap())
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    let mut found = lines(&source);
    found.sort();
    assert_eq!(
        found,
        (0..8)
            .map(|n| format!("2026-01-01 Item {n}"))
            .collect::<Vec<_>>()
    );
    remove(source);
}

#[test]
fn test_a_change_of_a_synced_file_is_committed() {
    let source = synced_source();
    add_item(&source, "2026-01-01 Pay rent").unwrap();
    let dir = source.path.parent().unwrap();
    assert_eq!(git(dir, &["log", "-1", "--format=%s"]), "rtm: add Pay rent");
    assert_eq!(git(dir, &["status", "--porcelain", "todo.txt"]), "");
    remove(source);
}

#[test]
fn test_outside_changes_refuse_a_change_and_leave_the_file() {
    let source = synced_source();
    std::fs::write(&source.path, "Buy milk\nEdited by hand\n").unwrap();
    let error = add_item(&source, "2026-01-01 Pay rent").unwrap_err();
    assert!(error.contains("not committed by rtm"), "{error}");
    assert!(error.ends_with("Nothing was changed."), "{error}");
    assert_eq!(lines(&source), ["Buy milk", "Edited by hand"]);
    remove(source);
}

#[test]
fn test_a_write_waits_for_the_repository_lock() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    // Two files of one repository; detection reads markers, so an empty .git is enough.
    let source = source_with("Buy milk\n");
    let dir = source.path.parent().unwrap().to_owned();
    std::fs::create_dir(dir.join(".git")).unwrap();
    let work = Source {
        path: dir.join("work.txt"),
        ..source.clone()
    };
    std::fs::write(&work.path, "Write report\n").unwrap();

    // Another process writing todo.txt holds the repository's lock.
    let held = WriteLock::acquire(&rtmsync::lock_target(&source.path)).unwrap();
    let released = Arc::new(AtomicBool::new(false));
    let writer = {
        let (work, released) = (work.clone(), released.clone());
        std::thread::spawn(move || {
            add_item(&work, "2026-01-01 Send report").unwrap();
            released.load(Ordering::SeqCst)
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(200));
    released.store(true, Ordering::SeqCst);
    drop(held);
    assert!(
        writer.join().unwrap(),
        "work.txt was written while its repository was locked"
    );
    assert_eq!(lines(&work), ["Write report", "2026-01-01 Send report"]);
    remove(source);
}

#[test]
fn test_files_outside_any_repository_do_not_wait_for_each_other() {
    let home = source_with("Buy milk\n");
    let work = source_with("Write report\n");
    // Another process writing home.txt holds its lock; work.txt has a lock of its own.
    let _held = WriteLock::acquire(&rtmsync::lock_target(&home.path)).unwrap();
    let (done, finished) = std::sync::mpsc::channel();
    {
        let work = work.clone();
        std::thread::spawn(move || {
            done.send(add_item(&work, "2026-01-01 Send report"))
                .unwrap();
        });
    }
    // A timeout, not a hang, if the write ever waits for the other file's lock.
    let result = finished
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the write waited for another file's lock");
    result.unwrap();
    assert_eq!(lines(&work), ["Write report", "2026-01-01 Send report"]);
    remove(home);
    remove(work);
}

#[cfg(unix)]
#[test]
fn test_adding_to_a_symlinked_file_is_refused_and_changes_nothing() {
    let (source, refusal) = symlinked_source_with("Buy milk\n");
    assert_eq!(
        add_item(&source, "Call mom").unwrap_err(),
        format!("{refusal} Nothing was changed.")
    );
    assert_eq!(
        std::fs::read_to_string(source.path.with_file_name("real.txt")).unwrap(),
        "Buy milk\n"
    );
    remove(source);
}
