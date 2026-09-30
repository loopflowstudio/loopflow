# Evidence for LOO-298

2026-09-29 · Jack Heart requires code-complete implementation, concept review with
him, resolved findings and final gates before verified merge. Publication is a
checkpoint. This ledger separates current proof from configured acceptance.

Prior results, failures and exact log references remain in
[the preceding committed ledger](https://github.com/loopflowstudio/loopflow/blob/d6dc8c43b73872b8b6e71b0996468d2432b23102/scratch/evidence.md)
and [handoff](parallel-execution.md). This replaces chronology, not obligations.
The complete [remaining matrix](remaining-work.md), [import contract](import-preservation.md),
[Chapters](chapters.md) and [native tradeoff](native-turn-retry-tradeoff.md) remain binding.

## Mechanical Flow step checkpoint

Mechanical boundaries execute as real child lf processes through the shared
Flow driver. Existing Flow start/result events name the child's Exec; the driver
consumes the selected result. Public subprocess proofs retain earlier success
across a later failure and resume a killed driver without replaying a surviving
step. Task stop/status preserve that selected step until its exact process dies;
missing process evidence stays unresolved. [The working design](exec-per-step.md)
retains agent-step consolidation and the remaining ownership cuts.

The initial full materialized matrix (`.lf/tmp/cut-i/exec-step-full.log`) ran all
2,032 tests: 2,030 passed, two failed, 16 skipped. One simulated worker bypassed
startup claim adoption and timed out; its fixture now adopts an actual sleep
process identity before driving the simulated provider. The landing proof still
expected in-process operation ancestry; it now checks driver → step → agent-issued
rebase and assigns repair/history to the step. That stronger assertion exposed
another fixture gap: its fake Codex invoked rebase before receiving the thread's
tool environment. It now executes during the simulated turn with its recorded
caller identity. The focused landing proof passes (`exec-step-land-repair.log`).
The repaired startup/stop checks pass seven tests with one nextest leaky-handle
diagnostic (`exec-step-repairs.log`, which also retains the intermediate landing
failure). No configured-provider proof follows.

`exec-step-full-green.log` passes the full isolated materialized rerun: 2,032
passed, 16 skipped, none unrun, fail-fast disabled, no leak diagnostic. Rust and
test bytes match `.lf/tmp/cut-i/exec-step-source.json`; the failed snapshot remains
in `exec-step-first-source.json`. `exec-step-publish-clippy.log`, formatting,
diff checks and architecture coverage pass. The source and materialized tests use
private stores, real lf subprocesses, and simulated provider/GitHub effects.
Swift is unchanged; its prior 291-test discovery pass is retained, not rerun.
No installed Home, configured provider or rendered Desktop acceptance is claimed.

## Discovery checkpoint

`discovery-full-green.log` passes the complete isolated materialized Rust matrix:
2,029 passed (one slow), 16 skipped, none unrun, fail-fast disabled.
`discovery-swift-green.log` passes all 291 Swift tests. All-target Clippy, formatting,
migration history, architecture and generated HTML checks pass. Rust/Swift/test
bytes match the full-run source receipt `.lf/tmp/discovery/source.json`.

The preceding full Rust run had 2,027 passes and two new fixture setup failures:
a completed Flow lacked ended_at, and a parent-filter fixture tried to mutate
immutable parentage. Corrected fixtures pass focused checks and the full rerun.
Swift's intermediate array mocks and hidden-row click failures were repaired;
its final suite includes both hosted MockWaveFixture failures from `142314682`.
These are local repairs, not a new hosted result.

[Discovery design](discovery.md) records public paging/template collision proofs,
5,000-row whole-command timings and mounted terminal retention across partial,
failed and successful refresh. Providers are scripted; terminal proof uses cat.
Configured acceptance and the deepest-first [remaining work](remaining-work.md)
stay open. The next cut gives each Flow step its own actual Exec.

## Published evidence

At `d6dc8c43b`, hosted run `36629797823` passes Rust, Swift, UI, Python, website,
end-to-end, installation, migration and static checks. Scratch-clear and its
aggregate remain red. The earlier non-fail-fast materialized run was 2,001/2,013;
its twelve failures and focused repairs remain in the preceding ledger. The new
hosted pass supersedes the missing-green-matrix limitation, not those observations.

Typed Flow results are published. Native schemas constrain decision and router
outputs; exact selected successful completion owns settlement. Source/canonical
proofs cover invalid→valid, bounded exhaustion, stale success and runtime children.
Public OpenCode uses a scripted provider. Real Codex uses local synthetic Responses
for correction, exhaustion and delayed removed-command rejection; that closes the
legitimate retry failure within that fixture only. Claude correlation has a native
message fixture. No configured Claude or configured-account acceptance follows.

## Captured-event CI repair

Hosted `243e3edee`, run `36633600273`, passed Swift but Rust stopped at the
stalled-status fixture with 907 tests unrun. Production and Rust/Swift fixtures
now name the selected Session event. The first complete, non-fail-fast isolated
materialized run exposed five more obsolete Flow assertions and two fixtures
relying on ambient Git. Those fixtures now own their Git context; Flow tests
resolve the real Session from captured history, retaining binding, Task identity,
rename, open and complete-history assertions.

`captured-ci-full.log` records 2,009 passes / seven failures / 16 skipped.
All seven repairs pass in `captured-ci-repairs.log` (21 tests). The final entire
matrix passes **2,016 tests, 16 skipped, none unrun** in `captured-ci-final.log`,
with one nextest leaky-handle diagnostic on the prepared-input timing fixture,
followed by all-target Clippy; formatting passes too. All logs are under
`.lf/tmp/cut-i/`. This is a disposable materialized copy with development provenance
and isolated data, using simulated provider/GitHub effects where applicable.
The current Swift Task Flow proof passes within the separate in-progress history
consumer build; that does not establish configured Desktop acceptance.
No new hosted result or whole-design completion follows from these local checks.

## Captured-event cut

Logs below are under `.lf/tmp/cut-i/`. These are source/disposable-copy checks,
with simulated providers where provider effects are exercised.

| Proof | Result and scope |
| --- | --- |
| `captured-public-proof-2.log`, `captured-repaired-proof.log` | Public suite 46/52 passed, six failed; all six repairs pass in the later 27-case proof. Retains Task/taskless continuation, Session/Ask replacement, binding, native receipts and indexed metadata. No claim of a fresh whole-suite rerun. |
| `captured-preservation-proof.log` | 207/215 passed, eight failed. Found reselectable old captures and missing capture Exec/Work projection, plus stale fixture columns/counts and inherited database pinning. These failures remain recorded. |
| `captured-preservation-repairs.log`, `captured-last-preservation-repairs-2.log`, `captured-final-repairs.log` | Composed repairs pass: publication interruption retains the same Ask/question/event, bind retains old attribution, final answers/account receipts survive, populated history keeps exact SQL/Started/native sequence, and live blocked feedback uses its owning database. Final four-case repair passes. |
| `captured-replay-proof.log`, `captured-replay-session.log` | Twelve Rust DTO/reservation/replay checks pass; the strengthened public replay consumer additionally passes using Session identity distinct from the artifact key. |
| `captured-swift-dto-2.log` | Sixteen Swift DTO checks pass after removing SessionRecord.run_id and converting capture references. The Monitor continuity assertion now compares Session identity. No rendered Desktop claim. |
| `captured-canonical.log` | Twelve materialized checks pass: populated draft/released upgrades, unknown SQL, native output settlement and runtime-child preservation. Dedicated build target avoids mixing authoring and canonical schemas. |

`captured-unknown-selection.log` and `captured-canonical-unknown.log` additionally prove an old Flow selector without
conversation membership is archived exactly and leaves current capture unknown.

Canonical source hashes are in `canonical-captured-event-source.json`; the copy
contains tracked and untracked source bytes. `captured-canonical-chapters.log` passes all 15 Chapter checks (one 68-second
rotation matrix). `captured-final-static.log` passes all-target Clippy; formatting,
whitespace and architecture coverage pass. Reuse unchanged evidence; do not rerun
whole suites merely because a lifecycle phase changed.

Concrete review findings fixed in this cut: publication compares a capture sequence;
replacement must append and cannot reactivate an earlier event; capture history
projects its own Exec/Work; a failed artifact publication leaves the reservation
recoverable; old unclassified Flow selectors and failure JSON retain original
bytes in `import_evidence`. No migration fabricates a Session from a provider label.
Historical artifact paths and prepared→launching ownership remain intact.

## History reader integration

The released proposal now uses captured event identity and typed Session history;
RunSnapshot and string-subject joins are removed. Native receipts without a capture
or start remain discoverable with unknown attribution and partial usage. The
subtract-only pass removes the completion wrapper, redundant Work tuple and Swift
input-identity wrapper. Original SQL/manifest time and attribution precede import
metadata; provider and recorder outcomes remain separate.

Logs are under `.lf/tmp/cut-i/`. `history-full.log` ran the entire materialized
Rust matrix without fail-fast: **2,013 passed, six failed, 16 skipped, none unrun**.
Four consumer expectations were stale; two import cases exposed overwritten
historical time/source. All six repairs plus bounded-history and orphan-discovery
checks pass in `history-matrix-repairs.log` (8/8). Review also reproduced an
unlinked turn borrowing another turn's Work filter (three rows instead of one);
`history-orphan-scope-red-exact.log` retains the failure and the eight-case repair
includes its correction. **Final `history-full-final.log`: 2,019 passed, 16 skipped, none unrun**,
followed by formatting and all-target Clippy. The exact disposable source receipt
is `.lf/tmp/history-event/source.json`; source/test hashes still match.
`history-docs-final-2.log` records regenerated website docs (the preceding
invocation named a nonexistent helper and did not change files).

`history-public-orphan.log` proves public runs/usage discovery, partial counters
and no borrowed post-bind Task ownership with a scripted OpenCode provider.
`history-budget-proof.log` proves pre-decode limits, retained unfinished entries,
complete exact-caller reads and visible exact-detail corruption. Swift's initial
39 selected checks had two stale expectations; their repaired DTO/Task-history
selection passes 18/18 in `history-swift-repair.log`. Nine lifecycle-scorecard
Python tests, architecture coverage and the prior all-target Clippy pass. These
are local fixture proofs; configured-provider/Desktop acceptance remains open.

## Limits retained at the finish line

- The released-populated public import bridge and its canonical-copy counterpart
  remain required. Schema fixtures are not that bridge or a backed-up real-Home
  conversion. Keep source artifacts, unknown membership/Exec identity, controller-
  and SQL-only evidence, Started, ancestry and exact replay until it passes.
- Native-only aggregate discovery has local fixture proof above. Saved-Flow
  inventory and Desktop paging/reconciliation remain the next implementation cut.
- Configured accounts, rendered Desktop continuity, complete managed/provider
  recovery and the real-Home procedure remain explicit obligations. Never run this
  branch against the installed Home or promote it before authorized delivery.
- Unknown mechanical completion requires inspection before retry; no exactly-once
  external-effect claim follows. Released-claim/live-child settlement and the
  original missing Task/PR publication cause remain unresolved incident evidence.
- Chapter fixtures preserve Task identity, exact Started, worktree, PR and capture
  across rotation and second-Home sync. They prove neither live Linear rotation,
  absence of another Home's work nor a distributed transaction.
- Dense metadata SQL timings were warm synthetic measurements. End-to-end cold/warm
  history/discovery measurements remain owed; earlier 314–339ms Exec p50 readings
  had uncontrolled OS caches and do not establish final product latency.

Prospective usage attribution and retained `flow blocked` feedback are operating
assumptions for Jack's review. No slice establishes Task completion or shipment.

## Common Task context and PATH steps · 2026-09-30

Working slice after `a35af5223`; Jack Heart's subsequent decisions are retained
in questions.md and [the command-equivalence audit](task-command-equivalence.md).

- `path-step-2.log`: one public CLI proof passed. PATH selects a wrapper for each
  step; after its first successful operation adds a synthetic future migration,
  the old driver refuses settlement and replay, preserving the selected effect.
  This is simulated schema advancement, not two released lf versions.
- `common-name.log`: shared launch surfaces/provider prompt proof passed.
  `checkout-context-2.log`: three public checkout tests passed, including landed
  Task context and branch identity independent of upstream. Comment refresh can
  fail without erasing confirmed Task context; the first run exposed this and
  the common reader now retains that context with a warning.
- `common-command-proof-4.log`: disposable Linux public CLI proof passed using
  real lf and scripted Linear/Codex/tmux. Managed Task step, direct skill and
  ordinary saved Flow carry the same name and Task seed; independent commands
  preserve the managed review. Revoked credentials in a Flow step mark the
  account missing in SQLite and public auth status. Existing driver-death,
  attached-steer, exact-completion consumption, Chapter rotation and second-Home
  sync-only adoption assertions also passed. Hash inventory and results are in
  `.lf/tmp/cut-i/common-command-proof-4-source.json` and its results directory.
- Earlier public attempts remain counterexamples: attempt 1 sampled before the
  selected native start; attempt 2 sampled before review input publication.
  The fixture now waits for those specific durable events. Attempt 3 found a
  production error-loss bug: Codex failed-turn completion discarded its inline
  error, leaving the account connected. The shared harness now forwards that
  error to existing account invalidation/failover; no Task classifier was added.
- `common-boundary.log`: two moved checkout-boundary tests passed.
  `common-account-preflight.log`: account preflight refusal passed without
  creating a registry. The common account selector enforces the execution
  boundary on each attempt, including inherited account routes and retries.

These are focused branch proofs. Configured account failover/control, repeated
managed decisions, typed blocked/keyed continuation, both-dead explicit retry,
release-before-publication failure and integrated materialized coverage remain
open where not already covered by identical retained bytes. No promotion,
installed-data access, publication or Task completion occurred in this slice.

Final static checks for this common-command slice: `cargo fmt`,
`cargo clippy --all-targets -- -D warnings` (`common-command-clippy-final.log`)
and `git diff --check` passed. Review caught the two snapshot races and the
lost inline provider error above; the public proof was rerun after each repair.
