use std::io::{self, Write};
use std::path::Path;

use crate::errors::{AgentWorktreeError, Result};
use crate::process::{command, CommandOptions};

const MIN_GIT_VERSION: (u32, u32, u32) = (2, 46, 0);
const DEVCONTAINER_INSTALL_SCRIPT: &str =
    "https://raw.githubusercontent.com/devcontainers/cli/main/scripts/install.sh";

fn parse_git_version(output: &str) -> Option<(u32, u32, u32)> {
    let token = output.split_whitespace().find(|word| word.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    let mut parts = token.split('.').map(|part| {
        let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse::<u32>().unwrap_or(0)
    });
    let major = parts.next()?;
    let minor = parts.next().unwrap_or(0);
    let patch = parts.next().unwrap_or(0);
    Some((major, minor, patch))
}

fn git_upgrade_hint() -> &'static str {
    match std::env::consts::OS {
        "macos" => "brew upgrade git",
        "linux" => "sudo apt-get install --only-upgrade git (or your distro's equivalent)",
        "windows" => "winget upgrade --id Git.Git",
        _ => "see https://git-scm.com/downloads",
    }
}

fn check_git(cwd: &Path, announce_ok: bool) -> Result<bool> {
    let detected = match command("git", &["--version"], CommandOptions::new(cwd).allow_failure(true)) {
        Ok(result) if result.exit_code == 0 => parse_git_version(&result.stdout),
        _ => None,
    };

    match detected {
        Some(version) if version >= MIN_GIT_VERSION => {
            if announce_ok {
                println!(
                    "git: OK ({}.{}.{}, supports `git worktree add --relative-paths`)",
                    version.0, version.1, version.2
                );
            }
            Ok(true)
        }
        Some(version) => {
            eprintln!(
                "git: found {}.{}.{}, but `git worktree add --relative-paths` requires {}.{}.{} or newer.",
                version.0, version.1, version.2, MIN_GIT_VERSION.0, MIN_GIT_VERSION.1, MIN_GIT_VERSION.2
            );
            eprintln!("  Upgrade git and re-run: {}", git_upgrade_hint());
            Ok(false)
        }
        None => {
            eprintln!(
                "git: not found. Install git {}.{}.{} or newer: {}",
                MIN_GIT_VERSION.0, MIN_GIT_VERSION.1, MIN_GIT_VERSION.2, git_upgrade_hint()
            );
            Ok(false)
        }
    }
}

fn devcontainer_installed(cwd: &Path) -> bool {
    matches!(
        command("devcontainer", &["--version"], CommandOptions::new(cwd).allow_failure(true)),
        Ok(result) if result.exit_code == 0
    )
}

fn confirm(prompt: &str) -> bool {
    eprint!("{prompt} [y/N] ");
    let _ = io::stderr().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}

fn install_devcontainer_cli(cwd: &Path) {
    let install_command = format!("curl -fsSL {DEVCONTAINER_INSTALL_SCRIPT} | sh");
    println!("Installing Dev Container CLI: {install_command}");
    let result = command("sh", &["-c", &install_command], CommandOptions::new(cwd).inherit_stdio(true));
    match result {
        Ok(result) if result.exit_code == 0 => {
            if devcontainer_installed(cwd) {
                println!("devcontainer CLI installed.");
            } else {
                println!(
                    "devcontainer CLI installed, but is not yet on PATH. Add it with:\n  export PATH=\"$HOME/.devcontainers/bin:$PATH\""
                );
            }
        }
        _ => {
            eprintln!("devcontainer CLI installation failed. Run manually: {install_command}");
        }
    }
}

fn check_devcontainer_cli(cwd: &Path, yes: bool, announce_ok: bool) {
    if devcontainer_installed(cwd) {
        if announce_ok {
            println!("devcontainer: OK");
        }
        return;
    }

    let install_command = format!("curl -fsSL {DEVCONTAINER_INSTALL_SCRIPT} | sh");
    if std::env::consts::OS == "windows" {
        println!(
            "devcontainer: not found. Install it with `npm install -g @devcontainers/cli` (see https://github.com/devcontainers/cli)."
        );
        return;
    }

    println!("devcontainer: not found.");
    if yes || confirm(&format!("Install it now by running `{install_command}`?")) {
        install_devcontainer_cli(cwd);
    } else {
        println!("Skipped. Install it later with: {install_command}");
    }
}

pub fn run_setup(yes: bool) -> Result<i32> {
    let cwd = std::env::current_dir()?;
    let git_ok = check_git(&cwd, true)?;
    check_devcontainer_cli(&cwd, yes, true);
    Ok(if git_ok { 0 } else { 1 })
}

/// Silently checks whether onboarding dependencies are satisfied, and only speaks up (and, for
/// the Dev Container CLI, offers to install) when something is actually missing. Used to trigger
/// onboarding automatically before `run`, without adding noise to a machine that's already set up.
pub fn ensure_dependencies(yes: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    if !check_git(&cwd, false)? {
        return Err(AgentWorktreeError::new(
            "git does not meet the requirements for `git worktree add --relative-paths` (see above)",
        ));
    }
    check_devcontainer_cli(&cwd, yes, false);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_git_version;

    #[test]
    fn parses_plain_version() {
        assert_eq!(parse_git_version("git version 2.46.0"), Some((2, 46, 0)));
    }

    #[test]
    fn parses_version_with_vendor_suffix() {
        assert_eq!(parse_git_version("git version 2.39.2 (Apple Git-143)"), Some((2, 39, 2)));
    }

    #[test]
    fn parses_version_missing_patch() {
        assert_eq!(parse_git_version("git version 2.46"), Some((2, 46, 0)));
    }

    #[test]
    fn rejects_garbage_output() {
        assert_eq!(parse_git_version("not a git version string"), None);
    }
}
