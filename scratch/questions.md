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
- Assumed wave `step_index` resets to 0 on `RunWave` and after a flow completes; if runs should resume from the prior index, we should skip these resets.
- Cron scheduling uses the latest ended step run for the wave (derived from `StepRun.wave_id`); if we need a different source of truth (e.g., wave iteration timestamps), add a dedicated field on Wave.
- Watch polling only checks `origin/main`; if repos use a different default branch, we may need to store the target ref per wave.

## Session connect + fork execution (2026-01-29)
- PTY crate choice: `portable-pty` vs `pty-process` vs raw `nix::pty`. Need to evaluate cross-platform support (macOS primary, Linux for containers).
- Session timeout: Should we kill long-idle interactive sessions? If so, what timeout? 4 hours like stuck runs?
- Fork branch parallelism: All branches at once, or honor slot limits per branch? Current assumption: each branch acquires a slot.
- ConnectWave currently launches `lf run --interactive` inside the daemon via PTY and returns an empty `prompt_file`. The control.proto RPC is unary (no output stream). Confirm whether ConnectWave should instead return a prompt file for clients to run locally, or whether we need to update the proto to support streaming output/input.
- Rust fork flow format uses `fork.branches` while current built-in flows use `fork.step` + `drafts` in Python. Confirm which format the Rust engine should support.
