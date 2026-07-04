# Codex Handoff — Vision Lab Communication Prototype

## Goal

Create a macOS-first monorepo prototype that validates a component communication architecture with a real MacBook camera, live vision processing, and a Tauri + React UI.

The project must grow into a local vision experimentation platform for learning objects and comparing detection approaches:

- classical normalized cross-correlation
- edge / contour matching
- later: keypoint matching and ML-based detection

The first version must be minimal, runnable, and structurally correct. Do not build a generic framework or distributed production system yet.

---

## Core architecture

The system has independently owned components:

```text
camera-mac        owns camera acquisition and frame production
vision-processing owns detector configuration, object templates, processing, results
recorder          owns session recording and replay metadata
operator-ui       Tauri + React interface
```

Initially, all components run **in one Tauri host process**.

However, each component must expose a typed async public API so it can later be used either:

1. as an in-process library client, or
2. as a client of the same component running behind IPC.

Do not expose component internals directly to consumers.

```text
UI / orchestrator
      │
      ├── CameraApi
      ├── VisionApi
      └── RecorderApi
              │
     local in-process adapters now
              │
       component runtime / actor
```

Future IPC support is intentionally out of scope for this first implementation, but the interfaces and contracts must not prevent it.

---

## Communication rules

Use these six distinct roles conceptually:

- **commands**: explicit async requests to change state or start work
- **events**: immutable facts for live updates
- **state**: current authoritative snapshot, used for recovery
- **logs**: structured diagnostics, not state
- **streams**: high-rate image and debug payloads
- **presence**: runtime liveness; in this monolith prototype, host-local presence is sufficient

### Required semantics

- Commands are asynchronous.
- A command response means only accepted/rejected, never “the UI state is now updated.”
- Components own their own state. Other components request changes by commands; they never mutate another component's state directly.
- UI state updates from component events and snapshots, regardless of whether a UI action, another component, or automation caused the change.
- Every component state has a monotonic `revision: u64`.
- Every component event has a monotonically increasing `sequence: u64` per runtime instance.
- Events include `source`, `event_id`, `timestamp`, `sequence`, optional `correlation_id`, and typed payload.
- State snapshots include `source`, `timestamp`, `revision`, and typed payload.
- On a sequence gap or component restart, consumers refresh state.
- Keep command IDs and correlation IDs even in the local-only implementation.
- Frames are a dedicated stream, never command replies or ordinary JSON events.
- Slow consumers must not block camera acquisition. Use bounded/latest-value behavior and count dropped frames.

---

## Repository structure

Create a Cargo workspace plus a Tauri v2 application with a React + TypeScript frontend.

```text
vision-lab/
├── Cargo.toml
├── rust-toolchain.toml
├── README.md
├── crates/
│   ├── comm-core/
│   ├── comm-local/
│   ├── vision-contracts/
│   ├── camera-mac/
│   ├── vision-processing/
│   ├── recorder/
│   └── system-mirror/
├── apps/
│   └── operator-ui/
│       ├── src-tauri/
│       └── ui/
└── docs/
    └── architecture.md
```

Use stable Rust and Tokio. Use current Tauri v2 conventions. Keep dependencies modest.

---

## Crate responsibilities

### `comm-core`

Pure contracts and shared types only. No Tauri, no AVFoundation, no detector implementation.

Provide:

```rust
ComponentId {
  component_type: String,
  component_name: String,
}

RuntimeInstanceId(String) // generated at startup; opaque

ComponentIdentity {
  component: ComponentId,
  instance_id: RuntimeInstanceId,
  version: String,
}

CommandId(Uuid)
CorrelationId(Uuid)
EventId(Uuid)
OperationId(Uuid)

Versioned<T> {
  source: ComponentIdentity,
  timestamp: SystemTime,
  revision: u64,
  value: T,
}

EventEnvelope<T> {
  event_id: EventId,
  source: ComponentIdentity,
  timestamp: SystemTime,
  sequence: u64,
  correlation_id: Option<CorrelationId>,
  payload: T,
}

CommandReceipt {
  command_id: CommandId,
  operation_id: Option<OperationId>,
  accepted_revision: u64,
}
```

Also provide a small generic API error type and an `EventStream<T>` alias based on `tokio::sync::broadcast` or a stream wrapper.

Do not over-generalize transport traits yet. Define only reusable primitives required by the local adapters.

### `comm-local`

In-process communication support.

Provide helpers for:

- typed command submission through Tokio channels
- event publication / subscription
- latest state snapshot storage
- bounded/latest-value frame handoff
- sequence/revision generation

The component runtime should be actor-like: externally visible state changes are serialized through one mailbox. No consumer may mutate component state via shared locks.

### `vision-contracts`

Typed public contracts for all domain components.

Define:

```rust
trait CameraApi: Send + Sync {
  async fn submit(&self, command: CameraCommand) -> Result<CommandReceipt, ApiError>;
  async fn get_state(&self) -> Result<Versioned<CameraState>, ApiError>;
  async fn subscribe(&self) -> Result<CameraEventStream, ApiError>;
  async fn subscribe_frames(&self) -> Result<FrameStream, ApiError>;
}

trait VisionApi: Send + Sync {
  async fn submit(&self, command: VisionCommand) -> Result<CommandReceipt, ApiError>;
  async fn get_state(&self) -> Result<Versioned<VisionState>, ApiError>;
  async fn subscribe(&self) -> Result<VisionEventStream, ApiError>;
}

trait RecorderApi: Send + Sync {
  async fn submit(&self, command: RecorderCommand) -> Result<CommandReceipt, ApiError>;
  async fn get_state(&self) -> Result<Versioned<RecorderState>, ApiError>;
  async fn subscribe(&self) -> Result<RecorderEventStream, ApiError>;
}
```

Use `async-trait` if needed.

Define minimal public types:

```rust
FrameMeta {
  frame_id: u64,
  timestamp: SystemTime,
  width: u32,
  height: u32,
  stride: u32,
  pixel_format: Gray8 | Rgb8,
}

Frame {
  meta: FrameMeta,
  bytes: Bytes,
}

Detection {
  frame_id: u64,
  timestamp: SystemTime,
  object_id: String,
  confidence: f32,
  bbox: RectF32,
  method: DetectorId,
  latency_us: u64,
}
```

Initial detector selection:

```rust
enum DetectorId {
  NormalizedCrossCorrelation,
  EdgeContourPlaceholder,
}
```

`EdgeContourPlaceholder` may report “not implemented” in v1, but the contracts and UI must make it selectable later.

### `camera-mac`

Owns macOS camera acquisition.

Requirements:

- use the built-in MacBook camera;
- implement camera lifecycle: `Disconnected | Connecting | Ready | Streaming | Error`;
- commands: `Connect`, `StartStream`, `StopStream`, `SetRequestedFps`;
- publish `CameraState` and lifecycle events;
- generate `Frame` records with monotonic `frame_id`;
- prefer grayscale output to simplify vision processing;
- bounded frame delivery: camera must continue running if a consumer is slow;
- count dropped frames;
- report a useful error if camera permission is missing or unavailable.

Use a suitable macOS Rust binding or a small native bridge. Keep the bridge isolated inside this crate. Do not make the rest of the workspace depend on AVFoundation types.

If direct AVFoundation integration delays the project, implement a clearly separated fallback `camera-sim` feature that produces a moving synthetic target, but leave the real camera implementation as the default target.

### `vision-processing`

Owns detector execution and object/template state.

Requirements:

- subscribe to `CameraApi` frame stream;
- commands:
  - `SelectDetector(DetectorId)`
  - `SetRoi(Option<RectF32>)`
  - `CaptureTemplate`
  - `StartProcessing`
  - `StopProcessing`
- `CaptureTemplate` takes the latest frame and ROI. Convert ROI to grayscale template storage.
- implement normalized cross-correlation template matching as the first real detector;
- processing must run asynchronously and must not block camera acquisition;
- process newest available frame rather than accumulating unbounded backlog;
- publish detection events and `VisionState`.

Minimal `VisionState`:

```rust
VisionState {
  lifecycle: Idle | WaitingForTemplate | Processing | Error,
  selected_detector: DetectorId,
  roi: Option<RectF32>,
  has_template: bool,
  input_fps: f32,
  processing_fps: f32,
  mean_latency_ms: f32,
  dropped_input_frames: u64,
  last_detection: Option<Detection>,
  error: Option<String>,
}
```

Do not require OpenCV initially. Prefer the user's existing Rust image-processing libraries when practical. Keep the detector interface internal and simple:

```rust
trait Detector: Send {
  fn detect(&mut self, frame: &Frame) -> Result<Option<Detection>, DetectorError>;
}
```

### `recorder`

Minimal but real recording component.

Requirements:

- commands: `StartRecording`, `StopRecording`;
- subscribes to camera frames and vision events;
- writes a session directory with:
  - selected raw grayscale frames at a bounded configurable rate;
  - JSONL for frame metadata, detections, and component events;
  - a manifest with app version, detector config, camera state, timestamps;
- must not block camera or vision processing;
- maintain recording state and dropped-frame counters.

No replay UI is required in the first slice, but write data in a format suitable for replay later.

### `system-mirror`

A host-side model for the UI. React must not subscribe directly to individual component internals.

Responsibilities:

- obtain initial snapshots from camera, vision, recorder;
- subscribe to their events;
- maintain a normalized system view;
- detect event sequence gaps and re-fetch snapshot;
- expose a serializable `SystemView` to Tauri;
- bridge updates to the frontend through Tauri events.

The system view should include component status, current state, last error, active operations if applicable, and recent event timeline.

---

## Operator UI

Implement a compact but usable Tauri + React UI.

Use React + TypeScript. Keep the UI clean and functional. A simple state store is enough.

Required screens/areas:

1. **System status**
   - camera / vision / recorder lifecycle
   - errors and dropped-frame counters

2. **Live camera panel**
   - render latest grayscale or RGB frame
   - draw selectable ROI
   - show detection bounding box and confidence

3. **Vision controls**
   - connect/start/stop camera
   - choose detector
   - set/clear ROI
   - capture template
   - start/stop processing

4. **Recorder controls**
   - start/stop recording
   - display output session path and counts

5. **Event timeline**
   - show recent typed events with source, sequence, correlation ID, and summary

The frontend must receive a normalized `SystemView` plus latest frame/overlay updates from the Tauri backend. It must not contain communication reconciliation logic.

---

## First end-to-end scenario

The app is complete when this works:

```text
1. Launch the Tauri app.
2. Connect to the MacBook camera.
3. Start camera streaming.
4. Display live camera frames.
5. User draws an ROI around an object.
6. User captures a template from the latest frame.
7. User starts normalized cross-correlation processing.
8. UI displays live detection box, confidence, FPS, latency, and dropped counts.
9. User starts recording.
10. Recorder writes a reproducible session directory with frames, events, detections, and manifest.
11. Stop processing, recording, and stream cleanly.
```

Also include a developer-only fault injection panel or commands for:

- pause/delay vision processing;
- drop every Nth vision event;
- restart the vision component runtime;
- start a duplicate simulated camera only when using simulation mode.

The system mirror should recover from state snapshots after an event sequence gap or runtime restart.

---

## Non-goals for first implementation

Do not implement these yet:

- remote IPC / Zenoh transport
- service discovery across hosts
- authentication or authorization
- durable event replay
- generic schema registry
- database persistence
- ML training or model hosting
- full object annotation tooling
- multiple camera synchronization
- a generic plugin system
- production-grade recorder throughput guarantees

Leave clear extension points, but do not add speculative complexity.

---

## Quality bar

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- unit tests for:
  - revision / sequence behavior;
  - local event subscription;
  - mirror resync after a simulated sequence gap;
  - normalized cross-correlation finds a known synthetic target;
- one integration test for the camera-sim feature covering the complete command → event → state → detection flow;
- README with macOS setup, camera permission notes, build/run commands, and architecture summary.

Use explicit error handling. Do not use panics for expected runtime failures.

---

## Suggested implementation order

1. Workspace, `comm-core`, `comm-local`, `vision-contracts`.
2. `camera-sim` feature and complete local camera API.
3. `vision-processing` with normalized cross-correlation.
4. `system-mirror` and a minimal Tauri UI.
5. `recorder`.
6. Real MacBook camera integration.
7. Fault injection and integration tests.
8. Improve UI only after the architecture and full vertical flow work.

---

## Design constraints to preserve

- State is authoritative; events are for responsiveness.
- Events can be duplicated or missed; sequence gaps trigger state refresh.
- Commands are intent, not state truth.
- A component owns and publishes its own state.
- UI observes state changes from the actual owner even when another component caused them.
- Image frames use an efficient, bounded, latest-oriented stream path.
- The local APIs are the future semantic contract for IPC clients.
- Keep the first version small enough to run and debug entirely on one MacBook.
