use agent_worktree::git::{parse_worktree_list, validate_lane_name, Worktree};

#[test]
fn parses_primary_and_linked_worktrees() {
    let worktrees = parse_worktree_list(
        "worktree /repo\nHEAD abc123\nbranch refs/heads/main\n\nworktree /repo/.agents/worktrees/fix-db\nHEAD def456\nbranch refs/heads/agent/fix-db\n",
    );

    assert_eq!(
        worktrees,
        vec![
            Worktree {
                branch: Some("main".to_string()),
                path: "/repo".to_string()
            },
            Worktree {
                branch: Some("agent/fix-db".to_string()),
                path: "/repo/.agents/worktrees/fix-db".to_string(),
            },
        ]
    );
}

#[test]
fn rejects_lane_names_that_could_escape_the_managed_directory() {
    let error = validate_lane_name("../outside").unwrap_err();
    assert!(error.message.contains("lane names may contain"));
}
