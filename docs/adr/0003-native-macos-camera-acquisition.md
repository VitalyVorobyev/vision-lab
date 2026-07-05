# ADR-003: Native macOS Camera Acquisition

## Status

Accepted

## Context

ADR-001 separates commands, events, state, and streams. Camera acquisition is a
good test of that model because camera selection and permission are low-rate
state, while frames are high-rate latest-value data.

The simulator is useful for CI, deterministic tests, and detector development,
but it is not an honest operator workflow. The desktop app needs to connect to
the camera hardware macOS exposes, including MacBook cameras and Continuity
cameras when available.

## Decision

`camera-mac` has two acquisition paths:

- simulator acquisition for CI and deterministic tests;
- native macOS acquisition through AVFoundation under the `real-camera`
  feature.

The Tauri desktop app opts into `real-camera` and calls
`CameraComponent::spawn_macos`. Tests continue to call
`CameraComponent::spawn_simulated`.

`vision-contracts` exposes normalized camera device, format, active selection,
and permission state. It does not expose AVFoundation, Objective-C, CoreMedia,
or CoreVideo types.

Camera device refresh and active config selection remain normal async camera
commands. State convergence is observed through `SystemView`; frames remain on
the latest-value frame stream.

## Consequences

The app can request macOS camera permission and enumerate AVFoundation devices,
including Continuity cameras when macOS reports them. iPhone and iPad support is
therefore limited to the Continuity Camera devices macOS exposes.

The native AVFoundation actor runs on a dedicated current-thread Tokio runtime
because retained AVFoundation objects are not `Send`.

The first native frame bridge publishes grayscale `Frame` payloads by copying
the luma plane for planar buffers or converting packed RGB/BGRA buffers. This
keeps ChESS processing on the existing grayscale path.

The simulator remains the CI backend and keeps deterministic detector tests
independent from physical hardware, permissions, and camera availability.
