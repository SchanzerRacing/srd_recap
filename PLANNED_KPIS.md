# Planned mission analysis and KPIs

This plan groups analysis into mission context, vehicle motion, stream timing,
message latency, and synchronization. It describes planned functionality.

## Shared scope and measurement rules

Calculate driving KPIs only while `/as_state` is `DRIVE`. Continue reading mission
and state signals outside these intervals to establish context and completion.
Group intervals by mission attempt; the selected mission alone does not identify
an attempt. The grouping policy depends on the actual mission/state definitions.

Use MCAP log time as the initial common basis for interval membership. Process
signals in timestamp order and use `[start, end)` boundaries: include entry into
`DRIVE`, exclude exit. Define ordering for equal timestamps. Timestamp subtraction
for latency is separate from interval membership; flag measurements whose source
reference predates entry into `DRIVE`.

### Timestamp vocabulary

| Timestamp | Meaning and use |
| --- | --- |
| Message stamp | Reference carried in the payload. May represent acquisition, image receipt, pose update, or message creation; verify per source. |
| Publication time | Message availability at the publisher, when genuinely available. Verify recorder conventions for missing or substituted values. |
| Recorded time | MCAP log time: observation at the recorder. Initial basis for cadence and interval membership. |
| Subscriber receipt / callback time | Instrumented timing at the consuming node, needed to measure actual synchronization wait and scheduling delay. |

Subtract timestamps only when their clocks are compatible and synchronized.
Record timestamp provenance and fallback usage; a populated publication timestamp
is not proof of actual publication timing. Negative ages/latencies, missing
stamps, and clock jumps are invalid measurements. Signed offsets between different
streams are meaningful and must retain their sign.

### Validity and aggregation

- Report units, interval, signal source, coverage, valid sample/pair counts, and
  invalid counts alongside each KPI. Missing data is unavailable, not zero.
- Preserve incomplete boundaries and recovery data loss in the result. Store the
  KPI-definition version and calculation policies for reproducibility.
- Do not connect samples across driving intervals, clock discontinuities, or
  invalid measurements. Do not connect delay-jitter pairs across timestamp-basis
  changes. Aggregate compatible results only, retaining fallback distinctions.
- Motion integration and differentiation require gap thresholds. Cadence analysis
  must preserve long gaps within valid intervals because they reveal stalls.
- Motion averages are time-weighted. Timing delay and jitter means are arithmetic
  means over valid observations/pairs. Aggregate sums and counts, not unweighted
  averages of interval averages.
- Extra context around exported clips must not extend KPI measurement intervals.

## Mission and driving intervals

| Information | Planned interpretation |
| --- | --- |
| Selected mission | Most recently observed mission selection; unknown until established by a recorded signal. |
| Driving start | Transition into `DRIVE`; repeated `DRIVE` messages do not open another interval. |
| Driving end | Transition out of `DRIVE`. |
| Driving duration | Sum of driving-interval durations for the attempt. Includes stationary time while in `DRIVE`. |
| Completion outcome | Completed, aborted, or unknown, based on explicit state transitions or mission-result signals. |

Leaving `DRIVE` does not by itself prove completion. A successful result may arrive
after the driving interval has ended. An explicit failure, emergency, or abort
signal can establish an aborted outcome; ambiguous exits remain unknown.

Boundary and missing-data handling:

- If the first observed state is `DRIVE`, start at that observation and mark the
  beginning as incomplete. Do not infer when driving began before it.
- If the recording ends during `DRIVE`, close the observed interval at the
  recording end and mark its end as incomplete. Do not infer mission completion.
- If mission selection changes during `DRIVE`, split the interval or flag the
  attempt as inconsistent. The policy is still to be decided.
- Retaining the last state between updates assumes it remains valid. If state
  updates are periodic, define a gap threshold beyond which state is uncertain.
- Define an ordering policy for state and measurement messages with identical
  timestamps before implementing boundary calculations.

## Vehicle motion

### Speed and distance

| KPI | Planned calculation | Source |
| --- | --- | --- |
| Maximum speed | Largest valid speed magnitude observed during `DRIVE`. | Speed or velocity topic, to be identified. |
| Distance traveled | Prefer the change in `/travel_distance` over each driving interval, if it is a cumulative distance counter. Sum across intervals belonging to the attempt. | `/travel_distance`. |
| Average speed | Distance divided by elapsed driving time covering the same measurement interval. | Distance and driving intervals, subject to matching data coverage. |

Confirm `/travel_distance` semantics before using endpoint subtraction:

- Units and whether the value is cumulative or incremental.
- Whether it resets at startup, mission selection, entry into `DRIVE`, or another
  event.
- Whether reverse motion increases total traveled distance.
- Update frequency and how to estimate values at driving boundaries.

For a cumulative counter without resets, interval distance is
`counter_at_end - counter_at_start`. A reset during an interval requires explicit
handling; a negative difference must not silently become traveled distance.

If the counter is unsuitable or unavailable, integrating speed is a possible
fallback. Trapezoidal integration between valid samples estimates distance as:

```text
distance += (previous_speed + current_speed) / 2 * elapsed_seconds
```

Use speed magnitude for traveled distance. Signed longitudinal velocity would
subtract reverse motion. Do not integrate across separate driving intervals or
long data gaps. Define boundary interpolation and gap thresholds before use.

Average speed must be time-weighted, not a simple average of irregularly spaced
samples. Stationary periods during `DRIVE` count toward the duration. If data
coverage is incomplete, report partial distance and calculate an average only
over matching covered time, clearly labeling it as a covered-time average.

### Steering activity

| KPI | Planned calculation |
| --- | --- |
| Maximum absolute steering velocity | Largest valid absolute steering-velocity value during `DRIVE`. |
| Average absolute steering velocity | Integral of absolute steering velocity divided by the corresponding covered driving time. |

Absolute values prevent left and right movements from canceling. Prefer a measured
steering-velocity signal when available. Otherwise, estimate from steering angle:

```text
steering_velocity = (current_angle - previous_angle) / elapsed_seconds
```

Confirm whether the source is actual or commanded steering, which angle it
represents, and whether units are radians or degrees. Report velocity in `rad/s`
or `degrees/s` consistently.

Do not differentiate across driving boundaries, long gaps, or nonpositive time
steps. Differentiation amplifies noise; define any filtering and spike rejection
before interpreting maxima. Handle angle wrapping if applicable to the signal.

## Stream cadence

Apply one rate and period-jitter calculation to each configured topic separately.
Use recorded timestamps, since payload stamps can refer to older source data.

```text
period[i] = recorded_time[i] - recorded_time[i - 1]
observed_rate_hz = number_of_valid_periods / sum(valid_periods_in_seconds)
period_jitter[i] = abs(period[i] - reference_period)
```

Report observed rate in Hz and maximum/mean period jitter in milliseconds. Prefer
an expected period configured per topic. Otherwise use the mean observed period
within the driving interval and label that reference explicitly: it measures
variability, not compliance with an intended rate. Confirm whether a stream is
periodic or event-driven before treating its variability as a fault.

Rate is the reciprocal of mean period, not the mean of instantaneous rates.
Combine period counts and durations across intervals without including gaps
between them. Flag nonpositive periods; retain long valid gaps. Fewer than two
eligible messages gives unavailable rate and jitter estimates. Report total
messages, usable periods, and rejected timing observations.

### Topic mapping

| Stream | Topic selection |
| --- | --- |
| Image | Candidate `/yolov8_pose/compressed/image`; confirm pipeline stage and report each camera separately. |
| Map | Candidates `/map`, `/map/global`, `/map/local`, `/map/start`; select relevant stream(s). |
| Path | Candidate `/slam_node/path`; confirm it is the intended path update. |
| Estimated velocity | `/est_vel` |
| Commanded velocity | `/cmd_vel` |
| EKF body velocity | `/sbg/ekf_vel_body` |

## Message age and pipeline latency

Use a shared calculation with explicitly configured reference stamps and endpoints:

```text
delay = endpoint_time - message_stamp
```

| Measurement | Reference stamp | Endpoint | Requested summaries |
| --- | --- | --- | --- |
| Image-to-path update | Path stamp, expected to retain the corresponding image receipt time. | Path publication time; recorded time when publication time is unavailable. | Delay summaries to finalize (mean/maximum and possibly percentiles); maximum and mean delay jitter. |
| EKF velocity age | `/sbg/ekf_vel_body` stamp. | Publication time, falling back to recorded time. | Minimum, maximum, mean age; maximum and mean delay jitter. |
| Estimated velocity age | `/est_vel` stamp. | Publication time, falling back to recorded time. | Minimum, maximum, mean age; maximum and mean delay jitter. |
| Control-command latency | `/cmd_vel` stamp, expected to identify the pose-update reference. | Recorded time, even if publication time exists. | Minimum, maximum, mean latency; maximum and mean delay jitter. |

Report delays in milliseconds and retain counts and endpoint provenance. A
recorded-time endpoint can include transport and recorder delays. Optional
stamp-to-publication and publication-to-recording breakdowns can separate those
observations when both timestamps are meaningful.

### Delay variation

The proposed delay-jitter convention is the absolute change between consecutive
valid delays from the same stream and timestamp basis:

```text
delay_jitter[i] = abs(delay[i] - delay[i - 1])
maximum_delay_jitter = max(delay_jitter)
average_delay_jitter = sum(delay_jitter) / number_of_valid_pairs
```

Apply this to every latency, message-age, and waiting-time measurement, including
any optional stage-delay breakdowns. Report milliseconds and valid pair
counts; fewer than two eligible updates gives unavailable jitter. This differs
from period jitter: it measures changes in delay, not deviation of update spacing
from a reference period. Keep both definitions versioned.

### Interpretation requirements

**Image-to-path:** verify the path stamp and corresponding image source. A stamp
carrying image capture time measures capture-to-update delay; one carrying path
creation time does not measure image-processing latency. Define correspondence
when several images contribute to a path or one image produces several updates.

**Velocity age:** these per-stream measurements help explain why one member of a
synchronized pair becomes available later, but do not establish pairing or actual
subscriber wait on their own.

**Control command:** verify the `/cmd_vel` schema contains the intended stamp; a
topic name alone does not imply a header. The desired interpretation spans SLAM's
pose-update reference through the new command computed from that pose and the
latest available path. A command-creation stamp instead measures a shorter stage.
Recorded time does not establish actuator receipt or application. The age of the
less frequently updated path is a separate contribution; establish path/pose/
command association before attributing delay to individual pipeline stages.

## Cross-stream synchronization

Analyze associated `/sbg/ekf_vel_body` and `/est_vel` messages using the same pair
set for stamp alignment and arrival-gap measurements. Keep these separate from
per-stream message age and cadence.

### Pair association

Prefer recorded pair identities or SLAM callback instrumentation. Offline
reconstruction needs the synchronizer implementation, configuration, queue sizes,
timestamp fields, and arrival order. Label inferred pairs as estimates when the
actual choices cannot be established; nearest-stamp matching alone is not proof
of the pairs selected by the approximate-time synchronizer.

Report unmatched messages, observable drops, and boundary exclusions separately.
Accepted pairs can hide measurements rejected by the synchronization policy. Do
not pair across driving intervals or clock discontinuities. Missing streams make
pair metrics unavailable.

### Stamp alignment

```text
signed_stamp_offset = est_vel.stamp - sbg.stamp
stamp_mismatch = abs(signed_stamp_offset)
```

Report minimum, maximum, and mean absolute mismatch in milliseconds, plus pair
counts and signed offsets showing which measurement is newer. The maximum is an
observed accepted-pair mismatch, not a guaranteed window for future runs.

### Arrival gap and waiting

Using comparable timing observations `a_est` and `a_sbg`:

```text
arrival_offset = a_est - a_sbg
pair_arrival_gap = abs(arrival_offset)
estimated_sbg_wait = max(0, arrival_offset)
estimated_est_wait = max(0, -arrival_offset)
```

Report minimum, maximum, and mean pair arrival gap and which stream arrives later.
Also report maximum and mean delay jitter for the arrival gap and each stream's
estimated wait, using consecutive associated pairs in a consistent order and
timestamp basis. Preserve which stream is waiting when interpreting these values.
Prefer SLAM subscriber receipt timestamps. Publication times indicate publisher
availability and recorded times indicate recorder observation; either is only a
proxy for subscriber arrival. Label proxy-based waits and avoid mixing timestamp
bases within a pair without explicitly flagging it.

The earlier member waits for the later member to become available. Actual
callback delay may additionally include synchronizer policy, queueing, and
executor scheduling, which require callback timing to measure.

## Representative recording reference

The following inventory was supplied as a reference from one recording. Counts
cover the supplied recording, not just `DRIVE` intervals. They are observations,
not expected rates. Duration and message timestamps are needed to calculate rate
and jitter; equal counts do not establish message correspondence.

`/travel_distance`, `/est_vel`, and `/cmd_vel` are absent from this inventory.
Keep their planned metrics conditional on availability; do not silently substitute
another topic. No dedicated mission-selection topic is apparent from the names;
inspect `srd_interfaces/msg/ASState` to determine whether it contains mission and
completion information.

For steering, inspect `/steering_state` and `/steering_motor/joint_states` before
choosing a signal. Confirm populated fields, joint identity, units, and whether
they describe motor motion or vehicle steering. For speed, inspect
`/sbg/ekf_vel_body` and `/imu/velocity`, including coordinate frame and validity.
Topic names and types alone do not establish these semantics.

| Topic | Message type | Messages |
| --- | --- | ---: |
| `/as_state` | `srd_interfaces/msg/ASState` | 12524 |
| `/autox/planner/transition_event` | `lifecycle_msgs/msg/TransitionEvent` | 2 |
| `/detections` | `srd_interfaces/msg/InferenceStamped` | 3621 |
| `/diagnostics` | `diagnostic_msgs/msg/DiagnosticArray` | 5698 |
| `/diagnostics_agg` | `diagnostic_msgs/msg/DiagnosticArray` | 125 |
| `/diagnostics_toplevel_state` | `diagnostic_msgs/msg/DiagnosticStatus` | 125 |
| `/ebs_trigger` | `std_msgs/msg/Bool` | 12524 |
| `/imu/data` | `sensor_msgs/msg/Imu` | 3069 |
| `/imu/nav_sat_fix` | `sensor_msgs/msg/NavSatFix` | 619 |
| `/imu/pos_ecef` | `geometry_msgs/msg/PointStamped` | 3088 |
| `/imu/temp` | `sensor_msgs/msg/Temperature` | 24641 |
| `/imu/utc_ref` | `sensor_msgs/msg/TimeReference` | 125 |
| `/imu/velocity` | `geometry_msgs/msg/TwistStamped` | 33941 |
| `/map` | `sensor_msgs/msg/PointCloud2` | 231 |
| `/map/global` | `sensor_msgs/msg/PointCloud2` | 231 |
| `/map/local` | `sensor_msgs/msg/PointCloud2` | 231 |
| `/map/start` | `sensor_msgs/msg/PointCloud2` | 231 |
| `/observations` | `sensor_msgs/msg/PointCloud2` | 3621 |
| `/parameter_events` | `rcl_interfaces/msg/ParameterEvent` | 445 |
| `/robot_description` | `std_msgs/msg/String` | 1 |
| `/rosout` | `rcl_interfaces/msg/Log` | 4618 |
| `/sbg/ekf_euler` | `sbg_driver/msg/SbgEkfEuler` | 3103 |
| `/sbg/ekf_nav` | `sbg_driver/msg/SbgEkfNav` | 3086 |
| `/sbg/ekf_quat` | `sbg_driver/msg/SbgEkfQuat` | 3099 |
| `/sbg/ekf_rot_accel_body` | `sbg_driver/msg/SbgEkfRotAccel` | 24856 |
| `/sbg/ekf_rot_accel_ned` | `sbg_driver/msg/SbgEkfRotAccel` | 3093 |
| `/sbg/ekf_vel_body` | `sbg_driver/msg/SbgEkfVelBody` | 24726 |
| `/sbg/gps_hdt` | `sbg_driver/msg/SbgGpsHdt` | 620 |
| `/sbg/gps_pos` | `sbg_driver/msg/SbgGpsPos` | 620 |
| `/sbg/gps_vel` | `sbg_driver/msg/SbgGpsVel` | 619 |
| `/sbg/imu_data` | `sbg_driver/msg/SbgImuData` | 24653 |
| `/sbg/status` | `sbg_driver/msg/SbgStatus` | 125 |
| `/sbg/utc_time` | `sbg_driver/msg/SbgUtcTime` | 125 |
| `/scs_error` | `srd_interfaces/msg/SCSError` | 12516 |
| `/slam_node/fov_polygon` | `geometry_msgs/msg/PolygonStamped` | 1 |
| `/slam_node/path` | `nav_msgs/msg/Path` | 231 |
| `/slam_node/visualization` | `visualization_msgs/msg/MarkerArray` | 231 |
| `/steering_motor/joint_states` | `sensor_msgs/msg/JointState` | 12531 |
| `/steering_motor/rpdo` | `canopen_interfaces/msg/COData` | 87721 |
| `/steering_state` | `srd_interfaces/msg/SteeringState` | 1253 |
| `/tf_static` | `tf2_msgs/msg/TFMessage` | 4 |
| `/yolov8_pose/compressed/image` | `sensor_msgs/msg/CompressedImage` | 3621 |
| `/yolov8_pose/resize/camera_info` | `sensor_msgs/msg/CameraInfo` | 3621 |

## Implementation sequence and open decisions

1. Inspect schemas and representative values for the configured topics. Resolve
   missing signals, units, coordinate frames, counter reset behavior, and stamp
   semantics before selecting decoders and field mappings.
2. Define mission attempts, completion/abort evidence, equal-timestamp ordering,
   and incomplete-state handling; implement and test driving-interval extraction.
3. Implement shared cadence, delay, and jitter calculations with validity and
   provenance reporting. Confirm expected periods and the proposed jitter
   conventions; finalize image-to-path delay summary statistics.
4. Establish synchronizer pairing and available receipt/callback instrumentation;
   verify image/path and pose/command timestamp propagation.
5. Implement motion calculations with boundary interpolation, gap thresholds,
   reset handling, and steering filtering. Validate representative recordings
   before comparing mission results.
