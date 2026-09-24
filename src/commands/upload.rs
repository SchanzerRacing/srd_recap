use crate::CommandOutcome;
use crate::cli::UploadArgs;
use anyhow::{Result, bail};

pub fn run(_args: UploadArgs) -> Result<CommandOutcome> {
    // Discover recordings using args.input_dir,
    // validate, recover, and merge them into a single MCAP file,
    // and write the result to args.output_dir.

    bail!("Uploading to Foxglove is not implemented yet");
}
