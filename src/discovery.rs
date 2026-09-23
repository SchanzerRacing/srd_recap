use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

type RecordingGroups = BTreeMap<PathBuf, BTreeMap<Option<u64>, PathBuf>>;

fn group_recordings(recordings: Vec<PathBuf>, input_dir: &Path) -> Result<RecordingGroups> {
    let mut groups = RecordingGroups::new();

    for recording in recordings {
        let relative = recording.strip_prefix(input_dir)?;
        let stem = relative
            .file_stem()
            .and_then(|stem| stem.to_str())
            .context("Recording filename is not valid UTF-8")?;

        let (base, index) = match stem.rsplit_once('_') {
            Some((base, suffix))
                if !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                (base, Some(suffix.parse::<u64>()?))
            }
            _ => (stem, None),
        };

        let output = relative
            .parent()
            .unwrap_or(Path::new(""))
            .join(format!("{base}.mcap"));

        let parts = groups.entry(output.clone()).or_default();

        // Avoid treating run.mcap and run_0.mcap as one group.
        ensure!(
            parts.is_empty() || parts.keys().all(|key| key.is_some() == index.is_some()),
            "Standalone and split recordings conflict for {}",
            output.display()
        );

        ensure!(
            !parts.contains_key(&index),
            "Duplicate part index for {}",
            output.display()
        );

        parts.insert(index, recording);
    }

    Ok(groups)
}

pub fn find_recordings(input: &Path) -> Result<Vec<PathBuf>> {
    let metadata = fs::symlink_metadata(input)
        .with_context(|| format!("Cannot inspect {}", input.display()))?;

    let mut recordings = Vec::new();

    if metadata.is_file() && is_mcap(input) {
        recordings.push(input.to_path_buf());
    } else if metadata.is_dir() {
        visit_directory(input, &mut recordings)?;
    } else {
        bail!("Expected an MCAP file or directory: {}", input.display());
    }

    recordings.sort();
    Ok(recordings)
}

fn visit_directory(
    directory: &Path,
    recordings: &mut Vec<PathBuf>,
) -> Result<()> {
    let entries = fs::read_dir(directory)
        .with_context(|| format!("Cannot read {}", directory.display()))?;

    for entry in entries {
        let entry = entry
            .with_context(|| format!("Cannot read entry in {}", directory.display()))?;
        let path = entry.path();
        let file_type = entry.file_type()
            .with_context(|| format!("Cannot inspect {}", path.display()))?;

        if file_type.is_dir() {
            visit_directory(&path, recordings)?;
        } else if file_type.is_file() && is_mcap(&path) {
            recordings.push(path);
        }
    }

    Ok(())
}

fn is_mcap(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "mcap")
}
