# Vision Lab Architecture

Vision Lab is a local, macOS-first vision experimentation platform. The first
implementation is a single Tauri host process, but the primary architecture is
the communication contract between independently owned components.

## Component Ownership

The host wires these components together:

- `camera-mac` owns frame acquisition and camera lifecycle.
- `vision-processing` owns detector selection, ROI/template state, processing,
  and detections.
- `recorder` owns session capture and all recording counters.
- `system-mirror` owns the UI-facing normalized system view.
- `operator-ui` renders React state and sends commands; it does not reconcile
  component event streams.

No component mutates another component's state. Cross-component changes happen
through typed async commands, snapshots, event subscriptions, and frame streams.

## Communication Roles

Vision Lab uses six communication roles deliberately:

- Commands are explicit async requests. A command response means
  accepted/rejected, not that UI state has already converged.
- Events are immutable facts emitted by the state owner for live updates.
- State snapshots are authoritative recovery points and include a monotonic
  `revision`.
- Logs are diagnostics and must not be treated as state.
- Streams carry high-rate image payloads. Frames are never JSON command replies
  or ordinary event payloads.
- Presence is host-local in v1: component identity plus runtime instance ID.

Every event envelope includes source identity, event ID, timestamp, per-runtime
sequence, and optional correlation ID. Consumers refresh snapshots when they
observe a sequence gap or lagged subscription.

## Runtime Shape

The local implementation is actor-like:

```text
React UI
  |
  | Tauri commands + Tauri events
  v
system-mirror  <--- component event streams + snapshots
  |
  +--> CameraApi   -> camera actor -> latest frame stream
  +--> VisionApi   -> vision actor -> detection events
  +--> RecorderApi -> recorder actor -> session files
```

`comm-core` contains semantic envelopes and IDs. `comm-local` contains bounded
mailboxes, event buses, latest-value streams, state cells, and monotonic
counters. Domain contracts live in `vision-contracts`, which intentionally does
not depend on detector implementation crates.

## Frame Path

Camera frames use a latest-value stream backed by `tokio::sync::watch`.
Publishing a new frame replaces the previous value instead of waiting for slow
consumers. Components report dropped or replaced frame counts as state, not as a
backpressure mechanism.

The Tauri backend emits the latest frame to React on a dedicated `frame` event.
`SystemView` carries only metadata, state, counters, recent event summaries, ROI,
and detection overlays.

## Algorithm Integration

`vision-processing` exposes normalized `AlgorithmId` values. Concrete algorithm
crate types stay internal to adapter implementations.

Implemented in v1:

- `TemplateNcc`: normalized cross-correlation template matching.
- `RadialSymmetry`: direct `radsym` circle detections from grayscale frames.
- `ChessCorners`: direct `chess-corners` corner detections from grayscale frames.
- `CalibrationTarget`: `calib-targets` chessboard detection with default params.

Selectable extension points:

- `RingGridTarget`: `ringgrid`, deferred until the public commands carry target
  layout configuration.
- `EdgeModelMatch`: intended for `vision-metrology` after that crate is
  published

The public detection contract remains stable across adapters: frame ID,
timestamp, confidence/score, optional bounding box, optional points, method, and
latency.

## Recorder Format

Recorder sessions are written under the Tauri process working directory in
`recordings/session-*` by default. A session contains:

- `manifest.json`
- `frames/` with grayscale PGM frames
- `frames.jsonl`
- `vision-events.jsonl`
- `detections.jsonl`

The recorder samples frames at a bounded rate and drops excess frames rather
than blocking camera or vision processing.

## Validation

Required checks:

```sh
./scripts/quality.sh
```

Run the desktop prototype with:

```sh
bun install --cwd apps/operator-ui/ui
cargo tauri dev --manifest-path apps/operator-ui/src-tauri/Cargo.toml
```
