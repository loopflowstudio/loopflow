Open questions / assumptions

- Assumed Rust core is a new crate at rust/lf-core with a root Cargo workspace. If this should live elsewhere, adjust paths.
- Flow loading currently supports only YAML/JSON under `.lf/flows/` and only linear `Step` items in `tick_flow`; fork/choose/loop are parsed but not executed yet.
- `run_step` shells to `lf --step <name>` as in the design doc; if the CLI expects `lf <step>` or different flags, the runner should be updated.
- Which Python flow behaviors should be left behind vs matched exactly?
- How much of prompt rendering should be configurable vs hard-coded?
- Which tokenizer is acceptable, and when do we fall back to byte limits?

## Daemon service implementation leeway (pick best approach)
- Retry semantics: whether the 3-retry cap is per step, per run, or per wave, and whether stuck-kill counts toward the cap.
- Capacity definition: whether "at capacity" means semaphore saturation only or includes queued work.
- Kill behavior: whether to send a graceful signal before a hard kill, and the grace period length.
- Failure outcome: whether a stuck kill immediately fails the run or retries first (and how retries are scheduled).
- Event replay log: persistence location, rotation/expiry policy, and whether sequence_id resets per process or persists across restarts.
- Minimal HTTP endpoints: exact endpoint list and response shapes for `/health`, `/status`, `/metrics`.
- Metrics scope: which specific metrics are required for Stage 3 (tick latency, queue depth, run counts, etc.).
- Rust lfd currently defaults new waves to flow "ship" and merge_mode MERGE_PR; confirm default flow/merge expectations.
- UpdateWave treats empty direction/area arrays as "no change" (can't clear); if clearing should be supported, add explicit flags.
- ConnectWave, ListFlows, ListWorktrees, and event streaming are stubbed (empty or unimplemented) until flow execution and session tracking are implemented.
- HTTP endpoints return JSON ({status, uptime_seconds, counts}); confirm desired shapes for /health, /status, /metrics.
