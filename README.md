# srd_recap

`srd_recap` is a Rust CLI project for recovering, merging, and analyzing MCAP
recordings from recorded test sessions. The goal is to extract key performance
indicators (KPIs), identify relevant segments, and automatically upload selected
clips to Foxglove with metadata and event annotations.

## Planned workflow

1. **Recover** interrupted or damaged recordings, preserving the original files
   and reporting whether recovery was complete or involved data loss.
2. **Merge** selected files from the same recording session into a usable timeline.
3. **Analyze** recording health and vehicle signals to calculate KPIs and detect
   relevant time intervals.
4. **Extract** standalone MCAP clips with configurable context before and after
   events, combining overlapping selections.
5. **Publish** selected clips to Foxglove and attach run summaries and event KPIs
   through Foxglove's native metadata system.

Initial detection targets include autonomous driving intervals, fault events,
and sensor gaps. Localization jumps and control tracking errors can follow when
the required signals are available.

## Planned command interface

Each operation will have its own subcommand. These names describe the intended
interface; argument syntax is still to be defined.

| Command | Purpose |
| --- | --- |
| `recap recover` | Salvage a recording and report its recovery outcome. |
| `recap merge` | Combine recordings from one session. |
| `recap analyze` | Extract KPIs, summarize recording health, and detect events. |
| `recap extract` | Export selected time intervals as MCAP clips. |
| `recap publish` | Upload clips and create or update Foxglove annotations. |
| `recap process <directory>` | Run a configured sequence of processing stages. |

## KPI extraction and annotations

KPI extraction is a core part of the project. Analysis results should feed both
clip-selection rules and the properties published to Foxglove.

The first analysis milestone will cover recording duration, per-topic message
counts and rates, and the largest message gaps. Signal decoding will then enable
driving duration, maximum speed, and distance. Further KPIs depend on the signals
present in each recording.

The planned Foxglove mapping is:

| Measurement scope | Annotation target | Example values |
| --- | --- | --- |
| Entire run | Session properties | Mission, completion status, distance, maximum speed, energy consumption, tracking-error RMS |
| Lap or maneuver | Typed event properties | Lap time, peak lateral acceleration, steering saturation duration |
| Incident | Typed event properties | Fault ID, localization correction magnitude, sensor-gap duration |

Each KPI should retain its units, measurement interval, data coverage, missing
inputs, validity, and definition version. Clip padding must not change the
measurement interval. Partially recovered recordings must remain identifiable
when comparing results. The publishing design should also allow annotations to
be updated after recalculation without uploading the recording again.

## Implementation direction

Recovery and merging will invoke the external `mcap` CLI through
`std::process::Command`, with arguments passed individually. A local adapter such
as `mcap_cli.rs` will handle executable discovery, version reporting, subprocess
output, and recovery outcomes. It must distinguish full recovery, partial
recovery with data loss, and failure according to the supported CLI version.
The CLI integration still needs to be implemented and validated.

The remaining design centers on a few boundaries:

- **Container reading and signal decoding:** use the Rust MCAP reader for records
  and handle ROS 2 payload decoding separately, starting with the message types
  needed for the first KPIs.
- **Detection and extraction:** detectors produce a start time, end time, and
  selection reason. Shared extraction logic handles padding and overlap merging.
- **Clip context:** preserve the static transforms, calibration, and any video
  decoding dependencies needed to inspect exported clips.
- **Publishing state:** persist clip identities, selection reasons, and
  upload/import status so interrupted work can resume without duplicate uploads.

## Development

With a Rust toolchain supporting edition 2024 and Cargo installed, run from the
repository root:

```sh
cargo build --locked
cargo run --locked
```

The current program prints `Hello, world!`. No MCAP processing or Foxglove
credentials are needed to run the scaffold.

The planned recovery and merge commands will require the `mcap` executable to be
installed separately and available on `PATH`.

## Roadmap

- [ ] Add the MCAP CLI adapter, check executable availability and version, and
  recover one real recording through `mcap recover`.
- [ ] Merge two real recording files through `mcap merge` and report recovery
  outcomes explicitly.
- [ ] Produce a JSON analysis report with recording and per-topic statistics.
- [ ] Decode mission/state and velocity signals for driving segmentation and KPIs.
- [ ] Add a fault detector and export selected clips with surrounding context.
- [ ] Publish clips with native Foxglove session/event properties and persistent
  upload history.
- [ ] Connect the stages through configurable directory processing.

## License

[MIT](LICENSE).
