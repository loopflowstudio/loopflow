# goals wave memory

Steers Loopflow toward persistent Goal-driven Waves: goals as the authored loop prompt, Asana as the live roadmap, and Concerto as the session surface.

## Shipped (waveagent model foundation)

- **Two-file wave surface** — `wave/<name>/` is now `GOAL.md` (intent) + `MEMORY.md` (this file). Both are injected into the wave loop's assembled context, so the agent reads its intent and memory each iteration.
- **lfq session cockpit** — designed as `lfq sessions` / `lfq attach <id>` over tmux. ⚠️ 2026-07-02: these subcommands are NOT in the shipped `lfq` CLI (only list/show/create/run/stop/delete/land/logs/usage/providers/auth/repos/token), and the operating prompt's `lfq worker run` doesn't exist either. Dispatch-through-lfd is real (HTTP `POST /v0/waves/{id}/dispatch`) but needs lfd running. Contract and CLI have drifted — see scratch/questions.md.
- **dispatch-through-lfd** — `POST /v0/waves/{id}/dispatch {flow, task}` launches a flow-against-task as its own attachable tmux session (`WaveRunSnapshot.task`); the loop dispatches work as separate sessions instead of running it inline.
- **Demo** — `scripts/demo_waveagent.sh` renders the goals wave prompt and shows MEMORY.md reaching context.

## Model (design settled)

A Wave is a durable named hub — intent (GOAL.md) + memory (MEMORY.md) + work-index (branches/PRs under the name) + a canonical live agent (a `TerminalSession` incarnation). "WaveAgent"/"Dispatch" are roles read off `(source, wave_run_id)`, not new types. Full design: `scratch/waveagent-sessions.md`.

## Roadmap status (2026-07-03 read)

- **Item 1 goal primitive — DONE, on main.** `.lf/goals/<name>.md` resolves + overrides builtins (`engine/flow.rs:740`), builtin goals compile in (`builtins.rs:202`), wave carries `goal:`. Metric "GOAL.md is the authored wave surface" ✓.
- **Item 2 Asana live roadmap — foundation LANDED on main 2026-07-03** (`c113ef04 asana-only: drop Linear/Notion, remove PM mirror and ingestion`; `d370450a lfd: collapse ontology to wave/run/session`). `git diff main..jack-heart.waveagent-roadmap-remote.20260702_2248` is now EMPTY — the branch is fully absorbed; delete it. Asana client (`lfd/pm/asana.rs`) + mirror removal on main. REMAINING: wire the live loop to read Asana each iteration + write status back end-to-end (the item's "Done when").
- **Item 3 wave-model-simplify — in-flight** as open PR #763 (waves: nest per-repo RepoWork).
- **Item 2 wave-budget (`spend_cap`) — RE-DISPATCHED 2026-07-03.** Prior 07-02 worker died mid-build: data layer complete but UNCOMMITTED in worktree `loopflow.spend-cap` — `money.rs` (Money integer-cents, tested), `SpendCap { rate, per_iteration }`, `Wave.spend_cap: Option<SpendCap>` (DTO no-default respected). Re-dispatched via harness Agent (lfd down) to finish: accounting off `cost_usd` seam → at-limit pause+block (`QueueBlockReason`) → chord rollup → Python/Swift DTO mirrors + `tests/fixtures/dto/` → behavioral tests. Commit-only, no push. Executive call unchanged (minimal hard cap + block→human in core; scratch/questions.md).
- **Noise:** PR #767 "goals: draft" is just this loop's own MEMORY/scratch bookkeeping on the goals branch — not real work; ignore/close.

## Next (not yet built)

- Close-the-loop: feed in-flight dispatches + PR state into re-measure (in progress).
- Attention as the loop's human-escalation channel for parked interactive steps.
- `prepare_goal_launch` + the canonical always-on agent session; supervisor + heartbeat.
- Fan-out branch/PR isolation before lifting `workers > 1`.
