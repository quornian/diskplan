use std::{fs::read_link, os::unix::fs::symlink, path::Path, process::Command};

use anyhow::{anyhow, Context as _, Result};

use crate::schema::Meta;

// The `install` command defaults to 755 for both files and directories
pub const DEFAULT_DIRECTORY_MODE: u16 = 0o755;
pub const DEFAULT_FILE_MODE: u16 = 0o644;

pub fn install_directory(path: &Path, meta: &Meta) -> Result<()> {
    let mut command = Command::new("install");
    add_meta_args(&mut command, meta, DEFAULT_DIRECTORY_MODE);
    command.arg("-d");
    run(command.arg(path)).with_context(|| format!("Failed to create directory {:?}", path))
}

pub fn install_file(path: &Path, source: &Path, meta: &Meta) -> Result<()> {
    let mut command = Command::new("install");
    add_meta_args(&mut command, meta, DEFAULT_FILE_MODE);
    command.arg(source);
    run(command.arg(path)).with_context(|| format!("Failed to create file {:?}", path))
}

pub fn install_link(path: &Path, target: &Path) -> Result<()> {
    if let Ok(existing) = read_link(path) {
        if existing == target {
            return Ok(());
        }
    }
    symlink(target, path)
        .with_context(|| format!("Failed to create symlink: {:?} -> {:?}", path, target))
}

fn add_meta_args(command: &mut Command, meta: &Meta, default_mode: u16) {
    if let Some(owner) = meta.owner() {
        command.args(["--owner", owner]);
    }
    if let Some(group) = meta.group() {
        command.args(["--group", group]);
    }
    let mode = meta.mode().unwrap_or(default_mode);
    command.args(["--mode", &format!("{:o}", mode)]);
}

fn run(command: &mut Command) -> Result<()> {
    eprintln!("Running: {:?}", command);
    let exit = command.status()?;
    match exit.code() {
        Some(0) => Ok(()),
        Some(code) => Err(anyhow!(
            "Command returned exit code {}: {:?}",
            code,
            command,
        )),
        None => panic!("Child process exited abnormally (signal): {:?}", command),
    }
}
