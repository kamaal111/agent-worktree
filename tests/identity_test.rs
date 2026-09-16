use std::path::Path;

use agent_worktree::devcontainer::unique_ids;
use agent_worktree::identity::lane_id;

#[test]
fn creates_stable_compose_compatible_lane_identities() {
    let first = lane_id(Path::new("/Users/example/Projects/My App"), "Fix/Payments");
    let second = lane_id(Path::new("/Users/example/Projects/My App"), "Fix/Payments");

    assert_eq!(first, second);
    assert!(first.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'));
    assert!(first.starts_with("aw-my-app-fix-payments-"));
}

#[test]
fn distinguishes_repositories_with_the_same_basename() {
    let first = lane_id(Path::new("/Users/one/project"), "feature");
    let second = lane_id(Path::new("/Users/two/project"), "feature");

    assert_ne!(first, second);
}

#[test]
fn deduplicates_resources_carrying_both_lane_and_compose_labels() {
    let ids = unique_ids(&[
        vec!["container-a".to_string(), "container-b".to_string()],
        vec!["container-b".to_string(), "container-c".to_string()],
    ]);

    assert_eq!(ids, vec!["container-a".to_string(), "container-b".to_string(), "container-c".to_string()]);
}
