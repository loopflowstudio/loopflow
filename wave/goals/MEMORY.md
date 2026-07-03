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

- **Item 2 Asana live roadmap READ-SIDE — DONE + verified on branch `jack-heart.asana-live` (commit `057207b8`, commit-only), 2026-07-03.** Worker committed cleanly this time (broke the uncommitted-worker streak). Added `ops::pm::render_wave_roadmap(repo, wave)` — resolves the wave's linked Asana project (`pm.asana_project` in GOAL.md) via token from the lfd credential store (`get_provider_token("asana")`, no new env/config seam), fetches open tasks via `AsanaClient::list_items`, renders a markdown block into `GoalRenderContext.roadmap`; logs + falls back to the old `wave/<name>` placeholder on any failure so the loop never crashes. Only the looping goal agent fetches; task/worker dispatches pass empty + skip. Grain call: **1 Wave ↔ 1 Asana project, 1 item ↔ 1 task**. 8 roadmap tests pass (fallback, mocked-HTTP render, goal-prompt integration); clippy/fmt clean. Design note: `scratch/asana-live-roadmap.md`.

## Next (not yet built)

- **Asana WRITE-BACK (closes Item 2's "Done when")**: on a dispatched task's completion, move its Asana task to Done (`complete_item`) + comment the PR link (`comment`); hook is `finish_completed_run` in `executor/wave/mod.rs` (where PR creation already fires), guarded to task/worker runs with a linked project, log-and-continue resilience. **Key blocker the read-side worker surfaced:** a dispatched run carries the task *title* (`run.task`), NOT the Asana *gid* — write-back must thread the gid through dispatch (resolve at dispatch time, stash on the run), not match by title. Detailed in `scratch/asana-live-roadmap.md`.
- Close-the-loop: feed in-flight dispatches + PR state into re-measure (in progress).
- Attention as the loop's human-escalation channel for parked interactive steps.
- `prepare_goal_launch` + the canonical always-on agent session; supervisor + heartbeat.
- Fan-out branch/PR isolation before lifting `workers > 1`.
