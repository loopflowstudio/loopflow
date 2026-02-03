# Design Review

## What was implemented
- Added WaveRun as the execution record and moved execution state off Wave (proto + store + server + HTTP status).
- Introduced WaveExecutor to run steps, fork/choose branches, and stream output via an OutputHub.
- Rewired loop/watch/cron/recovery loops and RunWave/ConnectWave/EndAgent to use WaveExecutor and WaveRun lifecycle.
- Removed loopflow-engine runtime/store/lf-engine binary and aligned lfd to be the primary execution path.

## Key choices
- Execution state now lives in WaveRun; Wave is config-only to match the lfd-primary design.
- Loop runs acquire scheduler slots before creating a WaveRun to avoid stuck "running" runs.
- StreamOutput is a server-side stream filtered by wave_run_id/agent_id, backed by a broadcast hub.

## How it fits together
- Triggers (loop/watch/cron/RunWave) create WaveRuns and call WaveExecutor.
- WaveExecutor uses loopflow-engine for flow parsing/prompt building, spawns agents, and persists Agent/WaveRun updates.
- OutputHub broadcasts agent output; StreamOutput filters per wave_run_id or agent_id.

## Risks and bottlenecks
- Watch/cron triggers still bypass scheduler slots; heavy concurrency could ignore max_slots.
- Interactive resumes (ConnectWave/EndAgent) start execution without scheduler acquisition.
- Fork branches run in parallel; failures abort remaining branches and cleanup is best-effort.

## What's not included
- LLM-driven choose selection (still deterministic).
- Fork retry/attempt tracking and max-retry behavior.
- Any wave config override fields on WaveRun.
