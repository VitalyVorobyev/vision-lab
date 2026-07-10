# Vision Lab Backlog

## Ready

### B-001: Restore the dependency-security baseline

- Scope: update the resolved `crossbeam-epoch` dependency for
  `RUSTSEC-2026-0204` without adding an advisory ignore.
- Acceptance: `cargo audit` reports no vulnerability and the full quality gate
  passes.

### B-002: Make coded-hex RingGrid a real Vision Lab capability

- Scope: add the validated configuration contract, detector adapter, Tauri
  command, Algorithm-config form, and point/bounding-box output.
- Acceptance: invalid configurations are atomic rejections; a valid target
  runs through the existing command/event/snapshot path; recordings capture the
  active configuration.

### B-003: Validate RingGrid behavior deterministically

- Scope: test synthetic target detection, Gray8/Rgb8 conversion, stride
  handling, contract validation, and system-mirror/recorder integration.
- Acceptance: fixtures are generated in code, coordinates are asserted in the
  image frame, and no binary test asset is needed.

## Needs decision

### B-004: Expand RingGrid target variants

- Decide whether rectangular layouts, imported v4/v5 target JSON, or editable
  plain/fiducial targets are the next operator workflow.
- Requires a public configuration and persistence decision before implementation.

### B-005: Add typed overlay geometry

- Define a versioned geometry union for ellipses, grids, labels, and masks with
  explicit coordinate-frame semantics.
- Requires an ADR and adapter-by-adapter migration plan.

## Later

- Session browsing and replay.
- Target presets and persistence.
- Structured diagnostics and supported camera exposure/gain controls.
- Remote IPC and browser deployment.
