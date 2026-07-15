# Stream human Linear edits into Task Sessions (W2-206)

## User-visible outcome

A human edits a Linear issue (title/description) or comments on it, and the
matching Task Session receives that direction within five seconds — no manual
`lf pm sync`, no `lf task steer`. Live work reacts promptly; stopped work
retains the direction and receives it exactly once on resume.

## Keystone finding

The whole feature is a **new writer** onto substrate that already exists. Task
direction lives in two durable tables shared by Project and Task Sessions
(`ChildRef` discriminator):

- `child_directives` — versioned, latest-wins "current direction".
- `child_commands` — FIFO audited inbox.

The live task runner (`task/runner.rs`) already drains `child_commands` two ways:
a **startup drain** (`runner.rs:128`, `claim_commands`) and a **200ms poll**
(`runner.rs:188-216`). `child_control::absorb_commands` already routes a `Steer`
to a live-steer (when `harness.capabilities().supports_steer`) or an interrupt,
and a `FollowUp` to a non-interrupting next-turn input. **We reuse all of it.**

So the feature reduces to: *observe Linear, and when a human edited the issue,
write the right `child_directives`/`child_commands` row.* The runner delivers it.

### The one trap: never auto-launch a dormant task

`ops::child::queue_command` (the path behind `lf task steer`/`follow`) calls
`session.launch(...)` when the session has no live process (`ops/child.rs:648`).
Only `Interrupt`/`Abandon` skip the launch. If the Linear observer reused that
path, a comment on a *stopped* task would relaunch its provider — violating
"stopped work ... receives it exactly once **when resumed**."

**Therefore the observer uses a dedicated persist-only ingestion path** that
writes the directive/command via the store verbs
(`create_child_command_with_directive` for a title/desc edit,
`create_child_command` for a comment) and appends the command/directive events,
but **never calls `session.launch`**. Delivery is left to the runner: immediate
if a process is live (its poll picks the row up), on next resume otherwise (its
startup drain). Exactly-once falls out of the existing drain + our cursor.

## Source of truth

- **Linear** is authoritative for issue revision (`updatedAt`), title,
  description, and comment identity (`comment.id`, `comment.user.id`).
- **`child_directives` / `child_commands`** remain authoritative for provider
  delivery, acknowledgment, status, and receipts.
- A new **observation cursor** (`task_linear_observations`, below) is the
  authoritative record of *what Linear state we have already turned into
  direction* — the exactly-once ledger. It is derived-from-Linear, never a
  second copy of task direction.

## Direction semantics

| Linear change | Loopflow write | Delivery |
|---|---|---|
| Human edits title or description | `Steer` command + directive `vN+1` (replacement), source `Linear` | live steer if supported, else interrupt/queue per existing contract; worker must `lf task acknowledge` before completion (already gated by `incorporated < current`) |
| New human-authored comment | `FollowUp` command, source `Linear` | FIFO, next turn, non-interrupting |
| Status / assignee / project / label / other metadata | none | — (`updatedAt` advances but title+description content is unchanged ⇒ ignored) |
| Loopflow-authored comment or PM writeback | none | — (author id == Linear `viewer.id` ⇒ skipped at read time) |

Directive text for an edit carries the new title + description in a shape the
worker reads as "the task definition changed"; comment text is the comment body
(prefixed with the human author for context). This is **Task control**, not Wave
Chat — the Linear issue id + existing Task Session are the target; unrelated
issues are never ingested.

## New durable state

`task_linear_observations` (one row per Task Session, FK `task_sessions(id)`):

- `session_id` PK
- `last_observed_revision` — Linear `updatedAt` (monotonic guard against
  out-of-order responses)
- `last_observed_title`, `last_observed_description` — content diff basis (an
  `updatedAt` bump with identical title+description is a metadata-only change ⇒
  ignored)
- `last_success_at` — last successful observation (drives status freshness)
- `degraded_reason` — `Some(text)` when the last poll failed (auth/quota/net),
  `None` when healthy
- `next_attempt_at` / backoff bookkeeping

`task_linear_ingested_comments` (child, `UNIQUE(session_id, comment_id)`) — the
comment exactly-once ledger. `INSERT OR IGNORE`; a `FollowUp` is enqueued only
for the newly-inserted ids (rowcount), so overlap/restart/duplicate polls cannot
double-deliver.

**Baseline at session creation:** seed the cursor from the launch snapshot —
`last_observed_*` = the issue at launch, `ingested_comments` = all comment ids
that predate session creation. Pre-existing comments are baselined, not replayed;
only comments/edits *after* the Task Session was created become surprise
direction.

## New source variant

`ChildCommandSource::Linear` (child_session.rs) — provenance so receipts,
`lf task status`, and Mac Active Sessions can show "direction from a Linear
edit," and so PM-writeback logic never mistakes it for something needing
writeback. Wire DTO change ⇒ add a `linear` case to the
`tests/fixtures/dto/child_control_activity.json` round-trip fixture. (No Swift
file mirrors `ChildCommandSource` directly; Active Sessions consume task status,
not the raw source enum — so no Swift change for the variant itself.)

## Linear read capability (net-new)

The client (`pm/linear.rs`) today can *post* comments (`commentCreate`) but reads
no `updatedAt`, no comments, and no viewer identity. Add:

- `viewer { id }` — Loopflow's own OAuth user id, fetched once and cached, so a
  comment is human direction iff `comment.user.id != viewer.id` (and not a bot
  actor). This is the feedback-loop guard — robust, not brittle text-prefix
  matching on `Shipped:` / `PR:`.
- `fetch_issue_observation(issue_id)` → `{ updatedAt, title, description,
  comments(first: N, orderBy: createdAt) { id, body, createdAt, user { id },
  botActor { id } } }`. Bounded page; stop at the first already-ingested id.

Auth reuses the existing OAuth token + proactive refresh (`resolve_pm_token`);
no new credential path.

## The observer (diff → ingest)

`reconcile_linear_observation(store, session, linear)` — pure-ish, unit-testable:

1. Read the cursor. If Linear `updatedAt <= last_observed_revision` and no new
   comments, no-op (monotonic guard drops stale/out-of-order responses).
2. Title or description content differs from cursor ⇒ persist-only ingest one
   replacement directive `vN+1`; CAS `last_observed_title/description`.
3. Each comment id absent from `ingested_comments` and authored by a non-viewer
   human ⇒ `INSERT OR IGNORE`; for newly-inserted ids, persist-only FIFO
   `FollowUp`.
4. Advance `last_observed_revision`, set `last_success_at`, clear
   `degraded_reason`.
5. On Linear failure: set `degraded_reason`, back off, **do not** advance the
   cursor and **do not** kill the task; retry catches up on recovery.

## Who polls (owners, single-owner per session)

- **Live task runner** — a bounded interval (~3s, backoff on failure) inside the
  existing `tokio::select!` loop. Meets the ≤5s live budget; its own poll
  delivers what it ingests. Single owner while live.
- **Wave-resident sweep** (`flowloop/wave.rs`, heartbeat/idle boundary) — over
  the wave's non-terminal Task Sessions **without** a live process. Makes
  stopped-session edits visibly pending before resume and catches up edits made
  during downtime. Single owner because live sessions are owned by their runner.
- **Resume catch-up** — one pass at runner startup, backstop when no resident
  ran. Delivery correctness never depends on a resident having been up.

No hot-poll: the resident sweep runs only while non-terminal Task Sessions exist;
the runner observer exists only while a runner is live.

## Absent & error states

- **Task, no Session** — no live target. Latest title/description already seed a
  later `lf task run` (launch snapshot); existing comments baselined.
- **Session, no live process** — persist-only; visible pending in status;
  delivered exactly once on resume via the startup drain.
- **Duplicate / overlap / restart / out-of-order** — cursor CAS + `INSERT OR
  IGNORE` + monotonic `updatedAt` make double-directive/double-follow-up
  impossible.
- **Linear auth/quota/network failure** — caught; task stays controllable;
  `lf task status` shows `last_success_at` + `degraded_reason`; catches up after
  recovery.

## Operational boundary

Normal edit → durable Task direction within 5s while resident. Bounded requests,
exponential backoff to a cap on failure, one owner per session, no hot-poll when
nothing needs observation.

## End-to-end proof

Against a mock Linear client (the `graphql()` funnel is already mocked in
`pm/linear.rs` tests) and the runner's `#[cfg(test)]` harness seams:

1. Edit description ⇒ exactly one directive `vN+1` + live-or-queued steer < 5s.
2. Two human comments ⇒ two FIFO follow-ups.
3. Stop/restart the observer between events ⇒ catch-up exactly once (cursor).
4. Loopflow-authored comment + metadata-only edit ⇒ neither reaches the worker.
5. Linear fails temporarily ⇒ task controllable, status stale + degraded reason,
   catches up on recovery.
6. **Dogfood:** comment on W2-206 while its worker runs ⇒ durable receipt with no
   manual `lf task steer`.

## Affected surfaces

- `pm/linear.rs` — new reads (`viewer`, `fetch_issue_observation`).
- store migrations — `task_linear_observations`, `task_linear_ingested_comments`.
- `store/child_sessions.rs` / `store/mod.rs` — cursor verbs; a persist-only
  directive/command insert reusing `create_child_command_with_directive` /
  `create_child_command` without launch.
- `child_session.rs` — `ChildCommandSource::Linear` (+ DTO fixture case; no Swift
  mirror).
- `ops/child.rs` / `ops/task.rs` — `reconcile_linear_observation` + persist-only
  ingest wrappers.
- `task/runner.rs` — live-observer interval; startup catch-up.
- `flowloop/wave.rs` — resident sweep.
- `ops/task.rs` task status + Swift Active Sessions — surface
  `last_success_at` / `degraded_reason`.

## Exclusions

Not Wave Chat; no full comment-history mirror; no ingestion from unrelated
issues; no redesign of provider steering semantics; no public webhook (a bounded
local reconciliation loop meets the 5s budget).

## PR sequence (serial, one Task)

1. **`linear-observe-core`** — Linear reads (viewer + issue observation), the two
   cursor tables + verbs, `ChildCommandSource::Linear` (+ mirror/fixture), the
   persist-only ingest path, `reconcile_linear_observation` with exactly-once +
   feedback-loop skip + baseline, unit tests. (Persisted rows already drained by
   the existing runner startup path.)
2. **`linear-observe-live`** — live-runner observer interval (≤5s) + backoff;
   `lf task status` degraded/last-observation fields. Integration test: live
   steer < 5s, FIFO follow-ups, degraded + recovery.
3. **`linear-observe-resident`** — wave-resident sweep over resumable sessions,
   resume catch-up backstop, Mac Active Sessions surfacing the durable state.
   Restart-exactly-once test.
