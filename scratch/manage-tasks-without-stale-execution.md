# Manage Tasks after delivery — LOO-408

Jack Heart authorized autonomous design, verification and landing on 2026-10-07,
including the expanded default-completion scope. No person-dependent review.

## Design

Task decisions change Task state, never execution evidence. Remove historical
execution from completion and explicit cancellation. Preserve reservations,
Session turns, process receipts, ancestry and actual live controls. Checkout
cleanup remains conservative and independent; uncertain or live work retains it.
Delivery checks still protect open PRs and committed additional work.

Verified merge completes by default. Explicit remaining work uses Task history,
with outcome, required evidence and next check; overdue means a decision is due,
never success. Explicit next-PR work remains open. Historical continuation intent
needs accepted-scope reconciliation, not age-based closure. Reads stay passive;
existing reconciliation operations own completion and retry.

## Delete — do not maintain

- Historical-unknown acceptance command/flag, writer, reader and exclusive tests;
  retain decoding of persisted historical events.
- Completion's execution scan and batched wrapper; no replacement stale predicate.
- Cancellation's execution prerequisite; retain cleanup-only exclusion.
- Terminal-Task projection that turns live execution into idle/history.
- Default keep-open delivery policy and manual completion recommendation after merge.
- Worker rotation wrapper and its obsolete completing-PR precondition; explicit
  `pr next` and a landing's named successor are the actual rotation authorities.

## Retained owners

Session input reservations, provider generations and exact process controls still
own launches and interruption. CI-repair admission prevents overlapping side
effects, and checkout guards protect restore/deletion. The cancellation intent
still survives an uncertain provider write. None of these owns Task completion.
Follow-up events share Task history and the existing completion transaction;
there is no new table, migration, worker or lifecycle state. An explicit empty
next PR remains scope to resolve; automatic completion cannot discard it.

## Remaining work

Sync and delivery remain. The implemented path removes completion execution
scans and per-record acceptance; Task follow-up events keep production work open.
CLI and shared Desktop operations preserve execution; terminal Tasks no longer
hide live Flows. Repository reconciliation also checks previously merged Tasks.

Installed 0.13.9 reproduces LOO-353 exactly. Source binaries never touch that
Home. Installed acceptance needs the published repair. Infrastructure's other
merged/open Tasks have specific retained scope: LOO-285 unattended scheduled
settlements; LOO-304 performance and soak; LOO-375 installed timing; LOO-390
prevention measurements. None is authorized closed by age or merged source.
Coordinate PR #1483 at sync (currently open); no competing rename or migration.

Checks: affected gate passed architecture/website/Swift; Rust snapshot failures resolved by focused Task/PR/landing/CLI/golden/doctor tests, final follow-up regression PASS; 388 final headless Swift tests PASS; fmt/clippy PASS; Linux cancellation proxy proof deferred to CI.
