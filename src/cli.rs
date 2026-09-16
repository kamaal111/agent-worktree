use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::arguments::{parse_arguments, CliCommand};
use crate::devcontainer::{destroy_environment, run_agent, start_environment, stop_environment, Agent};
use crate::doctor::diagnose;
use crate::errors::{AgentWorktreeError, Result};
use crate::git::{
    assert_clean_worktree, discover_repository, ensure_worktree, managed_worktrees, remove_worktree,
    worktree_for_name, Repository,
};
use crate::identity::lane_id;
use crate::lock::acquire_lane_lock;
use crate::setup::{ensure_dependencies, run_setup};

pub fn usage() -> &'static str {
    "Usage:
  agent-worktree [run] [--name NAME] [--agent codex|claude] [--base REF] [-- AGENT_ARGS...]
  agent-worktree list [--json]
  agent-worktree setup [--yes]
  agent-worktree doctor NAME
  agent-worktree stop NAME
  agent-worktree destroy NAME --yes

Aliases:
  codex-worktree [options] -- [codex arguments]
  claude-worktree [options] -- [claude arguments]

Each lane uses .agents/worktrees/NAME, branch agent/NAME, and an isolated
development-container/Compose namespace. Destroy refuses dirty worktrees and
retains the Git branch."
}

fn generated_name(agent: Agent) -> String {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    format!("{}-{}-{}", agent.as_str(), timestamp, std::process::id())
}

fn required_name(name: Option<String>, command_name: &str) -> Result<String> {
    name.ok_or_else(|| AgentWorktreeError::usage(format!("{command_name} requires a lane name")))
}

fn print_lanes_text(repository: &Repository) {
    for worktree in managed_worktrees(repository) {
        let name = Path::new(&worktree.path)
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();
        let id = lane_id(&repository.primary_root, &name);
        println!("{name}\t{}\t{id}", worktree.path);
    }
}

fn print_lanes_json(repository: &Repository) {
    let lanes: Vec<serde_json::Value> = managed_worktrees(repository)
        .into_iter()
        .map(|worktree| {
            let name = Path::new(&worktree.path)
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_default();
            let id = lane_id(&repository.primary_root, &name);
            serde_json::json!({
                "branch": worktree.branch,
                "id": id,
                "name": name,
                "path": worktree.path,
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&lanes).unwrap_or_default());
}

fn execute(argv: &[String], default_agent: Agent) -> Result<i32> {
    let options = parse_arguments(argv, default_agent)?;
    if options.help {
        println!("{}", usage());
        return Ok(0);
    }
    if options.version {
        println!("agent-worktree {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }

    if options.command == CliCommand::Setup {
        return run_setup(options.yes);
    }

    let cwd = std::env::current_dir()?;
    let repository = discover_repository(&cwd)?;

    if options.command == CliCommand::List {
        if options.json {
            print_lanes_json(&repository);
        } else {
            print_lanes_text(&repository);
        }
        return Ok(0);
    }

    if options.command == CliCommand::Run {
        ensure_dependencies(options.yes)?;
        let name = options.name.clone().unwrap_or_else(|| generated_name(options.agent));
        let common_git_directory = Path::new(&repository.common_git_directory);
        let mut lock = acquire_lane_lock(common_git_directory, &name)?;
        let result = (|| -> Result<i32> {
            let worktree_path = ensure_worktree(&repository, &name, options.base.as_deref())?;
            let id = lane_id(&repository.primary_root, &name);
            eprintln!("Starting {} in lane {name} ({id})", options.agent.as_str());
            start_environment(&worktree_path, &id)?;
            run_agent(&worktree_path, &id, options.agent, &options.agent_args)
        })();
        lock.release()?;
        return result;
    }

    let name = required_name(options.name.clone(), options.command.as_str())?;
    let worktree = worktree_for_name(&repository, &name)?;
    let id = lane_id(&repository.primary_root, &name);
    let worktree_path = Path::new(&worktree.path);

    if options.command == CliCommand::Doctor {
        let diagnostics = diagnose(worktree_path)?;
        if diagnostics.is_empty() {
            println!("Lane {name} has no known isolation leaks.");
            return Ok(0);
        }
        for diagnostic in &diagnostics {
            println!("{}: {}", diagnostic.file, diagnostic.message);
        }
        return Ok(1);
    }

    if options.command == CliCommand::Stop {
        stop_environment(worktree_path, &id)?;
        println!("Stopped lane {name}; its worktree and data remain available.");
        return Ok(0);
    }

    if !options.yes {
        return Err(AgentWorktreeError::usage(
            "destroy requires --yes because it deletes lane containers, volumes, and the worktree",
        ));
    }
    let common_git_directory = Path::new(&repository.common_git_directory);
    let mut lock = acquire_lane_lock(common_git_directory, &name)?;
    let result = (|| -> Result<()> {
        assert_clean_worktree(worktree_path)?;
        destroy_environment(worktree_path, &id)?;
        remove_worktree(&repository, worktree_path)
    })();
    lock.release()?;
    result?;
    let branch = worktree.branch.clone().unwrap_or_else(|| format!("agent/{name}"));
    println!("Destroyed lane {name}; branch {branch} was retained.");
    Ok(0)
}

pub fn run(argv: Vec<String>, default_agent: Agent) -> i32 {
    match execute(&argv, default_agent) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            if error.is_usage {
                eprintln!("{}", usage());
            }
            1
        }
    }
}
