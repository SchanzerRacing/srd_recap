use anyhow::Result;
use crate::cli::UploadArgs;
use crate::CommandOutcome;

pub fn run(args: UploadArgs) -> Result<CommandOutcome> {
    // Discover recordings using args.input_dir,
    // validate, recover, and merge them into a single MCAP file,
    // and write the result to args.output_dir.
    todo!()
}
