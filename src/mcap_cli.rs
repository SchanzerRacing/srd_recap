use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, ExitStatus};

pub fn doctor(file: &Path) -> Result<ExitStatus> {
    Command::new("mcap")
        .arg("doctor")
        .arg(file)
        .status()
        .with_context(|| format!("Could not run mcap doctor for {}", file.display()))
}

pub fn recover(input: &Path, output: &Path) -> Result<ExitStatus> {
    Command::new("mcap")
        .arg("recover")
        .arg(input)
        .arg("-o")
        .arg(output)
        .status()
        .with_context(|| format!("Could not run mcap recover for {}", input.display()))
}
