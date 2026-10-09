# LOO-418 implementation assumptions

Jack Heart reaffirmed the existing Linear policy in the ongoing conversation:
no further race-policy decision is needed. Observed Linear conflicts win; unseen
concurrency is best effort. Gate now removes the agent-added zero-overwrite requirement and asserts the
observed overwrite as a documented limit, without ignoring the test.
Jack Heart also decided to keep the local Task usable while remote export is
pending. Both policy questions are settled; neither blocks implementation or gate.

Reconciled 2026-10-09. Jack Heart required the full lifecycle on PR #1499 before
demo on October 7. The launch/alias-only milestone was the agent's scope reduction. Accepted decisions live in [the design](make-a-task-up-to.md).

Jack Heart revised completion on October 8: end triggers `task complete`; it does
not define Task status. Linear completion changes status without moving Workflow
or stopping Processes. This supersedes the earlier alias. Local implementation preserves durable failed-trigger retries, newer reopen
decisions, and unresolved delivery facts. Shared local planning is integrated. Combined Rust and disposable installation
checks pass; Swift remains with capable CI and configured acceptance with Jack's demo.

Implemented defaults below remain unreviewed by Jack:

- Waited landing polls every 15 seconds for 30 minutes; the real waiting loop
  now has timeout-expiry proof. Retry and interruption have focused evidence.
- Redundant `-c` and serial `--next` are removed with the PR-chain implementation.
- Surface due follow-ups on the owning Wave's next operation. Filing installs no
  timer; an unattended check must already be authorized in its brief. Exact-time
  wakeups are outside scope. Present this behavior in the complete demo.
- `--design PATH` transfers the child-specific plan without replacing newer
  child work. Preserve scratch isolation and stacked sync.
- LOO-385 overlaps. Recheck before any parallel implementation or external
  disposition; no closure or transfer is authorized by this work.

The original four slices, landing corrections and independent completion trigger
are present. Earlier checks cover the predecessor; current focused evidence is
in the design. The composed simulated-provider lifecycle has CLI/monitor wire proof; updated
Swift fixtures still need capable-CI execution. The local filing/completion and held-Flow composition now pass in Docker.
Completion-before-filing recovery is implemented, including the operator’s finishing
Flow and retained checkout. Historical remote-export uncertainty, Swift
verification and the complete configured demo remain. No landing or installed-Home
migration is authorized.

Implementation choice: `task follow-up --key` identifies an obligation across retries; the default is `follow-up`, and additional obligations use distinct keys. New local receipts pin child ID, Project, content and date; provider Team/state
are resolved by common export, not pinned at filing. Historical receipts retain
their original provider payload; conversion now preserves its unknown network outcome in the common export receipt.

Wait interruption uses the existing global exit 130 handler (a stopped Flow, without retry); timeout remains held exit 3. The public CLI test exposed a conflict with a second Tokio Ctrl-C handler, so that duplicate handler was removed.

October 8 waited-land repair and already-merged success decisions, with their
failed first attempts, are retained in the design and at
`07eeef61d:scratch/questions.md`, “Review corrections.” Those decisions remain;
new relation-delivery checks do not replace their acceptance evidence.

Jack requested retry from the failed Flow step; filed Infrastructure LOO-435 on
October 8, linked to LOO-418/PR #1499. This branch only corrects propagation of
held exit 3, which the Flow previously turned into failure and retried. Bare land
waiting by default remains an unaccepted suggestion. Historical finite landing
was for continuation after caller exit (#1382), then watcher-owned repair
(#1384); the inspected record does not establish a particular crash as the cause.

Jack Heart (October 8) requested no duplicate Task data types and explicit
`--wait-and-fix` naming. The design records the LOO-406 integration boundary: shared
optional placement/identity and local follow-up filing/completion, not DTO merging
alone. Existing LOO-406 types are reused; its active checkout is unchanged.

October 8 integration: Jack Heart authorized stacking on committed LOO-406
`558a39232`. Merge `9f54c5b43` and its repairs adopt local creation/status while
retaining independent completion and durable end requests. Jack retained Linear-wins
for observed conflicts and best effort for unseen edits. The enabled counterexample
records an overwritten unseen reopening without claiming an atomic write. Foreground export now owns provider relation delivery. Atomic filing/recovery and independent local reopening are implemented. Historical
remote-creation uncertainty and remaining configured acceptance stay explicit in the design. Neither permits restoring Workflow/status coupling or a parallel provider writer. No landing is authorized.

October 8 implementation review: a locally filed child could export without its
relation, and displayed links kept their old local name/date. Foreground export
now settles relations from retained intents, independently of source completion;
shared reads use the child's current saved values. Fixture-only provider outages
and lost replies cover this path, not live Linear acceptance.

Implementation choice (2026-10-08): `task reopen` changes planning only, through
its common state-delivery owner, returning planning to unstarted while retaining
Started and live execution. Repeated calls do not add a PR or move Workflow.
Historical filing receipts lack a send/attempt boundary: conversion preserves
unknown export state; a missing provider issue does not authorize another create.
The child exists locally in its pinned Project. Automatically exporting a truly
uncreated historical issue remains unresolved, rather than guessing it was unsent.

Jack Heart decided: keep the local Task usable while export is pending. Retain
attempted/unknown historical creation until exact readback; no speculative retry
rule is required for this change. Local child conversion is implemented; absence
alone does not authorize another create. Pending export is synchronization state,
not a reason to block use of the local Task or completion of its source.

Recovery choice (2026-10-08): only unresolved merged delivery permits first filing
after Done. The finishing Flow is identified by its compiled single
`follow-through` step, not a privileged name. Ordinary Flow/Workflow starts and
resolved-disposition guards remain. The same check preserves cancellation,
abandonment, deletion and planning-ownership refusals. This implements Jack's
independent completion contract; it supplies no new execution or scope approval.

Context limit (2026-10-08): the omitted launch inventory was read from
`.lf/tmp/context/c7bbdff190d5c85ea8677e348a4af1eec82ac3ac21cfbb6ee62d18b14b277a6f.md`.
`lf context --skill realign` reports generated Task/PR launch inventory over
16,000 tokens; shrinking authored scratch or memory cannot fix that source.
Its historical serial-PR boilerplate also conflicts with this branch's accepted
contract. No Task directive, generated snapshot or limit was changed to hide
either conflict. Product has no child memories here. The October 9 gate sample is about 18,535/16,000 tokens; inventory hashes
change that count between edits. The launch overage remains a context-assembly gap, not a reason to delete
accepted decisions or weaken the Task brief.
