# Main implementation handoff

LOO-298 · 2026-09-28. Main owns executable edits, builds, Git and this handoff.
Supervisor owns the other compact scratch documents. Read the full
[contract](data-model-one-table-per.md), [remaining scope](remaining-work.md),
[import obligations](import-preservation.md) and [evidence](evidence.md).

## Current change and proof

Flow selects an exact AgentSession native start under its version/claim and
conversation driver/provider generation. Cursor advancement consumes that turn's
successful completion transactionally in `flow_events`. A surviving engine is
read back after driver loss; the original Exec outcome remains unknown.
Explicit retry after confirmed engine exit releases the selected boundary without
inventing an outcome. `flow resume --retry` retains that instruction through
managed Task dispatch; `task run --retry` and an explicit repair reason use the
same preparation with existing Task adoption/agent/unblock policy.

Automatic retries in one Exec may replace a failed/interrupted selected turn,
retaining every outcome and usage receipt. Reselection clears the failed turn's
decision/router candidate atomically. Live/successful selections, old versions,
claims and provider generations cannot be replaced. Review caught that merely
changing `selected_start` inherited a failed turn's Advance; that public RED is
retained by the supervisor. Missing replacement decision now blocks; a failed
Iterate can be replaced by successful Advance.

Candidate SHA `894aded0d8c465a971d4006ed6016e2c3ae38bbd1d6eb175100c7cadd473d701`:
`.lf/tmp/execution-model/native-recovery-{automatic,both,decision-missing,decision-replace}-combined/results.json`
all pass through actual CLI/Codex0.157.1 with synthetic Responses/private Homes.
Running/completed driver-loss repeats also pass on these identical bytes.
Prior separate candidate passes and original failures remain in evidence.md.

`.lf/tmp/cut-i/flow-navigation-tests.log`: 4/4 pass (exact selection and both hosted
CI failures plus managed retry). Managed proof uses public Flow-to-Task dispatch,
retained adoption refusal and synthetic claimed-successor history consumed by the
shared driver. It does not prove configured managed Codex launch/account continuity.
Earlier compile failures in `flow-final-recovery-tests.log` and
`flow-retry-parity-check.log` remain retained; corrected checks pass.
`flow-recovery-clippy.log`: all-target Clippy passes. Formatting, Ruff, migration
history (55 shipped unchanged) and architecture inventory pass. `canonical-recovery.log`: 5/5 pass on 23 drafts materialized into a disposable
0.12.25 canonical source copy, including managed recovery and empty-draft upgrade.
No broad gate or new hosted CI is claimed.

Against published `7e2101b41`, production +434/-78 (net +356): Rust/Swift +413/-75,
SQL +17/-0, Python scripts +4/-3. Method strips trailing tests, excludes tests/docs,
disables rename accounting and includes the new SQL draft. Receipt:
`.lf/tmp/execution-model/flow-recovery-counts.json`. This is one owner conversion
checkpoint; Runs and their fallback still exist. No deletion or code-complete claim.

## Docs-first checkpoint and next dependency

Recovery is checkpointed, rebased onto main `d9632d833` and published as
`26a0270af` on PR1296. Captured installed control completed rebase bookkeeping;
Task and Git retain the same base. The reconciled Session launch/retry proof
passes and preserves exact executable/Home/database plus failed-launch evidence.

Jack requested docs/README before further owner conversion. The docs-first change
rewrites the front doors and active architecture/user guides around Exec,
AgentSession and FlowSession. Architecture Reference owns one cutover-status
matrix mapping spec to remaining implementation; its checked source inventory
retains genuine transitional Run dependencies and removes deleted Chapter owners.
Examples use current parser spellings (connect exists; --restart does not).
CLI bind states/writes the permanent target; Desktop confirms. No shipment claim.

Proof: README/index and portable architecture checks pass (2 selected tests).
Generated architecture.html and website docs refreshed. Checked source snapshot
passes architecture coverage: APIs36, process5, tables35, projections6, providers6,
subprocess25, shims3. Receipt `.lf/tmp/cut-i/docs-architecture-source.json`.
The in-place checker failed only on ignored rebase prompt copies under `.lf/log/`;
those original logs remain untouched. This is a source-snapshot pass, not a green
in-place scan. Review corrected old Chapter commands/packet prose, failed-turn
navigation authority and ambiguous bind confirmation. Supervisor's coherent
scratch updates belong in this checkpoint.

Docs-first checkpoint: `4bb44b999`. The following repair addresses the hosted
metric-portfolio counterexample:
26a027 CI36507832289 / job109213159143 has 882 passes, one failure, 13 skipped,
1,050 unrun. `ops::flow::tests::telemetry_flow_persists_the_portfolio_reading`
loses an observed Wave reading when current Project planning is unavailable.
Raw log: `.lf/tmp/execution-model/recovery-26a027-rust-ci.log`.

Local RED reproduced all four affected metrics/Flow failures. The repair discovers
Wave instruments and joins persisted readings even without an unambiguous Project.
Optional target planning distinguishes unavailable from known empty. Fresh values
with unknown targets retain their value/window in the coordinated Rust/Swift
`target_unavailable` cause; stale/missing evidence stays intact. CLI/Desktop row
targets use ChapterUnavailable for the row's Wave, including stale/never-observed
rows; known empty remains unset. No target plan or KR is mutated.

Proof: `metric-final-rust.log` passes 26 focused metrics/Flow/CLI/DTO checks;
`metric-swift-2.log` passes 22 DTO and Wave presentation checks; `metric-clippy.log`
passes all-target Clippy. Logs are under `.lf/tmp/cut-i/`. The first Rust compile
caught the missing reason arm; the first Swift run passed 21/22 and caught its
stale aggregate headline expectation. Both observations remain in their logs.
The final tests cover missing, empty, targeted and ambiguous planning over one
saved observation, plus per-Wave unavailable target rendering. This is fixture
and presentation-model evidence, not a rendered Desktop or full hosted result.

Published docs/metrics head was `fa263c19d`. Rebase onto `aab595e6a`
completed at `81b57c91f`, preserving telemetry's finished-in-window reader and
missing-evidence behavior. PR1296 is published with auto-merge off. Hosted Rust
36510330745 stopped after 928 passes at the OAuth fixture's absent Project status;
other substantive hosted jobs passed, while active scratch remains outstanding.

Current retained conversion moves mechanical starts/results from Run capture to
Flow history in the shared Task/taskless executor. One actual process retains one
Exec across several boundaries. A forward draft preserves historical mechanical
Run receipts and unknown start/Exec evidence; earlier Run bytes remain for the
full import. Claims alone write no Started; beginning operation history sets it
atomically. Version/claim/start/Exec checks fence results; an absent completion
stays unknown and requires inspection or explicit retry. Agent publication,
selection membership and verdict/route authority still depend on Run.

Proofs under `.lf/tmp/cut-i/`: `flow-operation-owner-red.log` observed two Runs
for two in-process operations. `flow-operation-owner-green.log` then passed the
public taskless ownership/history proof. `flow-operation-boundaries.log` passed
interruption/retry, OAuth recovery and taskless CLI; the managed CLI refused at
installation routing before executing the worker. `flow-operation-store-2.log`
passed populated migration preservation, exact native selection, interruption
and OAuth, then exposed an obsolete managed two-Run assertion and a malformed
telemetry command in the failure fixture. `flow-operation-error-observation.log`
prints the latter's actual parser refusal: no missing-generator effect was reached.
The fixture now uses the existing `__telemetry-scorecard` spelling. The managed
reducer now checks no Runs, separate results, claim/release non-Started and set-once
Started. `flow-operation-store-3.log` passes all seven checks, including both public CLI cases.

The managed public proof is ignored in ordinary suites with its precise disposable
OS requirement and wired into `scripts/test_task_installation.py --test
task_operation_starts_with_durable_history_after_claim_only_failure`.
`flow-operation-managed-os.log` records Docker's ten-second preflight timeout;
no container was created and no installed Home was accessed. This remains unproven.
Migration namespace/history check passes: 24 drafts, 55 shipped files unchanged.
`canonical-operation.log` passes six selected checks after materializing all 24
drafts into a disposable 0.12.25 source copy. The later comment-only correction
in flows.rs does not change those tested operations. Formatting and Ruff pass;
all-target Clippy passes (`flow-operation-clippy.log`). The source-snapshot architecture check passes
(36 APIs, 5 process boundaries, 35 SQLite owners, 6 projections, 6 providers,
25 subprocess edges, 3 seams); receipt `operation-architecture-source.json`.

Independent actual-CLI crash/retry proof inspected:
`.lf/tmp/execution-model/supervisor-mechanical-crash-1/results.json`, candidate
SHA256 `33be09c872e4c82c786237a1b21f23d59c024c33a56a1be247fe95f548182467`.
Private telemetry writes an effect, then the fixture kills its exact lf group.
With the template deleted, ordinary resume refuses and retains one effect;
explicit retry completes with two total effects, zero Runs/AgentSessions and the
original Exec outcome/exit still null. This proves public standalone recovery
with synthetic local effects, not the outstanding managed OS installation path.

Production delta against `81b57c91f`: +218/-64 (net +154), including +59 SQL;
receipt `operation-line-counts.json`. Counts strip Rust test modules and exclude
tests/docs/proof scripts. Removed `run_op`'s RunSpec/CaptureHandle/publication and
operation input sidecar writes, plus the synthesized interrupted Run outcome.
No file is deleted by this cut and agent Run ownership remains. Review corrected
the early claim-only Started writer, obsolete ownership assertions and hidden
telemetry spelling. No full-conversion or configured acceptance claim.

Next resume execution owners and the full remaining-work matrix: publication,
decision membership and mechanical results to final Session owners; offline
import before Run deletion; indexed reads, usage, Rust/Swift DTOs, Desktop,
Chapters, dense measurements and configured acceptance. Active builtin chapter
instructions and ops/task.rs still need final consistency conversion from
`lf wave new-chapter` to the supported repository command. Do not edit immutable
migration history. No landing, auto-merge, promotion or Flow navigation.

## Control and proof discipline

Use captured installed control in [parallel-work](parallel-work.md). Never run
source drafts against installed Home. Saved feature invocation remains
`1f9ba70e-f0e9-41d4-9722-948d7bc4ce8c`, implement iteration9; no restart selected.
Single build slot; disposable proof Homes, inherited LF/LOOPFLOW authority scrubbed,
four Cargo workers/nice +10. `.lf/tmp/cut-i/run.py` owns proof environment/logs.
Canonical tests materialize only a disposable source copy. Native fixtures copy
the candidate before executing. Do not restore recursive archived scratch logs.
