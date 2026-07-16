# Stream human Linear edits into Task Sessions (W2-206)

## User-visible outcome

A human edits a Linear issue (title/description) or comments on it, and the
matching Task Session receives that direction within seconds — no manual
`lf pm sync`, no `lf task steer`. Delivery is **webhook-driven**: Linear pushes
the change to a Loopflow receiver, so there is no polling loop and no periodic
sweep. Live work reacts promptly; stopped work retains the direction and
receives it exactly once on resume.

> **Scope note (webhooks, superseding the polling design).** Ingestion is a
> verified Linear webhook, not a bounded reconciliation poll. The planned
> live-runner poll interval and wave-resident sweep are **dropped**. The durable
> exactly-once store foundation (cursor + comment ledger + atomic apply) is
> **preserved** — webhooks are simply the source that feeds it. A single
> **catch-up read** (`observe_issue`) on Session start/resume recovers events
> missed while the receiver was down; that is one bounded read, not a poll.

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
- `last_success_at` — last applied event (drives status freshness)
- `degraded_reason` — `Some(text)` when a Linear read/catch-up failed, `None`
  when healthy

`task_linear_ingested_comments` (child, `PRIMARY KEY(session_id, comment_id)`) —
the comment exactly-once ledger. `INSERT OR IGNORE`; a `FollowUp` is created only
for a newly-inserted id, so a redelivered webhook cannot double-deliver.

**Baseline at session creation:** `seed_linear_observation` seeds the cursor in
the `create_task_session` transaction — `last_title`/`last_description` = the
launch snapshot, `last_revision` = `""` so any real Linear `updatedAt` wins the
monotonic guard. The comment ledger is **not** pre-seeded: a webhook only fires
for changes *after* subscription, so pre-existing comments are never delivered
and never need baselining. (The one-shot catch-up read reconciles only
title/description edits, so it cannot replay old comments either.)

## New source variant

`ChildCommandSource::Linear` (child_session.rs) — provenance so receipts,
`lf task status`, and Mac Active Sessions can show "direction from a Linear
edit," and so PM-writeback logic never mistakes it for something needing
writeback. Wire DTO change ⇒ the `ChildControlSource` enum is mirrored in Swift
(`swift/Loopflow/Models/ChatTurn.swift`) and exercised by the
`child_control_activity.json` round-trip fixture (Rust + `DTOFixtureTests.swift`);
add the `linear` case to both mirrors.

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

## Webhook ingestion

Linear pushes each change to a Loopflow HTTP receiver. The receiver verifies,
maps the event onto the durable substrate, and returns `200` fast (all work is a
few local store writes). One event → one durable write; nothing polls.

**Receiver** (`lf pm webhook serve`, axum): a single route
`POST /linear/webhook`.

1. **Verify** (`verify_linear_signature`): recompute `HMAC-SHA256(secret,
   raw_body)` and constant-time-compare (`subtle`) to the `Linear-Signature`
   header (hex). Reject if the body's `webhookTimestamp` is not within a
   tolerance (60s) of now — replay defense. A failed verification is `401` and
   writes nothing.
2. **Parse** (`LinearWebhookEvent`): `type`, `action`, `data`, `updatedFrom`
   (changed fields, present on updates), `actor`, `webhookTimestamp`.
3. **Resolve target**: `get_task_session_by_issue(data.issue_id)`. No Session ⇒
   `200`, ignore (no live target; a later `lf task run` seeds from the issue).
4. **Filter**: skip unless `Issue`/`update` (with `title` or `description` in
   `updatedFrom`) or `Comment`/`create`. Skip when the actor is Loopflow's own
   Linear user (`actor.id == viewer_id`) — the feedback-loop guard. Metadata-only
   issue updates carry neither field in `updatedFrom` ⇒ ignored.
5. **Write through the durable substrate** (reuses the exactly-once foundation):
   - **Issue title/description edit** → `apply_linear_observation` with the new
     `{revision=updatedAt, title, description, comments: []}` — the same
     monotonic-revision guard + content CAS produces one replacement directive
     `vN+1`, or nothing if a duplicate delivery.
   - **Human comment** → `apply_linear_comment(session_id, comment_id, command)`
     — the same `task_linear_ingested_comments` ledger; a `FollowUp` is created
     only on the comment id's first insertion, so Linear's at-least-once
     redelivery cannot double-deliver.
6. Return `200`.

**Dedup & reorder.** Linear webhooks are at-least-once and not strictly ordered.
- *Duplicate delivery*: the comment ledger (unique `comment_id`) and the issue
  content CAS + monotonic revision make a redelivered event a no-op.
- *Out-of-order issue edits*: the monotonic `last_revision` guard drops an edit
  whose `updatedAt` is older than what we already applied, so a late delivery
  never reverts direction.
- *Comments* are independent follow-ups; arrival order is their FIFO order, which
  is acceptable (no cross-comment ordering contract).

**Baseline is at Session creation, not first event.** Because a webhook only
fires for changes *after* subscription, pre-existing comments are never
delivered — the "replay existing comments" risk disappears. The cursor is
**seeded when the Task Session is created** (`seed_linear_observation`, in the
`create_task_session` transaction) from the launch snapshot's title/description
(revision seeded empty so the first real edit always wins the monotonic guard).
A first issue-edit webhook then diffs against the launch content and fires only
on a real change.

**Catch-up (bounded, one read — not a poll).** On Task Session start/resume the
runner does a single `observe_issue` + `reconcile_linear_observation` to recover
any edit whose webhook was missed while the receiver was down. Exactly-once holds
because it flows through the same cursor/ledger. This is the *only* place the
read capability is used, and it runs once per start, never on a timer.

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

Normal edit → durable Task direction within seconds of the human's action: the
webhook arrives push-driven and the receiver does only a handful of local store
writes before `200`. No polling, no periodic sweep, no per-Session timer. The
receiver is one process for the host (not per Session); it holds no lock — the
store's cursor/ledger are the only coordination, so concurrent deliveries are
safe.

## Config & operations

- **Signing secret**: `LF_LINEAR_WEBHOOK_SECRET`, sourced from Doppler
  (`doppler run -- lf pm webhook serve`), never committed. The receiver
  refuses to start without it.
- **Loopflow actor**: the receiver fetches `viewer_id()` once at startup (the
  Linear user Loopflow's OAuth token authenticates as) to filter its own events.
- **Registration**: `lf pm webhook register --url <public-url>` issues
  Linear's `webhookCreate` mutation (resource types `Issue`, `Comment`; the
  returned secret is stored via Doppler). Documented as a one-time op; a human
  can equally create it in Linear settings and set the secret.
- **Exposure**: self-hosted default — the receiver binds a local port; a
  human-owned reverse proxy / tunnel gives Linear a public HTTPS URL. The deploy
  scaffolding (service unit, port, proxy note) rides the release-infra docs, not
  this Task's code.
- **Health**: `lf task status` surfaces `last_success_at` (last applied event)
  and `degraded_reason`. A receiver that is down simply stops applying events;
  the catch-up read on next Session start reconciles the gap.

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

- `pm/linear.rs` — reads (`viewer`, `observe_issue`) + `webhookCreate` mutation.
- store migrations — `task_linear_observations`, `task_linear_ingested_comments`
  (shipped) + cursor seed at `create_task_session`.
- `store/child_sessions.rs` / `store/sqlite/child_sessions.rs` — cursor/ledger
  verbs (shipped); `apply_linear_comment`, `seed_linear_observation` (new).
- `child_session.rs` + `swift/Loopflow/Models/ChatTurn.swift` —
  `ChildCommandSource::Linear` / `ChildControlSource.linear` (shipped).
- `ops/linear_observe.rs` — `reconcile_linear_observation` (catch-up path,
  shipped) + `ingest_linear_webhook` mapping (new).
- **`webhook/linear.rs` (new)** — signature verification, `LinearWebhookEvent`
  parse, the axum handler.
- **`lf pm webhook serve|register` (new)** — the receiver command + config.
- `task/runner.rs` — one catch-up read at start/resume (no timer).
- `ops/task.rs` task status + Swift Active Sessions — surface
  `last_success_at` / `degraded_reason`.
- ~~`flowloop/wave.rs` resident sweep~~ — **dropped**.
- ~~`task/runner.rs` poll interval~~ — **dropped** (replaced by webhook push).

## Exclusions

Not Wave Chat; no full comment-history mirror; no ingestion from unrelated
issues; no redesign of provider steering semantics. **No polling loop and no
resident sweep** (dropped in the webhook re-scope). The public-URL exposure
(reverse proxy / tunnel / TLS) and its deploy units are host-operations, carried
by release-infra docs, not this Task's code.

## PR sequence (serial, one Task)

1. **`linear-read`** ✅ *(commit a983f8e63)* — Linear read capability
   (`viewer_id`, `observe_issue` → `IssueObservation`/`IssueComment`) and the
   `ChildCommandSource::Linear` provenance variant across both wire mirrors
   (Rust + Swift). Unit-tested against the mock GraphQL server; the variant's
   wire shape is pinned.
2. **`linear-observe-store`** ✅ *(commit 1f92e8a2d)* — migration 0.11.014 (the
   two cursor tables) + `Store::apply_linear_observation` (atomic, exactly-once)
   + `mark_task_linear_degraded` + `ops::linear_observe::reconcile_linear_observation`
   (baseline, monotonic-revision guard, feedback-loop skip, versioned directive).
   Planner unit tests + an end-to-end store integration test
   (`tests/linear_observe_tests.rs`).
3. **`linear-webhook`** ⏭ *this re-scope* — the webhook receiver:
   `verify_linear_signature` (HMAC-SHA256 + `webhookTimestamp` replay guard),
   `LinearWebhookEvent` parse, `ingest_linear_webhook` mapping onto the durable
   substrate (`apply_linear_observation` for edits, `apply_linear_comment` for
   comments), the axum handler + `lf pm webhook serve`, `webhookCreate`
   registration, and `seed_linear_observation` at Session creation. Unit tests
   (signature, parse, mapping) + an integration test that POSTs a signed body and
   asserts one directive / one deduped follow-up / a skipped self-authored event.
4. **`linear-webhook-ops`** — `lf task status` degraded/last-event fields, the
   one-shot catch-up read at Session start, Mac Active Sessions surfacing, and the
   release-infra deploy note (service unit, proxy, secret). Restart /
   missed-event catch-up test.

## Landmarks for the webhook slice

- The exactly-once foundation is untouched and reused: an **issue edit** rides
  `apply_linear_observation` with `comments: []`; a **comment** rides the new
  `apply_linear_comment` over the same `task_linear_ingested_comments` ledger.
- **Seed the cursor at creation** (`create_task_session` transaction) so the
  first issue-edit webhook diffs against the launch content instead of
  baselining (and swallowing) it. Webhooks never deliver pre-existing comments,
  so no comment baseline is needed.
- Linear signs `HMAC-SHA256(secret, raw_body)` → hex in `Linear-Signature`;
  the body's `webhookTimestamp` (ms) gates replay. `updatedFrom` names the
  changed fields — presence of `title`/`description` there *is* the
  metadata-vs-content test.
- Deps already vendored: `hmac`, `sha2`, `hex`, `subtle`, `axum`.
