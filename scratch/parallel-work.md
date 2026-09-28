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

Current published head **705645cd1ad36808460e8f0e9e4767d1d8cc7191**, base
**a2b59ed50666a433c97cb9002476e74351723759**, [PR #1296](https://github.com/loopflowstudio/loopflow/pull/1296).
Supervisor independently verified remote branch, GitHub head, Task publication
and presentation heads. Auto-merge is absent. CI **36491822378** finished with failure;
its Rust job failed after 669 passes, two failures, 13 skipped and 1,258 unrun.
The two tests are `ad_hoc_batch_launch_uses_generic_run_record_without_planning_registry`
and `replay_uses_recorded_request_without_the_planning_store`: both deliberately
select an unwritable database while expecting agent capture to succeed. Source
inspection confirms the obsolete expectation conflicts with Jack's writable-store
decision. Supervisor relayed exact failures and preservation constraints in
comment `025bc27b-b194-40f2-9c73-03fef2b27a7a`: writable private empty planning
stores for success cases; retain replay prompt/parent/Home and stale-authority
assertions plus refusal before provider launch on store failure. Log:
`ancestry-checkpoint-rust-ci.log`. Migration, architecture, Python, website, lint,
smoke, installation, Swift and UI compile jobs pass; scratch-clear fails on
retained working notes. No full passing CI result follows.
Post-rebase Clippy passes in 15.06s
(`ancestry-rebase-clippy.log`), and the corrected import/report plus reconciled
Feature checks pass as recorded below. The saved invocation remains implement
2/13, iteration 9, same invocation and worker Run. Main has resumed headless
Flow AgentSession ownership; publication does not finish the cutover.

The new Flow conversation test first fails before provider launch because no
Session exists (`flow-conversation-red.log`), then passes its SQL reservation/retry
checks (`flow-conversation-admission.log`, 26.48s compile, 1.306s test). It retains
Session identity/title and the failed attempt, rejects stale publication, and
creates no AgentSession for a mechanical operation. This is not native retry proof.
Supervisor found its next reachable dependency: TaskLauncher explicitly supplies
no native resume identity, capture claims a replacement provider generation, and
Codex startup creates a new app-server/thread. A retained Session row alone can
therefore mask native conversation replacement while a detached engine survives.
Verified comment `7d2ae7eb-f3cc-4e8e-9d90-36504127987c` requires the accepted
Task/taskless native-history, account, live-engine exclusion and exact Flow
completion proof through this cut. This is source evidence, not a live failure;
main still owns the implementation and public proof.

The actual CLI test next failed because Session inventory gave a headless Flow
conversation Unknown membership (`flow-conversation-cli.log`). Main changed the
surface to read its stored Flow and the Run's recorded node/iterations, including
past occurrences. `flow-conversation-and-ci.log` passes **4/4** (29.62s compile,
11.780s tests): failure/retry/review, both hosted fixture repairs, and store
refusal before provider launch. Its compile warned about the now-unused
`SessionFlowMembership::of_position`; this result is not a clean Clippy pass.

The in-progress native patch reuses an endpoint/thread and propagates the resume
token. Supervisor source review identifies remaining distinctions: a stored
endpoint is not live-engine evidence; the thread must survive endpoint loss;
resuming an in-progress turn must not unconditionally send new initial input;
and equality of thread IDs does not validate the retained account/native Home.
Verified comment `bd17d589-71a4-4bd4-92fa-bb0e0ff693ba` supplies these concrete
cases to the same worker. These are source observations of unfinished code, not
executed provider failures or reasons to add another product object. Native
retry after engine loss, driver-loss recovery and exact history consumption remain
unproven.

A subsequent real Codex / synthetic Responses proof passes the **taskless Flow
failure → retry** path: `native-flow-retry-1/results.json`, CLI SHA-256
`a4fcad5a4d948fd9fdc5437b6afd0d3eb6f794eb2161fbf7e73149ef93dca8c9`.
The first command exits 1 after the controlled provider error; retry exits 0.
The same Session, native thread, endpoint, provider generation and provider-owner
Exec survive; one engine is created. Prior history is byte-identical, with
separate failed and completed turns. The agent-issued child names the new driver
and retained provider generation. Supervisor inspected the assertions and receipt.
This is not a configured-account or managed-Task proof; the Flow still settles
through its Run attempt. It does not close the three source gaps above or prove
atomic consumption of the selected native completion.

The supervisor then reproduced **engine-loss retry failure** against those same
CLI bytes, in `supervisor-flow-engine-loss-2/results.json`. After a controlled
provider failure, the copied fixture gracefully stops its exact PID/start/group
and confirms termination. Public `lf flow resume --retry` exits 1 with
`No such file or directory (os error 2)` instead of reopening the saved thread.
This turns the stale-endpoint hypothesis into a real CLI counterexample over
real Codex and synthetic Responses. Verified comment
`e91eedae-9a46-41df-9f85-e9d2338c857d` gives main the probe and required repair.
The first probe invocation failed before launch because the repo venv lacked
websockets; the second used the script's declared uv dependencies and reached
this product failure. Both logs remain. Only the disposable fixture engine was
stopped; no installed Home or configured provider account was touched.
The exact failing probe source is retained beside that receipt as `probe.py`
with its SHA-256. The working probe now has engine-replacement success assertions:
stable Session/thread and prior history, one provider-generation increment, new
provider-owner Exec, new child provenance, exactly one replacement engine. Its
original live-engine-only postconditions required no replacement and were
unreachable at the observed failure. Revising those postconditions does not
change the retained failure or claim a passing recovery result.

Main then rebuilt and ran that corrected engine-loss probe successfully:
`native-flow-engine-loss-1/results.json`, CLI SHA-256
`2b07fbcb7a30905c7e0b932ec92f60731ffd8625419841261ba74cd8e9835cf3`.
Build `flow-engine-build.log` passes in 19.34s. The fixture confirms the old
engine stopped; retry exits 0, Session/native thread stay identical, provider
generation advances 1→2, the provider-owner Exec changes, and the child command
names generation 2/new owner. Prior history stays byte-identical and the two
turns remain failed/completed. Exactly one replacement engine is admitted.
Supervisor inspected the result and the probe's full assertions; no redundant
rerun was needed. This closes the reproduced dead-engine case for taskless Flow
and private native Home only. Managed-account selection, surviving-turn driver
loss and exact Flow history consumption are still open.
The final native pair, `native-flow-live-final` and `native-flow-loss-final`,
both passes on CLI `4ac6ec1310b509c3ba97884902a5a1f5dc7982da6c08ea540b216a68e98835f3`:
live retry retains generation 1, dead-engine retry advances 1→2. All-target
Clippy passes before the subsequent graph repair (`flow-native-clippy.log`,
14.91s). These are matching private-fixture cases, not a full gate.

Independent `supervisor-flow-recovered-origin-1` passes on the same `4ac6ec13`
CLI. The fixture deliberately deletes only a failed native turn's SQL completion,
retains its start, stops the exact engine, and retries publicly under generation
2. Public Session history restores the failure under generation 1/original Exec
and shows success under generation 2/new Exec; repeated reads are identical.
Session/thread, prior start and two distinct outcomes survive. The receipt retains
the exact probe source/digest. Comment `18375b37-9bf0-4e8b-97da-0d23c73bef7e`
relays this result. Missing SQL completion was simulated; actual write failure,
missed-usage recovery, managed accounts and Flow's exact consumption are not proven.

The next ownership test batch did not execute: `conversation-engine-ownership.log`
failed compilation because a test still called the deleted review-only
`SessionFlowMembership::of_position`. Main is repairing that fixture against the
current reader. This does not invalidate the separately built CLI probe, but no
passing ownership-suite result follows from it.

After deleting that obsolete call, `conversation-engine-ownership-2.log` passes
**4/4** (26.12s compile, 1.373s tests): retained nested capture, recovered receipt
missingness/conflicts, Flow conversation reservation/retry, and copied-Home
history without live driver/endpoint/process authority. Those scopes differ:
the nested test now checks capture only, not the public Session projection.

Supervisor found a resulting DTO mismatch: `surface` emits the stored preorder
numeric node as text, while Desktop's `WorkspaceBreadcrumbBar.flowTarget` still
looks it up in a graph keyed by structural strings. The nested fixture's key
`1/fix/1` and stored node 3 differ; nodes after an XOR diverge too. Comment
`f103c020-37f9-49c1-8c39-08c4c984bc00` requests coordinated graph/membership/Swift
conversion and a stored-Session-to-graph regression, preserving the accepted
numeric-node model. This is source evidence, not a rendered Desktop observation.
The green capture-only test does not establish that consumer boundary.

Main's replacement behavioral regression reaches the reported mismatch:
`stored-session-graph-red.log` compiles in 26.80s, then fails its stored-Session
projection at `Some("3")` versus `Some("1/fix/1")` (1.304s test). The fixture
reserves real SQL Session/Run rows at nested, post-XOR and initial nodes, moves
the cursor, and checks retained earlier/current occurrences, labels and iteration
tuples. This is an executed Rust projection failure, still not a rendered UI
result. Main is preserving the current graph wire contract until the coordinated
numeric DTO conversion; that conversion remains required for the final model.

The repaired stored-Session test passes in `stored-session-graph-green.log`
(26.45s compile, 1.341s test). The existing FlowGraph now resolves its stored
preorder node into the existing wire key without consulting the current cursor;
nested, post-XOR and earlier/current projections agree. This preserves the
current Desktop contract; numeric graph/DTO migration is still unfinished.

Final checkpoint static analysis passes in `flow-native-final-clippy-2.log`
(14.92s); its preceding attempt flagged the new test's environment MutexGuard
held across await points and is retained as a failed check. `canonical-engine.log` materializes all 22 drafts as
0.12.25.001_release in a disposable source copy and passes **4/4** (26.79s compile,
2.004s tests): receipt recovery/conflicts, copied-Home authority removal, stored
Session-to-graph projection, and canonical initialization/upgrade preservation.
These do not establish installed-Home migration or the unfinished ownership paths.

A separate **driver-loss / surviving-engine** probe also fails against the same
CLI bytes: `supervisor-flow-driver-loss-1/results.json`. The native turn reaches
a held synthetic response, the fixture kills its own lf process group (exit -9),
and PID/start evidence confirms the separately grouped Codex engine survives.
Public `flow resume --retry` refuses before attachment: `Flow ... is waiting for
Run ...; its completion is not recorded`. The Flow remains current without a
failure record. This supersedes the predicted busy-send as the earliest observed
blocker on that path; the busy-send source concern remains later in recovery.
Verified comment `7e34bc04-bb84-4104-a57e-d65e8985186a` ties the counterexample
to the planned exact Session-history consumption and Run removal. Do not infer
Run success or interruption from process disappearance. This probe does not prove
native turn completion after driver death; it establishes surviving engine and
public recovery refusal. Fixture children were cleaned up under exact identity.

Earlier checkpoint **2958d289c0e4087118e6920390fa6c8975c216d8**, base
**c512813b5b333f2ae012503b1781fcbd458be67a**, [PR #1296](https://github.com/loopflowstudio/loopflow/pull/1296).
Remote branch, GitHub and Task PR row independently agree. Manual rebase was
conflict-free, zero commits behind the freshly observed main. Auto-merge is off.
Jack's publication direction was relayed in comment
`de0d4762-6acb-4535-af07-45c9ac20d8c0`; it makes this implementation checkpoint
available to dependent Tasks, without claiming completion.

CI **36487487807** belongs to that earlier checkpoint. Python, website, installation,
smoke, migration, architecture, Rust lint, Swift and UI compile checks passed.
Rust stopped at two review-completion fixtures that cannot resolve `lf` on the
clean CI PATH (`controller/task/mod.rs:952`), after 11 passes, with 1,912 tests
unrun and 13 skipped. The previous canonical initialization failures now pass.
Exact failures: `completing_a_final_review_finishes_the_flow` and
`concurrent_review_completions_settle_once`; log
`.lf/tmp/cut-i/repair-checkpoint-rust-ci.log`. Relayed to main in Task comment
`e24c013b-85d9-4fb5-8194-962d141e4116`; fixture executable selection must be
isolated without weakening production launch behavior or outcome assertions.
Local checkpoint **d8665a753799f69178a41e0bdc90ba10405432e0** now names the
owners in place and repairs those fixtures; a conflict-free manual rebase leaves
zero commits behind then-observed main `c512813b5`. It is not yet the published head.
Fresh remote inspection later found main at `a2b59ed50666a433c97cb9002476e74351723759`,
while the published branch remains `2958d289c`. Supervisor relayed the new drift
and Jack's standing rebase/publication direction in verified Task comment
`5324d830-0205-4fa8-a03f-412283525cf4`: checkpoint the coherent tested ancestry
work, inspect and apply `lf rebase`, then publish the existing PR. Main remains
the sole Git owner; this is neither a completed rebase nor a publication claim.
The GitHub comparison confirms one upstream commit, PR #1319: retired memory
skills consolidate into realign, and queue adds rebase/realign before gate.
Its overlap includes `docs/lf.md`, Task controller expectations, catalog tests,
Flow graph indexes and prompt goldens. Reconcile new catalog behavior while
retaining the current Task's captured invocation; no captured graph refresh is
authorized by this upstream change.
The ancestry checkpoint was `91341c16b` before rebase. Main resolved the
`docs/lf.md` conflict and completed integration: local head
`7f357827a4872ec3632d0e657b17b50563888514`, merge base `a2b59ed50`, zero behind
and 82 ahead. Supervisor inspected the post-rebase change set and retained
realign/queue documentation. Publication has not yet been observed. All-target
Clippy passes after boxing the enlarged lookup enum value
(`ancestry-clippy-2.log`, 14.71s); the preceding large-enum failure remains recorded.
All four affected review tests pass (`review-executable-pins.log`, 1.572s),
including two additional unguarded callers found by the supervisor. Initial public
rename checks passed Flow retry/review and inventory scoping but exposed two old
table assertions and the shared stdio-only Codex stand-in. The repaired shared
fixture accepts Unix WebSocket transport; its first replay retained a Nextest
LEAK. After exact fixture teardown repair, both affected Flow tests pass without
a leak (`provider-fixture-teardown.log`, 16.791s). All-target Clippy passes in
16.59s (`owner-names-clippy.log`). Production engine survival is unchanged.
These local results do not change the published head's red Rust result or
establish completed owner conversion. Main is moving current Task/Wave attribution
onto AgentSession while preserving historical work/usage and set-once Started.
The current ancestry review identified child admission as part of that same cut:
Ask inheritance reads the caller Run, and `create_run` has a separate caller-Run
inheritance path. After prospective bind, new child work must use the admitted
current AgentSession assignment while old outcomes/usage remain unchanged.
This is source-derived proof scope, not a newly executed failure. Test read-only
Exec admission against a separate untouched Task; an already-bound Task cannot
prove that observation leaves Started null. Relayed in comment `df80df1a` with
the method-name/fixture clarification retained in the following comment.
The in-progress test `binding_is_prospective_and_only_future_work_starts_the_task`
introduced a contract mismatch: it expects no Started timestamp after binding.
`docs/architecture-reference.md` explicitly includes first bind, preserving Jack's
first-assignment decision in `questions.md` (comment `2ecb585f`). Supervisor
directed main to retain atomic first-bind Started while preserving old usage,
and replace the obsolete Runs-only evidence validator without fabricating a Run.
The working design now states that distinction explicitly; this clarifies the
existing contract, not a new product choice. Main acknowledged the conflation
and changed the draft to include AgentSession ownership in Started evidence,
with set-once insertion/bind triggers. The renamed test now requires first-bind
Started without historical reattribution. The corrected focused run now passes
all three checks in `conversation-ancestry.log` (32.06s compilation, 16.340s
execution): first-bind timestamp and retry preservation, unchanged prior work
and usage, and post-bind child/Ask inheritance. Supervisor inspected the public
CLI assertions: a done Task retains its terminal state, observation of a separate
untouched Task leaves Started null, and replaced-provider provenance is rejected
before another provider launch. Provider provenance and Ask launch are synthetic;
this is not configured-provider acceptance or Run removal. The earlier green
`prospective-bind-2.log` includes the withdrawn future-work-only expectation and
does not establish the corrected contract. Direction: comment
`e9112de8-ec07-421a-a83b-e7a2e0043f7a`.
The broader ancestry batch is not green. `ancestry-migration-and-sessions.log`
records 10 passes, six failures and 12 unrun: the populated seed supplies a
Task Flow's own cwd (the schema requires NULL and resolves the Task worktree),
and five Ask fixtures lose caller context during branch isolation. Main moved
those fixture callers into the private Home's Run tree and scrubbed ambient
authority. The next batch still fails the Flow seed before upgrade (three passes,
one failure, 22 unrun); after correcting cwd, `ancestry-upgrade.log` reaches its
next invalid seed value, `position_version=0`. After correcting that value,
`ancestry-upgrade-and-remaining.log` completes all 23 selected checks: 21 pass,
two fail, 44.036s execution after 30.71s compilation. The populated upgrade now
passes with prior Run attribution, existing Started/Done timestamps and unknown
usage attribution retained. All six Session CLI checks pass. Remaining failures
are the older headless assertion that requires no Session, and an import-report
path comparison of `/var` against canonical `/private/var`. Source inspection
confirms these exact mismatches; their repairs and final replay remain pending.
`ancestry-readers-repair.log` then passes the headless launch/reader check and
reaches an additional stale import assertion: the expected kind is `interactive`
but the DTO now says `conversation` with a separate interactive flag. The full
import test's corrected replay now passes in `ancestry-import-repair.log`
(8.343s; only canonical path and expected kind changed in that repair).
Its existing historical headless
expectations retain Run rows without Sessions and therefore cannot prove the
final AgentSession import contract even after this fixture passes.
Canonical-materialized replay now passes four checks in `canonical-ancestry.log`
(27.24s compile, 2.053s execution, 1,539 library tests unselected). The disposable
source packages all 21 drafts as `0.12.25.001_release`; owner/capture/native-history
preservation, ancestry/Started upgrade, prospective bind history and empty-draft
initialization pass. Source receipt: `canonical-ancestry-source.json`. This is
neither published release bytes nor an installed-Home conversion.
Supervisor found a remaining ancestry reader in `Import::start`: it reports newly
started Tasks by enumerating Runs. A Task already started by binding can have
no historically attributed Run, so a later import can misreport it as newly
started. This is source evidence, not an executed regression or a claimed
timestamp rewrite. Verified comment `6f2b644e-d86f-4ca0-a05e-f6aa9bfb78f1`
directs main to use the existing Started-column reader and prove bind-before-import
preserves the timestamp and omits the Task from `tasks_started`, retaining dry-run
and untouched-Task behavior. `import-started-red.log` now reproduces the error
through the private real CLI: dry-run reports the already-bound Task as newly
started (6.102s; 20.36s compile). Main replaced the Run inventory with
`Store::task_started`; all three focused repair/rebase checks pass in
`import-started-and-rebase.log` (29.65s compile, 8.918s execution): the new
regression, existing import preservation and reconciled Feature navigation.
The regression verifies both dry-run and import, unchanged original
Run ancestry and the retained bind timestamp; its history input is synthetic.
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
Main removed 13.2 GiB of this checkout's rebuildable Loopflow package artifacts.
Jack's subsequent cleanup question prompted a fresh inventory: 71.27 GiB free,
51.97 GiB of active build roots, zero inactive build output and 12.95 GiB uv cache.
The current `lf wt prune --dry-run` names only clean dogfood, 74 MiB. No worktree
was removed. Supported `uv cache prune`, run outside an enclosing uv invocation,
still timed out at 15 seconds; no forced eviction or cache-prune success follows.
These observations supersede the earlier three-candidate list and 75.48 GiB sample.

## Retained execution evidence

An actual copied-CLI admission diagnostic uses a disposable Home and no provider:
`command-observation-cpxkch7g/receipt.json` under `.lf/tmp/execution-model/`,
CLI SHA `c1c213a271fb717fe5004e0e2374f5b030e94a4e50ff92bb5aee24fa5e9fe9b3`.
Two `session list` commands each create one succeeded Exec/exit 0; rename of a
missing Session creates one failed Exec/exit 1. Help/version exit 0 and rejected
arguments exit 2 without an Exec. This confirms parsed-command recording and the
early-parse admission gap. Help/version/error take 9–11ms; repeated empty listing
takes 752ms in this single diagnostic sample, not a dense benchmark. No installation,
screenshot, provider or real Home ran. Reproduction: `supervisor-command-observation.py`.
The remaining every-process contract must reconcile fast parser paths and bootstrap
authority; this probe neither resolves that implementation nor establishes acceptance.

The provider probes below use actual Codex 0.157.1, synthetic local Responses, controlled
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

Latest fixed production-prefix measurement, published checkpoint `705645cd1` versus
`a2b59ed50`: **+13,576 / -29,448 = -15,872** across Rust/Swift,
Python/shell and SQL. Tests/docs excluded, no rename detection, trailing test
modules excluded with the known trailing production block retained. Repeating
the method reproduced the prior `7f357827a` receipt exactly before measuring the
new checkpoint. Receipt: `.lf/tmp/execution-model/status-counts-705645cd1.json`.
This includes the committed ancestry work and import-report repair; ongoing
headless Flow conversion is excluded. The earlier local checkpoint was
**+13,240 / -29,446 = -16,206** against `c512813b5`.
Changing merge bases changes the comparison; moved files alone never count as
removed code. Earlier published count remains in `status-counts-e13f29909.json`.

The detailed previous supervision ledger, exact intermediate failures, old
allocations and receipt paths are preserved in the
[published repair checkpoint](https://github.com/loopflowstudio/loopflow/blob/2958d289c0e4087118e6920390fa6c8975c216d8/scratch/parallel-work.md).
This curation removes repeated launch context, not evidence or acceptance
obligations. Earlier local-only publication instructions in that history were
superseded by Jack's checkpoint publication request. The working design remains
the finish line; no requirement is waived by this ledger.
