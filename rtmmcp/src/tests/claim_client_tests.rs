use super::*;

/// A fresh scratch directory under the system temp directory.
fn scratch() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmmcp-claim-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    dir
}

#[test]
fn test_only_the_first_server_of_a_client_is_first() {
    let dir = scratch();
    let first = claim_in(&dir, 4242);
    assert!(first.first);
    assert!(!claim_in(&dir, 4242).first);
    // Another client's server is first in its own client.
    assert!(claim_in(&dir, 4343).first);
    // Once the first server exits, the next one takes over.
    drop(first);
    assert!(claim_in(&dir, 4242).first);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_a_restarted_server_waits_for_the_old_one_to_exit() {
    let dir = scratch();
    let old = claim_in(&dir, 4242);
    let exiting = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        drop(old);
    });
    assert!(claim_in(&dir, 4242).first);
    exiting.join().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}
