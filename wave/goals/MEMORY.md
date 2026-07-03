# goals wave memory

Steers Loopflow toward persistent Goal-driven Waves: goals as the authored loop prompt, Asana as the live roadmap, and Concerto as the session surface.

## Shipped (waveagent model foundation)

- **Two-file wave surface** — `wave/<name>/` is now `GOAL.md` (intent) + `MEMORY.md` (this file). Both are injected into the wave loop's assembled context, so the agent reads its intent and memory each iteration.
- **lfq session cockpit** — designed as `lfq sessions` / `lfq attach <id>` over tmux. ⚠️ 2026-07-02: these subcommands are NOT in the shipped `lfq` CLI (only list/show/create/run/stop/delete/land/logs/usage/providers/auth/repos/token), and the operating prompt's `lfq worker run` doesn't exist either. Dispatch-through-lfd is real (HTTP `POST /v0/waves/{id}/dispatch`) but needs lfd running. Contract and CLI have drifted — see scratch/questions.md.
- **dispatch-through-lfd** — `POST /v0/waves/{id}/dispatch {flow, task}` launches a flow-against-task as its own attachable tmux session (`WaveRunSnapshot.task`); the loop dispatches work as separate sessions instead of running it inline.
- **Demo** — `scripts/demo_waveagent.sh` renders the goals wave prompt and shows MEMORY.md reaching context.

## Model (design settled)

A Wave is a durable named hub — intent (GOAL.md) + memory (MEMORY.md) + work-index (branches/PRs under the name) + a canonical live agent (a `TerminalSession` incarnation). "WaveAgent"/"Dispatch" are roles read off `(source, wave_run_id)`, not new types. Full design: `scratch/waveagent-sessions.md`.

## Roadmap status (2026-07-02 read)

- **Item 1 goal primitive — DONE, on main.** `.lf/goals/<name>.md` resolves + overrides builtins (`engine/flow.rs:740`), builtin goals compile in (`builtins.rs:202`), wave carries `goal:`. Metric "GOAL.md is the authored wave surface" ✓.
- **Item 2 Asana live roadmap — built, unmerged, NO PR** on branch `jack-heart.waveagent-roadmap-remote.20260702_2248` (full read/write client: `list_items`/`complete_item`/`comment`/`claim_item`; mirror+ingestion removed; Asana-only auth). My goals branch still uses the on-disk mirror. Next loop: land it or open its PR.
- **Item 3 wave-model-simplify — in-flight** as open PR #763 (waves: nest per-repo RepoWork).
- **Item 2 wave-budget (`spend_cap`) — DISPATCHED 2026-07-02** to a worker on worktree `spend-cap`. Executive call: minimal hard cap + block→human in core, expose cost signal + pause primitive (scratch/questions.md). Reuses existing `usage_summary(wave=)`/`--billing` cost seam.

## Next (not yet built)

- Close-the-loop: feed in-flight dispatches + PR state into re-measure (in progress).
- Attention as the loop's human-escalation channel for parked interactive steps.
- `prepare_goal_launch` + the canonical always-on agent session; supervisor + heartbeat.
- Fan-out branch/PR isolation before lifting `workers > 1`.
