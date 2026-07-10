# Vision Lab Roadmap

## Strategy

Keep Vision Lab focused on a reproducible local macOS experimentation loop.
Prioritize a small number of runnable, observable detectors over broad UI or
transport expansion. Commands remain async intent, snapshots remain
authoritative, and every visible control must be backed by a real contract.

## Phase 1: Coded-hex RingGrid target

**Outcome:** an operator can configure a physical coded-hex RingGrid target,
run it against the live camera, and record the exact target configuration.

- Repair the dependency-security gate.
- Add a Vision Lab-owned RingGrid target contract and adapter.
- Expose the configuration in Algorithm config and add the runnable tool to the
  console.
- Validate with synthetic fixtures, the simulated component stack, and a native
  macOS smoke test using the matching printed target.

**Exit criteria:** `RingGridTarget` produces image-pixel point overlays and a
bounding box; invalid configurations are rejected without state changes; the
recording manifest contains the active configuration; `./scripts/quality.sh`
passes.

## Phase 2: Reproducible experiment review

**Outcome:** recorded sessions can be inspected and replayed to compare detector
runs without a live camera.

- Define the replay input contract from the existing manifest, frame records,
  and vision events.
- Add a read-only session browser and replay source.
- Preserve original frame metadata and configuration in every comparison.

## Phase 3: Calibration depth

**Outcome:** operators can use more target variants and inspect the geometry
the detector actually produced.

- Add rectangular and imported target layouts only after a configuration ADR.
- Design typed ellipse, grid, and label geometry with explicit coordinate-frame
  semantics before extending `Detection`.
- Add persisted target presets only when their ownership and migration behavior
  are specified.

## Phase 4: Platform expansion

**Outcome:** transport and hardware capabilities grow without weakening the
local contract model.

- Add structured diagnostics and supported camera controls.
- Evaluate remote IPC before browser deployment; neither is in the current
  local experimentation critical path.

## Dependencies and constraints

- `vision-contracts` never exposes concrete detector crate types.
- RingGrid marker centers remain in image-pixel coordinates; board geometry is
  configuration input, not an output coordinate-frame change.
- `vision-metrology` remains deferred until it is published.
- Every phase must pass `./scripts/quality.sh` before handoff.
