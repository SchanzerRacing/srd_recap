use anyhow::{Context, Result, ensure};
use regex::Regex;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::LazyLock,
};
use walkdir::WalkDir;

static ROSBAG_TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^rosbag2_[0-9]{4}_[0-9]{2}_[0-9]{2}-[0-9]{2}_[0-9]{2}_[0-9]{2}$")
        .expect("rosbag timestamp regex must be valid")
});

#[derive(Debug, PartialEq)]
pub struct RecordingGroup {
    pub output: PathBuf,
    pub parts: Vec<PathBuf>,
}

pub fn find_recordings(root: &Path, excluded: &[&Path]) -> Result<Vec<PathBuf>> {
    let mut recordings = Vec::new();

    let entries = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !excluded.iter().any(|path| entry.path().starts_with(path)));

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

pub fn group_recordings(recordings: Vec<PathBuf>, root: &Path) -> Result<Vec<RecordingGroup>> {
    // Relative output path -> part index -> source path.
    // None represents a standalone recording without a numeric suffix.
    let mut groups: BTreeMap<PathBuf, BTreeMap<Option<u64>, PathBuf>> = BTreeMap::new();

    for recording in recordings {
        let relative = recording.strip_prefix(root)?;

        let stem = relative
            .file_stem()
            .and_then(|value| value.to_str())
            .context("Recording filename is not valid UTF-8")?;

        let (base, index) = split_recording_stem(stem)?;

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

fn split_recording_stem(stem: &str) -> Result<(&str, Option<u64>)> {
    if ROSBAG_TIMESTAMP.is_match(stem) {
        return Ok((stem, None));
    }

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

    Ok((base, index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn returns_error_when_paths_dont_exist() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;

        assert!(find_recordings(temp_dir.path().join("nonexistent").as_path(), &[]).is_err());

        Ok(())
    }

    #[test]
    fn returns_empty_when_no_recordings_exist() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;

        let recordings = find_recordings(temp_dir.path(), &[])?;

        assert!(recordings.is_empty());

        Ok(())
    }

    #[test]
    fn ignores_non_mcap_files() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let mcap_file = temp_dir.path().join("some.mcap");
        let other_file = temp_dir.path().join("other_file");

        fs::write(&mcap_file, &[])?;
        fs::write(&other_file, &[])?;

        let recordings = find_recordings(temp_dir.path(), &[])?;

        assert_eq!(recordings, vec![mcap_file]);

        Ok(())
    }

    #[test]
    fn finds_recordings_recursively() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let original_file = temp_dir.path().join("original.mcap");
        let nested_dir = temp_dir.path().join("session");
        let nested_file = nested_dir.join("nested.mcap");

        fs::create_dir_all(&nested_dir)?;
        fs::write(&original_file, &[])?;
        fs::write(&nested_file, &[])?;

        let mut recordings = find_recordings(temp_dir.path(), &[])?;
        let mut expected = vec![original_file, nested_file];

        recordings.sort();
        expected.sort();

        assert_eq!(recordings, expected);

        Ok(())
    }

    #[test]
    fn excludes_directory_subtrees() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let root = temp_dir.path();
        let original_file = root.join("original.mcap");
        let some_excluded_dir = root.join("recovered");
        let some_excluded_file = some_excluded_dir.join("some.mcap");
        let other_excluded_dir = root.join("merged");
        let other_excluded_file = other_excluded_dir.join("other.mcap");
        let nested_dir = other_excluded_dir.join("nested");
        let nested_file = nested_dir.join("nested.mcap");
        let nonexistent_dir = root.join("nonexistent");

        fs::create_dir_all(&some_excluded_dir)?;
        fs::create_dir_all(&nested_dir)?;

        fs::write(&original_file, &[])?;
        fs::write(&some_excluded_file, &[])?;
        fs::write(&other_excluded_file, &[])?;
        fs::write(&nested_file, &[])?;

        let excluded = vec![
            some_excluded_dir.as_path(),
            other_excluded_dir.as_path(),
            nonexistent_dir.as_path(),
        ];

        let recordings = find_recordings(root, &excluded)?;

        assert_eq!(recordings, vec![original_file]);

        Ok(())
    }

    #[test]
    fn returns_empty_groups() -> Result<()> {
        let root = Path::new("root");
        assert!(group_recordings(vec![], root)?.is_empty());

        Ok(())
    }

    #[test]
    fn rejects_recordings_outside_root() -> Result<()> {
        let root = Path::new("root");
        let recordings = vec![Path::new("other_root").join("other.mcap")];

        assert!(group_recordings(recordings, root).is_err());

        Ok(())
    }

    #[test]
    fn groups_standalone_recordings() -> Result<()> {
        let root = Path::new("root");
        let recordings = vec![root.join("some.mcap"), root.join("other.mcap")];

        let groups = group_recordings(recordings, root)?;

        let expected = vec![
            RecordingGroup {
                output: PathBuf::from("other.mcap"),
                parts: vec![root.join("other.mcap")],
            },
            RecordingGroup {
                output: PathBuf::from("some.mcap"),
                parts: vec![root.join("some.mcap")],
            },
        ];

        assert_eq!(groups, expected);

        Ok(())
    }

    #[test]
    fn preserves_nonnumeric_suffixes() -> Result<()> {
        let root = Path::new("root");

        let groups = group_recordings(vec![root.join("rec_final.mcap")], root)?;

        let expected = vec![RecordingGroup {
            output: PathBuf::from("rec_final.mcap"),
            parts: vec![root.join("rec_final.mcap")],
        }];

        assert_eq!(groups, expected);

        Ok(())
    }

    #[test]
    fn groups_parts_in_numeric_order() -> Result<()> {
        let root = Path::new("root");
        let recordings = vec![
            root.join("rec_10.mcap"),
            root.join("rec_1.mcap"),
            root.join("rec_2.mcap"),
        ];

        let groups = group_recordings(recordings, root)?;

        let expected = vec![RecordingGroup {
            output: PathBuf::from("rec.mcap"),
            parts: vec![
                root.join("rec_1.mcap"),
                root.join("rec_2.mcap"),
                root.join("rec_10.mcap"),
            ],
        }];

        assert_eq!(groups, expected);

        Ok(())
    }

    #[test]
    fn keeps_sessions_separate() -> Result<()> {
        let root = Path::new("root");
        let recordings = vec![
            root.join("a/rec_0.mcap"),
            root.join("a/rec_1.mcap"),
            root.join("b/rec_0.mcap"),
        ];

        let groups = group_recordings(recordings, root)?;

        let expected = vec![
            RecordingGroup {
                output: PathBuf::from("a/rec.mcap"),
                parts: vec![root.join("a/rec_0.mcap"), root.join("a/rec_1.mcap")],
            },
            RecordingGroup {
                output: PathBuf::from("b/rec.mcap"),
                parts: vec![root.join("b/rec_0.mcap")],
            },
        ];

        assert_eq!(groups, expected);

        Ok(())
    }

    #[test]
    fn rejects_standalone_split_conflict() -> Result<()> {
        let root = Path::new("root");
        let recordings = vec![root.join("rec.mcap"), root.join("rec_0.mcap")];

        assert!(group_recordings(recordings, root).is_err());

        let recordings = vec![root.join("rec_0.mcap"), root.join("rec.mcap")];

        assert!(group_recordings(recordings, root).is_err());

        Ok(())
    }

    #[test]
    fn rejects_duplicate_part_indices() -> Result<()> {
        let root = Path::new("root");
        let recordings = vec![root.join("rec_1.mcap"), root.join("rec_01.mcap")];

        assert!(group_recordings(recordings, root).is_err());

        Ok(())
    }

    #[test]
    fn preserves_timestamp_without_part_index() -> Result<()> {
        let root = Path::new("root");
        let recordings = vec![root.join("rosbag2_2026_08_14-16_31_11.mcap")];

        let groups = group_recordings(recordings, root)?;

        let expected = vec![RecordingGroup {
            output: PathBuf::from("rosbag2_2026_08_14-16_31_11.mcap"),
            parts: vec![root.join("rosbag2_2026_08_14-16_31_11.mcap")],
        }];

        assert_eq!(groups, expected);

        Ok(())
    }

    #[test]
    fn splits_timestamp_and_part_index() -> Result<()> {
        let root = Path::new("root");
        let recordings = vec![root.join("rosbag2_2026_08_14-16_31_11_0.mcap")];

        let groups = group_recordings(recordings, root)?;

        let expected = vec![RecordingGroup {
            output: PathBuf::from("rosbag2_2026_08_14-16_31_11.mcap"),
            parts: vec![root.join("rosbag2_2026_08_14-16_31_11_0.mcap")],
        }];

        assert_eq!(groups, expected);

        Ok(())
    }
}
