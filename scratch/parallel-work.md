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
  Codex80014. Numeric Flow wire and landing helper admission are published;
  main is integrating Exec discovery and its public reader. Recheck status/ps before treating these as live.
- Supervisor owns this index and isolated read-only inspections; no second
  executable writer or competing builds. Control Session
  `run_9c16dbbe2b04440db8469e9b4964912c` has human title `loopflow`.
- Exec discovery contributor tool3514 completed exit0; Exec9ef1a796/PID23045
  is absent from fresh ps. Its `scratch/proposal-exec-discovery.md` and private
  patch are released to main with the search counterexample below. No contributor
  remains active from that contribution; main owns every integration and build.
- OpenCode decision-authority research tool74230 completed exit0; its Exec
  `b5a25b92-817b-4246-b1f4-b0ff8920c86b`/PID29810 is absent from fresh ps.
  [Handback](proposal-opencode-decision-authority.md) is released to main;
  artifact SHA `503fd045d18f52efd33bdac3b4f8c9781df5143872eb4e44afc3da4895933fff`.
  No apply-ready patch/build/provider proof. Supervisor checked29 hashes (only
  main's execs.rs drifted) and the selection/recovery source findings. Verified
  release comment `b8c72d43-428c-4c85-9286-777083e5a79a`; private receipt
  `.lf/tmp/opencode-decision-proposal/supervisor-review.json`.
- Desktop Session-controls proposal is active via tool25845, Exec
  `7ea3b81d-d5a7-4ca3-9d17-03e1b876cc52`, PID5172. It owns only
  `scratch/proposal-desktop-session-controls.md` and private patch directory
  `.lf/tmp/desktop-session-controls-proposal/`. Explicit headless discovery and
  permanent bind confirmation; no working source/build/PM/Git changes. Main must
  await terminal handback before publishing this artifact. Verified assignment
  `66d68e4d-cc30-4236-9813-5868097a48a3`.



Preserve saved implement → compress → review-slice → concept-review →
loop-decide → human-demo order. Supervisor chooses no edge. Earlier concept
reviews were not code-complete reviews. Completed reviews are not fresh approval;
supervisor direction is not a new decision attributed to Jack.

## Delivery and immediate counterexamples

Published **a866926d01d91d98b7e0593010d0cdc74617a186**, based on
**6e7189926ede81b3edd05c8d81955c6cbe67a4e2**. GitHub PR1296 and Task publication
agree. Optional Linear PR-link enrichment failed JSON decoding; acknowledged
GitHub publication survives. No auth repair is justified by that optional failure.

Current CI36591364925: Rust job109485005231 failed the retained OpenCode
`a_taskless_step_records_its_decision_on_the_invocation`:1,855 passes, one failure,
15 skips,137 unrun,162.390s. Both decisions lack original-turn authority. Log:
`.lf/tmp/cut-i/ci-a866926d0-rust.log`. Landing scenario passed on this head.
Architecture, migrations, lint, installation, Python, website, UI and smoke passed
at completion; Swift also passed, scratch-clear and aggregate failed. No full green.

Prior CI36587831497 is terminal. Swift, UI, installation and other executed checks pass
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
Managed topology passed in `numeric-flow-wire-managed-2.log` (1.84s) in the
disposable Linux installation. Final all-target Clippy and architecture passed
(`numeric-thread-clippy-2.log`, `numeric-thread-architecture.log`); published in
fa7c76f9d. The following documentation-only a866 checkpoint included the completed
Exec proposal. Its terminal handback was verified; preserve its unapplied status
and history, with no rewrite. Comments0123f521/520f266d retain this reconciliation.

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
evidence; current hosted rerun is recorded above, and full integration remains.

| Boundary | Retained proof and limit |
| --- | --- |
| Run SQL table removal | `historical-table-public-2.log`:6cases; `historical-table-canonical.log`:released/development populated preservation. Every old column retained as immutable input evidence, unknown attachment stays unknown. Not complete wire/native acceptance. |
| Exec entry/exit | `exec-entry-proof.log`:12; `exec-early-boundaries-2.log`:3; `exec-entry-settlement.log`:17; matching Linux interruption1. Exact exits/early paths pass in private fixtures; landing thread repair is recorded above. |
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

The OpenCode handback adds a distinct provider gap: both direct CLI and SSE paths
lack the selected native history; direct retry loses Session identity, display
busy IDs are not native history, and recovery always opens CodexConnection.
An unselected ordinary step can pass consume_selected_in without proving native
completion. Main's next native proof is actual OpenCode with a scripted model,
exact request identity and before-tool ordering, followed by one shared adapter.
No token-only fixture repair or new hook/proxy/interface was selected. This does
not establish Codex's failed-thread environment behavior for OpenCode.

## Exec discovery proposal handback

[Exec discovery](proposal-exec-discovery.md) supplied a four-file/7-hunk patch,
+261/-33 production, no migration or wire change. Patch:
`.lf/tmp/exec-discovery-proposal/exec-discovery.patch`, SHA
`33f0f1496e10182e332129b195c78c1b1ad8296db48bf439494d6d8633052dd9`.
Artifact SHA `2b85953ac0561625614d0af945b5b4853fe2ae800343cb1f5ee58ad46f61c105`.
Supervisor checked98hashes (only main's remaining-work.md drifted), read the
query/model/tests and verified read-only applicability. Private format/replay and
10,004Exec SQLite probes pass; no Rust/application/dense latency proof.

Review found a concrete search mismatch: the actual writer stores JSON argv;
raw `instr(command, 'pr land')` cannot match `["lf","pr","land","--strict"]`.
Proposed tests used plain command strings. Main must use writer-shaped fixtures
and searchable command text while preserving raw/unknown evidence and literal
contains semantics. `.lf/tmp/exec-discovery-proposal/supervisor-review.json`
retains the SQL-expression counterexample and source hashes. Ownership is released;
main integrated the query into working `lf exec list/show`, preserving raw command
and searching parsed argv text. `exec-discovery-rust-3.log` passes the four store
cases, one Rust DTO case and the existing live-reader regression, but fails its CLI
seed (`status` versus `work_state`). The next CLI failure exposed a real reader dependency: full WorkCatalog decoding
required an unrelated Project name. Main replaced that path with direct
Task/Wave ID queries; the unnamed historical Project remains in the fixture.
Only the first failure was a fixture correction. `exec-discovery-cli-final-2.log` then
passes the public command case in4.445s over seeded historical rows. No provider
was run in that case; it proves CLI search/paging/lookup/Work selection. `exec-discovery-swift.log` also passes its one matching wire test. This is
working-tree evidence, not a published checkpoint or full paging/density/Desktop
acceptance. Main retains all final integration and testing ownership.

Repository scope review found a second concrete reader defect: Wave-name lookup
runs before the current-repo filter, so two repositories with an infrastructure
Wave cause false ambiguity. `.lf/tmp/exec-discovery-proposal/supervisor-repo-selector.json`
replays the exact SQL over a two-repo fixture; it is not compiled CLI proof.
Verified comment `c99d1772-4662-4564-aaea-f28e1a668ef4` asks main to scope name
resolution and extend the public proof, preserving stable IDs and --all ambiguity.

The repository selector correction now passes the extended public proof in
`exec-discovery-repo.log` (6.009s), including two repos, --all ambiguity and stable
IDs. `exec-discovery-dense-2.log` and `.lf/tmp/exec-discovery-measure/results.json`
measure working binarye41f0393 on a disposable current-schema fixture:100,001Execs,
20,000Sessions,5,000Flows,1,000Tasks,117,071,872bytes. List p50/p95=314.32/319.61ms;
detail318.48/321.11ms; contains miss338.94/340.85ms. Twenty repeated fresh-process
samples follow the first; cache uncontrolled, no provider/installed Home. Whole-CLI
latency only; startup/SQL/payload separation, plans and Desktop remain unproved.
Initial seed failed the capture-ID constraint; repaired seed retains constraints.

## Historical NULL-title classification still open

Supervisor inspected released `v0.12.24`: `children.rs` Task insert/update writes
`plan.title` as a String, while `0.11.036_delete_sessions.sql` adds a nullable title
and copies it from the latest old Task Session. The `0.12.20` position migration
copies Task positions without checking title; the current review migration inserts
that title into a NOT NULL Session column. Retained failing fixture remains real,
but a supported released writer producing that exact NULL-title/review combination
has not been established. Do not invent a title or rewrite an applied draft on this
source evidence alone. Further released-source inspection narrows the question:0.12.15 drops all old
work_flow_positions and0.12.16 creates an empty replacement, so pre-0.11.036
position history cannot directly survive into this input. The0.12.16 controller
creates its human position from ControlledTask; review serving/open reads Task;
its Task reader requires a String title and inserts/updates write plan.title.
Store-level set_flow_position itself checks Work readiness, not Task title.
A complete intervening-release caller audit remains before ruling the synthetic
combination out; do not claim an observed real-Home upgrade failure.

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

Published a866 versus base6e718: **+16,505 / −30,210 = net−13,705** production.
Rust/Swift+15,299/−30,148; Python/shell+48/−62; SQL+1,158/−0. Receipt:
`.lf/tmp/execution-model/status-counts-a866926d0.json`. Corrected production
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
