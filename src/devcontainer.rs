use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::errors::{fail, Result};
use crate::process::{command, CommandOptions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub fn as_str(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }

    pub fn parse(value: &str) -> Option<Agent> {
        match value {
            "claude" => Some(Agent::Claude),
            "codex" => Some(Agent::Codex),
            _ => None,
        }
    }
}

pub fn find_devcontainer_config(worktree_path: &Path) -> Result<PathBuf> {
    let candidates = [
        worktree_path
            .join(".devcontainer")
            .join("devcontainer.json"),
        worktree_path.join(".devcontainer.json"),
    ];
    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    fail(format!(
        "no devcontainer configuration found in {}",
        worktree_path.display()
    ))
}

fn lane_environment(id: &str) -> Vec<(String, String)> {
    vec![
        ("AGENT_WORKTREE_ID".to_string(), id.to_string()),
        ("COMPOSE_PROJECT_NAME".to_string(), id.to_string()),
    ]
}

fn workspace_args(worktree_path: &Path) -> Vec<String> {
    vec![
        "--workspace-folder".to_string(),
        worktree_path.to_string_lossy().to_string(),
        "--mount-git-worktree-common-dir".to_string(),
        "true".to_string(),
    ]
}

pub fn start_environment(worktree_path: &Path, id: &str) -> Result<()> {
    find_devcontainer_config(worktree_path)?;
    let mut args: Vec<String> = vec!["up".to_string()];
    args.extend(workspace_args(worktree_path));
    args.push("--id-label".to_string());
    args.push(format!("agent-worktree.id={id}"));
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    command(
        "devcontainer",
        &args_ref,
        CommandOptions::new(worktree_path)
            .env(lane_environment(id))
            .inherit_stdio(true),
    )?;
    Ok(())
}

pub fn run_agent(
    worktree_path: &Path,
    id: &str,
    agent: Agent,
    agent_args: &[String],
) -> Result<i32> {
    let mut args: Vec<String> = vec!["exec".to_string()];
    args.extend(workspace_args(worktree_path));
    args.push(agent.as_str().to_string());
    args.extend(agent_args.iter().cloned());
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = command(
        "devcontainer",
        &args_ref,
        CommandOptions::new(worktree_path)
            .env(lane_environment(id))
            .inherit_stdio(true)
            .allow_failure(true),
    )?;
    Ok(result.exit_code)
}

pub fn stop_environment(worktree_path: &Path, id: &str) -> Result<()> {
    let containers = docker_container_ids(id, worktree_path, false)?;
    if !containers.is_empty() {
        let mut args = vec!["container".to_string(), "stop".to_string()];
        args.extend(containers);
        let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
        command("docker", &args_ref, CommandOptions::new(worktree_path))?;
    }
    Ok(())
}

fn docker_resource_ids(kind: &str, id: &str, cwd: &Path) -> Result<Vec<String>> {
    let filter = format!("label=com.docker.compose.project={id}");
    let result = command(
        "docker",
        &[kind, "ls", "--quiet", "--filter", &filter],
        CommandOptions::new(cwd).allow_failure(true),
    )?;
    if result.exit_code != 0 {
        return fail(if result.stderr.is_empty() {
            format!("could not list Docker {kind}s for {id}")
        } else {
            result.stderr
        });
    }
    Ok(split_lines(&result.stdout))
}

fn container_ids_for_label(id: &str, label: &str, cwd: &Path, all: bool) -> Result<Vec<String>> {
    let mut args = vec!["container".to_string(), "ls".to_string()];
    if all {
        args.push("--all".to_string());
    }
    let filter = format!("label={label}={id}");
    args.push("--quiet".to_string());
    args.push("--filter".to_string());
    args.push(filter);
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = command(
        "docker",
        &args_ref,
        CommandOptions::new(cwd).allow_failure(true),
    )?;
    if result.exit_code != 0 {
        return fail(if result.stderr.is_empty() {
            format!("could not list Docker containers for {id}")
        } else {
            result.stderr
        });
    }
    Ok(split_lines(&result.stdout))
}

fn split_lines(value: &str) -> Vec<String> {
    if value.is_empty() {
        Vec::new()
    } else {
        value.split('\n').map(str::to_string).collect()
    }
}

pub fn unique_ids(groups: &[Vec<String>]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for group in groups {
        for item in group {
            if item.is_empty() {
                continue;
            }
            if seen.insert(item.clone()) {
                result.push(item.clone());
            }
        }
    }
    result
}

fn docker_container_ids(id: &str, cwd: &Path, all: bool) -> Result<Vec<String>> {
    let by_lane_label = container_ids_for_label(id, "agent-worktree.id", cwd, all)?;
    let by_compose_label = container_ids_for_label(id, "com.docker.compose.project", cwd, all)?;
    Ok(unique_ids(&[by_lane_label, by_compose_label]))
}

pub fn destroy_environment(worktree_path: &Path, id: &str) -> Result<()> {
    let containers = docker_container_ids(id, worktree_path, true)?;
    if !containers.is_empty() {
        let mut args = vec![
            "container".to_string(),
            "rm".to_string(),
            "--force".to_string(),
        ];
        args.extend(containers);
        let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
        command("docker", &args_ref, CommandOptions::new(worktree_path))?;
    }

    let volumes = docker_resource_ids("volume", id, worktree_path)?;
    if !volumes.is_empty() {
        let mut args = vec!["volume".to_string(), "rm".to_string()];
        args.extend(volumes);
        let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
        command("docker", &args_ref, CommandOptions::new(worktree_path))?;
    }

    let networks = docker_resource_ids("network", id, worktree_path)?;
    if !networks.is_empty() {
        let mut args = vec!["network".to_string(), "rm".to_string()];
        args.extend(networks);
        let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
        command("docker", &args_ref, CommandOptions::new(worktree_path))?;
    }
    Ok(())
}
