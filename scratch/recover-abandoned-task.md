# W2-145 — Recover an abandoned Task without losing work or PR history

## User-visible outcome

A user whose Task attempt was **abandoned** (unsafe provider/process/execution
context) can run one explicit command:

```
lf task recover W2-145
```

and get a *linked successor* Task Session that adopts the same worktree and the
same serial PR sequence, carries the directive forward, and pins a fresh
execution context — with the predecessor kept intact as history. No manual
SQLite or git repair. Recovery never happens automatically: terminal
abandonment is only ever crossed by this deliberate call.

`recover` refuses, naming why and pointing at the next legal action, when the
Task is not recoverable: a completed Task (work is done), a non-abandoned live
Session (use `resume`), or an abandoned Session whose worktree or execution
context cannot be adopted safely (start a new Task).

`lf task status W2-145` after recovery shows a fresh non-terminal Session,
`predecessor`/`successor` linkage, the intact PR sequence, and the carried
directive. The predecessor row still reads `abandoned` with its reason.

## Source of truth

The control-plane SQLite store (release-scoped, `0.11` migration namespace at
`rust/loopflow/src/store/migrations/` — **not** `lfd.db`).

- **The durable Task is the chain of `task_sessions` rows sharing one Linear
  `issue_id`, ordered by `created_at`.** Today that relation is degenerate:
  `task_sessions` has table-level `UNIQUE(issue_id)`, `UNIQUE(issue_identifier)`,
  `UNIQUE(worktree)`, so there can only ever be one row per issue. This is the
  single blocker to a successor.
- **`task_prs`** rows (branch, base_commit, sequence, publication, merge,
  ci_observation) are the serial PR artifacts. They reference `task_session_id`.
- **`child_directives`** (keyed by `ChildRef::Task(session_id)`) are the
  directive history.
- Every roadmap/status view is derived from these; none is authoritative.

Mirror the already-shipped Project-Session precedent exactly:
`0.11.002_project_session_successors.sql` replaced Project's column `UNIQUE`
with a **partial unique index** `WHERE status NOT IN ('completed','abandoned')`,
and `reserve_project_session` (ops/project.rs:212) creates a new-id successor
when the predecessor is terminal, converging concurrent creators on the partial
index. Predecessor/successor is *derived by ordering*, not a stored column
(`project_snapshot`, ops/project.rs:660-689). Task recovery is the same shape.

## Design

### PR 1 — successor model + `lf task recover` + proof  (branch `recover-abandoned-task`)

**Migration `0.11.005_task_session_successors.sql`** (table rebuild, mirroring
0.11.002): rebuild `task_sessions` dropping the three table-level `UNIQUE`
(`issue_id`, `issue_identifier`, `worktree`) and re-adding each as a partial
unique index conditioned on `status NOT IN ('completed','abandoned')`, plus a
non-unique index on `issue_id` for history reads. `task_prs.branch UNIQUE` is
untouched — PR rows are re-pointed, not duplicated, so branches never collide.

**`task_session_by_issue` (store, sqlite/child_sessions.rs:475)** must resolve
to the *current attempt*: return the single non-terminal session if one exists,
else the most-recent terminal one by `created_at`. The current `count > 1 =>
error` path becomes "prefer non-terminal; if two non-terminal exist, that is the
invariant violation to error on" (the partial index makes it impossible).

**`recover_task(issue, reason: Option<String>)` op (ops/task.rs), wired as
`lf task recover <issue> [--reason ...] [--json]` (TaskCommand::Recover in
lf/mod.rs, dispatch in bin/lf.rs).** Steps, all inside one store transaction
`recover_task_session(predecessor, successor, prs, initial_directive)`:

1. Load current session by issue.
   - Not terminal → error: "Task {id} is {status}; resume it with `lf task
     resume {id}` — recover is only for abandoned Tasks."
   - `Completed` → error: "Task {id} is completed; its work shipped. Start a new
     Task rather than recovering."
   - `Abandoned` → proceed to safety gates.
2. **Safety gates** (any failure → refuse, no successor, exact recovery text):
   - `execution` is `None` (predates pinning) → refuse: legacy context can't be
     relaunched safely; run the Linear task fresh. (This is the W2-151/W2-166
     stranded shape from wave memory.)
   - Recorded `worktree` is absent on disk, or is not the git worktree for the
     active PR's `branch` → refuse: worktree cannot be adopted; start a new Task.
3. Build **successor** `TaskSession`: new `TaskSessionId`; same `issue`,
   `worktree`, `workspace_slug`, `wave_id`, `project_session_id`, `agent`,
   `provider`; **fresh** `pinned_execution_context()`; `provider_session_id =
   None` (a new body, no inherited transcript); `latest_process = None`;
   `abandon_intent = None`; `status = Waiting`, `status_reason = "recovered from
   abandoned Session {predecessor.id}: {reason}"`; `current_directive_version =
   1`, `incorporated_directive_version = 0`.
4. **Carry directive forward**: read the predecessor's current directive text;
   create the successor's `ChildDirective::initial` with that text. (History of
   the predecessor's own directives stays on the predecessor via its events.)
5. **Adopt the PR sequence**: re-point every `task_prs` row of the predecessor
   (merged history + the one active PR alike) to the successor `id`, sequence and
   branch/base/merge evidence unchanged. This keeps the serial sequence
   continuous so `pr next` computes the correct next sequence, and guarantees
   exactly one active PR under the current attempt — never a parallel active PR.
   Safe against `idx_task_prs_open` (unique on `task_session_id` WHERE not
   merged/abandoned): re-pointing moves rows, and only the single active PR
   counts toward that partial index, so the successor holds exactly one.
6. Append events: `Started`/a `Recovered` marker on the successor; on the
   predecessor nothing is mutated (its `abandoned` status and reason are
   immutable history).
7. **Convergence**: the INSERT of the successor collides on the partial unique
   `issue_id` index if another recover already created one; catch the collision,
   re-read `task_session_by_issue`, and return the existing non-terminal
   successor. Two concurrent `recover` calls therefore converge on exactly one
   attempt (mirrors reserve_project_session's collision handling, ops/project.rs
   :291-307).

Recovery does **not** launch a process. The successor lands `Waiting`; the human
resumes it (`lf task resume`) or the runner adopts it. This keeps "recover =
deliberate decision" separate from "start a body," and keeps the leased-body
supervision model untouched (`supervisor_restart_bar` already bars terminal and
open-PR sessions; the successor is a normal `Waiting` session it may adopt).

### PR 2 — actionability on the shared surface  (named via `lf pr land --next task-actionability`)

Expose the recommended action on the shared Now/Roadmap + status DTOs so CLI,
Mac, and iOS bucket identically without re-deriving the rule.

Add `TaskActionHint { action: TaskAction, reason: String }` where `TaskAction ∈
{ recover, resume, start_next_pr, complete, none }`, derived in Rust
(lf/commands/waves.rs) from durable status × PR phase × abandon-intent, and
stamp it on `RoadmapTask` and `TaskDetailSnapshot` (required field, no serde
default — DTO rule). Mirror in Swift (`RegistryQuery` Codable path) and add to
`tests/fixtures/dto/` round-trip. `next_move` (owner) stays; `action` is the
verb. For an abandoned current attempt → `recover`, owner `Human`.

## End-to-end proof

Store-level e2e tests in `rust/loopflow/src/ops/child.rs` / `ops/task.rs` using
the existing tempdir-sqlite harness (`open_store`, `make_wave`/`make_project`/
`make_task`/`make_task_pr`, seen at child.rs:826-1002). One test per contract
scenario:

1. **interrupted resume** — an interrupted (non-terminal) Session resumes the
   *same* Session; `recover` refuses it (not abandoned).
2. **safe abandoned successor** — abandon a Session with an adoptable worktree +
   active working PR; `recover` yields a new-id `Waiting` successor, same
   worktree, PR re-pointed, directive carried, predecessor still `abandoned`.
3. **unsafe legacy context** — abandoned Session with `execution = None`;
   `recover` refuses with the "run fresh" text; no successor row created.
4. **concurrent recover** — two `recover` calls on the same abandoned issue
   converge on one non-terminal successor (second returns the first).
5. **submitted Task** — abandoned Session whose active PR is `Open`; recover
   adopts the open PR (one active PR, no duplicate) and the successor can answer
   review via resume.
6. **multi-PR Task between PRs** — predecessor with a merged PR #1 and no active
   PR; recover's successor adopts the merged history and `pr next` computes
   sequence 2 continuously.
7. **completed Task** — `recover` refuses a `Completed` Session.

**Dogfood**: abandon one real stranded Task (W2-151 / W2-166 shape) and recover
it through its next PR with one worktree, one active writer, complete history,
no manual DB/git repair — the acceptance the directive names.

## Absent / error states

- No Session for the issue → `recover` errors "no Task Session for {issue}".
- Abandoned but unsafe (legacy context / missing worktree) → refuse with the
  exact next action (start a new Task); never a half-built successor.
- Concurrent recover → converge, never two active attempts.
- `resume`/supervisor on an abandoned Session stay barred (unchanged); only
  `recover` crosses abandonment, and only into a fresh `Waiting` successor.

## Operational boundary

Recovery is a handful of local SQLite writes in one transaction plus a worktree
existence probe — no network, no provider launch. It must be daemon-less and
sub-second, like the other `lf task` mutations.

## Exclusions

- No automatic recovery. Supervision (W2-135) may replace a *body*; it never
  crosses terminal abandonment. Unchanged here.
- Not W2-172's serial-PR / remote-observation repair.
- PR-body launch stays with `resume`/the runner; `recover` only mints the
  successor.
- Mac/iOS rendering of the action hint is consumption of PR 2's DTO, tracked
  under mac-surface-ux / ios-surface-ux, not built here.
