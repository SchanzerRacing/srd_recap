use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

pub struct RecordingGroup {
    pub output: PathBuf,
    pub parts: Vec<PathBuf>,
}

pub fn find_recordings(
    root: &Path,
    excluded: &[&Path],
) -> Result<Vec<PathBuf>> {
    let mut recordings = Vec::new();

    let entries = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            !excluded.iter().any(|path| entry.path().starts_with(path))
        });

    for entry in entries {
        let entry = entry?;

        let is_mcap = entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "mcap");

        if entry.file_type().is_file() && is_mcap {
            recordings.push(entry.into_path());
        }
    }

    Ok(recordings)
}

pub fn group_recordings(
    recordings: Vec<PathBuf>,
    root: &Path,
) -> Result<Vec<RecordingGroup>> {
    // Relative output path -> part index -> source path.
    // None represents a standalone recording without a numeric suffix.
    let mut groups: BTreeMap<PathBuf, BTreeMap<Option<u64>, PathBuf>> =
        BTreeMap::new();

    for recording in recordings {
        let relative = recording.strip_prefix(root)?;

        let stem = relative
            .file_stem()
            .and_then(|value| value.to_str())
            .context("Recording filename is not valid UTF-8")?;

        let (base, index) = match stem.rsplit_once('_') {
            Some((base, suffix))
                if !base.is_empty()
                    && !suffix.is_empty()
                    && suffix.bytes().all(|byte| byte.is_ascii_digit()) =>
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

        ensure!(
            parts.keys().all(|key| key.is_some() == index.is_some()),
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

    Ok(groups
        .into_iter()
        .map(|(output, parts)| RecordingGroup {
            output,
            parts: parts.into_values().collect(),
        })
        .collect())
}
