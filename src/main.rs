use anyhow::Result;
use clap::Parser;
use std::process::ExitCode;

use cli::{Cli, Commands};

mod cli;
mod commands;
mod discovery;
mod mcap_cli;

pub enum CommandOutcome {
    Success,
    Warnings,
}

fn run() -> Result<CommandOutcome> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Prepare(args) => commands::prepare::run(args),
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
