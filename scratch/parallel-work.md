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
  Exec discovery and OpenCode native history are published at6c0981dbd; main is integrating Desktop controls. Recheck status/ps before treating these as live.
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
- Desktop Session-controls contributor tool25845 completed exit0; Exec
  `7ea3b81d-d5a7-4ca3-9d17-03e1b876cc52`/PID5172 is absent from fresh ps.
  [Handback](proposal-desktop-session-controls.md) is released to main: artifact
  SHAcc9cc1550fe1; private patch SHA1dddcf4634ee in
  `.lf/tmp/desktop-session-controls-proposal/desktop-session-controls.patch`.
  18files/40hunks, proposal production+258/-35. Supervisor checked all18
  original/proposed hashes without drift and read-only applicability; no build
  or behavioral proof. Keep complete inventory separate from visibility and
  retain the conversation workspace when binding to a differently placed Task.
  Exact stable IDs stay in transport; normal confirmation should show name and
  issue/title. Verified release comment `654c1fe3-550b-4d57-87f1-1731f71bba87`;
  private `supervisor-review.json` retains the review. No contributor remains
  active from those completed contributions; main owns integration/proofs and may
  publish coherent verified work.
- Session inventory contributor tool50386 completed exit0; Exec
  `61307e9f-02ed-42fb-aa2b-efc4aef09c2e`/PID92660 is absent from fresh ps.
  [Handback](proposal-session-inventory.md), SHAa9d473b35b0d, is released as a
  partial proposal. Private patch SHA187a8fe3b816, three files/eight hunks,
  production+122/-44 (net+78). Supervisor found no source drift and read-only
  applicability passes; no compilation or behavior proof. It caches full Flow
  decoding once per referenced Flow and removes graph construction, but still
  reads captures and Session history. Reuse useful hunks in the accepted typed
  summary/detail conversion; this is not completed discovery or grounds for a
  separate broad gate on a temporary cache. The current SQL identity reducer
  validates nonidentity old envelopes too; narrowing it mechanically changes
  error behavior. Keep detail/actions strict while implementing passive metadata
  reads with retained missingness, owner/import consistency, and coordinated
  Desktop consumers. See private supervisor-review.json and verified release comment
  `0358b61e-0f2f-4de6-92f7-91f59f195d04`.
  No contributor remains active; main owns source integration and testing.


- A metadata-summary contributor is launched via tool27771, Exec
  `82595041-9f27-4289-98d9-9bb1e68e930b`, PID11958. It owns ONLY ignored
  `.lf/tmp/metadata-discovery-proposal/`. Assignment builds on the partial audit
  and targets typed metadata-only Session/Flow summaries with coherent existing
  writer/import/DTO ownership. Passive discovery can defer unrelated payload
  validation to selected detail/actions; exact action/native validation remains.
  No temporary capture cache counts as completion. No working source/scratch,
  builds/tests, branch binary/Home/provider experiment, PM/Git mutation or new
  worker is authorized. No publication hold; main continues recovery and delivery.
  Verified coordination comment `6bfb7385-94ce-4a42-b516-1b86fab727f0`.



Preserve saved implement → compress → review-slice → concept-review →
loop-decide → human-demo order. Supervisor chooses no edge. Earlier concept
reviews were not code-complete reviews. Completed reviews are not fresh approval;
supervisor direction is not a new decision attributed to Jack.

## Delivery and immediate counterexamples

Published **6c0981dbd2561b20a3e87f89a7d0da8bfdee2823**, based on
**6e7189926ede81b3edd05c8d81955c6cbe67a4e2**. Fresh GitHub PR1296 and Task
publication agree. CI36599037523 rust-test109511330688 failed after503 passes,
one failure,15 skips and1480 unrun (20.805s): `opencode_trace_error_turn` still
expects synthetic started/completed from map_event, but actual output is error
only. All four opencode_trace tests share that old replay helper. Complete log:
`.lf/tmp/cut-i/ci-6c0981dbd-rust.log`; clean copy retains the exact assertion.
Verified direction `af257f68-e386-40fc-a2bd-75a9ac86204a` asks main to migrate
or replace obsolete trace proof at the native history boundary, preserving unknown
usage, output/tool correlation and error semantics; never restore idle completion.
Other passed jobs: architecture,migrations,lint,Python,website,smoke,installation.
Swift/UI were running at inspection; scratch-clear fails with retained notes.
No full green. Main is integrating Desktop controls and owns this CI repair.
No landing or promotion.

Previous CI36591364925: Rust job109485005231 failed the retained OpenCode
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

Local checkpoint **915aa82ff** adds verified Exec discovery; publication remains
a866; the Desktop contributor has now handed back and its publication hold is released. Main has begun the actual OpenCode
protocol probe (binary1.18.33). Initial ordering proof now passes in
`.lf/tmp/opencode-native-proof/result.json`: real native binary SHA139ddeb6a46b,
scripted local model/private Home; permission.tool.messageID resolves an assistant
whose parentID equals the submitted input. Shell marker is absent before reply,
present afterward; retained messages distinguish tool-calls from terminal stop.
This is native ordering/readback evidence only, not Loopflow adapter/retry/recovery,
remembered-permission/shared-engine or configured-account acceptance.
Follow-up actual-binary receipts: `repeat-once.json` passes two requests;
`repeat-always.json` fails because the second shell marker appears without a
permission event. `restart-always.json` passes after terminating/restarting only
its private owned server; `/api/permission/saved` returned an empty list before
restart. Supervisor inspected the receipts and process sequence. That demonstrates
this fixture's in-memory approval reset, not a safe shared-engine restart policy
or universal before-tool admission. Preserve all three outcomes when choosing
and proving the shared adapter.
The main-owned handoff misattributed contributor/supervisor proposal/review/release
actions to Jack; verified comment `666c4261-88a3-4412-873f-65c18f4ae3ab` requests
attribution correction before publication, without changing authorized scope.

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

Native adapter review during implementation found two concrete mismatches.
`map_permission` expected requestID but actual1.18.33 permission.asked uses id;
verified comment `f56d40eb-ba02-4b16-94c1-608614fd0099` retains that source/native
comparison. Main's actual adapter probe then timed out with one start/no completion,
corrected the field/fixture, and reported a passing Flow probe with two native
starts/completions. Full probe inspection and retry/recovery proof remain.
Second: native History emits start/completion under user-message ID while mapped
text/items still use synthetic busy-interval turn_UUID. `final_answer` joins text
to success by turn_id, so the new split loses the conclusion. Verified comment
`d0b00c99-66e9-4568-be76-8c0a7aa9db97` requests native correlation throughout and
an actual final-answer assertion. Actual candidate5638c140015e retained observations subsequently confirm the split
for both Sessions: text IDs and successful completion IDs have no overlap. Verified
follow-up `b6b4915e-7e28-4a5f-98c7-1bda3db080a4`; exact event/result bytes are
preserved in `.lf/tmp/opencode-native-proof/supervisor-output-mismatch-evidence/`.
The adapter's current pass checks process exit and final Flow state, not final
answer, exact consumption or usage totals. Private supervisor receipts live in
`.lf/tmp/opencode-native-proof/`; no root build/provider execution.

Remaining connect-consumer source finding: public `lf runs ID --resume` still
calls runs::resume_run → util::resume_session directly (no remote endpoint or
Session driver claim), bypassing connect_live_codex claim/relay/history recovery.
Common spawn still checks deletion and publishes clients; no live duplicate-agent
reproduction is claimed. Verified comment `a1e1fad7-8f42-48af-8731-7fbb4170ec3d`
keeps this in the accepted one-connect-owner conversion. Route via retained Session
identity/common connect or deliberately remove the obsolete surface; no new guard.
Replay remains a separate captured-input operation, with its own preservation.

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

## Desktop controls integration proof (working, 2026-09-29)

Main applied the released proposal, including human-readable confirmation with
stable transport IDs. Supervisor inspected the working CLI, binding model,
workspace routing and mounted proof. `desktop-controls-rust.log` passes5 public
CLI/import/wire checks in11.871s; model/navigation run reports67 Swift passes.
`desktop-controls-mounted.log` passes the extended real-PTY test in5.531s: hiding
headless work and binding to a Task in a different checkout retain the same pane
objects, focus and typed draft. Preview/commit calls use fixture transport and
partly call the model directly; this is not installed-app/real-Task acceptance or
an end-to-end click proof for every popover control. Final metadata conversion,
Desktop paging and recorded native account recovery remain distinct requirements.

## OpenCode recovery checks reviewed (2026-09-29)

Actual candidate05473216 `adapter-output-green/adapter-result.json` records exit0, completed Flow and
both public final answers, each “Fixture completed.” (6.394s). Actual OpenCode
1.18.33 used a scripted local model and private Home; configured-account and
full recovery acceptance remain unproven. The earlier mismatched-output evidence
is preserved separately, not overwritten by this pass.

Supervisor read the new source/assertions and logs. `opencode-flow-retry-proof.log`
passes four focused checks: native output/permission mapping, history reducer,
public taskless decision and Task Flow failure/retry/review. The public fixture
uses a simulated OpenCode HTTP server; it asserts two starts/completions/consumed
boundaries, one allowed decision and rejection of the ordinary work decision,
plus final answers. `opencode-disconnect-proof.log` passes the tool-effect then
transport-loss case: one start, no native completion/consumption, one launch and
failed command. It does not establish real provider process termination.

`opencode-history-proof-2.log` passes the SQL/history reducer check: after input
and driver replacement, old usage stays with its original input/Exec; duplicate
completion counts once; decreasing tokens retain the prior maximum plus a gap.
This is seeded-store/reducer evidence, not live driver reconnection. Clippy's
first pass failed a single-match style lint; `opencode-adapter-clippy-final.log`
then passes all targets in16.54s. `opencode-automatic-retry-proof-3.log` passes
the public simulated-server retry case in7.38s: three starts/completions across
two Flow Sessions, two consumed successes, same admitted input for retry, earlier
caller rejected, 40/10 retained usage and final answer. The real OpenCode failure
probe instead recovered within the provider's own request (two starts total),
so it does not prove Loopflow automatic retry. Supervisor verified delivery
direction comment `1692d731-673e-4faa-9f6c-d813cee0fc10`: after the bounded
native probe and final focused/static checks, publish the coherent repair before
widening the next cut. Main still matches GitHub main6e718; no rebase needed
at that observation. The active metadata proposal imposes no publication hold.
No full-green CI, valid Codex retry, installed Home or final concept review follows.

## Historical NULL-title fixture classification

The retained NULL-title/current-review fixture still proves its SQL rejection;
it is not an observed installed-Home defect. Source review through v0.12.24 found
no supported writer producing that combination. 0.12.15 drops old positions and
0.12.16 recreates an empty table; subsequent first-position writers require a
Task with a String title. Ready only updates an existing position. Across thirteen
snapshots, the title-omitting `create_task_work` helper is called only after the
complete `TASK_INSERT` in the same transaction; it is not an independent writer.
Task plan writes preserve the String title. Private
`.lf/tmp/historical-title-review/{title-writer-audit.json,classification.md}`
retains candidates, transaction snippets, source hashes, interpretation and limits.

Classify as an incomplete synthetic seed with released reachability unproven.
Keep the original failure and the corrected populated fixture distinct; no runtime
title fallback or applied-draft rewrite is justified by this evidence. A concrete
imported counterexample reopens it. No private Home was inspected, no product test
ran, and broad import/conflict/missingness preservation remains required.

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

Published6c0981dbd versus base6e718: **+17,680 / −30,618 = net−12,938**
production. Rust/Swift+16,474/−30,556; Python/shell+48/−62; SQL+1,158/−0.
Receipt `.lf/tmp/execution-model/status-counts-6c0981dbd.json` reproduces the
prior915 measurement before computing this head. Corrected production prefixes,
no rename detection, tests/docs excluded; dirty Desktop/proposal changes excluded.
Same-method OpenCode cut from915: +686/−420 = net+266. Main's cut report measures
+684/−418 (same net); preserve method details rather than merging gross counts.
Earlier publisheda866 net−13,705 and local915 net−13,204 receipts remain retained.

Older scratch:62files/676,767bytes in `.lf/tmp/scratch-consolidation-20260928/scratch/`
with SHA manifest; eleven LOO291 originals under `.lf/tmp/context-archive-7e2101b41/`.
Commit7e2101b41 preserves published originals. Useful evidence needs a durable
owner before delivery clears scratch. Historical models never override decisions.

This index's exact pre-curation bytes are archived at `.lf/tmp/scratch-curation-20260929-control/parallel-work-e6a3e35467bc.md`
(SHA-256 `e6a3e35467bcdd66b14eb2258451b640851b0a231cd5b2b72b066454ce35ddaa`). Main handoff, design, full scope and evidence are
unchanged. Older index is also committed at
`4ced9467fde48b88b232c346af054ff9e3ad8f58:scratch/parallel-work.md`.
