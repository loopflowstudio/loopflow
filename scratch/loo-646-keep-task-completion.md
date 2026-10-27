# LOO-646: Keep Task completion through rate limits and unrelated tracker edits

Status: implemented 2026-10-27 (A, B and C in one diff); realigned 2026-10-27, awaiting gate. Jack Heart's acceptance in the Task brief is binding.

## Problem

A Task that was verified complete fell back to `started` because of a sequence of
Linear events, not because anyone decided to reopen it. LOO-615 completed locally at
15:32:28 UTC. Linear's rate limit had started at 15:31:51 and was paused until
16:31:52. The completion delivery `77b8d08d` and the verification comment `fd789d51`
were queued. After the reset, the comment published at 16:36:10.556. That moved the
issue revision to 16:36:10.473 and left the state `started`. At 16:36:17 the next
delivery read adopted `started` (event 70761) and settled the delivery as a conflict.
An operator had to run `lf task complete` (delivery `754fec32`) to recover.

Operators need a verified Task to stay complete through tracker outages. A real,
newer reopening in Linear must still win.

## Reproduction findings (live store read-only, code at 7f36ec1b7a)

Live rows for LOO-615: delivery 338 (`77b8d08d`) has `attempted=1, settled=1`,
`base_revision=15:22:51.472Z`, `base_state=started` and
`conflict_json={"revision":"16:36:10.473Z","state":"started",…}`. Delivery 343
(`754fec32`) is settled completed. Settling clears `error`, so the last delivery
error is gone. The planning observation that adopted `started` came from
`deliver_task_state`'s own `issue_ownership` read: `check_repository` skips Tasks
that are already terminal locally.

`reconcile_in` (`store/sqlite/task_state_delivery.rs`) adopts the observed state
whenever `attempted && observed_revision != base_revision`, even if the observed
state equals `base_state`. Three things combine to make that fire on unrelated edits:

1. **Our own comment counts as an intervening edit.** `sync_repository_deliveries`
   (`ops/linear_observe.rs`) runs `sync_task_comments` and `sync_task_state` at the
   same time with `tokio::join!`. `task_sync` deliberately runs state first, so that
   "our own field acknowledgement cannot … look like an intervening reopening". The
   background path skips that ordering. The comment published at 16:36:10 while the
   state attempt was running.
2. **"Attempted" is set before the request leaves.** `attempt_task_state` sets
   `attempted=1`, then calls `set_item_state` → `graphql`. That runs
   `check_before_write` (a Linear team delta read with up to 5 s of its own) and
   `cooldown.check()` before it sends anything. All of this sits inside
   `deliver_task_state`'s 5 s outer timeout. A slow pre-write read times out the
   attempt with `attempted=1` even though no mutation was sent. With concurrent
   comment delivery and many processes resuming at the 16:31:52 reset, this is the
   most likely way 77b8d08d became "attempted". `reject_task_state` only clears it
   for a RateLimited reply on a first attempt.
3. **No evidence is consulted.** After an uncertain attempt, any revision change
   (a comment, a field edit, another person's comment) counts as a possible
   completion-then-reopening. Linear can tell the difference: `Issue.history`
   entries carry `fromState`/`toState` (`WorkflowState.type`) and `updatedAt`.
   Public schema checked 2026-10-27: `history(first, after, orderBy: createdAt|updatedAt)`
   has no filter or sort direction, so the client pages and filters itself.

The retained evidence cannot tell finding 2's timeout apart from a later
post-reservation failure, because settling erased `error`. Both lead to the same
wrong adoption, and the tests below cover both.

## Demo

`cargo test -p loopflow --lib task_completion_survives_` against the provider
fixture. Completion is rate-limited. The queued comment publishes after the reset
and moves the revision. Background sync then settles the delivery `completed`
locally and in the fake Linear, with one completion write, no Graph process and no
`Adopted Linear state` event. A sibling test reopens the issue in Linear after an
uncertain write and the reopening is adopted. Installed check after landing:
`lf task status LOO-615 --json` is unchanged (completed, delivery 343 settled), and
the historical conflicting row 338 is untouched.

## Chosen approach

Uncertainty is resolved by provider evidence, and it is narrowed at the source.

### A. Reserve at the send boundary
- In `deliver_task_state`, call `check_before_write(&repository)` before
  `attempt_task_state`. Its 10 s team-check throttle (`begin_linear_team_check`)
  makes the second call inside `graphql` a no-op. A slow or failed pre-write read
  then leaves `attempted=0`.
- `reject_task_state` stays the rule for a positive rate-limit rejection. A
  `cooldown.check()` refusal never sends, so it is a rejection too, and it is
  already reported as `RateLimited`.

### B. Order our own effects
- `sync_repository_deliveries` runs `sync_task_state` before `sync_task_comments`
  for each Task, the same order `task_sync` already documents. Tasks still run in
  parallel with each other.

### C. Resolve an uncertain attempt from evidence (the general fix)
New rule for `attempted` with observed state == `base_state`:

| Evidence | Meaning | Action |
|---|---|---|
| observed revision == base revision | nothing changed; write did not land | clear `attempted`, deliver |
| Linear history: no entry with `updatedAt > base_revision` whose `toState.type != base_state` | only unrelated edits (comments, fields, same-type moves) | clear `attempted`, set `base_revision` to the observed revision, deliver |
| history shows the state left `base_state` after base | a real transition (our write landed and was reopened, or someone moved it) | adopt the observed state as a conflict (today's branch, event and `conflict_json` unchanged) |
| history unavailable (Asana/Notion, read failure, more than 10 pages) | unknown | keep pending and attempted; error says uncertain; only an explicit retry overrides |

- `reconcile_in` gets a `StateHistory { Unknown, Unchanged, Changed }` argument. The
  delta reader (`acceptance.rs` ingestion) always passes `Unknown`, so it never
  clears or adopts on an unchanged state with a changed revision. It now leaves
  that case pending instead of adopting. Only `deliver_task_state`, while holding
  the per-Task delivery lock, gathers evidence and passes `Unchanged`/`Changed`.
- `PlanningClient::state_history_since(issue, base_revision, base_state) ->
  LinearResult<StateHistory>`: Linear pages `issue.history { nodes { updatedAt
  toState { type } } pageInfo }` with 50 per page and stops at 10 pages (→ `Unknown`).
  Asana/Notion return `Unknown`.
- Explicit retry (`lf task complete`, `lf task sync --retry-updates`) still applies
  the history rule. A proved reopening is adopted, not overwritten. Retry only
  overrides `Unknown`.
- A previous request that is still in flight and lands after the evidence read
  writes the same target again, which is idempotent. A reopening after that is a
  new revision and is judged again.

## Source of truth and surfaces

- `task_state_deliveries` stays the delivery record: one row per local decision,
  `attempted` meaning "a request may have reached the provider". There is no new
  column or migration. Clearing `attempted` and moving `base_revision` forward only
  happen after evidence proves the earlier request had no effect.
- Task `planning_state` and the events are unchanged except for one thing: an
  unrelated revision after an uncertain write no longer adds "Adopted Linear state".
- Consumers: `lf task status` / `task_planning_sync` lines, Desktop pending-sync
  display (same wire shape), and `lf task sync`. There are no DTO changes.

## Absent and error states

- Linear unavailable or rate-limited during the history read: `Unknown`, delivery
  pending, error retained. The next sync after the reset retries the evidence read.
- Missing `base_revision` (the issue was never observed): today it counts as
  changed. With history, `Unchanged` requires an explicit `updatedAt` bound, so a
  missing base gives `Unknown`.
- A history entry that has `toState: null` (a non-state edit) is unrelated.

## Exclusions

- Linear has no expected-revision guard. The race where a reopening lands between
  observation and mutation stays documented in place (existing comment).
- No repair of LOO-615's live rows. It is already completed, and row 338's conflict
  stays as history.
- Comment, field and creation deliveries keep their own uncertainty rules
  (`planning_delivery.rs`, `planning_export.rs`).
- Release publication and installation belong to the Infrastructure primary
  conversation.

## Delete — do not maintain

- The `!(attempted && observed_revision != base_revision)` heuristic in
  `reconcile_in` and its comment ("After an uncertain write, a changed revision may
  represent completion/reopening"). The evidence table replaces it.
- The concurrent `tokio::join!(sync_task_comments, sync_task_state)` in
  `sync_repository_deliveries`.

## Forbidden outcomes

- Settling a delivery `completed` without a provider observation of `completed`.
- Clearing `attempted` from the delta reader, or without the delivery lock.
- A second delivery table, a "probably delivered" flag, or a timer-based
  assumption that uncertainty has expired.
- Overwriting a reopening that history proves, including on explicit retry.

## Overlap

PR #1733 (LOO-622, open) drops `move_seq` from `queue_in` and renames the Workflow
helpers in `task_planning_tests.rs`. This change does not touch `queue_in`. Rebase
onto whichever lands first and keep #1733's Graph-model renames. #1736 and #1719
touch no files here. #1720 is merged into the base. Realign 2026-10-27: #1733 still
open; origin/main has no commits past the base.

## Tests (one PR)

- `task_completion_survives_rate_limit_and_unrelated_comment_revision`: rate-limited
  reply, comment moves the revision, background sync → settled completed, one
  completion write, no adoption event, no Graph. Its `slow_team_check` phase proves
  A (a slow pre-write check leaves an unsent request `attempted=0`).
- `task_completion_uncertain_write_retries_after_unrelated_revision`: lost request
  (`attempted=1`), comment moves the revision → history `Unchanged` → delivered.
- `task_completion_unknown_history_stays_pending_until_explicit_retry`: history
  route fails → pending, no write, no adoption; explicit retry delivers.
- `task_completion_explicit_retry_adopts_reopening_proved_by_history`: proved reopening wins over
  `lf task complete`; existing `…lost_reply_adopts_linear_reopening` and
  `…preserves_linear_reopening_*` still pass.
- B (state before comment in background sync) has no dedicated test: concurrent
  order is not deterministic in the fixture; the sequential code carries it.

## Done when

- A completion rejected by a rate limit and followed by an unrelated comment or
  field revision settles `completed` locally and in the provider fixture through
  background `sync_repository_deliveries`. No operator command, Graph replay or
  database edit is needed.
- A reopening that Linear history proves is adopted, and its conflict row and
  event are retained. An unknown outcome stays pending and is never reported as
  delivered.
- `cargo test -p loopflow --lib task_planning_tests` passes, and
  `cargo clippy --all-targets -- -D warnings` is clean (gate, headless).
- After landing, `lf task status LOO-615 --json` shows completed and synchronized.
  Live discrepancies are reported, not repaired.

## Implementation notes (2026-10-27)

- `StateHistory` lives in `crate::linear`; `deliver_task_state` gathers it at both
  observations (ownership read and mutation readback). The readback re-reads the
  reserved row so its base reflects any forward move; it is required by
  `task_completion_preserves_linear_reopening_during_delivery` (reopened before the reply).
- A history read failure becomes `Unknown` (debug-logged); the retained error is
  "Prior state delivery remains uncertain".
- Fixture: `mark_issue_updated` records each edit in `_history` with the current
  state (conservative stand-in for `toState: null`); `comment_moves_issue`,
  `history_unavailable` and `slow_team_check` drive the new tests.

## Concept review (2026-10-27, headless)

There are no changes. The model stays one delivery row whose `attempted` flag
means "a request may have reached the provider", plus a three-valued evidence
read. Accepted decisions settle the two behavior shifts. First, the delta reader
no longer adopts on a revision change alone: a real reopening is still adopted
at the next background delivery pass, where history proves it. Second,
Asana/Notion uncertainty stays pending instead of adopting a possible reopening,
because missing evidence is unknown. Both are recorded in `questions.md`. The
uncertainty message does not name its override (`lf task sync --retry-updates`).
That gap is shared by every "remains uncertain" delivery message, so it is left
out of this Task.

## Remaining

- Gate: full `task_planning_tests`, `linear_observation_tests`, clippy.
- After landing: `lf task status LOO-615 --json` read-only check.

## Check results
- `cargo test -p loopflow --lib task_completion_` → 18 passed (new: survives_rate_limit…, uncertain_write_retries…, unknown_history_stays_pending…, explicit_retry_adopts_reopening…); affected suites and clippy deferred to gate.
- compress: readback history read proved required (`task_completion_` → 17/18 without it). Realign: source unchanged, no rerun; gate owns `task_planning_tests`, `linear_observation_tests`, clippy.
- Gate: `cargo test -p loopflow --lib -- task_planning_tests linear_observation_tests --test-threads=2` → 86 passed (slow on a loaded host; 6 threads stalled past 30m); `cargo fmt --check`, `cargo clippy -p loopflow --all-targets -- -D warnings` clean.
