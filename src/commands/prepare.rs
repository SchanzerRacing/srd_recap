use anyhow::{Context, Result, bail, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{CommandOutcome, cli::PrepareArgs, discovery, mcap_cli::McapCli};

pub fn run(args: PrepareArgs, mcap: &impl McapCli) -> Result<CommandOutcome> {
    let input_dir = args
        .input_dir
        .canonicalize()
        .context("Could not resolve the input directory")?;

    ensure!(input_dir.is_dir(), "Input must be a directory");

    let output_base = args.output_dir.unwrap_or_else(|| input_dir.clone());
    let recovered_dir = output_base.join("recovered");
    let merged_dir = output_base.join("merged");

    fs::create_dir_all(&recovered_dir)?;
    fs::create_dir_all(&merged_dir)?;

    let recovered_dir = recovered_dir.canonicalize()?;
    let merged_dir = merged_dir.canonicalize()?;

    ensure!(
        !input_dir.starts_with(&recovered_dir) && !input_dir.starts_with(&merged_dir),
        "An output directory cannot equal or contain the input directory"
    );

    ensure!(
        !recovered_dir.starts_with(&merged_dir) && !merged_dir.starts_with(&recovered_dir),
        "Recovered and merged directories must be separate"
    );

    let recordings = discovery::find_recordings(&input_dir, &[&recovered_dir, &merged_dir])?;

    ensure!(
        !recordings.is_empty(),
        "No MCAP recordings found in {}",
        input_dir.display()
    );

    let groups = discovery::group_recordings(recordings, &input_dir)?;
    let mut had_warnings = false;

    for group in groups {
        let merged_path = merged_dir.join(&group.output);

        if merged_path.is_file() {
            eprintln!("Already prepared: {}", merged_path.display());
            continue;
        }

        let mut usable_parts = Vec::new();

        for source in group.parts {
            let relative = source.strip_prefix(&input_dir)?;
            let recovered_path = recovered_dir.join(relative);

            match usable_recording(&source, &recovered_path, &mut had_warnings, mcap)? {
                Some(path) => usable_parts.push(path),
                None => {
                    eprintln!("Warning: skipping {}: no usable messages", source.display());
                    had_warnings = true;
                }
            }
        }

        if usable_parts.is_empty() {
            eprintln!(
                "Warning: skipping group {}: no usable recordings",
                group.output.display()
            );
            had_warnings = true;
            continue;
        }

        write_output(&merged_path, |temporary| {
            match usable_parts.as_slice() {
                [single] => {
                    fs::copy(single, temporary)?;
                }
                multiple => {
                    let status = mcap.merge(multiple, temporary)?;

                    ensure!(
                        status.success(),
                        "Merge failed for {}: {status}",
                        merged_path.display()
                    );
                }
            }

            Ok(true)
        })?;

        eprintln!("Prepared: {}", merged_path.display());
    }

    Ok(if had_warnings {
        CommandOutcome::Warnings
    } else {
        CommandOutcome::Success
    })
}

fn usable_recording(
    source: &Path,
    recovered: &Path,
    had_warnings: &mut bool,
    mcap: &impl McapCli,
) -> Result<Option<PathBuf>> {
    // Reuse a previously completed recovery.
    if recovered.is_file() {
        eprintln!("Reusing recovered recording: {}", recovered.display());
        return nonempty_recording(recovered, mcap);
    }

    // An empty file cannot contain any messages.
    if fs::metadata(source)?.len() == 0 {
        return Ok(None);
    }

    let status = mcap.doctor(source)?;

    match status.code() {
        Some(0) => return nonempty_recording(source, mcap),
        Some(1) => {} // Attempt recovery.
        _ => bail!("mcap doctor failed for {}: {status}", source.display()),
    }

    write_output(recovered, |temporary| {
        let status = mcap.recover(source, temporary)?;

        match status.code() {
            Some(0) => {}
            Some(3) => {
                eprintln!("Warning: recovered {} with data loss", source.display());
                *had_warnings = true;
            }
            _ => bail!("Recovery failed for {}: {status}", source.display()),
        }

        // Header/footer-only input may recover to a valid, empty MCAP.
        // Returning false discards that temporary output.
        mcap.has_messages(temporary)
    })
}

fn nonempty_recording(path: &Path, mcap: &impl McapCli) -> Result<Option<PathBuf>> {
    Ok(if mcap.has_messages(path)? {
        Some(path.to_path_buf())
    } else {
        None
    })
}

// The callback returns true to publish the file, or false to discard it.
fn write_output(
    destination: &Path,
    write: impl FnOnce(&Path) -> Result<bool>,
) -> Result<Option<PathBuf>> {
    let parent = destination
        .parent()
        .context("Output path has no parent directory")?;

    fs::create_dir_all(parent)?;

    let temporary_dir = tempfile::tempdir_in(parent)?;
    let temporary = temporary_dir.path().join("output.mcap");

    if !write(&temporary)? {
        return Ok(None);
    }

    fs::rename(&temporary, destination)
        .with_context(|| format!("Could not publish {}", destination.display()))?;

    Ok(Some(destination.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::ExitStatus;

    struct NoMcapCalls;

    impl McapCli for NoMcapCalls {
        fn doctor(&self, _file: &Path) -> Result<ExitStatus> {
            panic!("doctor should not be called");
        }

        fn recover(&self, _input: &Path, _output: &Path) -> Result<ExitStatus> {
            panic!("recover should not be called");
        }

        fn merge(&self, _inputs: &[PathBuf], _output: &Path) -> Result<ExitStatus> {
            panic!("merge should not be called");
        }

        fn has_messages(&self, _file: &Path) -> Result<bool> {
            panic!("has_messages should not be called");
        }
    }

    #[test]
    fn publishes_completed_output() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let source = temp_dir.path().join("source.mcap");
        let destination = temp_dir.path().join("nested/destination.mcap");

        fs::write(&source, b"test")?;

        let result = write_output(&destination, |temporary| {
            fs::copy(source, temporary)?;
            Ok(true)
        })?;

        assert_eq!(result, Some(destination.clone()));

        let contents = fs::read(&destination)?;

        assert_eq!(contents, b"test");

        Ok(())
    }

    #[test]
    fn discards_empty_output() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let source = temp_dir.path().join("source.mcap");
        let destination = temp_dir.path().join("destination.mcap");

        fs::write(&source, b"test")?;

        let result = write_output(&destination, |temporary| {
            fs::copy(source, temporary)?;
            Ok(false)
        })?;

        assert_eq!(result, None);
        assert!(!destination.is_file());

        Ok(())
    }

    #[test]
    fn preserves_existing_output_when_discarded() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let source = temp_dir.path().join("source.mcap");
        let destination = temp_dir.path().join("destination.mcap");

        fs::write(&source, b"test")?;
        fs::write(&destination, b"preserved")?;

        let result = write_output(&destination, |temporary| {
            fs::copy(source, temporary)?;
            Ok(false)
        })?;

        assert_eq!(result, None);

        let contents = fs::read(&destination)?;

        assert_eq!(contents, b"preserved");

        Ok(())
    }

    #[test]
    fn preserves_existing_output_on_failure() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let source = temp_dir.path().join("source.mcap");
        let destination = temp_dir.path().join("destination.mcap");

        fs::write(&source, b"test")?;
        fs::write(&destination, b"preserved")?;

        let result = write_output(&destination, |temporary| {
            fs::copy(source, temporary)?;
            bail!("Simulated write failure");
        });

        assert!(result.is_err());

        let contents = fs::read(&destination)?;

        assert_eq!(contents, b"preserved");

        Ok(())
    }

    #[test]
    fn rejects_missing_source() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let source = temp_dir.path().join("some.mcap");
        let recovered = temp_dir.path().join("recovered/some.mcap");

        let mut had_warnings = false;
        let result = usable_recording(&source, &recovered, &mut had_warnings, &NoMcapCalls);

        assert!(result.is_err());
        assert!(!had_warnings);
        assert!(!recovered.is_file());

        Ok(())
    }

    #[test]
    fn skips_empty_source() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let source = temp_dir.path().join("some.mcap");
        let recovered = temp_dir.path().join("recovered/some.mcap");

        fs::write(&source, &[])?;

        let mut had_warnings = false;
        let result = usable_recording(&source, &recovered, &mut had_warnings, &NoMcapCalls)?;

        assert_eq!(result, None);
        assert!(!had_warnings);
        assert!(!recovered.is_file());

        Ok(())
    }
}
