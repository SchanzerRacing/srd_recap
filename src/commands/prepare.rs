use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::CommandOutcome;
use crate::cli::PrepareArgs;
use crate::discovery;
use anyhow::{Result, ensure};

pub fn run(args: PrepareArgs) -> Result<CommandOutcome> {
    let input_dir = args.input_dir.canonicalize()?;
    let recovered_dir = PathBuf::from("recovered").canonicalize()?;
    let merged_dir = args
        .output_dir
        .unwrap_or_else(|| PathBuf::from("merged"))
        .canonicalize()?;

    fs::create_dir_all(&recovered_dir)?;
    fs::create_dir_all(&merged_dir)?;

    ensure!(
        !input_dir.starts_with(&recovered_dir)
            && !input_dir.starts_with(&merged_dir)
            && !recovered_dir.starts_with(&merged_dir)
            && !merged_dir.starts_with(&recovered_dir),
        "Output directories must be separate and must not contain the input directory"
    );

    let recordings = discovery::find_recordings(&input_dir)?
        .into_iter()
        .filter(|path| !path.starts_with(&recovered_dir) && !path.starts_with(&merged_dir))
        .collect::<Vec<_>>();

    ensure!(
        !recordings.is_empty(),
        "No MCAP recordings found in {}",
        input_dir.display()
    );

    let recovered = discovery::find_recordings(&recovered_dir)?;
    let merged = discovery::find_recordings(&merged_dir)?;

    if recordings.is_empty() {
        anyhow::bail!("No MCAP recordings found in {}", args.input_dir.display());
    }

    let mut failed = Vec::new();

    for recording in &recordings {
        eprintln!("Validating {}", recording.display());

        let status = crate::mcap_cli::doctor(recording)?;

        if !status.success() {
            eprintln!("Validation failed for {}: {status}", recording.display());
            failed.push(recording);
        }
    }

    if !failed.is_empty() {
        anyhow::bail!(
            "{} recording(s) failed validation; recovery is not implemented yet",
            failed.len()
        );
    }

    Ok(CommandOutcome::Success)
}

enum RecoveryOutcome {
    Usable(CommandOutcome),
    Empty,
}

fn recover(input: &Path, output: &Path) -> Result<RecoveryOutcome> {}
