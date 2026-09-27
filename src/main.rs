use anyhow::Result;
use clap::Parser;
use std::process::ExitCode;

use cli::{Cli, Commands};

mod cli;
mod commands;
mod utils;
mod mcap_cli;
mod tracking;

pub enum CommandOutcome {
    Success,
    Warnings,
}

fn run() -> Result<CommandOutcome> {
    let cli = Cli::parse();

    let mcap = mcap_cli::RealMcapCli {
        verbose: cli.verbose > 0,
    };

    match cli.command {
        Commands::Prepare(args) => commands::prepare::run(args, &mcap),
        Commands::Analyze(args) => commands::analyze::run(args),
        Commands::Upload(args) => commands::upload::run(args),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(CommandOutcome::Success) => ExitCode::SUCCESS,
        Ok(CommandOutcome::Warnings) => ExitCode::from(3),
        Err(error) => {
            eprintln!("Error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
