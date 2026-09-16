use std::path::Path;
use std::process::{Command, Stdio};

use crate::errors::{fail, AgentWorktreeError, Result};

pub struct CommandOptions<'a> {
    pub allow_failure: bool,
    pub cwd: &'a Path,
    pub env: Option<Vec<(String, String)>>,
    pub inherit_stdio: bool,
}

impl<'a> CommandOptions<'a> {
    pub fn new(cwd: &'a Path) -> Self {
        Self { allow_failure: false, cwd, env: None, inherit_stdio: false }
    }

    pub fn allow_failure(mut self, value: bool) -> Self {
        self.allow_failure = value;
        self
    }

    pub fn env(mut self, env: Vec<(String, String)>) -> Self {
        self.env = Some(env);
        self
    }

    pub fn inherit_stdio(mut self, value: bool) -> Self {
        self.inherit_stdio = value;
        self
    }
}

pub struct CommandResult {
    pub exit_code: i32,
    pub stderr: String,
    pub stdout: String,
}

#[cfg(unix)]
fn signal_exit_code(status: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal().map(|_| 128)
}

#[cfg(not(unix))]
fn signal_exit_code(_status: &std::process::ExitStatus) -> Option<i32> {
    None
}

pub fn command(executable: &str, args: &[&str], options: CommandOptions) -> Result<CommandResult> {
    let mut cmd = Command::new(executable);
    cmd.args(args).current_dir(options.cwd);
    if let Some(env) = &options.env {
        for (key, value) in env {
            cmd.env(key, value);
        }
    }

    let (exit_code, stdout, stderr) = if options.inherit_stdio {
        cmd.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
        let status = cmd
            .status()
            .map_err(|error| AgentWorktreeError::new(format!("could not run {executable}: {error}")))?;
        let exit_code = status.code().or_else(|| signal_exit_code(&status)).unwrap_or(1);
        (exit_code, String::new(), String::new())
    } else {
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let output = cmd
            .output()
            .map_err(|error| AgentWorktreeError::new(format!("could not run {executable}: {error}")))?;
        let exit_code = output.status.code().or_else(|| signal_exit_code(&output.status)).unwrap_or(1);
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        (exit_code, stdout, stderr)
    };

    if exit_code != 0 && !options.allow_failure {
        let message = if stderr.is_empty() {
            format!("{executable} {} failed with exit code {exit_code}", args.join(" "))
        } else {
            stderr
        };
        return fail(message);
    }

    Ok(CommandResult { exit_code, stderr, stdout })
}
