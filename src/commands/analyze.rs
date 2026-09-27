use crate::{
    CommandOutcome,
    cli::AnalyzeArgs,
    tracking::{segments, stats},
    utils::{decoding::Decoder, discovery},
};
use anyhow::{Context, Result, ensure};
use chrono::DateTime;
use memmap2::Mmap;
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    os::unix::process,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
struct RecordingReport {
    mcap: PathBuf,
    segments: Vec<SegmentReport>,
}

impl RecordingReport {
    pub fn new(mcap: &str) -> Self {
        Self {
            mcap: PathBuf::from(mcap),
            segments: Vec::<SegmentReport>::new(),
        }
    }
}

#[derive(Serialize, Default)]
struct SegmentReport {
    start: i64,
    end: i64,
    start_truncated: bool,
    end_truncated: bool,
    successful: bool,
    travel_distance: Option<f64>,
    max_velocity: Option<f64>,
    avg_velocity: Option<f64>
}

#[derive(Default)]
struct Kpis {
    travel_distance: stats::BaselineDelta,
    velocity: stats::MinMaxMeanStdDev,
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
    let file = fs::File::open(recording).context("Couldn't open MCAP file")?;
    let filename = recording
        .file_name()
        .and_then(|name| name.to_str())
        .context("path has no valid UTF-8 filename")?;

    let mapped = unsafe { Mmap::map(&file) }.context("Couldn't map MCAP file")?;
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

    let mut report = RecordingReport::new(filename);
    let mut tracker = segments::Tracker::default();
    let mut kpis = Kpis::default();

    for message in mcap::MessageStream::new(&mapped)? {
        let message = message?;

        let decoded = decoders
            .get(&message.channel.id)
            .expect(format!("decoder for {} should exist", message.channel.topic).as_str())
            .decode(&message.data)?;

        if message.channel.topic == "/as_state" {
            let state = decoded.u8("state")?;
            let secs = decoded.i32("header.stamp.sec")? as i64;
            let nsecs = decoded.u32("header.stamp.nanosec")? as i64;
            let stamp = secs * 1_000_000_000 + nsecs;

            if let Some(event) = tracker.observe(stamp, state) {
                process_segment_event(event, &mut report, &mut kpis);
            }
            continue;
        }

        if !tracker.driving() {
            continue;
        }

        if message.channel.topic == "/travel_distance" {
            kpis.travel_distance.sample(decoded.f64("data")?);
            continue;
        }

        if message.channel.topic == "/est_vel" {
            kpis.velocity.sample(decoded.f64("twist.linear.x")?);
            continue;
        }
    }

    if let Some(event) = tracker.finish() {
        process_segment_event(event, &mut report, &mut kpis);
    }

    for segment in &report.segments {
        let t0 = DateTime::from_timestamp_nanos(segment.start);
        let t1 = DateTime::from_timestamp_nanos(segment.end);

        println!(
            "Found {}successful {:.3}s driving segment starting at {}",
            if segment.successful { "" } else { "un" },
            (t1 - t0).as_seconds_f64(),
            t0.format("%H:%M:%S%.3f").to_string(),
        );

        if segment.start_truncated {
            eprintln!("Segment start is truncated");
        }

        if segment.end_truncated {
            eprintln!("Segment end is truncated");
        }
    }

    Ok(report)
}

fn process_segment_event(event: segments::Event, report: &mut RecordingReport, kpis: &mut Kpis) {
    match event {
        segments::Event::Start {
            timestamp,
            truncated,
        } => {
            let segment = SegmentReport {
                start: timestamp,
                start_truncated: truncated,
                ..Default::default()
            };

            report.segments.push(segment);
            *kpis = Kpis::default();
        }
        segments::Event::Stop {
            timestamp,
            truncated,
            successful,
        } => {
            let segment = report
                .segments
                .last_mut()
                .expect("a stop event should have a matching start event");

            segment.end = timestamp;
            segment.end_truncated = truncated;
            segment.successful = successful;
            segment.travel_distance = kpis.travel_distance.delta();
            segment.max_velocity = kpis.velocity.max();
            segment.avg_velocity = kpis.velocity.mean();
        }
    }
}

fn write_summary(output: &Path, reports: &[RecordingReport]) -> Result<()> {
    let fd = fs::File::create(output).context(format!("Could not create {}", output.display()))?;
    serde_json::to_writer_pretty(fd, reports).context("Failed to serialize segment reports")?;
    Ok(())
}
