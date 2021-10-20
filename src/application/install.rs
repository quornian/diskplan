use std::{fs::read_link, os::unix::fs::symlink, path::Path, process::Command};

use crate::definition::meta::Meta;

use super::ApplicationError;

// The `install` command defaults to 755 for both files and directories
pub const DEFAULT_DIRECTORY_MODE: u16 = 0o755;
pub const DEFAULT_FILE_MODE: u16 = 0o644;

pub fn install_directory(path: &Path, meta: &Meta) -> Result<(), ApplicationError> {
    let mut command = Command::new("install");
    add_meta_args(&mut command, meta, DEFAULT_DIRECTORY_MODE);
    command.arg("-d");
    run_for(path, command.arg(path))
}

pub fn install_file(path: &Path, source: &Path, meta: &Meta) -> Result<(), ApplicationError> {
    let mut command = Command::new("install");
    add_meta_args(&mut command, meta, DEFAULT_FILE_MODE);
    command.arg(source);
    run_for(path, command.arg(path))
}

pub fn install_link(path: &Path, target: &Path) -> Result<(), ApplicationError> {
    if let Ok(existing) = read_link(path) {
        if existing == target {
            return Ok(());
        }
    }
    symlink(target, path).map_err(|e| ApplicationError::IOError(path.into(), e))
}

fn add_meta_args(command: &mut Command, meta: &Meta, default_mode: u16) {
    if let Some(owner) = meta.owner() {
        command.args(["--owner", owner]);
    }
    if let Some(group) = meta.group() {
        command.args(["--group", group]);
    }
    let mode = meta.permissions().map(|p| p.mode()).unwrap_or(default_mode);
    command.args(["--mode", &format!("{:o}", mode)]);
}

fn run_for(path: &Path, command: &mut Command) -> Result<(), ApplicationError> {
    eprintln!("Running: {:?}", command);
    let exit = command
        .status()
        .map_err(|e| ApplicationError::IOError(path.into(), e))?;
    match exit.code() {
        Some(0) => Ok(()),
        Some(code) => Err(ApplicationError::CommandError(
            path.into(),
            code,
            format!("{:?}", command),
        )),
        None => panic!("Child process exited abnormally (signal): {:?}", command),
    }
}
