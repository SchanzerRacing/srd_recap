use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "recap",
    version,
    about = "Prepare, analyze, and upload MCAP test recordings",
    long_about = "Process recorded test sessions: validate, recover, and merge MCAP files, extract KPIs and interesting intervals, and publish recordings with annotations to Foxglove.",
    after_help = "Run 'recap <COMMAND> --help' for command-specific arguments and options."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Verbose output
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Validate, recover, and merge recordings
    ///
    /// Search the input directory recursively for MCAP files, then use the external
    /// mcap CLI to validate, recover, and merge recordings. Report recovery outcomes,
    /// including any data loss, and write processed recordings to the output directory.
    Prepare(PrepareArgs),

    /// Extract KPIs and detect interesting intervals
    ///
    /// Search the input directory recursively for MCAP recordings and generate an
    /// analysis report with recording statistics, KPIs, and detected event intervals.
    Analyze(AnalyzeArgs),

    /// Upload recordings and annotations to Foxglove
    ///
    /// Upload an MCAP file or recursively discover recordings in a directory for
    /// the specified Foxglove device. Supply an analysis report to include KPI
    /// metadata and event annotations.
    Upload(UploadArgs),
}

#[derive(Debug, Args)]
pub struct PrepareArgs {
    /// Output directory; defaults to [input_dir]
    #[arg(short, long, value_name = "DIRECTORY")]
    pub output_dir: Option<PathBuf>,

    /// Root directory to search recursively
    #[arg(default_value = ".", value_name = "DIRECTORY")]
    pub input_dir: PathBuf,
}

#[derive(Debug, Args)]
pub struct AnalyzeArgs {
    /// Destination for the analysis report, defaults to a file in the working directory
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Root directory to search recursively for MCAP recordings
    #[arg(default_value = ".", value_name = "INPUT")]
    pub input: PathBuf,
}

#[derive(Debug, Args)]
pub struct UploadArgs {
    /// Analysis report containing KPIs and annotations
    #[arg(short, long, value_name = "FILE")]
    pub report: Option<PathBuf>,

    /// Foxglove device associated with the recordings
    #[arg(short, long, value_name = "ID")]
    pub device: String,

    /// MCAP file or directory to search recursively
    #[arg(default_value = ".", value_name = "INPUT")]
    pub input: PathBuf,
}
