# W2-175 — Interactive parent block & resume (runtime rendezvous)

Branch: `jack-heart/let-an-agent-hand-interactive-interactive-parent-resume` (Task PR 2)

## What already exists (do not rebuild)

| Slice | PR | State |
|---|---|---|
| Handoff data model (`interactive_handoff.rs`): `InteractiveHandoff`, `Parent{Wave,Project,Task}`, `Status`, `Outcome`, `OpenInteractiveHandoff`, `InteractiveHandoffAttach` DTO | #935 | merged |
| Store ops (`store/interactive_handoffs.rs` + sqlite): `open` (idempotent replay + cwd guard), `attach` (first-write-wins), `finish` (first terminal wins, conflicting stale = error), `claim_interactive_handoff_wake` (exactly-once) with restart / parent-replacement / concurrent-terminal tests | #935 | merged |
| CLI `lf handoff open/status/attach/complete/back/fail` (+ `--json`) | #935 | merged |
| DTO fixture `tests/fixtures/dto/interactive_handoff_attach.json` + Rust/Swift round-trip | #935 | merged |
| Engine emits `FlowAction::WaitInteractive` / `DeferInteractive` from `InteractionPolicy` | #941 | merged |
| Task persists `resolved_flow` / `flow_cursor` / `flow_iteration` / `interaction_policy`; `Playhead::resume_root` replays position | #942 | merged |

**The gap this PR closes:** `WaitInteractive` is *emitted by the engine and consumed
nowhere*. The Task runner (`task/runner.rs`) drives the playhead purely through
`prepare_task_flow_step` → `start_task_flow_turn`, treating every skill step as an
ordinary harness turn. Nothing opens a handoff, parks the parent, or wakes it. The
store primitive to do this exactly-once already exists and is fully tested; this PR
is the wiring plus its end-to-end runner tests.

## User-visible outcome

A Task body that reaches a deliberately-interactive rendezvous opens exactly one
durable handoff, and its Task Session parks as `Waiting` (needs-human, owner
Human). The human attaches with the descriptor already pinned in #935, does the
work, and runs `lf handoff complete|back|fail`. The **same** Task then wakes
**exactly once**: `complete`/`back` advance the flow past the interactive step and
the Task continues headlessly; `fail` parks the Task `Blocked` with the reason.
Parent process death, app restart, host restart, repeated attach, and a stale
completion never orphan the handoff, never double-wake, and never silently skip
the interactive step.

## Source of truth

- **`interactive_handoffs` row** — the durable "this parent is waiting on a human"
  marker and the wake-once ledger (`wake_claimed_at` / `wake_claimed_by_generation`).
  Introduces no new table.
- **`TaskSession.flow_cursor` / `flow_iteration`** (#942) — the replay-safe flow
  position. **Invariant this PR enforces: `flow_cursor` never advances past an
  interactive step until that step's handoff is terminal *and* its wake is
  claimed.** This one invariant is what makes block/resume replay-safe across body
  generations and restarts — everything else is derived from re-reading these two
  records at body birth.
- `TaskSession.status` (`Waiting` / `Blocked`) is a *projection* of the row above,
  not an independent truth; body birth re-derives it.

## The rendezvous — a turn-boundary check, not a new skill type

Wiring lives entirely in `task/runner.rs`. Two seams:

### 1. Body birth — resolve any prior rendezvous before running

Immediately after `Playhead::resume_root` (runner.rs:97) and before the first
`prepare_task_flow_step`, resolve the current step's handoff state:

- **No handoff for `(parent, current step)`** → normal turn (unchanged path).
- **Handoff exists, non-terminal** (`waiting`/`attached`) → the human hasn't
  finished. Re-park: set status `Waiting`, append no new work, end the body
  without advancing `flow_cursor`. (Replay of a still-open handoff — idempotent.)
- **Handoff terminal, wake unclaimed** → `claim_interactive_handoff_wake(id,
  generation)`. On the winning claim (returns `true`): append a Task event
  recording the outcome (evidence), then
  - `Completed` / `HandedBack` → advance the playhead past the interactive step
    (`flow_cursor` += 1 via the normal finish-turn path) and continue the flow;
  - `Failed` → set status `Blocked` with the failure reason and end the body
    (operator-resumable; flow does **not** cross the step).
  If `claim` returns `false` (another generation already woke) → treat as already
  resolved: advance/observe per the recorded outcome without re-appending
  evidence. This is the concurrent-completion / double-body case.

The handoff is keyed by parent + "one unresolved at a time" (store invariant), so
"the current step's handoff" is "the parent's one open handoff"; the row's
`body_generation` and reason tie it to the step that opened it.

### 2. Reaching a `WaitInteractive` step — open the handoff and park

When the flow's current action for a `Require`-policy interactive skill is
`WaitInteractive` (use `next_action_with_policy(items, cursor,
session.interaction_policy)` at the step boundary, replacing the implicit
"every skill is a turn" assumption in `prepare_task_flow_step`):

- Build `OpenInteractiveHandoff` from the step + session: `parent =
  Task(session.id)`, `home` from the session's wave home, `cwd = session.worktree`,
  `provider` / `provider_session_id` from the live harness, `body_generation =
  lease.generation`, `reason` from the step, `environment` = the child env the
  interactive provider needs (`LF_HOME`, `LF_TASK_SESSION_ID`, …), `attach_argv` =
  the tmux attach argv for the interactive session.
- `store.open_interactive_handoff(request)` (idempotent: replay returns the
  existing row, so re-reaching the step after a restart reuses the same handoff and
  preserves provider history + worktree).
- Set status `Waiting` and end the body **without** advancing `flow_cursor`. The
  parent is now parked; the interactive step remains the current step.

`DeferInteractive` (defer policy) is out of scope here — see Exclusions.

### 3. Waking the parent — `lf handoff complete|back|fail` enqueues a resume

`finish` in `lf/commands/handoff.rs` already loads the handoff (so `handoff.parent`
is in hand). After a successful `finish_interactive_handoff`, when the parent is a
`Task`, enqueue a `Resume` on that Task Session (reusing `ops::child` resume
plumbing) so a new body generation spawns promptly. Exactly-once is **not** provided
by this enqueue — it is provided by `claim_interactive_handoff_wake` in seam 1;
the enqueue is only the mechanism that gets a body running. A duplicate resume, a
supervision-driven respawn, and a manual `lf task resume` all converge on the same
single claim.

## End-to-end proof

Deterministic runner tests (sqlite temp store, mock harness), one scenario per
contract clause:

1. **Local handoff + resume.** Task flow with an interactive step → body opens the
   handoff, parks `Waiting`, `flow_cursor` unchanged. `lf handoff complete` →
   next generation claims the wake once, appends the outcome event, advances
   `flow_cursor` past the step, flow continues. Assert the *same* handoff id
   throughout and provider history preserved.
2. **Repeated attach** is idempotent (store test exists; add a runner-level assert
   that attach does not perturb `flow_cursor` or status).
3. **Parent process replacement / restart before terminal** → reopen store, new
   generation re-reads a still-`waiting` handoff, re-parks, does not advance.
4. **App / host restart after completion** → new generation claims once and
   advances; a second generation's `claim` returns `false` and does not re-append
   evidence or double-advance.
5. **Child body death → `fail`** → parent wakes once, parks `Blocked` with the
   reason, flow does not cross the step.
6. **Explicit hand-back (`back`)** → parent wakes once, advances past the step
   (work treated as human-completed for that step), evidence recorded.
7. **Concurrent completion** → two finishers race (store test exists); add runner
   assert that at most one generation advances the flow.
8. **Stale completion** (finish an already-terminal handoff with a different
   outcome) → error, first outcome preserved (store test exists).

Commands to run:
`cargo test -p loopflow interactive_handoff`, `cargo test -p loopflow --lib`
(runner tests), `cargo clippy -p loopflow -- -D warnings`, `cargo fmt --check`.

## Affected surfaces & consumers

- `rust/loopflow/src/task/runner.rs` — the two rendezvous seams + tests (primary).
- `rust/loopflow/src/lf/commands/handoff.rs` — `finish` enqueues a Task parent
  resume after a terminal outcome.
- `rust/loopflow/src/engine/flow.rs` — no change; `next_action_with_policy` is
  consumed for the first time.
- `rust/loopflow/src/interactive_handoff.rs`, `store/interactive_handoffs.rs`,
  `store/sqlite/interactive_handoffs.rs` — **unchanged** (contract already correct);
  used as-is.
- Wire DTO `InteractiveHandoffAttach` — **unchanged**; already pinned by the #935
  fixture. No new fields, so no Swift/Rust mirror churn.

## Absent & error states

- No handoff for the step → ordinary headless turn.
- Handoff already unresolved for the parent when a second open is attempted → store
  replay returns the existing row (`created == false`); the runner treats it as the
  same rendezvous.
- Terminal but already-claimed wake → `claim` returns `false`; observe the recorded
  outcome, do not re-append evidence, do not double-advance.
- `Failed` outcome → parent `Blocked`, reason surfaced, step uncrossed.
- Handoff row present but the interactive provider session (tmux) is gone → the
  human/operator records `fail`; the runner never infers terminal state from tmux
  liveness (bytes stay outside the API).

## Operational boundary

- Rendezvous is **store-only**: no network, no busy-poll. The parent parks (body
  exits) and a later generation performs the single atomic wake claim. Durable
  across process/app/host restart because the block lives in `flow_cursor` +
  the handoff row, not in memory.
- Wake claim is one atomic `UPDATE … WHERE wake_claimed_at IS NULL` (already built).

## Implemented (this PR)

The rendezvous landed as a **pure reader** over the handoff the agent opens via
the existing `lf handoff open` CLI — faithful to "an agent *deliberately* opens":

- `task/interactive_rendezvous.rs` — `pending()` (the one unresolved handoff:
  non-terminal, or terminal-but-unclaimed) and `resolve()` (reads + claims the
  wake once, returning `None` / `Waiting` / `Resume { outcome, fresh }`). Unit
  tested against a temp store.
- `task/runner.rs`, two seams both keyed on `pending`/`resolve`:
  - **Birth reconcile** (before the provider starts): `resolve()` → advance past a
    resolved step (Completed/HandedBack), block on Failed, or park while Waiting.
  - **Post-turn park**: if `pending()` is `Some` after a completed turn, the agent
    opened a handoff — clear the body as interrupted (cursor holds) and park
    `Waiting`. Using `pending` (not "non-terminal only") closes a double-advance:
    a handoff that goes terminal *within* the turn still parks, so the next
    birth claims+advances exactly once instead of the turn advancing too.
- `lf handoff complete|back|fail` wakes a Task parent via
  `ops::task::resume_task_async` (best-effort; exactly-once is the wake claim, not
  the enqueue). `task_resume` was split into a sync wrapper + async core so the
  handoff command can wake without a nested runtime.

**Known limitation (documented in `scratch/questions.md`):** a crash in the
microsecond between claiming the wake and persisting the advanced cursor can let
the next body re-run the interactive step and open a *duplicate* handoff. Rare,
self-correcting (a human completes the duplicate), and closable later with a step
key on the handoff row or a single atomic claim+advance.

## Exclusions (deliberate, follow-up Tasks)

- **Project / Wave runner wiring.** The rendezvous helper is written
  parent-agnostic (the handoff parent enum already spans all three), but only the
  Task runner is wired and tested here.
- **`DeferInteractive` / `--skip-interactive` obligation persistence.** Deferring a
  required interaction records a debt obligation — a separate slice (noted in #942).
  This PR handles only the `Require` / `WaitInteractive` rendezvous.
- **Mac Active Sessions rendering and external presentation adapters** — separate
  Tasks per the delivery note. The attach descriptor they consume is already pinned.
- **Agent-initiated mid-turn `lf handoff open`** as a distinct trigger. The
  turn-boundary check in seam 1 already resolves *any* open handoff for the parent,
  so an agent that calls `lf handoff open` mid-turn is parked by the same post-turn
  check; a dedicated mid-turn interrupt path is not added in this PR.
