---
requires: wave-agent-design.md (the decided design), wave-shakedown.md (live results)
produces: the ordered build list adapting the implementation to the day's design shifts
---
# Wave agent — next steps

What exists (through the live shakedown) vs what today's higher-level
decisions change. Each item: what to build, what it adapts, status.

## Where the implementation stands

Built, committed, proven live: journal spine + folds + MindState machine;
mind on codex app-server (0.142.5 protocol, live-smoked); steering ops +
interrupt with deadline; open-turn streaming; store-direct registration +
one-brain (row + endpoint-file floor) + observation polling; `lf q worker
run` (placement, env contract); worktree ownership scheme; lf
self-registration with OwnSession self-completion; migrate-on-open healing
the wave_run_id drift. Gates 1–4 of the shakedown passed; PR #796 open.

The decisions that now outrun the implementation: **one calling convention
(lf commands)**, **ambient wave context in every lf run**, **waves outward /
zero centers**, **Concerto-as-viewer**, **thread = chosen speech +
collapsed activity**.

## Build next, in order

### 1. `lf chat` + `lf memory` — the speech surface  [IN FLIGHT]
Agent dispatched. `lf chat` (own wave from env/worktree; `--parent` walks
ancestry via the store to the parent's registered endpoint; `--wave`
explicit); `lf memory show|update|add` with the server as MEMORY.md's sole
writer at the ORIGIN repo; new `say`-op on the wire entering the mind's
queue with attribution; `ChatTurn.from` (Optional) mirrored Rust/Swift +
fixture; operating prompt teaches the vocabulary; `lf q worker run`
instructs workers to end with an `lf chat` report.
**Adapts:** tags plan (dropped for exec); thin WorkerFinished.summary;
mind's worktree-copy MEMORY.md bug.

### 2. Ambient wave context in every `lf` run  [NEXT — Jack, 2026-07-04]
Context assembly (engine) gains two sections on EVERY flow/step run:
`<wave_chat_recent>` (last N turns, hard token cap, compact rendering) and
`<wave_memory>` (MEMORY.md). Wave resolved from env/worktree; **no wave →
sections empty, and `lf chat`/`lf memory` become silent no-ops** — flows
stay wave-agnostic, the vocabulary is safe in every prompt. Read path:
live server GET /conversation → read-only journal fold → empty. Workers
dispatched into a wave arrive knowing the conversation, not just the task.
**Adapts:** context inheritance story; worker quality; generalizes what
render_goal did only for the wave agent.

### 3. Thread rendering: speech vs activity
With `from`-attributed emissions and reactive-reply turns, Concerto renders
the thread as what was *said* (user, mind replies, `lf chat` posts —
prominent) plus collapsed activity (tool items, progress turns). Mind's
narration guidance updated to match. Smallest honest UI: MessageRow caption
for `from`, collapse-by-default for pure-activity turns.
**Adapts:** "narration: every turn" from the original design.

### 4. Shakedown gates 5–7  [gate 5 unblocked by the PATH fix]
Gate 5 (Claude-drivable): mind dispatches a real worker via `lf q worker
run` in the demo repo — worktree naming, tmux attach, WorkerDispatched/
Finished in the journal, `<in_flight>` in the next heartbeat, worker report
via `lf chat` (after item 1). Gates 6–7 (Jack): Concerto WaveChat against
the demo wave; then the real goals wave, one supervised session.

### 5. Ledger convergence  [after lf-meta lands]
The run-ledger branch (047_run_events, `lf runs`/`lf trace`) merges: wave
observation folds run_events (richer than session polling); `lf trace`
becomes the per-run descent surface. Watch the 047/048 migration ordering
(048 already reserves around it).

### 6. Center-audit cleanups  [architecture-wave territory, coordinate]
Fat-daemon routes Concerto's fleet surfaces still call; python lfq deletion;
the executor/trigger paths the collapse item hard-cuts. Not this branch's
work — but this branch shouldn't add to them. Standing question for every
new feature: "does this create a center?"

## Done-when for this branch (revised)
The MVP done-when (wave-agent-design.md §4) plus: a worker's `lf chat`
report visibly lands in the thread and the next heartbeat's context; an
`lf design` run inside the wave demonstrably receives `<wave_chat_recent>`;
`lf chat` outside any wave exits 0 silently.
