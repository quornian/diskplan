use std::{path::Path, process::Command};

use crate::definition::meta::Meta;

use super::ApplicationError;

pub fn install_directory(path: &Path, meta: &Meta) -> Result<(), ApplicationError> {
    let mut command = Command::new("install");
    command.arg("-d");
    if let Some(owner) = meta.owner() {
        command.args(["--owner", owner]);
    }
    if let Some(group) = meta.group() {
        command.args(["--group", group]);
    }
    if let Some(perms) = meta.permissions() {
        command.args(["--mode", &format!("{:o}", perms.mode())]);
    }
    run_for(path, command.arg(path))
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
