use agent_worktree::arguments::{parse_arguments, CliCommand};
use agent_worktree::devcontainer::Agent;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn parses_a_codex_run_with_passthrough_arguments() {
    let options = parse_arguments(&args(&["run", "--name", "fix-db", "--agent", "codex", "--", "exec", "--full-auto"]), Agent::Codex)
        .unwrap();

    assert_eq!(options.agent, Agent::Codex);
    assert_eq!(options.agent_args, vec!["exec".to_string(), "--full-auto".to_string()]);
    assert_eq!(options.base, None);
    assert_eq!(options.command, CliCommand::Run);
    assert!(!options.help);
    assert!(!options.json);
    assert_eq!(options.name, Some("fix-db".to_string()));
    assert!(!options.version);
    assert!(!options.yes);
}

#[test]
fn uses_the_launcher_alias_as_the_default_agent() {
    let options = parse_arguments(&args(&["--name", "fix-db"]), Agent::Claude).unwrap();

    assert_eq!(options.agent, Agent::Claude);
    assert_eq!(options.command, CliCommand::Run);
}

#[test]
fn rejects_unsupported_agents() {
    let error = parse_arguments(&args(&["--agent", "other"]), Agent::Codex).unwrap_err();
    assert!(error.message.contains("codex or claude"));
}

#[test]
fn parses_a_guarded_destroy_command() {
    let options = parse_arguments(&args(&["destroy", "fix-db", "--yes"]), Agent::Codex).unwrap();

    assert_eq!(options.command, CliCommand::Destroy);
    assert_eq!(options.name, Some("fix-db".to_string()));
    assert!(options.yes);
}
