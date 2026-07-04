# Vision Lab Agent Notes

## Project Direction

Vision Lab is a communication-first local vision lab. Preserve the component
ownership model:

- `camera-mac` owns image acquisition and frame streams.
- `vision-processing` owns detector configuration, ROI/template state, and
  detections.
- `recorder` owns session persistence.
- `system-mirror` owns the UI-facing normalized state and sequence-gap recovery.
- React renders `SystemView` and latest frames; it must not reconcile component
  internals.

Commands are async intent. Events are immutable facts. Versioned snapshots are
authoritative. Frames are a dedicated latest-value stream, not command replies
or normal JSON events.

## Tooling

- Use Bun for all frontend dependency and script work. Do not add npm lockfiles.
- Use crates.io dependencies for published image-processing crates:
  `chess-corners`, `calib-targets`, `radsym`, and `ringgrid`.
- Defer `vision-metrology` integration until that crate is published.
- Keep `vision-contracts` independent from concrete detector crate types.
- Do not introduce local path dependencies for sibling repos unless explicitly
  requested.

## Quality Gate

Run the standard gate before handing work back:

```sh
./scripts/quality.sh
```

The gate includes:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo audit`
- `cargo deny check`
- `bun install --cwd apps/operator-ui/ui --frozen-lockfile`
- `bun run --cwd apps/operator-ui/ui lint`
- `bun run --cwd apps/operator-ui/ui typecheck`
- `bun run --cwd apps/operator-ui/ui build`
- `(cd apps/operator-ui/ui && bun audit)`

If a gate cannot run because a tool is missing or a remote advisory service is
unavailable, state that explicitly and keep the rest of the gate passing.
Known unmaintained transitive RustSec advisories are ignored in
`scripts/quality.sh` and `deny.toml` with rationale; do not add new ignores
without documenting why no direct safe upgrade exists.

## Frontend

The UI should stay compact, technical, and task-focused. Use React + Vite +
TypeScript inside Tauri. Keep the frontend thin: invoke typed Tauri commands,
render `SystemView`, render latest frames, and send ROI/command changes back to
the backend.

Avoid adding decorative UI. Prefer dense, readable controls and predictable
operator workflows.
