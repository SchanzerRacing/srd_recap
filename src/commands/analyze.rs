use crate::CommandOutcome;
use crate::cli::AnalyzeArgs;
use anyhow::{Result, bail};

pub fn run(_args: AnalyzeArgs) -> Result<CommandOutcome> {
    // Discover recordings using args.input,
    // analyze them, and write the report to args.output.

    bail!("Analyzing is not implemented yet");
}
