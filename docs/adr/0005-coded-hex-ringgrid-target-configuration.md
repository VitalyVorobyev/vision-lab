# ADR 0005: Coded-Hex RingGrid Target Configuration

## Status

Accepted.

## Context

`RingGridTarget` is an advertised algorithm extension point, but it could not
run because it lacked a physical target layout. The published `ringgrid` crate
has a rich target model, while Vision Lab needs a small stable contract that
works across commands, snapshots, recordings, and the operator UI.

## Decision

Vision Lab exposes `RingGridTargetConfig`: one sequential-ID, 16-sector coded
hex layout with rows, long-row columns, pitch, inner/outer ring radii, and ring
width in millimeters. The vision component validates it through `ringgrid`
before changing state and rejects updates while processing.

The configuration lives in `VisionState` for the process lifetime and is
captured in recorder manifests. `ringgrid` target and detector types remain
internal. Detected marker centers are published through the existing
`Detection.points` field in image-pixel coordinates, with a derived bounding
box and no new geometry union.

## Consequences

- The UI exposes an editable target form only for this supported layout.
- Recordings remain reproducible without introducing preset persistence.
- Rectangular/plain/fiducial/imported layouts require a later contract decision.
- Rich ellipse, grid, and label overlays require a separate versioned geometry
  design; this decision does not change existing detection-coordinate semantics.
