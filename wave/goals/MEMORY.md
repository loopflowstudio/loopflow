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
- **Item 2 Asana live roadmap — foundation LANDED on main 2026-07-03** (`c113ef04 asana-only: drop Linear/Notion, remove PM mirror and ingestion`; `d370450a lfd: collapse ontology to wave/run/session`). Asana client (`lfd/pm/asana.rs`) + mirror removal on main. Absorbed branch `waveagent-roadmap-remote` + its worktree DELETED 2026-07-03 (diff vs main was empty). REMAINING: wire the live loop to read Asana each iteration + write status back end-to-end (the item's "Done when") — **the next big move once spend_cap lands.**
- **Item 3 wave-model-simplify — in-flight** as open PR #763 (waves: nest per-repo RepoWork).
- **Item 2 wave-budget (`spend_cap`) — DONE on branch `jack-heart.spend-cap`, PR-ready (commit-only, not pushed), 2026-07-03.** SAME uncommitted-worker failure mode recurred: the re-dispatched agent did the test-build fixes + chord rollup + at-limit queue wiring in `loop_ticker.rs` (+151) but left it all UNCOMMITTED. Checkpoint-committed at `70894907`, then finished + committed `140f9802`. Final state: `money.rs` (integer-cents, 4 tests), `spend.rs` (chord rollup + at-limit, 7 tests incl. `two_level_chord_enforces_parent_ceiling_against_sum_of_children`), `loop_ticker.rs` at-limit queue wiring (`small_cap_blocks_once_accrued_cost_crosses_ceiling`, `single_expensive_iteration_trips_per_iteration_ceiling`), `Wave.spend_cap`, `QueueBlockReason::SpendCapExceeded`, migration 042, WaveDto contract test + **Python wire mirror in `models.py` (Money+SpendCap, verified parses capped/uncapped)**. **`cargo test -p loopflow --lib` = 938 pass**; only failure is pre-existing unrelated `journal::terminal_run_events_clear_context` (also fails on main). Swift `Wave` is a curated UI subset (manual dict parse, already omits workers/stack_count) → per CLAUDE.md UI-state carve-out, `spend_cap` lands in Swift when Concerto surfaces it (item 3), NOT speculatively. No `wave.json` DTO fixture exists; WaveDto covered by Rust `contract_tests`. Ready for a human to `lf op land`. **Lesson: dispatched workers keep not committing — always check worktree `git status` for uncommitted work before re-dispatching.**
- **Noise:** PR #767 "goals: draft" is just this loop's own MEMORY/scratch bookkeeping on the goals branch — not real work; ignore/close.

- **Item 2 Asana live roadmap read-side — DISPATCHED 2026-07-03** to worktree `loopflow.asana-live` (branch `jack-heart.asana-live.20260703_0826`). The seam: `GoalRenderContext.roadmap` (flow.rs:143) is hardcoded to the dead placeholder `format!("wave/{}", wave.name())` in `executor/wave/mod.rs:162`; that string is what the goal prompt's "Roadmap handle:" renders. `AsanaClient` (`lfd/pm/asana.rs`) exists but is used ONLY in tests — never wired into a live loop path. Worker brief: fetch live Asana tasks → render into `roadmap` (fallback to placeholder when no token / fetch fails, never crash the loop), find/add the token seam, executive grain call = **1 Wave ↔ 1 Asana project, 1 item ↔ 1 task**, design note at `scratch/asana-live-roadmap.md`. Commit-only. Write-back (Done + PR-link on dispatch completion) is the NEXT task, explicitly deferred.

## Next (not yet built)

- Asana WRITE-BACK: on dispatched-task completion, move the Asana task to Done + comment the PR link (read-side dispatched; this closes Item 2's "Done when").
- Close-the-loop: feed in-flight dispatches + PR state into re-measure (in progress).
- Attention as the loop's human-escalation channel for parked interactive steps.
- `prepare_goal_launch` + the canonical always-on agent session; supervisor + heartbeat.
- Fan-out branch/PR isolation before lifting `workers > 1`.
