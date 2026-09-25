use anyhow::{Context, Result, ensure};
use std::{
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
};

pub trait McapCli {
    fn doctor(&self, file: &Path) -> Result<ExitStatus>;
    fn recover(&self, input: &Path, output: &Path) -> Result<ExitStatus>;
    fn merge(&self, inputs: &[PathBuf], output: &Path) -> Result<ExitStatus>;
    fn has_messages(&self, file: &Path) -> Result<bool>;
}

pub struct RealMcapCli {
    pub verbose: bool,
}

impl McapCli for RealMcapCli {
    fn doctor(&self, file: &Path) -> Result<ExitStatus> {
        let mut command = Command::new("mcap");
        command.arg("doctor").arg(file);

        if !self.verbose {
            command.stdout(Stdio::null());
            command.stderr(Stdio::null());
        }

        command
            .status()
            .with_context(|| format!("Could not run mcap doctor for {}", file.display()))
    }

    fn recover(&self, input: &Path, output: &Path) -> Result<ExitStatus> {
        Command::new("mcap")
            .arg("recover")
            .arg(input)
            .arg("-o")
            .arg(output)
            .status()
            .with_context(|| format!("Could not run mcap recover for {}", input.display()))
    }

    fn merge(&self, inputs: &[PathBuf], output: &Path) -> Result<ExitStatus> {
        Command::new("mcap")
            .arg("merge")
            .args(inputs)
            .arg("-o")
            .arg(output)
            .status()
            .with_context(|| format!("Could not run mcap merge for {}", output.display()))
    }

    fn has_messages(&self, file: &Path) -> Result<bool> {
        let output = Command::new("mcap")
            .arg("info")
            .arg(file)
            .output()
            .with_context(|| format!("Could not run mcap info for {}", file.display()))?;

        ensure!(
            output.status.success(),
            "mcap info failed for {}: {}",
            file.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );

        let stdout =
            String::from_utf8(output.stdout).context("mcap info returned invalid UTF-8")?;

        let count = stdout
            .lines()
            .find_map(|line| line.strip_prefix("messages:"))
            .context("mcap info did not report a message count")?
            .trim()
            .parse::<u64>()
            .context("Could not parse the MCAP message count")?;

        Ok(count > 0)
    }
}
