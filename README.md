# Vision Lab

Vision Lab is a macOS-first local vision experimentation prototype. The first
version runs all components in one Tauri host process, but communication
contracts are treated as the primary architecture.

Read [docs/architecture.md](docs/architecture.md) for the component model,
command/event/state semantics, and validation workflow.

## Workspace

- `crates/comm-core`: shared IDs, envelopes, snapshots, receipts, and API errors.
- `crates/comm-local`: local command mailboxes, event buses, state cells, and
  latest-value streams.
- `crates/vision-contracts`: public camera, vision, recorder, frame, state, and
  detection contracts.
- `crates/camera-mac`: camera component; v1 defaults to a deterministic
  simulator.
- `crates/vision-processing`: template NCC processor and detector adapter
  boundary.
- `crates/recorder`: bounded session recorder.
- `crates/system-mirror`: UI-facing normalized system view and resync logic.
- `apps/operator-ui`: Tauri v2 + React + Vite + TypeScript desktop app.

## Setup

Requirements:

- macOS
- stable Rust
- Bun
- Tauri prerequisites for macOS development

Install frontend dependencies:

```sh
bun install --cwd apps/operator-ui/ui
```

Run the full quality gate:

```sh
./scripts/quality.sh
```

Run the desktop app:

```sh
cargo tauri dev --manifest-path apps/operator-ui/src-tauri/Cargo.toml
```

## Current Scope

Implemented:

- communication-first Rust workspace
- simulated camera frame stream
- template capture and normalized cross-correlation processing
- published feature-detector wiring for `radsym`, `chess-corners`, and
  `calib-targets`
- session recording to JSONL plus PGM frames
- system mirror with sequence-gap resync behavior
- compact Tauri/React operator UI

Not implemented yet:

- real AVFoundation camera bridge
- remote IPC transport
- browser deployment
- `ringgrid` runtime adapter configuration; the crate is present, but target
  layout commands are not part of the v1 UI contract yet
- `vision-metrology` integration, deferred until its crate is published

The public algorithm selection includes extension points for `radsym`,
`ringgrid`, `chess-corners`, and `calib-targets`. `vision-metrology` remains a
documented future adapter.
