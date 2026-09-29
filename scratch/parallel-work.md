# LOO-298 control and scratch index

2026-09-29 · Jack Heart requested autonomous progress to **code-complete concept
review**, Codex only. Useful parallel contributions and regular rebase/publication
are authorized. Landing, auto-merge, promotion, real-Home migration and branch
binary access to the installed Home are not.

## Read order

1. [Accepted model](data-model-one-table-per.md): Exec, AgentSession, FlowSession.
2. [Main handoff](parallel-execution.md): implementation and next proof.
3. [Full remaining scope](remaining-work.md): no slice substitutes for completion.
4. [Import preservation](import-preservation.md) and [Chapters](chapters.md).
5. [Evidence](evidence.md) and [open assumptions](questions.md).

## Live ownership

- Main Run `run_344d4bfe1bf040b99a43e65434efbf97` owns executable edits,
  builds/tests, Git, cleanup and its handoff. Saved feature invocation
  `1f9ba70e-f0e9-41d4-9722-948d7bc4ce8c`, implement iteration10.
  Verified worker Exec `ef8e22ec-9e08-4762-b166-5033132405e7`, PID79038,
  Codex80014. It has applied the numeric Flow wire proposal and is repairing
  landing helper admission. Recheck status/ps before treating these as live.
- Supervisor owns this index and isolated read-only inspections; no second
  executable writer or competing builds. Control Session
  `run_9c16dbbe2b04440db8469e9b4964912c` has human title `loopflow`.
- Bounded Codex contribution tool3514 / Exec
  `9ef1a796-443f-4173-b76d-e0d5e0c643da`, PID23045, owns only
  `scratch/proposal-exec-discovery.md` and `.lf/tmp/exec-discovery-proposal/`.
  It prepares an unapplied query/model/store patch and focused proof proposal;
  no shared source, builds, Home/provider/Git/PM mutations. CLI naming, Swift,
  Session lifecycle and Flow wire are excluded. Verified coordination comment
  `bec1e094-c3b8-4c51-8d93-75aff36ce2fc`. Wait for exact handback before adoption.

Preserve saved implement → compress → review-slice → concept-review →
loop-decide → human-demo order. Supervisor chooses no edge. Earlier concept
reviews were not code-complete reviews. Completed reviews are not fresh approval;
supervisor direction is not a new decision attributed to Jack.

## Delivery and immediate counterexamples

Published **de5fcea9c6442e23cbf55a809e78c0e1df5819cb**, based on
**6e7189926ede81b3edd05c8d81955c6cbe67a4e2**. GitHub PR1296 and Task publication
agree. Optional Linear PR-link enrichment failed JSON decoding; acknowledged
GitHub publication survives. No auth repair is justified by that optional failure.

CI36587831497 is terminal. Swift, UI, installation and other executed checks pass
except scratch-clear and aggregate. Rust109472788986: **1,736 passed, one failed,
15 skipped, 253 unrun**, 180.948s. `lf_pr_land_waits_for_authoritative_merged_observation`
fails at land_tests.rs1889 because ci-fix cannot find an admitted Exec. Log:
`.lf/tmp/cut-i/ci-de5fcea9c-rust.log`; verified comment
`7663e20c-44de-4352-8ab6-33ab2d16d95b`.

Source explanation: RUN_CONTEXT is thread-local while CLI identity is process-wide;
landing dispatches ci-fix through spawn_blocking. The same process therefore loses
its admitted Exec on that thread. Main independently confirmed this source path.
Saved Flow mechanical operations can launch helpers on a blocking thread too.
Verified comment `76bf775b-6f52-4479-ac7b-a5c9452f58ab` names that adjacent proof.
Repair shared process identity, preserving strict admission and one terminal result.
Local admission failure reproduced in `landing-exec-red.log`. The process-context
repair now passes the six-scenario test in `landing-exec-final-2.log` (20.301s),
plus retained missing/malformed Exec admission checks. Supervisor review requested
an explicit stored merged-state/commit assertion for the added Flow scenario:
its stdout exception alone did not assert that outcome. Verified comment
`ec98165c-7eab-45a8-bd35-a1b90f76903e`. `landing-exec-merged.log` now passes20.713s: stored merged/merge-head,
one command completion and exactly two real Execs (caller plus nested rebase).
Main is checking managed topology in the disposable Linux harness before static
checks/publication; no current hosted rerun yet.

Prior CI858444d91 failed OpenCode taskless decision authority after1,841 passes,
one failure,15 skips,134 unrun. Both child decisions lacked original-turn authority;
log `.lf/tmp/cut-i/ci-858444d91-rust.log`. Current earlier cutoff does not clear it.
The older isolated materialized matrix (1,925 passes/33 failures/15 skips) and
per-failure dispositions remain `.lf/tmp/cut-i/supervisor-matrix-dispositions.json`.
Later focused passes never make those earlier snapshots green.

## Recovery and prompt size

Old main Run4f129 ended failed after its controller mistook quoted fixture output
for an execution denial. Source removed that scanner and its regression passed;
the pinned control binary predates the repair. Verified inspection showed the
read succeeded and a trailing rg found no match (exit1). Resume cleared the blocker
without navigating the Flow. No capability/auth change was needed.

First continuation Run `run_a12ee9b58fa749fc93d7caedb4df73aa` failed before work:
Codex rejected1,070,524 input characters at its1,048,576 limit. Supervisor preserved
the complete numeric proposal, removed only117,273 bytes of duplicate inline patch
from Markdown, and advanced the same boundary. The current worker then launched,
acknowledged its handoff and executed commands. Verified launch comment
`e812da79-0080-4a5a-91b7-a9f7449b07a6`. Keep proposal patches on disk with links/hash;
never inline large patches or dump whole provider fixture JSON into prompts.

## Released proposals and retained proofs

Numeric proposal: [compact handback](proposal-numeric-flow-wire.md), main owns
integration/proof. Full original `.lf/tmp/numeric-flow-wire-proposal/proposal-with-inline-patch.md.txt`
SHA `4834bdd5cae8d90407d6365003d026282d20c1bdc6f4258c9725dfcdc096c700`;
patch `numeric-flow-wire.patch` in that directory SHA
`897ed66fc09cc37ed15a33b734329aafb22e8f0fe727952f6dee48b5acbd2ca2`.
29files/93hunks,+187/-196 production by proposal method. Hunk replay/format checks
and applicability to de5 passed. Main integrated the proposal.
`numeric-flow-wire-rust.log`:13passes with one
LEAK annotation; CLI/import4passes; `numeric-flow-wire-dto.log`:12passes including
a clean repeat of that case. Earlier annotation stays observed. Swift compilation
found one numeric-current/string fallback at TaskFlowView154; main repaired it.
A stale stalled-node fixture assertion then failed and was corrected against its
capture. `numeric-flow-wire-swift-3.log` passes8tests in5suites (12.937s),
including mounted terminal/draft/companion retention. Earlier failures remain
evidence; full final integration and hosted rerun are still outstanding.

| Boundary | Retained proof and limit |
| --- | --- |
| Run SQL table removal | `historical-table-public-2.log`:6cases; `historical-table-canonical.log`:released/development populated preservation. Every old column retained as immutable input evidence, unknown attachment stays unknown. Not complete wire/native acceptance. |
| Exec entry/exit | `exec-entry-proof.log`:12; `exec-early-boundaries-2.log`:3; `exec-entry-settlement.log`:17; matching Linux interruption1. Exact exits/early paths pass in private fixtures; landing thread gap above remains. |
| Live Exec reader | `exec-owner-live-reader.log`:7; public no-Git/private-store2; corrected command-context1. Exec row owns identity, OS receipts own liveness. Final paging/wire/dense proof remains. |
| Chapter/default Flow | `chapter-public-cli-3.log` passes public default Flow, rotation and second-Home sync, preserving started Task/worktree/PR/invocation. Disposable Linux and synthetic providers; no configured Linear/installed acceptance. |
| Desktop ancestry | `desktop-ancestry-handback.md`:25navigation checks and mounted PTY retention on handback bytes. Final numeric wire/headless discovery/configured Desktop remain separate. |
| Native usage | Missing-baseline public seed reproduced90 instead of60; `native-baseline-gap-red.log` then green plus retained dedup/peak regression. Public corrected seed60 with onegap; `native-baseline-public-recovery.log`:120/30 once after both lf receivers exit. Real Codex/scripted Responses; inspector remains, no zero-native-client/configured-account proof. |

Logs above live under `.lf/tmp/cut-i/`. Original supervisor baseline seed remains
`.lf/tmp/execution-model/supervisor-native-baseline-gap/results.json`; do not
rerun its writer over that failing receipt. Native usage's earlier canonical
snapshot predates immediate-predecessor repair; retain exact snapshot limits.

## Unresolved native navigation contract

[Native retry options](research-native-retry-options.md) and
[bounded tradeoff](native-turn-retry-tradeoff.md) retain the paired red results.
The failed-turn delayed child race was reproduced with real Codex and synthetic
Responses. The caller-token repair rejects it but also rejects a legitimate retry:
a failed loaded native thread retains its tool environment. Unsubscribe/resume and
interrupt did not fix it; successful-thread control refreshed the environment.
Jack has not selected a new decision interface. No provider fork, tool proxy,
shared-engine kill, new attempt object or retry-contract change is authorized by
these probes. Existing independent work continues. Valid retry plus stale-child
rejection and shared-sibling preservation remain one acceptance obligation.

## Research to reuse

- [Indexed discovery](research-indexed-discovery.md): SQL-only20kSessions/5kFlows
  experiment, not CLI latency. Preserve nullable historical membership; Desktop
  currently requests unlimited inventory. New Exec proposal builds on this.
- [Session wire/Desktop](research-session-wire.md):73-file audit; final typed
  history/headless discovery and consumer/paging migration remain.
- [Run removal](research-final-run-removal.md): historical evidence is not unused
  CRUD. Table removal now has populated proof; active consumers still need removal.
- [Exec admission](research-exec-admission.md) and [proposal](proposal-exec-entry.md):
  integrated early-path work, superseded by current source/proofs where noted.

## Control commands and authority

Use `uv run python .lf/tmp/cut-i/control-checkpoint.py …`, which pins:
- binary `/Users/jack/.lf/bin/lf-f5ef8d640340e9f8b9e36d17d84de83e14e905305c43e959fd00c49a322a527f`;
- Home `/Users/jack/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea`;
- LF_BIN/LF_CONTROL_BIN, LF_HOME/LF_CONTROL_HOME, LF_DB_PATH/LF_CONTROL_DB_PATH.

Bare lf selects another Home. Refresh Session list each turn/after mutation;
inspect task status and ps for actual current authority. No lf ask for Jack here.
Use `--task LOO-298` for contributions; Wave binding once relocated research to
repo root. Source proofs scrub inherited LF/LOOPFLOW and select disposable Homes;
installation proofs require disposable OS account/container. Never signal unclaimed
processes, infer death from silence, or start a competing writer on timeout.

## Measurement and preserved history

Published de5 versus base6e718: **+16,319 / −30,036 = net−13,717** production.
Rust/Swift+15,113/−29,974; Python/shell+48/−62; SQL+1,158/−0. Receipt:
`.lf/tmp/execution-model/status-counts-de5fcea9c.json`. Corrected production
prefixes, no rename detection, tests/docs excluded. Changed bases prevent reading
successive branch totals as incremental cuts; working patches are excluded.

Older scratch:62files/676,767bytes in `.lf/tmp/scratch-consolidation-20260928/scratch/`
with SHA manifest; eleven LOO291 originals under `.lf/tmp/context-archive-7e2101b41/`.
Commit7e2101b41 preserves published originals. Useful evidence needs a durable
owner before delivery clears scratch. Historical models never override decisions.

This index's exact pre-curation bytes are archived at `.lf/tmp/scratch-curation-20260929-control/parallel-work-e6a3e35467bc.md`
(SHA-256 `e6a3e35467bcdd66b14eb2258451b640851b0a231cd5b2b72b066454ce35ddaa`). Main handoff, design, full scope and evidence are
unchanged. Older index is also committed at
`4ced9467fde48b88b232c346af054ff9e3ad8f58:scratch/parallel-work.md`.
