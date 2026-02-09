# Flow progress pipeline & commit visibility

Show flow steps as an expanded pipeline during execution, and surface commits/diffs while running — not just when idle.

## What to build

Two bugs with one root cause pattern (data exists but isn't surfaced):

1. **Pipeline pills show "ship roadmap" as one blob** — `flowSteps` is never populated on WaveViewModel because the API doesn't return resolved flow steps. FlowProgressPills already renders multi-step pipelines beautifully, it's just data-starved.

2. **No commit/diff progress while running** — `commitLogSection` and `diffStatSection` only render in the `status == .idle` branch. The Wave model already carries `commits` and `diffStat` while running.

## Data structures

### Rust: Add `flow_steps` to WaveDto

```rust
// dto.rs — WaveDto
pub struct WaveDto {
    // ... existing fields ...
    pub flow_steps: Vec<String>,  // NEW: resolved step names for this flow
}
```

### Rust: Resolve flow steps in build_wave_dto

```rust
// routes/mod.rs — build_wave_dto
let flow_steps = load_flow_steps(&wave.flow, &std::path::Path::new(&wave.repo))
    .unwrap_or_default();

// Pass to wave_dto(...)
```

Reuse `load_flow_steps` from `routes/flows.rs` — it already does `load_flow → expand_flow → extract_step_names`. Move it (or make it pub) so `routes/mod.rs` can call it.

### Swift: Parse `flow_steps` in Wave model

```swift
// Wave.swift — add field
public var flowSteps: [String]

// LocalWaveService.parseWaveFromJSON — read it
let flowSteps = json["flow_steps"] as? [String] ?? []
```

### Swift: Remove `flowSteps` from WaveViewModel

`flowSteps` moves from WaveViewModel (where it was always nil) to Wave (where it comes from the API). WaveViewModel.flowSteps becomes a passthrough:

```swift
// WaveViewModel
public var flowSteps: [String] { api.flowSteps }
```

No more optional. Falls back to `[flow]` at the call site if empty:

```swift
// WaveDetailPanel — already does this
FlowProgressPills(
    steps: wave.flowSteps.isEmpty ? [wave.flow] : wave.flowSteps,
    ...
)
```

## Key functions

### Rust side

- `load_flow_steps(name, repo) -> Option<Vec<String>>` — already exists in `routes/flows.rs`. Make it `pub(super)` or move to a shared location so `build_wave_dto` can use it.
- `wave_dto(...)` — add `flow_steps: Vec<String>` parameter.
- `build_wave_dto(...)` — call `load_flow_steps` and pass result.

### Swift side

- `parseWaveFromJSON` — read `flow_steps` from JSON into `Wave.flowSteps`.
- Remove `flowSteps` from `WaveViewModel.init` params. Make it a computed property delegating to `api.flowSteps`.
- `runProgressSection` — show `commitLogSection` and `diffStatSection` when running, not just idle.

## Constraints

- `load_flow_steps` does file I/O (reads flow YAML, expands nested flows). It's called from `build_wave_dto` which already runs git commands via `spawn_blocking`. The flow resolution should also happen inside the `spawn_blocking` block, or be cheap enough to run inline.
- The WebSocket `connected` event sends all waves — this also goes through `build_wave_dtos`, so flow steps will be included on initial connect too.
- Don't break the existing idle-state layout. Commits/diffs should appear in both running and idle states.

## Done when

1. Running wave shows expanded pipeline pills: `ingest > kickoff > implement > compress > gate > consolidate` with current step highlighted.
2. Commits and diff stat appear below the pipeline while the wave is running.
3. `cargo test --all` passes.
4. `swift test --package-path swift` passes.
