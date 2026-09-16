use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use crate::errors::{fail, Result};

#[derive(Debug)]
pub struct LaneLock {
    lock_path: PathBuf,
    released: bool,
}

impl LaneLock {
    pub fn release(&mut self) -> Result<()> {
        if self.released {
            return Ok(());
        }
        self.released = true;
        let _ = fs::remove_file(&self.lock_path);
        Ok(())
    }
}

impl Drop for LaneLock {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

#[cfg(unix)]
fn process_is_running(pid: i32) -> bool {
    let result = unsafe { libc::kill(pid, 0) };
    if result == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

#[cfg(not(unix))]
fn process_is_running(_pid: i32) -> bool {
    true
}

fn create_lock(lock_path: &Path) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(lock_path)?;
    let write_result = file.write_all(format!("{}\n", std::process::id()).as_bytes());
    if write_result.is_err() {
        drop(file);
        let _ = fs::remove_file(lock_path);
        write_result?;
    }
    Ok(())
}

pub fn acquire_lane_lock(common_git_directory: &Path, name: &str) -> Result<LaneLock> {
    let lock_directory = common_git_directory.join("agent-worktree").join("locks");
    let lock_path = lock_directory.join(format!("{name}.lock"));
    fs::create_dir_all(&lock_directory)?;

    match create_lock(&lock_path) {
        Ok(()) => return Ok(LaneLock { lock_path, released: false }),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }

    let contents = fs::read_to_string(&lock_path).unwrap_or_default();
    if let Ok(owner_pid) = contents.trim().parse::<i32>() {
        if owner_pid > 0 && process_is_running(owner_pid) {
            return fail(format!("lane {name} is already in use by process {owner_pid}"));
        }
    }

    let _ = fs::remove_file(&lock_path);
    match create_lock(&lock_path) {
        Ok(()) => Ok(LaneLock { lock_path, released: false }),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            fail(format!("lane {name} was claimed by another process"))
        }
        Err(error) => Err(error.into()),
    }
}
