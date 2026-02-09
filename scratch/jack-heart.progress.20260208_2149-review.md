# Review: Flow progress pipeline & commit visibility

## What was implemented

Two data-plumbing fixes that surface existing information in the UI:

1. **Flow steps in the API** — `build_wave_dto` now calls `load_flow_steps` to resolve the flow's step names and includes them as `flow_steps: Vec<String>` in `WaveDto`. The Swift side parses this into `Wave.flowSteps` and `WaveViewModel.flowSteps` becomes a computed passthrough. `FlowProgressPills` now renders the full pipeline (e.g. `ingest > kickoff > implement > compress > gate > consolidate`) with the current step highlighted.

2. **Commits and diff stat while running** — `runProgressSection` in `WaveDetailPanel` now shows `commitLogSection` and `diffStatSection` regardless of wave status, not just in the idle branch.

## Key choices

- **`pub(super)` visibility** for `load_flow_steps` rather than `pub` — keeps the function scoped to the `routes` module where it's needed, without exposing it to the wider crate.

- **`tokio::join!` for concurrent I/O** — `load_flow_steps` does file I/O (reads flow YAML, expands nested flows). Running it concurrently with `infer_wave_git_state` avoids adding latency to the existing `build_wave_dto` path.

- **`Vec<String>` not `Option<Vec<String>>`** — flow steps default to an empty vec when resolution fails. The UI handles the empty case by falling back to `[wave.flow]`. No need for Option nesting.

- **Removed stored `flowSteps` from WaveViewModel** — it was always nil since nothing populated it. Now it's a computed property delegating to `api.flowSteps`, which comes from the API.

## How it fits together

```
Rust: build_wave_dto
  ├── infer_wave_git_state (git ops)    ─┐
  └── load_flow_steps (YAML I/O)        ─┤ tokio::join!
                                          │
  wave_dto(..., flow_steps)              ─┘ → WaveDto JSON

Swift: LocalWaveService.parseWaveFromJSON
  → Wave.flowSteps: [String]
  → WaveViewModel.flowSteps (computed)
  → WaveDetailPanel.runProgressSection
      ├── FlowProgressPills(steps: ...)
      ├── commitLogSection
      └── diffStatSection
```

## Risks and bottlenecks

- **`load_flow_steps` on every `build_wave_dto` call** — This reads flow YAML from disk each time. For the initial WebSocket connect (which sends all waves), this means N file reads. In practice N is small (< 20 waves) and the YAML files are tiny. If it becomes a bottleneck, flow steps could be cached per wave or resolved once on flow change.

- **`build_wave_dtos` is sequential** — The `for wave in waves` loop calls `build_wave_dto` serially. With `tokio::join!` inside each call the per-wave work is concurrent, but waves themselves aren't parallelized. Not a concern at current scale.

## What's not included

- **Fork visualization** — `extract_step_names` flattens fork branches into the step list. A fork with 3 branches shows as 3 sequential steps rather than a parallel group. The design doc didn't call for fork-aware rendering.

- **Step-level status** — The pills show which step is current via `stepIndex`, but don't distinguish completed vs upcoming steps with different styling. Existing `FlowProgressPills` behavior is preserved.
