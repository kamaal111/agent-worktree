use std::fs;
use std::path::Path;

use agent_worktree::lock::acquire_lane_lock;

use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(prefix: &str) -> std::path::PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}-{unique}",
        std::process::id(),
        fastrand_like()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fastrand_like() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

#[test]
fn prevents_two_live_processes_from_claiming_the_same_lane() {
    let root = temp_dir("agent-worktree-lock");
    let mut lock = acquire_lane_lock(&root, "feature").unwrap();

    let error = acquire_lane_lock(&root, "feature").unwrap_err();
    assert!(error.message.contains("already in use"));

    lock.release().unwrap();
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn recovers_a_stale_lane_lock() {
    let root = temp_dir("agent-worktree-lock");
    let lock_directory = root.join("agent-worktree").join("locks");
    fs::create_dir_all(&lock_directory).unwrap();
    fs::write(lock_directory.join("feature.lock"), "999999999\n").unwrap();

    let mut lock = acquire_lane_lock(&root, "feature").unwrap();
    lock.release().unwrap();

    assert!(!Path::new(&lock_directory.join("feature.lock")).exists());
    fs::remove_dir_all(&root).unwrap();
}
