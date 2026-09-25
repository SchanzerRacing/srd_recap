use crate::{
    CommandOutcome,
    cli::AnalyzeArgs,
    utils::{decoding::Decoder, discovery},
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use memmap2::Mmap;
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, PartialEq)]
struct DrivingSegment {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
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

    for recording in recordings {
        eprintln!("Analyzing: {}", recording.display());
        analyze_recording(&recording, &output_dir)?;
    }

    Ok(CommandOutcome::Success)
}

fn analyze_recording(recording: &Path, _output: &Path) -> Result<()> {
    let fd = fs::File::open(recording).context("Couldn't open MCAP file")?;
    let mapped = unsafe { Mmap::map(&fd) }.context("Couldn't map MCAP file")?;
    let summary = mcap::Summary::read(&mapped)
        .context("Couldn't read MCAP file summary")?
        .expect("summary should exist");
    let stats = summary.stats.expect("summary should contain stats");

    println!("Creating decoders for {} channels", summary.channels.len());

    let mut decoders = HashMap::<u16, Decoder>::new();

    for channel in summary.channels.values() {
        let schema = channel.schema.as_deref().expect("message should have a schema");
        let decoder = Decoder::new(&schema.name, schema.data.as_ref())?;
        decoders.insert(channel.id, decoder);
    }

    println!("Decoding {} messages", stats.message_count);

    let mut driving_start: Option<DateTime<Utc>> = None;
    let mut segments = Vec::new();

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

            if state == 3 {
                if driving_start.is_none() {
                    driving_start = Some(stamp);
                }
            } else if let Some(start) = driving_start.take() {
                segments.push(DrivingSegment {
                    start,
                    end: stamp,
                });
            }
        }
    }

    if segments.is_empty() {
        return Ok(());
    }

    for segment in segments {
        println!(
            "Found {:.3}s driving segment starting at {}",
            (segment.end - segment.start).as_seconds_f64(),
            segment.start.format("%H:%M:%S%.3f").to_string()
        );
    }

    Ok(())
}
