# ADR 0001: Communication-First Local Monolith

## Status

Accepted.

## Context

Vision Lab starts as a single-process macOS prototype, but the important
long-term boundary is not process layout. It is the semantic contract between
camera acquisition, vision processing, recording, and operator UI.

If the first version exposes direct shared state or component internals, later
IPC work would become a rewrite. If the first version implements a generic
distributed framework, the prototype would carry unnecessary complexity before
the live camera and detector loop is proven.

## Decision

Use a local monolith with explicit communication roles:

- typed async commands
- immutable events
- authoritative versioned state snapshots
- structured diagnostics
- latest-value frame streams
- host-local presence through component identity and runtime instance IDs

Each component owns its state and serializes externally visible changes through
its runtime actor. The UI talks to components through Tauri commands and observes
the normalized `SystemView` from `system-mirror`.

Future IPC is out of scope for v1, but the local APIs are the semantic contract
that an IPC client must preserve.

## Consequences

- Command receipts never imply that UI state is updated.
- React does not subscribe to component internals or repair event gaps.
- Sequence gaps and lagged subscriptions trigger snapshot refresh.
- Frames stay on a dedicated latest-value stream and are not JSON events.
- The first version remains debuggable on one MacBook while preserving the
  boundaries needed for later transport work.

## Rejected Alternatives

- Direct shared state between UI and components: faster to prototype, but it
  destroys ownership and recovery semantics.
- Starting with egui: simpler desktop loop, but it conflicts with the chosen
  React + Vite frontend path.
- Implementing IPC immediately: architecturally tempting, but it adds transport
  failure modes before the local processing loop is validated.
