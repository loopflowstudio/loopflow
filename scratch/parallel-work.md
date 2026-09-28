# LOO-298 parallel work

2026-09-28 · Supervisor ledger · Jack Heart requested autonomous management to a
code-complete concept review, Codex only, with useful parallelism and regular
rebasing/publication for dependent Tasks.

## Current work and ownership

The [working design](data-model-one-table-per.md) owns the accepted model and
completion matrix. Main's [implementation ledger](parallel-execution.md) owns
executable progress. This file owns contributor placement and interpretation
of independent proof; it does not choose a Flow edge.

Main Run `run_8e4800ebef6045699e0be1fac668e6cb` owns all executable files,
builds, cleanup and Git. Its captured feature invocation
`1f9ba70e-f0e9-41d4-9722-948d7bc4ce8c` remains implement, index 2/13,
iteration 9. Exec `60cc4bdb-6d29-481f-b5a7-de4b7d74b57d` and Codex process
were confirmed live after publication. The completed contributors/probes below
are terminal; their file ownership has returned to main. The bounded discovery
audit `run_4246108f8d4c4144a14bfb6d0c38e8bf` (handle 45352, exit 0) wrote only
`scratch/parallel-discovery.md`; supervisor inspected its report and core cited
source, then returned ownership to main. No executable edits or builds were
delegated. Supervisor owns this
ledger and the design, and may run isolated nonbuilding proofs.

The next implementation is the physical AgentSession/FlowSession conversion and
Run removal. Required follow-through: explicit restart, exact successful history
consumption by Flow, complete historical import, normalized usage, typed
Desktop ancestry/history and pane retention, indexed discovery measurements,
integrated checks, compression and code-complete concept review. Renaming the
tables alone does not finish the cutover. Keep the captured Flow; no restart,
recompile, advance, completion, landing, auto-merge or Home promotion is selected.

## Control selection

Machine installation selection changed independently during supervision. Bare
`lf` selects a Home without this Task; that absence is not worker death.
Control uses the worker's captured installed binary:

```text
/Users/jack/.lf/bin/lf-f5ef8d640340e9f8b9e36d17d84de83e14e905305c43e959fd00c49a322a527f
```

Set both `LF_HOME`/`LF_CONTROL_HOME` to
`/Users/jack/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea`
and both DB overrides to its `loopflow.db`. This is the captured installed
control, not a branch binary or promotion. No machine-selection/auth repair is
part of this work. Every source proof uses disposable Homes and scrubs inherited
execution authority. No branch draft binary may access the installed Home.

## Published checkpoint and proof

Published head **2958d289c0e4087118e6920390fa6c8975c216d8**, base
**c512813b5b333f2ae012503b1781fcbd458be67a**, [PR #1296](https://github.com/loopflowstudio/loopflow/pull/1296).
Remote branch, GitHub and Task PR row independently agree. Manual rebase was
conflict-free, zero commits behind the freshly observed main. Auto-merge is off.
Jack's publication direction was relayed in comment
`de0d4762-6acb-4535-af07-45c9ac20d8c0`; it makes this implementation checkpoint
available to dependent Tasks, without claiming completion.

CI **36487487807** is the current checkpoint run. Python, website, installation,
smoke, migration, architecture, Rust lint, Swift and UI compile checks passed.
Rust stopped at two review-completion fixtures that cannot resolve `lf` on the
clean CI PATH (`controller/task/mod.rs:952`), after 11 passes, with 1,912 tests
unrun and 13 skipped. The previous canonical initialization failures now pass.
Exact failures: `completing_a_final_review_finishes_the_flow` and
`concurrent_review_completions_settle_once`; log
`.lf/tmp/cut-i/repair-checkpoint-rust-ci.log`. Relayed to main in Task comment
`e24c013b-85d9-4fb5-8194-962d141e4116`; fixture executable selection must be
isolated without weakening production launch behavior or outcome assertions.
The initial five public rename checks pass Task Flow failure/retry/review and
inventory scoping; two stale `count("sessions")` assertions and the shared
stdio-only Codex stand-in fail. Main is reconciling these fixtures. The latter
also serves landing, publication and release tests, so one shared fixture
conversion is required. Supervisor found two additional unguarded review-helper
callers (`restarting_a_human_node_reuses_the_same_task_position` and
`stale_human_decisions_cannot_target_a_replacement_invocation`); their clean-PATH
replay is due, not a new observed CI failure.
Focused repair now passes all four review tests (`review-executable-pins.log`,
1.572s). The shared Codex stand-in accepts Unix WebSocket transport; the four
selected public checks pass their assertions (`renamed-owner-cli-repair.log`,
15.932s), but the Started case is marked Nextest LEAK. Exact fixture cleanup is
still unproven and remains with main. These local results do not change the
published head's red Rust result or establish completed owner conversion.
Scratch-clear failed because the required working evidence remains. The prior
CI run **36482349277** failed canonical Rust initialization, installation,
Python and website checks. Those failures remain history, not green results.

| Repair/proof | Actual result and limit |
| --- | --- |
| Canonical initialization | Empty-draft RED reproduced nested transaction; repair retains one transaction owner and previous-generation backup. Four draft checks pass. Seven tests pass after materializing 19 drafts in a disposable source copy, including all three actual CI failures and populated backup preservation; 31.14s compile, 4.373s execution, 1,534 library tests unselected. Logs and exact source hashes: `.lf/tmp/cut-i/canonical-{init-red,init-repair,materialized-repair}.log`, `canonical-repair-source.json`. |
| Installation | First three disposable-account cases pass, including retained-pair recommendation. Fourth initially tried opening an unprepared review without tmux; then failed a stale error-string assertion. Final fixture explicitly seeds a published review and passes in 17.31s after 23.69s compilation: stale feedback rejected without Flow change, installed feedback consumed once, private feedback/events retained. Native launch is synthetic, not proven. `task-installation-repair.log`, `task-review-installation-{repair,final}.log`. |
| Python installer | Contributor removed obsolete daemon flags, retained CLI candidate promotion/no-direct-activation assertions. RED then four focused simulated activation checks pass, 2.18s. [Report](parallel-release-test.md). |
| Website | Security browser check and generated HTML freshness pass. Main reconciled the additional portable-page Run wording; final portable check passes in 0.82s. [Report](parallel-website-ci.md). |
| Resource recovery | Real private nested uv reproduces lock conflict. Production 15-second prune deadline reports timeout and continues eligible build cleanup. Six tests pass in 0.43s; actual default-timeout nesting returns in 15.15s. Ruff/format/whitespace pass. No forced or shared-cache cleanup by contributor. [Report](parallel-resource-recovery.md). |
| Final static checks | Formatting, all-target Clippy (16.70s), architecture and migration checks pass on the repair checkpoint; 55 released migrations unchanged. No full green gate follows. |

Completed contributors: release `run_47fe4b2589894b7abd55819969e7ec3d`
(handle 3483), website `run_4709ca9579ee40fe985e4771e81e0102` (95016),
resource `run_c136605cdb4442bda1f8571eb511b5f3` (52241); each exit 0.
Supervisor read each report, actual output and diff before returning ownership.

The resource inventory found only 74 MiB in the clean merged worktree selected by
`lf wt prune --dry-run`; no worktree was removed. Read-only preflight passed at
65.84 GiB, then builds consumed headroom. Main's later recovery passed at
64.6 GiB, explicitly retaining a uv timeout. Free-space floor remains 64 GiB.
Own incremental cache was empty; `swift/.build` was 2.0 GiB. These dated
observations do not authorize deleting another active worktree's outputs.
Latest cleanup observation: main removed 13.2 GiB of this checkout's rebuildable
Loopflow package artifacts. Supervisor's fresh inventory passes at 75.48 GiB;
inactive build roots are empty, uv cache is 12.95 GiB, and no further cleanup
was launched. The dry-run now names agent-870a75c2, discord and dogfood;
remote-gone/closed/stale labels alone do not authorize discarding authored work.

## Retained execution evidence

Public probes use actual Codex 0.157.1, synthetic local Responses, controlled
protocol clients and private Homes. They are neither configured-provider nor
rendered Desktop acceptance. Receipt directories below are under
`.lf/tmp/execution-model/`. Earlier failed receipts remain intact.

- **Busy input after handoff:** `supervisor-busy-start-regression-2/results.json`
  passes on CLI SHA
  `f2b9f5ab7113cf4c5b1a08017927c791ad79d7da51f09c0e584eda10187600b4`.
  B receives the same active native turn after connecting, preserves A's Exec
  and provider generation, and records one completion/usage. The `-1` strict
  regression fails on the older candidate. Diagnostic scripts that exit zero
  after recording reply timeout are not passes. Native comparison established
  that turn/start may add input to an existing turn.
- **Approval:** `supervisor-public-approval-1` rejects the old client's answer;
  the selected client executes one marker and the sibling survives. Native
  replay also works when A closes before B connects (302ms gap), but another
  native client remains on the sibling. [Native report](parallel-approval-probe.md).
- **Paginated recovery:** `supervisor-pagination-1` recovers 104 exact
  completions across a 100-item page, retains 103 unknown origins and is
  idempotent. Only the latest missed turn supplies a usage receipt (20/5;
  lifetime 2100/525). The other 102 turns' missing usage stays unknown; no
  inferred token split or lifetime-total reassignment.
- **No clients:** `supervisor-zero-clients-1` closes both lf clients and the
  passive native inspector. A marker is written before a fresh inspector
  connects; reconnect records one completion and 120/30 lifetime usage while
  the original Exec stays interrupted. The marker proves command execution
  before inspection, not whole-turn completion before inspection.
- **Public lifecycle:** `public-live-history-2` retains exact selected child
  ancestry after handoff, rejects old start/steer/interrupt, and old headless
  SIGINT/130 leaves selected and sibling turns alive. Earlier
  `supervisor-driverless-history-2` had a passive native inspector; do not
  reinterpret it as the later zero-client case.
- **Rebase integration:** 20/21 focused checks initially exposed stale DTO
  rejection of H7 Project history. Correction batch passed ten DTO/history
  tests; 47 selected Swift tests passed, including complete 101-Session
  inventory. Copied-Home proof retains history, clears live driver/endpoint
  authority and advances generation; it does not prove rendered panes.
- **Ordinary admission:** repaired unknown-Wave command returns error/1 and
  retains a failed Exec; successful inventory retains its succeeded Exec.
  The four-test admission/safety batch includes a Nextest LEAK result for
  exact-process evidence. Assertion success does not prove clean settlement.
  File-journal obstruction has public preservation proof; installation/preflight
  and screenshot observation exceptions still need explicit final disposition.

Source review previously found replayed usage could inherit the observing
replacement's generation. Current recorder uses stored turn origin through the
event writer instead; fresh gen1/missed-usage/gen2 public restart proof remains
due. Same-engine handoff cannot establish provider-replacement correctness.

## Remaining review inputs

These reports constrain the complete conversion; their existence is not proof
that their recommendations or fixes have been implemented:

- [Import review](reviews/parallel-import-review.md): autonomous/terminal
  taskless Flows, completed keyed Asks, old members/outcomes/ancestry, conflicting
  idempotent input, interrupted import and exact membership. Never fabricate
  an Exec from a historical Run without actual process evidence.
- [Handoff/history review](reviews/parallel-handoff-history-review.md):
  separate provider completion from driver exit and exact Flow consumption.
  Subsequent public probes close named transport cases only.
- [H7 review](reviews/parallel-h7-review.md) and
  [follow-up](parallel-h7-followup.md): preserve raw Project content, status
  conflicts, existing Projects, partial mutation retry, second-Home convergence
  and uncertain Task retirement. Fixture proof is not live Linear rotation.
- [Publication](parallel-publication.md), [stacking](parallel-stacking.md),
  [incidents](parallel-incidents.md): preserve acknowledged remote identity and
  Task/PR links; dependent Tasks share placement rules. Scope/disposition stays
  with the existing reports, not duplicate Tasks.
- [Performance](parallel-performance.md),
  [post-rebase projection](parallel-rebase-projection.md),
  [initialization](parallel-initialization.md): complete consumer changes and
  final dense query/latency evidence remain due. Current Session SQL limits
  before payload reads, but Exec discovery and Run removal are still incomplete.
  Bound Tasks absent from current roadmap must not become orphans.
- [Discovery audit](parallel-discovery.md): Exec lacks summary/detail queries
  and typed Task command context; current Run readers hydrate before their cap.
  Session inventory already filters before enrichment but still joins Run and
  scans title substrings. Desktop uses unlimited inventory to avoid offset races.
  Keyset paging, prefix search and fixture scale are proposals, not decisions or
  measured results; preserve/document actual search behavior. Keep command context
  distinct from immutable work attribution, and exact OS receipts for control.
  Main owns the integrated reader/DTO replacement and actual dense measurements.

## Measurement and history

Last fixed published production-prefix measurement, at `e13f29909` versus
`c512813b5`: **+13,189 / -29,434 = -16,245** across Rust/Swift,
Python/shell and SQL. Tests/docs excluded, no rename detection, trailing test
modules excluded with the known trailing production block retained.
Receipt: `.lf/tmp/execution-model/status-counts-e13f29909.json`.
Do not present it as a fresh count of later edits. Changing merge bases changes
the comparison; moved files alone never count as removed code.

The detailed previous supervision ledger, exact intermediate failures, old
allocations and receipt paths are preserved in the
[published repair checkpoint](https://github.com/loopflowstudio/loopflow/blob/2958d289c0e4087118e6920390fa6c8975c216d8/scratch/parallel-work.md).
This curation removes repeated launch context, not evidence or acceptance
obligations. Earlier local-only publication instructions in that history were
superseded by Jack's checkpoint publication request. The working design remains
the finish line; no requirement is waived by this ledger.
