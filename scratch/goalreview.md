# Goal Review: Runtime Model Reduction

This branch reduced the lfd runtime model to three product nouns — **Wave**,
**Run**, **Session** — and deleted the overlapping prototype layers. The full
design lives in the code and git history now; this file is the reviewer's
record of what shipped, how to check it, and what remains.

## What shipped

- **`Run`** absorbs the old `WaveRun` + `AgentRun` (execution/result lineage,
  flattened — no `WaveRunSnapshot`).
- **`Session`** absorbs the old `TerminalSession` + conversation session
  (attachable live control surface, `use = wave_agent | worker | palette`,
  optional `run_id` and `parent_session_id`).
- **`AgentLaunch`** and the launch-envelope DTOs are gone: `POST
  /v0/waves/{id}/run` and `/workers` return the durable `Session`; connection
  info comes from `POST /v0/sessions/{id}/attach`.
- Wire fields `wave_run_id` / `terminal_session_id` are gone → `run_id` /
  `session_id`. Rust exports, HTTP DTOs, Python models, Swift models, and DTO
  fixtures speak `Run` / `Session`.
- The `/dispatch` route + `lf op dispatch` are gone; worker launch is
  `lfq worker run`.

## How to validate

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all
uv run pytest python/tests/
swift test --package-path swift
uv run python scripts/check_swift_multiplatform_boundaries.py
```

DTO fixture tests under `tests/fixtures/dto/` should fail if `wave_run_id`,
`terminal_session_id`, `object = "terminal_session"`, or `object =
"agent_launch"` reappear, or if a required field silently defaults.

## What remains (folded into wave/goals)

- **Wave ancestry** was dropped from the durable `Wave` type during the
  reduction, so `WaveAgentTree.child_waves` is always empty. Reintroduce it —
  see `wave/goals/2-wave-ancestry.md`. This blocks the goals-as-chord model.
- **`lf goal`** local command still exists (`rust/loopflow/src/lf/commands/goal.rs`);
  the target was to reduce it to a thin call into the lfd-backed wave-agent
  session API rather than render/launch locally. Minor follow-up cleanup.
