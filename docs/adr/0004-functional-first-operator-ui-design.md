# ADR 0004: Functional-First Operator UI Design

## Status

Accepted.

## Context

The operator UI needs a stronger visual language for camera acquisition, vision
processing, recording, and system health. The design handoff proposes a compact
command-console direction with top-level sections, a live canvas, tool rail,
contextual inspector, camera configuration, and health/log views.

Some handoff details describe future capabilities that the current contracts do
not expose, including full algorithm parameter schemas, preset diffing, richer
geometry overlays, and diagnostic log streams. Rendering those details as
visible controls would make the UI look more complete than the system actually
is and would weaken the communication-first ownership model.

## Decision

Use the handoff for visual language, hierarchy, and information architecture,
but require every visible operator control to be backed by current commands,
state snapshots, latest-frame data, or recent event summaries.

The React UI may reshape existing state into top-level sections and local UI
state such as overlay visibility, but it must not invent configuration,
detector capabilities, logs, or component internals that are not present in the
current contracts.

## Consequences

- The operator UI can evolve visually without expanding backend semantics.
- Unsupported algorithm extension points stay out of primary runnable controls.
- Future full algorithm configuration, preset management, richer geometry
  overlays, and diagnostic log streams require explicit contract work before
  becoming visible UI surfaces.
- React remains thin: it renders `SystemView`, latest frames, and command
  affordances instead of reconciling component internals.

## Rejected Alternatives

- Literal prototype implementation: faster visual parity, but it would add
  non-functional controls and implied capabilities.
- Staying with the previous sidebar-only shell: conservative, but it would not
  establish a durable operator-console hierarchy for future tools.
