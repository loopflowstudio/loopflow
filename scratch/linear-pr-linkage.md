# Link every published PR on its Linear Task (W2-210)

## Status

The linkage itself is **implemented and tested** on this branch (3 commits, pushed).
One directive clause is **not yet met**: "surface degraded writeback clearly." That
is the remaining pursue target — see *Remaining work*.

PR 1 (`jack-heart/linear-pr-linkage`) is not yet opened on GitHub.

## User-visible outcome

Whenever a Task Session publishes a PR — `lf pr open`, `lf pr submit`, or
`lf pr land` (with or without `--create-pr`) — the owning Linear issue carries a
live link to that GitHub PR: a **first-class Linear attachment** (rich card in the
issue's sidebar) plus a **concise comment** with the PR URL and current publication
state. Running any of these commands again, or transitioning state, **updates the
existing attachment and comment in place** instead of piling up duplicates.

If Linear writeback fails, the GitHub publication still succeeds; the failure is
recorded on the PR and visible to the operator, and the next lifecycle command
retries the idempotent writeback.

## Source of truth

- **Linkage evidence** lives on the `TaskPr` row (`task_prs`), the existing
  per-serial-PR source of truth. Three nullable columns (migration `0.11.014`):
  - `linear_attachment_id` — the Linear attachment; its presence routes the next
    write to `attachmentUpdate` instead of a second `attachmentLinkURL`.
  - `linear_comment_id` — the loopflow-managed comment; its presence routes to
    `commentUpdate` instead of `commentCreate`.
  - `linear_link_error` — `NULL` when the last writeback linked cleanly; the last
    error string when it degraded. Cleared on the next success.
- **Linear issue identity** — `session.launch.issue.id`, the stable UUID captured
  at Task launch.
- **Publication state** — derived from the `TaskPr` alone (`phase()` plus
  `publication.after_merge`). No new state is invented; the label is a projection.

## The one shared idempotent path (shipped)

All three commands converge on **`attach_task_github_pr`** (`ops/task.rs`), called
right after the GitHub PR URL/number is known:

- `lf pr open` → `ops/pr.rs`
- `lf pr submit` / `lf pr land` → `ops/land.rs` (via `prepare_pr`)

It holds the write lease and has the Task Session, active `TaskPr`, and GitHub PR in
scope. `link_pr_to_linear` runs there, before the single store write that persists
both the GitHub attach and the linkage result. Non-Task PRs early-return `Ok(false)`
— manual PRs get no Linear writeback, unchanged.

### Writeback algorithm (as built)

`ops/pm.rs::link_pr_with_client`, given a `PrLinkRequest` and the prior ids:

1. **Attachment** — `attachment_id` present → `attachmentUpdate(id, title, subtitle)`;
   absent → `attachmentLinkURL(issueId, url, title, subtitle)`, store the returned id.
2. **Comment** — `comment_id` present → `commentUpdate(id, body)`; absent →
   `commentCreate`, store the returned id.
3. Success clears `linear_link_error`; any Linear error records it and returns `Ok`
   with partial progress preserved (an id obtained before the failure rides back).

Stored ids — not Linear's URL dedupe — are what make repeat commands idempotent.
`attachmentLinkURL`'s native dedupe by (issue, url) is only the backstop for a crash
between the Linear call and the local persist.

State label (`ops/task.rs::pr_link_state_label`): `Merged` / `Abandoned` /
`Open · completes task on merge` (after_merge = CompleteTask) / `Open · in review`.

## Remaining work — surface degraded writeback

The directive requires degraded writeback to be surfaced clearly. Today
`linear_link_error` persists and rides `lf task status --json` (via
`TaskSessionSnapshot.prs`), but the **human-facing** surface does not show it:
`bin/lf.rs` renders each PR as `PR {seq}: {phase}  {provider}  {branch}{placement}`
and never mentions linkage health. An operator whose Linear token expired sees a
clean status line while every publish silently fails to link.

**Target:** the PR line names a degraded linkage. Precedent to mirror is the
session-level `PM writeback: {state}` line already printed directly above it — the
same status command already surfaces the *completion* writeback's health, so the
per-PR linkage health belongs in the same reading.

**Proof:** a `TaskPr` carrying `linear_link_error` renders a status line that names
the failure; one with `linear_link_error: None` renders unchanged (no noise on the
healthy path). Covered by a `bin/lf.rs`-level test of the PR line, or by driving
`lf task status` against a store row with the error set.

Optional, only if it stays small: have the publish command warn once when linkage
degrades. `ops/pr.rs` and `ops/land.rs` both hold a `progress` handle at the
`attach_task_github_pr` call, so surfacing it there means threading `&impl Progress`
into that function. Status rendering is the durable surface and comes first; the
transient warning is a nicety, not the requirement.

## Absent and error states

- **No Task Session / no store** (manual PR): early-return, no linkage attempted.
- **No PM token / expired token**: `resolve_context` fails → recorded in
  `linear_link_error`, GitHub result preserved, retried next command. Proven at
  integration level: the pr/land suites run this path with no Linear token and
  publication still succeeds.
- **Failure mid-writeback** (attachment linked, comment fails): the attachment id is
  persisted and the error recorded; the retry updates the attachment in place and
  creates the missing comment. Covered by `link_pr_records_error_then_completes_on_retry`.

## Operational boundary

Two Linear round-trips inside `attach_task_github_pr` while the write lease is held —
same shape as the existing `complete_task` writeback. Failure is fast-tolerated
(record and continue), never retried in a blocking loop.

## End-to-end proof (shipped)

- `pm::linear::tests::pr_linkage_maps_to_attachment_and_comment_mutations` — the
  three new mutations map to the right GraphQL.
- `ops::pm::tests::link_pr_creates_attachment_and_comment_on_first_publish` — initial
  publication.
- `ops::pm::tests::link_pr_updates_existing_linkage_without_duplicating` — existing
  linked PR + state transition; asserts `commentUpdate`/`attachmentUpdate` and **no**
  `commentCreate`/`attachmentLinkURL`.
- `ops::pm::tests::link_pr_records_error_then_completes_on_retry` — retry after
  failure, partial progress preserved.
- `store::tests::task_pr_persists_linear_linkage` — the three columns round-trip.
- Full lib suite (1217) + pr/land/status/agent integration suites pass; clippy clean.

## Exclusions and residuals

- **Merge-time refresh deferred.** The background reconciler
  (`reconcile_task_pr_with_authority`) fires on every poll; refreshing linkage there
  would hit Linear each poll unless gated to the merge transition. The directive
  scopes to publication *commands*. The existing completion writeback still posts its
  own "Shipped: <url>" comment, so a completed task is not link-less.
- **Comment crash window (accepted).** `commentCreate` has no native dedupe, so a
  crash between Linear's response and the store write could post a second comment on
  retry. Repeated user commands never duplicate; only that window is exposed.
- No changes to GitHub PR creation/rebase/auto-merge mechanics.
- No linkage for non-Task (manual) PRs — they have no Linear issue.
- No `lf pr publish` verb exists; "publish" in the directive maps to the internal
  `PrPublication` concept / `pr open`.
- No backfill onto PRs that merged before this ships.
- Submit-vs-land (manual-gate vs auto-merge) is not on the PR model and is not part
  of the state label.
