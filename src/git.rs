use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::{fail, Result};
use crate::process::{command, CommandOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub branch: Option<String>,
    pub path: String,
}

pub struct Repository {
    pub common_git_directory: String,
    pub managed_directory: PathBuf,
    pub primary_root: PathBuf,
    pub worktrees: Vec<Worktree>,
}

fn is_valid_lane_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
}

pub fn validate_lane_name(name: &str) -> Result<()> {
    if !is_valid_lane_name(name) {
        return fail("lane names may contain only letters, numbers, dots, underscores, and dashes");
    }
    Ok(())
}

pub fn parse_worktree_list(output: &str) -> Vec<Worktree> {
    let mut entries = Vec::new();
    let mut current: Option<Worktree> = None;
    for line in output.split('\n') {
        if let Some(rest) = line.strip_prefix("worktree ") {
            if let Some(worktree) = current.take() {
                entries.push(worktree);
            }
            current = Some(Worktree {
                path: rest.to_string(),
                branch: None,
            });
        } else if let Some(rest) = line.strip_prefix("branch refs/heads/") {
            if let Some(worktree) = current.as_mut() {
                worktree.branch = Some(rest.to_string());
            }
        } else if line.is_empty() {
            if let Some(worktree) = current.take() {
                entries.push(worktree);
            }
        }
    }
    if let Some(worktree) = current.take() {
        entries.push(worktree);
    }
    entries
}

pub fn discover_repository(cwd: &Path) -> Result<Repository> {
    let result = command(
        "git",
        &["worktree", "list", "--porcelain"],
        CommandOptions::new(cwd).allow_failure(true),
    )?;
    if result.exit_code != 0 {
        return fail("run this command from inside a Git repository");
    }
    let worktrees = parse_worktree_list(&result.stdout);
    let primary = worktrees.first().cloned();
    let primary = match primary {
        Some(value) => value,
        None => return fail("Git did not report a primary worktree"),
    };
    let primary_root = fs::canonicalize(&primary.path)?;
    let common_git_directory = command(
        "git",
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        CommandOptions::new(&primary_root),
    )?
    .stdout;
    Ok(Repository {
        common_git_directory,
        managed_directory: primary_root.join(".agents").join("worktrees"),
        primary_root,
        worktrees,
    })
}

fn exists(filepath: &Path) -> bool {
    filepath.symlink_metadata().is_ok()
}

pub fn ensure_worktree(repository: &Repository, name: &str, base: Option<&str>) -> Result<PathBuf> {
    validate_lane_name(name)?;
    let filepath = repository.managed_directory.join(name);
    let registered = repository
        .worktrees
        .iter()
        .find(|worktree| resolve(Path::new(&worktree.path)) == filepath);
    if registered.is_some() {
        return Ok(filepath);
    }
    if exists(&filepath) {
        return fail(format!(
            "{} exists but is not a registered Git worktree",
            filepath.display()
        ));
    }

    let branch = format!("agent/{name}");
    let branch_exists = command(
        "git",
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
        CommandOptions::new(&repository.primary_root).allow_failure(true),
    )?
    .exit_code
        == 0;

    fs::create_dir_all(&repository.managed_directory)?;
    let filepath_str = filepath.to_string_lossy().to_string();
    if branch_exists {
        command(
            "git",
            &[
                "worktree",
                "add",
                "--relative-paths",
                &filepath_str,
                &branch,
            ],
            CommandOptions::new(&repository.primary_root),
        )?;
        return Ok(filepath);
    }

    let remote_default = command(
        "git",
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
        CommandOptions::new(&repository.primary_root).allow_failure(true),
    )?
    .stdout;
    let selected_base = base
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .or_else(|| Some(remote_default).filter(|value| !value.is_empty()))
        .unwrap_or_else(|| "HEAD".to_string());
    command(
        "git",
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{selected_base}^{{commit}}"),
        ],
        CommandOptions::new(&repository.primary_root),
    )?;
    command(
        "git",
        &[
            "worktree",
            "add",
            "--relative-paths",
            "-b",
            &branch,
            &filepath_str,
            &selected_base,
        ],
        CommandOptions::new(&repository.primary_root),
    )?;
    Ok(filepath)
}

fn resolve(path: &Path) -> PathBuf {
    if path.is_absolute() {
        normalize(path)
    } else {
        normalize(&std::env::current_dir().unwrap_or_default().join(path))
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        use std::path::Component;
        match component {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            other => result.push(other.as_os_str()),
        }
    }
    result
}

pub fn managed_worktrees(repository: &Repository) -> Vec<Worktree> {
    let prefix = format!(
        "{}{}",
        repository.managed_directory.display(),
        std::path::MAIN_SEPARATOR
    );
    repository
        .worktrees
        .iter()
        .filter(|worktree| {
            resolve(Path::new(&worktree.path))
                .to_string_lossy()
                .starts_with(&prefix)
        })
        .cloned()
        .collect()
}

pub fn worktree_for_name(repository: &Repository, name: &str) -> Result<Worktree> {
    validate_lane_name(name)?;
    let expected = repository.managed_directory.join(name);
    repository
        .worktrees
        .iter()
        .find(|entry| resolve(Path::new(&entry.path)) == expected)
        .cloned()
        .ok_or_else(|| {
            crate::errors::AgentWorktreeError::new(format!("lane does not exist: {name}"))
        })
}

pub fn assert_clean_worktree(worktree_path: &Path) -> Result<()> {
    let status = command(
        "git",
        &["status", "--porcelain"],
        CommandOptions::new(worktree_path),
    )?
    .stdout;
    if !status.is_empty() {
        return fail(format!(
            "refusing to destroy a dirty worktree: {}",
            worktree_path.display()
        ));
    }
    Ok(())
}

pub fn remove_worktree(repository: &Repository, worktree_path: &Path) -> Result<()> {
    command(
        "git",
        &["worktree", "remove", &worktree_path.to_string_lossy()],
        CommandOptions::new(&repository.primary_root),
    )?;
    Ok(())
}
