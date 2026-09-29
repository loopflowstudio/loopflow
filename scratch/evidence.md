# Evidence for LOO-298

2026-09-29 · Curated after metadata publication. Recorded checks describe their
own source snapshots. This curation runs no behavioral suite. Jack requires full
implementation, concept review with him, resolved findings and required gates
before verified merge. Publication is a checkpoint, not acceptance.

## Current proof and counterexamples

All log names below are under `.lf/tmp/cut-i/` unless a different root is named.
The complete [remaining matrix](remaining-work.md), [import contract](import-preservation.md),
[Chapter obligations](chapters.md) and [native tradeoff](native-turn-retry-tradeoff.md)
remain unchanged. The [handoff](parallel-execution.md) names next work and ownership.

| Evidence | Observed result and boundary |
| --- | --- |
| `metadata-unit.log` | Five metadata checks passed: unknown state/occurrence, unavailable placement, malformed detail retained in inventory, immutable membership receipts and filtering before decoding. |
| `metadata-public-density.log`, `metadata-public-density-fixed.log` | Retained initial failures: fixture FK seed/order and Ask replacement attributed `declared` instead of `inherited`. The latter survived the density correction; these logs remain red. |
| `metadata-repairs.log` | Eight passed, one Ask attribution failure. Includes bundled SQLite density and seven native-client checks. The subsequent prepared-launch edit postdated this binary. |
| `metadata-resume-ask.log` | Rebuilt public Ask lifecycle passes with independent membership, inherited attribution, retained identity/name/feedback and stale-input exclusion. |
| `metadata-final-rust.log` | 26 assertion passes in 8.616s, **one leaked-handle report** on native-client publication. Not clean process-settlement evidence. |
| `metadata-final-projection.log` | Two passes in 3.121s after removing the unused public Flow-summary API: Session metadata still tolerates unreadable detail and native-client publication/resume passes. No leak report. Does not explain or erase the prior intermittent leak. |
| `metadata-public-sessions.log` | All 34 public Session cases ran without fail-fast: 33 passed, checkout attribution fixture failed. Its helper preferred the historical manifest's two-value source over unchanged Session admission's four-value source. |
| `metadata-public-attribution.log` | Four passes in 11.726s after correcting that helper: checkout/declared work, Ask replacement, continuing children and bind/Started/prior attribution. Production attribution unchanged by this correction; no full-suite rerun claimed. |
| `metadata-swift.log` | Retained worker run finished: 68 tests in nine suites passed, including wire, controls, rename, off-roadmap retention, Monitor and chrome. |
| `metadata-monitor-mount.log` | Eight tests in three suites passed in 9.164s. Monitor now awaits bounded actual window attachment after splitting; terminal/surface/draft/companion/focus assertions and Chapter history remain. Both chrome sizes pass. Not a hosted reproduction or configured Desktop acceptance. |
| `metadata-final-clippy.log`, `metadata-migrations.log`, `metadata-architecture.log`, `metadata-docs.log` | All-target Clippy passed in 16.96s; migration history, architecture and doc regeneration passed. Formatting/whitespace also passed. No full gate. |
| `metadata-source.json` | Source hashes and production-prefix count versus d4a66125c: +394/-10, net +384 Rust/Swift; excludes SQL, tests, docs/generated and scratch. Metadata is a reader improvement, not a net reduction. |
| `import-tui-closure-red.log` | Both required public cases reproduced: independent TUI/no-native-ID and earlier TUI Flow member became headless and open, despite recorded closure. |
| `import-tui-closure-green.log` | Seven import cases pass in 8.224s after merging constructors: surface/closure, exact prior node, repeats, retired-file removal, saved Ask/review, replaced inputs and SQL-only members. Native identity stays missing; no fabricated native success or Exec. Disposable filesystem import, not installed upgrade. Production +34/-79 (net -45) versus 636abc941; earlier net -50 count superseded. |

The metadata checkpoint is published as `e6aa8d4c775c5c02bccd63ca80d6b616a3b60077`
on PR #1296 after conflict-free rebase onto main
`a3820bf7e493b7d533a45677b7945020adf80721` (v0.12.25).
`metadata-published-pr.json` verifies the GitHub head/base; Task read is retained
in `metadata-published-task.json`. Tests above precede this conflict-free rebase.
No final integrated CI result is inferred.

## Hosted CI and original matrix remain distinct

| Recorded snapshot | Result / disposition |
| --- | --- |
| Materialized diagnostic `implement-materialized-matrix-2.log` | 1,925 passed / 33 failed / 15 skipped across all 1,958 selected tests, 45 binaries, 263.361s, exit 100; no leak/timeout. Exact snapshot is `implement-matrix-source.json`. First setup failed on copying the Ghostty submodule as a file before any tests. |
| `implement-setup-recheck.log` | Supported isolated Git checkout plus ambient DB removal: 13/15 formerly failing setup-sensitive tests passed. Cannot attribute each recovery to one of those two fixture changes. |
| `supervisor-matrix-dispositions.json` | At that historical checkpoint, 29/33 had focused passes, one assertion pass had a process leak, three native/caller cases remained. Later native proofs retain their own bytes/limits; this is not one green materialized matrix. All 33 original failures stay in the archived ledger. |
| `implement-quoted-output-red.log`, `implement-quoted-output-red-2.log` | First repeat accidentally overlaid the source registry and unregistered the materialized batch; missing-table failures are invalid behavior evidence. Restored registry reaches the real completed-review classifier rejection; populated migration assertions pass. Production scanner deletion later passes retained review and genuine failure cases. |
| 59ed94ea5 · CI36600147964 | Rust 660 passed / 2 failed / 15 skipped / 1321 unrun: fake OpenCode executables still emitted one-shot output instead of serve HTTP/SSE. Swift 286 tests / five issues: four stale chrome assertions and Chapter-transfer history observation. Source/fixture repairs preserve original assertions; raw logs remain. |
| `command-provider-fixtures{,-2}.log` | 54 selected command/provider tests: first 52 pass, second 53 pass; replay and DB fixture corrections continued afterward. First run had LEAK annotations; second had none. Separate replay-native (1 pass), shared decision/retry (2 passes) and mounted proof (two tests, two chrome sizes) retain their exact scopes. |
| 636abc941 · CI36601948200 | Rust 840 passed / 1 failed / 15 skipped / 1141 unrun: native-client resume opened absent ambient dev DB. Swift 286 tests / one window-identity issue; chrome expectations and separate UI job pass. Metadata checkpoint isolates the fixture before capture and observes native mount completion. These local passes are not fresh hosted green. |

Older hosted heads, job IDs, exact failure text, source hashes and focused repairs
are preserved in the committed ledger linked below. Scratch-clear is intentionally
red while working notes remain. A later isolated pass does not upgrade an earlier
matrix, failed setup or skipped/unrun target. No assertion pass with leaked handles
proves cleanup. Fresh source checks found no surviving owned test process after the
metadata batch, but the leaked-handle cause is unresolved.

## Native retry and recovery boundaries

- **Valid Codex decision retry remains red.** Original late-child reproduction
  (`native-turn-caller-late` predecessors) allowed an old native turn to decide
  after a retry was selected. Original-turn fencing now rejects that child, but
  `native-turn-caller-replace/results.json` rejects the legitimate replacement
  too because its tools retain the earlier environment. Both results must remain.
- `native-idle-resume-config-1` names an idle experiment but observed `systemError`:
  unsubscribe/resume under generation 2 still emitted generation 1. Interrupting
  the completed failed turn returned no active turn and stopped before resume.
  Successful-idle control `native-success-resume-config-1` updates the environment
  without replacing thread/engine. Fresh-engine rehost failed with an active-writer
  response. These observations narrow the failure; none selects a new interface.
  The exact scripts/protocol evidence and cost/proof choices remain in the native
  tradeoff/research documents. Do not repeat the same product question elsewhere.
- Ordinary nondecision Codex automatic retry later passed with zero Run rows,
  failed and successful native history, 40/10 usage, one Session/thread/Exec and
  exact once-only consumption (`agent-admission-native-zero-run-2`). This does not
  prove valid decision retry or configured-account operation.
- Original driver-death/engine-survival and both-dead cases initially failed on
  missing completion/connection evidence. Repaired standalone tests retain native
  completion independently of interrupted/unknown Exec outcomes. Recovery of a
  deliberately removed receipt is a simulation, not an actual database write
  failure. Recovered usage under the original generation remains its own obligation.
- Shared-engine sibling, stale/current approval and live-history proofs cover their
  exact callers. Zero-client marker-before-inspection proves tool execution before
  inspection, not whole-turn completion. A reconnect or window cannot certify
  provider death. Keep ambiguous process evidence explicit.
- OpenCode public scripted-server decision/retry retains original request authority,
  both outcomes, output and usage; earlier caller is rejected. Actual OpenCode
  1.18.33 with a local scripted model completes the Flow and retains final answers.
  Its injected failure is handled inside OpenCode, not Loopflow automatic retry.
  Disconnect after a tool effect retains unknown completion. Interactive reconnect,
  configured accounts and complete managed/provider recovery remain unproven.

## Import, ownership and incident limits

The original mechanical red observed two Runs/zero AgentSessions/one Exec for two
in-process operations. The replacement records each boundary in Flow history and
preserves earlier success when a later operation fails. SIGKILL recovery without a
receipt remains uncertain and requires explicit retry; no exactly-once external
effect claim follows. Task Started is set on real reservation/assignment, retained
through replacement, and not set by inspection. Preserve claim-only, bind-race and
unknown old evidence cases in the full import contract.

Populated draft and canonical source-copy upgrades have bounded passing receipts;
they are not backed-up real-Home conversion. Controller-only, SQL-only, unknown
membership/native identity, multiple Runs per actual Exec, interrupted/idempotent
import, four Session origins, historical closure and usage missingness remain
covered or owed exactly as listed in import-preservation.md. NULL-title's original
synthetic red remains; supported released writer reachability was not found. No
runtime fallback or applied-migration rewrite follows from that synthetic case.

PR publication now retains acknowledged GitHub identity before optional reads;
original PR1296 missing-Task-row cause remains unknown. Existing-root stacking
preserves fork, Task/worktree/PR/capture history and held-claim exclusion. Different
existing-parent reparenting remains separate. Cancellation/provider deletion does
not establish process termination: the released-claim/live-owned-child counterexample
remains a settlement obligation. Do not restore duplicate deletion or signal from
causal ancestry.

Chapter simulation includes two Waves, twelve interrupted provider mutations,
retry from either private store, same-name no-op, exact Started retention and
second-Home adoption via sync. `chapter-public-cli-3.log` exercises Project-default
launch, rotation and sync through the public Linux CLI with scripted providers.
No live Linear rotation, absence of work on another Home, distributed transaction
or configured-worker acceptance is established. Final populated-schema preservation
and any changed execution-owner boundary still need their integrated proof.

## Measurement limits

Bundled SQLite 3.53.2, 20,000 Sessions / 5,000 Flows: indexed review first 100 / offset 4500 /
empty offset 5100 take 0.38 / 1.41 / 4.09 ms; absent-title takes 2.06 ms. Removing only
pending-review index takes 0.57 / 267.82 / 2181.93 ms and 2.19 ms, identical ID sets.
EXPLAIN uses covering membership/pending indexes without summary JSON functions.
This is warm synthetic SQL evidence. Cold cache, CLI startup/schema/query/payload
partition, complete Flow inventory and Desktop paging remain owed.

Earlier Exec discovery fixture used 100,001 Execs / 20,000 Sessions / 5,000 Flows and
measured whole-process p50 around 314–339ms, p95 around 320–341ms. OS caches were
uncontrolled; these figures do not isolate query cost. Earlier empty-inventory
median 2.354→0.704s involved other changed bytes and establishes no causal or
final dense-data claim. Help/version/rejected-argument journal gaps had their own
Exec-entry repair; its recorded proofs remain in the historical handoff, not
substitutes for final command/wire inventory.

## Exact historical record

The [complete previous ledger](https://github.com/loopflowstudio/loopflow/blob/e6aa8d4c775c5c02bccd63ca80d6b616a3b60077/scratch/evidence.md) and [implementation chronology](https://github.com/loopflowstudio/loopflow/blob/e6aa8d4c775c5c02bccd63ca80d6b616a3b60077/scratch/parallel-execution.md)
are retained at commit `e6aa8d4c775c5c02bccd63ca80d6b616a3b60077`.
Private exact copies and the machine-readable section/proof-reference index are
under `.lf/tmp/scratch-curation-20260929-metadata/`. Original bytes were compared
with committed bytes before replacement:

- `parallel-execution.md`: 133,112 bytes, SHA-256 `026bdf2d4a95689c8ec622a09c06e2b8b12a4b4f4a81b944d2c263b7c46b9549`.
- `evidence.md`: 70,619 bytes, SHA-256 `afd4ce5026b688f708024f4a6982a88cd65e588b5e111e97fc1575783177ddcd`.

Archived ledger sections (links point to the exact original lines):

- [Published CI and retained red results](https://github.com/loopflowstudio/loopflow/blob/e6aa8d4c775c5c02bccd63ca80d6b616a3b60077/scratch/evidence.md#L7)
- [Working recovery proof reviewed after consolidation](https://github.com/loopflowstudio/loopflow/blob/e6aa8d4c775c5c02bccd63ca80d6b616a3b60077/scratch/evidence.md#L494)
- [Remaining failure: driver and engine both lost](https://github.com/loopflowstudio/loopflow/blob/e6aa8d4c775c5c02bccd63ca80d6b616a3b60077/scratch/evidence.md#L519)
- [Automatic retry: retained failure and repaired public path](https://github.com/loopflowstudio/loopflow/blob/e6aa8d4c775c5c02bccd63ca80d6b616a3b60077/scratch/evidence.md#L539)
- [Both-dead recovery: repaired standalone path](https://github.com/loopflowstudio/loopflow/blob/e6aa8d4c775c5c02bccd63ca80d6b616a3b60077/scratch/evidence.md#L563)
- [Failed-turn decision: reproduced and repaired](https://github.com/loopflowstudio/loopflow/blob/e6aa8d4c775c5c02bccd63ca80d6b616a3b60077/scratch/evidence.md#L590)
- [Retained local proof](https://github.com/loopflowstudio/loopflow/blob/e6aa8d4c775c5c02bccd63ca80d6b616a3b60077/scratch/evidence.md#L738)
- [Measurement limits](https://github.com/loopflowstudio/loopflow/blob/e6aa8d4c775c5c02bccd63ca80d6b616a3b60077/scratch/evidence.md#L899)
- [Command/provider and mounted CI repair (2026-09-29)](https://github.com/loopflowstudio/loopflow/blob/e6aa8d4c775c5c02bccd63ca80d6b616a3b60077/scratch/evidence.md#L919)
- [Historical TUI closure import (2026-09-29)](https://github.com/loopflowstudio/loopflow/blob/e6aa8d4c775c5c02bccd63ca80d6b616a3b60077/scratch/evidence.md#L959)

This consolidation preserves original red results rather than turning historical
failures into current passing claims. Unsettled acceptance stays in the active full
scope documents; repeated patches/log chronology stays in the exact archive.

## Structured-result CI repair (2026-09-29)

Published `61c69c6` failed the membership fixture in hosted Rust run 36625295038; Swift/UI passed. The fixture now publishes and completes an exact typed native turn. A non-fail-fast materialized copy ran all 2,013 local tests: 2,001 passed, 12 failed. The legacy-verdict fixture now checks retained bytes without granting navigation; the landing fixture owns a disposable Git repo. All three repaired cases pass (`published-ci-final-repairs.log`). Remaining failures pass in focused checks with development provenance, an isolated build target, Git context, and the runner's inherited DB pin removed (`published-ci-isolated-target.log`, `published-ci-account-isolation.log`, `published-ci-git-context.log`, `published-ci-auth-context.log`). Formatting/all-target Clippy pass. Logs are under `.lf/tmp/cut-i/`. This is composed local evidence, not a fresh green hosted matrix or whole-design completion. Captured-event work remains uncommitted.
