# Execution-model handoff

LOO-298 · Realigned for Jack Heart · 2026-09-30.

The [contract](data-model-one-table-per.md), [remaining work](remaining-work.md),
[import obligations](import-preservation.md) and [Chapters](chapters.md) retain
whole-design acceptance. No Task completion, installation or shipment is claimed.

## Current position

Item 2 is done. One started Flow retains one FlowSession through every loop
pass; retry keeps its node/iteration position and Iterate advances return
counters. The forward migration preserves original SQL in immutable evidence,
folds child membership/history into the root, and takes active selection only
from the deepest current pass. Recursive checkpointing and driver-lock
indirection are gone. Implementation `a66f42a0a` replayed as `cbd2b1d7c`;
`4f2ec5491` simplifies checkpoints.

Reconciliation `27e4d776e` follows the rebase onto main `a6b1bc3df`. Main's typed
command discovery and flattened step settings remain. Saved inventory reaches
SQL, selected Skills come from the captured boundary before catalog lookup,
and Ask handles reserved Skill names. The short CLI guide remains in
`docs/lf.md`; detailed contracts live in `docs/lf-reference.md`.

Item 3 is implemented in the current pass, together with Jack Heart's requested
rebase CI repairs. The forward draft preserves historical caller tokens and
removes the dead column/index; Rust, Swift, fixtures and environment handling
no longer carry them. Session/provider-generation parent resolution is retained.
Main's command grammar remains, and shared-command checkpointing now recognizes
checkout and inherited declarations. [Current evidence](evidence.md#caller-token-removal-and-rebase-repairs--2026-09-30)
records the requested file suites, focused migration/ancestry proofs and any
unexecuted installation check. Next is naming, then released-populated import
and final docs. Naming proposals remain proposals.

Jack's current feature Flow uses implement → compress → rebase → realign →
loop-decide per item, with focused checks and hosted CI. Full local Rust coverage
belongs before the final gate. Delivery proceeds through publish, Jack's demo,
queue preparation and land; there is no separate slice or concept-review step.
This realign checkpoint is authorized to commit, publish and stop.

## Evidence and limits

- [Loop-pass evidence](evidence.md#one-flowsession-through-loop-passes--2026-09-30):
  seven source and seven materialized checks, five public CLI/discovery checks,
  then 14 compression checks. These precede the rebase and use isolated stores
  and scripted providers.
- [Rebase evidence](rebase-main.md): 11 discovery tests, one exact-completion
  taskless Flow test and one reserved-name Ask test; formatting and all-target
  Clippy pass. This is the focused proof for the reconciled execution paths,
  not a new full-matrix or configured acceptance result.
- The [evidence ledger](evidence.md) retains earlier full-matrix results,
  counterexamples and repairs. Old runtime-child tests are historical receipts,
  not proof that child FlowSessions should return.
- Configured providers/Desktop, complete released-populated public/canonical
  import, dense cold/warm measurements, Chapter preservation, cancellation with
  live work and real-Home conversion remain governed by the remaining matrix.
  Branch binaries must not touch the installed Home.

The realignment changes plans, reference prose and Infrastructure memory only.
The contract now matches child Exec ownership for mechanical steps. Memory
records Jack's prospective-bind and structured-Blocked decisions and current
verification cadence; `questions.md` is unchanged. No new behavioral test was
needed: code and schema are unchanged from the reconciled checkpoint. Fresh
architecture coverage, portable-HTML consistency (one test), formatting and diff
checks pass. All-target Clippy is reused from the unchanged rebase source.

## Working boundaries

Main owns this worktree's source, builds, integration and Git. Preserve supervisor
edits to questions, findings and the contributor index. Released proposals are
hunks to review, not whole files to overwrite:

- `.lf/tmp/final-history-proposal/`: original proposal retained as evidence;
  integrated and adapted to events in this slice, including orphan native receipts.
- `.lf/tmp/flow-discovery-proposal/`: adapted and integrated; repository observation
  precedes the write lock, `--sessions` preserves template names. Loop passes now share one FlowSession.
- `.lf/tmp/released-import-proposal/`: authored, unexecuted public import bridge;
  require an executed result and effective provider isolation, source and canonical.

Use `uv run python .lf/tmp/cut-i/control-checkpoint.py ...` for installed control
operations only. Bare lf can choose another store. Never point branch bytes at
that installed data. Source proofs use disposable Homes with LF_/LOOPFLOW_ authority
removed by `.lf/tmp/cut-i/run.py`. No installation promotion is authorized.
