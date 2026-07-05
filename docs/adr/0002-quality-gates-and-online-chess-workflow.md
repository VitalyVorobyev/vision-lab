# ADR 0002: Quality Gates and Online ChESS Workflow

## Status

Accepted.

## Context

Vision Lab is meant to demonstrate communication and state ownership between
local system components, not only detector output. The first online workflow
therefore needs to exercise the full path: camera frames, vision processing,
immutable events, authoritative snapshots, the system mirror, and React
overlays.

The project also needs early automatic quality control. Because the codebase is
small, long functions and local lint suppressions should be treated as design
pressure rather than accepted drift.

## Decision

Use ChESS corner detection as the default online workflow. The operator UI
provides a one-click action that submits existing commands in sequence:
connect camera, start stream, select `ChessCorners`, and start processing. This
does not create a backend batch command; command receipts remain acceptance
signals, while actual state convergence is observed through `SystemView`.

Quality gates run locally through `scripts/quality.sh` and in GitHub Actions on
macOS. The gate forbids Rust `allow` attributes, denies
`clippy::allow_attributes`, denies `clippy::too_many_lines`, and sets the
function-length threshold to 80 lines.

## Consequences

- The first happy path demonstrates the communication model end to end.
- Template NCC remains available, but it is a manual ROI/template workflow.
- The UI remains thin: it sequences operator intent and renders mirrored state.
- Refactoring is required when functions grow past the agreed threshold.
- Lint exceptions require changing policy, not adding local `allow` attributes.

## Rejected Alternatives

- Backend `run_chess_workflow` command: simpler for the UI, but it hides the
  command/event/state behavior this project is meant to show.
- App auto-start: convenient for demos, but it removes explicit operator intent.
- Broad `clippy::pedantic`: useful later, but too noisy for the current quality
  policy because it mixes API docs, style, and cast guidance with the requested
  structural limits.
