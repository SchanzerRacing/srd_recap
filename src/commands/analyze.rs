use anyhow::Result;
use crate::cli::AnalyzeArgs;
use crate::CommandOutcome;
use crate::discovery;

pub fn run(args: AnalyzeArgs) -> Result<CommandOutcome> {
    // Discover recordings using args.input,
    // analyze them, and write the report to args.output.
    let recordings = discovery::find_recordings(&args.input)?;

    if recordings.is_empty() {
        anyhow::bail!("No MCAP recordings found in {}", args.input.display());
    }

    Ok(CommandOutcome::Success)
}
