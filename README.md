# srd_recap

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

`recap` is a Rust CLI for preparing MCAP recordings from test sessions. It
recursively discovers recordings, recovers damaged files, and merges split
recordings. KPI extraction, event detection, clip extraction, and publishing to
Foxglove are planned.

## Installation

Install Rust and Cargo using the [official Rust installation instructions](
https://www.rust-lang.org/tools/install).

Build and install from the repository root with a Rust toolchain supporting
edition 2024:

```sh
cargo install --path . --locked
recap --help
```

Ensure Cargo's binary directory (normally `~/.cargo/bin`) is on `PATH`. Rerun the
install command after code changes to update the installed executable.

Preparation also requires the external `mcap` CLI on `PATH`, with `doctor`,
`recover`, `merge`, and `info` commands. The adapter reads the `messages:` count
from `mcap info` output. See the [official MCAP CLI installation instructions](
https://mcap.dev/guides/cli).

## Commands

| Command                                      | Status                                                                                      |
| -------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `recap prepare [DIRECTORY] [-o DIRECTORY]`   | Discover, validate, recover, and merge recordings. Input defaults to the current directory. |
| `recap analyze [INPUT] [-o FILE]`            | Placeholder; returns an explicit not-implemented error.                                     |
| `recap upload [INPUT] --device ID [-r FILE]` | Placeholder; returns an explicit not-implemented error.                                     |

Use `recap <COMMAND> --help` for arguments and options.

```sh
recap prepare ./recordings
recap prepare ./recordings -o ./processed -v
```

## Preparation behavior

`-o`/`--output-dir` selects a base directory for both `recovered/` and `merged/`.
By default, that base is the input directory. Relative paths within the input
are preserved:

```text
recordings/
├── session/
│   ├── run_0.mcap
│   └── run_1.mcap
├── recovered/
│   └── session/         # Recovered inputs, when needed
└── merged/
    └── session/
        └── run.mcap
```

Discovery searches recursively for lowercase `.mcap` files and excludes the
selected recovered and merged directories.

For each group, preparation:

1. Skips an existing merged output and prints `Already prepared`.
2. Reuses existing recovered files, printing `Reusing recovered recording`.
3. Skips zero-byte sources; otherwise runs `mcap doctor`. Doctor exit code `1`
   triggers recovery, while other nonzero outcomes fail preparation.
4. Uses `mcap info` to check for messages. Empty recordings are skipped with a
   warning. Recovery exit code `3` is accepted with a data-loss warning.
5. Copies a single usable part or calls `mcap merge` for multiple usable parts.
   A group with no usable parts is skipped with a warning.

## KPI extraction and annotations

KPI extraction is a core part of the project. Analysis results should feed both
clip-selection rules and the properties published to Foxglove.

The first analysis milestone will cover recording duration, per-topic message
counts and rates, and the largest message gaps. Signal decoding will then enable
driving duration, maximum speed, and distance. Further KPIs depend on the signals
present in each recording.

The planned Foxglove mapping is:

| Measurement scope | Annotation target      | Example values                                                                              |
| ----------------- | ---------------------- | ------------------------------------------------------------------------------------------- |
| Entire run        | Session properties     | Mission, completion status, distance, maximum speed, energy consumption, tracking-error RMS |
| Lap or maneuver   | Typed event properties | Lap time, peak lateral acceleration, steering saturation duration                           |
| Incident          | Typed event properties | Fault ID, localization correction magnitude, sensor-gap duration                            |

Each KPI should retain its units, measurement interval, data coverage, missing
inputs, validity, and definition version. Clip padding must not change the
measurement interval. Partially recovered recordings must remain identifiable
when comparing results. The publishing design should also allow annotations to
be updated after recalculation without uploading the recording again.

## Development

```sh
cargo build --locked
cargo run --locked -- prepare --help
cargo test --locked
```

The code is organized into argument parsing (`src/cli.rs`), discovery and filename
grouping (`src/discovery.rs`), preparation (`src/commands/prepare.rs`), and external
MCAP calls (`src/mcap_cli.rs`). Preparation accepts the `McapCli` trait; production
uses `RealMcapCli`, while tests can supply fake implementations.

Unit tests cover discovery, grouping (including ROS bag timestamp names), output
publication and preservation, and empty or missing sources. These tests do not
establish compatibility with an installed MCAP CLI or replace validation using
real recordings.

## Roadmap

- [x] Add the external MCAP CLI adapter and preparation pipeline.
- [x] Group split recordings numerically and preserve standalone ROS bag timestamps.
- [ ] Expand tests for recovery outcomes and real MCAP CLI integration.
- [ ] Produce a JSON analysis report with recording and per-topic statistics.
- [ ] Decode mission/state and velocity signals for driving segmentation and KPIs.
- [ ] Add fault detection and export selected clips with surrounding context.
- [ ] Upload clips with native Foxglove session/event properties and persistent
  upload history.
- [ ] Connect the stages through configurable directory processing.
