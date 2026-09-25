use crate::{
    CommandOutcome,
    cli::AnalyzeArgs,
    utils::{decoding::Decoder, discovery},
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use memmap2::Mmap;
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
struct RecordingReport {
    mcap: PathBuf,
    segments: Vec<SegmentReport>,
}

#[derive(Serialize)]
struct SegmentReport {
    segment: DrivingSegment,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
struct DrivingSegment {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    start_truncated: bool,
    end_truncated: bool,
    successful: bool,
}

pub fn run(args: AnalyzeArgs) -> Result<CommandOutcome> {
    let input_dir = args
        .input
        .canonicalize()
        .context("Could not resolve the input directory")?;

    ensure!(input_dir.is_dir(), "Input must be a directory");

    let output_dir = args.output.unwrap_or_else(|| input_dir.clone());

    fs::create_dir_all(&output_dir)?;

    let output_dir = output_dir.canonicalize()?;

    let recordings = discovery::find_recordings(&input_dir, &[])?;

    ensure!(
        !recordings.is_empty(),
        "No MCAP recordings found in {}",
        input_dir.display()
    );

    let mut reports = Vec::<RecordingReport>::new();

    for recording in recordings {
        println!("Analyzing: {}", recording.display());
        reports.push(analyze_recording(&recording)?);
    }

    let summary = output_dir.join("summary.json");

    println!("Writing summary to {}", summary.display());
    write_summary(&summary, &reports)?;

    Ok(CommandOutcome::Success)
}

fn analyze_recording(recording: &Path) -> Result<RecordingReport> {
    let fd = fs::File::open(recording).context("Couldn't open MCAP file")?;
    let mapped = unsafe { Mmap::map(&fd) }.context("Couldn't map MCAP file")?;
    let summary = mcap::Summary::read(&mapped)
        .context("Couldn't read MCAP file summary")?
        .expect("summary should exist");
    let stats = summary.stats.expect("summary should contain stats");

    println!("Creating decoders for {} channels", summary.channels.len());

    let mut decoders = HashMap::<u16, Decoder>::new();

    for channel in summary.channels.values() {
        let schema = channel
            .schema
            .as_deref()
            .expect("message should have a schema");
        let decoder = Decoder::new(&schema.name, schema.data.as_ref())?;
        decoders.insert(channel.id, decoder);
    }

    println!("Decoding {} messages", stats.message_count);

    let mut tracker = DrivingSegmentTracker::new();

    for message in mcap::MessageStream::new(&mapped)? {
        let message = message?;

        let decoded = decoders
            .get(&message.channel.id)
            .expect(format!("decoder for {} should exist", message.channel.topic).as_str())
            .decode(&message.data)?;

        if message.channel.topic == "/as_state" {
            let state = decoded.u8("state")?;
            let secs = decoded.i32("header.stamp.sec")?;
            let nsecs = decoded.u32("header.stamp.nanosec")?;
            let stamp = DateTime::from_timestamp(secs as i64, nsecs as u32)
                .expect("timestamp should be valid");
            tracker.observe(stamp, state);
        }
    }

    let segments = tracker.finalize();

    let mut reports = Vec::<SegmentReport>::new();

    for segment in segments {
        println!(
            "Found {}successful {:.3}s driving segment starting at {}",
            if segment.successful { "" } else { "un" },
            (segment.end - segment.start).as_seconds_f64(),
            segment.start.format("%H:%M:%S%.3f").to_string(),
        );

        if segment.start_truncated {
            eprintln!("Segment start is truncated");
        }

        if segment.end_truncated {
            eprintln!("Segment end is truncated");
        }

        reports.push(SegmentReport { segment: segment })
    }

    let file_name = recording
        .file_name()
        .expect("recording file name should exist");

    Ok(RecordingReport {
        mcap: PathBuf::from(file_name),
        segments: reports,
    })
}

fn write_summary(output: &Path, reports: &[RecordingReport]) -> Result<()> {
    let fd = fs::File::create(output).context(format!("Could not create {}", output.display()))?;
    serde_json::to_writer_pretty(fd, reports).context("Failed to serialize segment reports")?;
    Ok(())
}

struct DrivingSegmentTracker {
    segments: Vec<DrivingSegment>,
    was_driving: Option<bool>,
}

impl DrivingSegmentTracker {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            was_driving: None,
        }
    }

    pub fn observe(&mut self, timestamp: DateTime<Utc>, state: u8) -> () {
        match (self.was_driving, state) {
            (None, 3) => {
                self.segments.push(DrivingSegment {
                    start: timestamp,
                    end: timestamp,
                    start_truncated: true,
                    end_truncated: true,
                    successful: false,
                });
            }
            (Some(false), 3) => {
                self.segments.push(DrivingSegment {
                    start: timestamp,
                    end: timestamp,
                    start_truncated: false,
                    end_truncated: true,
                    successful: false,
                });
            }
            (Some(true), 3) => {
                if let Some(segment) = self.segments.last_mut() {
                    segment.end = timestamp;
                }
            }
            (Some(true), 1) | (Some(true), 2) | (Some(true), 4) => {
                if let Some(segment) = self.segments.last_mut() {
                    segment.end = timestamp;
                    segment.end_truncated = false;
                }
            }
            (Some(true), 5) => {
                if let Some(segment) = self.segments.last_mut() {
                    segment.end = timestamp;
                    segment.end_truncated = false;
                    segment.successful = true;
                }
            }
            _ => (),
        }

        self.was_driving = Some(state == 3);
    }

    pub fn finalize(self) -> Vec<DrivingSegment> {
        self.segments
    }
}
