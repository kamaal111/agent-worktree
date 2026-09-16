use agent_worktree::cli::run;
use agent_worktree::devcontainer::Agent;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(run(argv, Agent::Codex));
}
