# Flow Steps & Running-State Commits: Review

## What was implemented

Wire the expanded flow step names from the Rust backend through to the Swift UI so the `FlowProgressPills` component shows actual step names (e.g., "implement", "compress", "gate") instead of just the flow name. Also show commit log and diff stat while a wave is running, not only when idle.

## Key changes

**Rust (API layer)**
- `WaveDto` gains `flow_steps: Vec<String>` — always present, empty when no flow is configured.
- `build_wave_dto` runs `infer_wave_git_state` and `load_flow_steps` concurrently via `tokio::join!`, avoiding serial blocking calls.
- `load_flow_steps` visibility widened to `pub(super)` so routes/mod.rs can call it.

**Swift (model + UI)**
- `Wave` model gets `flowSteps: [String]` (non-optional, defaults to `[]`).
- `WaveViewModel` removes its own `flowSteps: [String]?` field and proxies through `api.flowSteps` instead — single source of truth.
- `WaveDetailPanel.runProgressSection` uses `wave.flowSteps` for pill labels and shows commit log + diff stat sections while running.
- `LocalWaveService.parseWaveFromJSON` parses `flow_steps` from the API response.

**Python (model)**
- `Wave` model gains `flow_steps: list[str]` with empty-list default, matching the Rust DTO.

## How it fits together

```
lfd API (Rust)
  └─ build_wave_dto
       ├─ infer_wave_git_state (git log, diff --stat) ─┐
       └─ load_flow_steps (expand flow definition)    ──┤ tokio::join!
                                                        ▼
                                            WaveDto { flow_steps, commits, diff_stat }
                                                        │
                                            ┌───────────┴───────────┐
                                            ▼                       ▼
                                    Swift (Concerto)         Python (lfq)
                                    Wave.flowSteps           Wave.flow_steps
```

The data flows one direction: Rust resolves flow steps at API response time, clients consume them.

## Risks and bottlenecks

- **Flow step resolution adds latency to wave list/detail calls.** Mitigated by running it concurrently with git state inference via `tokio::join!`. Both are already `spawn_blocking` so they share the blocking thread pool.
- **`load_flow_steps` reads flow YAML from disk on every API call.** For the typical wave count (< 10) this is fine. If wave count grows significantly, caching would help.
- **Empty `flow_steps` fallback in UI.** When the flow can't be resolved (deleted/invalid flow name), the UI falls back to `[wave.flow]` — showing just the flow name. Graceful but loses step granularity.

## What's not included

- No caching of flow step resolution results.
- No Concerto UI tests for the running-state commit/diff display — these are view-layer changes best verified visually.
