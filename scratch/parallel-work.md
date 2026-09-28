# LOO-298 parallel work

## Publication priority and busy-turn repair — 2026-09-28

Fixed rebased checkpoint measurement, excluding current uncommitted repair:
`ce97003cf` against `c512813b5` reports Rust/Swift +12,423 / −29,314;
Python/shell +19 / −48; SQL +626 / −0. Total **+13,068 / −29,362 =
−16,294** production-prefix lines. Receipt:
`.lf/tmp/execution-model/branch-counts-ce97003cf.json`. Same corrected method,
tracked committed files only, no rename detection, tests/docs excluded. The
base changed during rebase, so the difference from earlier counts is not a
measurement of the integration repair or newly authored growth.

Jack requested the rebased branch be pushed because dependent Tasks need its
changes. The next coherent verified checkpoint is authorized for publication;
do not wait for the remaining physical owner conversion. Landing, installation
and real-Home conversion remain outside this checkpoint. Main retains Git/build
ownership. Observed local head `ce97003cf` includes remote main `c512813b5`;
remote branch still named `d07e56933` at this observation.

The strict public busy-turn regression now passes on copied candidate SHA-256
`f2b9f5ab7113cf4c5b1a08017927c791ad79d7da51f09c0e584eda10187600b4`.
Receipt: `.lf/tmp/execution-model/supervisor-busy-start-regression-2/results.json`.
After driver handoff, the current client's additional input receives the same
active native turn, retains its original Exec and provider generation, and
records one successful completion with usage present. The earlier RED receipt
remains `supervisor-busy-start-regression-1`. This uses real Codex with synthetic
Responses, a controlled protocol client and disposable Homes; it proves neither
rendered UI nor Flow consumption. Handle 39003 completed with exit 0. No source
edit or build accompanied this replay.

The post-rebase focused suite completed 20 passes and one failure in
`status_preserves_stranded_tasks_without_a_project_layer`: its assertion that
Projects are absent contradicts the H7 Project history projection. Main owns
reconciling that assertion while retaining the stranded-Task proof. This is a
failing suite, not publication readiness.

The targeted correction subsequently passed all ten checks in
`.lf/tmp/cut-i/rebase-dto-busy.log` (compile 31.60s, tests 1.423s): nine DTO
checks plus the busy-turn known/unknown-origin regression. The renamed status
test retains stranded Tasks alongside Project history. This repairs the named
failure; it does not establish a full gate or publication.

## Fixed-checkpoint measurement — 2026-09-28

At `d07e569330c8dedceb5dd238ce9b22a7b6137006`, versus retained merge base
`5bdcef6b65419d5db2583ee791cede2f1a95b3df`, the comparable production-prefix
method reports Rust/Swift **+11,288 / −29,763 = −18,475**. Python/shell is
**+19 / −48 = −29**; SQL is **+595 / −0**. Compared with `03330279f`, this
checkpoint adds a net 302 production Rust/Swift lines and 79 SQL lines.
Receipt: `.lf/tmp/execution-model/branch-counts-d07e56933.json`.
The method uses no rename detection and excludes test paths and trailing Rust
test modules while retaining the known trailing production block. It does not
count moved files as net deletions, excludes current dirty edits, and is neither
the final delta nor evidence that the Run owner has been removed.

## Current independent reviews — 2026-09-28

New Session-history source review retains a provider-replacement counterexample:
`codex_history::record` labels every `thread/tokenUsage/updated` with its
observing driver's provider generation. Resume after replacement can replay the
last usage for an older turn; if that receipt was missed earlier, first insertion
would label old work with the new provider generation. Completion snapshots
already retain unknown generation. Main owns preserving the recorded turn
origin when available and missingness otherwise, with a gen1-start/missed-usage/
gen2-resume regression. This is source evidence, not an executed restart failure;
the earlier same-engine generation-1 probes do not distinguish it.

Supervisor extended the public connect fixture in a private script to exercise
provider completion after both `lf` clients exit. Candidate SHA-256
`e883464af7f572506acf79388ee5332144bd0cd6e27dfa0c7942207878a8604c`
passes: a third held turn starts, both CLI clients terminate with 130, the real
Codex engine completes with no Loopflow receiver, and a later public connect
records one completion. The original starting Exec remains interrupted/130;
earlier Session history and replay stay unchanged. A passive native observer
remained connected to inspect the engine; this is not a zero-native-client test.
Receipt: `.lf/tmp/execution-model/supervisor-driverless-history-2/results.json`;
reproduction: `.lf/tmp/execution-model/supervisor-driverless-history.py`.
The first probe retained only a count of recovered usage events. That observed
one event, so the second explicitly verifies its payload: lifetime totals are
120 input/30 output after prior 80/20, with the third turn's original starting
Exec retained. Although Thread/Turn snapshots have no usage field, native
resume supplies a separate usage notification. Do not generalize the schema
observation into inability to recover this measured usage. Aggregate `lf usage`,
Flow consumption, missed multiple turns, restart and rendered UI remain unproven.

The native approval proof also passes the opposite connection order. The private
variant `.lf/tmp/approval-no-client-probe/probe.py` closes A, waits, then creates
B. Raw timestamps show 302ms without a selected-thread client; B resumes the
same pending request, answers once and completes one marker write. The sibling
remains active and later completes; owned engine cleanup exits 0. Receipt:
`.lf/tmp/approval-no-client-probe/candidate-1/results.json`. This extends the
original report's ordering limit without rewriting its observation. The sibling
client remains connected, so it does not prove that every engine connection may
disappear. Both supervisor probes use synthetic localhost responses, explicit
private Homes and no source edit/build. All probe processes are terminal.

Main's `conversation-history-focused.log` records two passing Rust checks
(1.308s; compile29.95s): Session-history DTO preservation and recovered completion
with missing start/usage plus conflicting evidence rejection. Supervisor also
inspected `public-connect-history-1/results.json`: two native completions,
nonzero usage, original turn starters, exact selected child ancestry and replay
deduplication pass through public commands. These are scoped receipts, not the
physical AgentSession/FlowSession conversion or removal of Run.

Control-path selection changed during supervision at 13:09 PDT. Machine
`active.json` now selects published 0.12.24, CLI `lf-d7bf7c66843517e437c870e2dea0beb52717f4633f73441eedba01ff5e7bc401`,
store `/Users/jack/.lf/loopflow.db`. Bare `lf task status LOO-298` reports no
Task, and bare Run reads report the wrong-Home absence. This does not establish
worker death. The live worker's retained manifest names its captured installed
runtime `/Users/jack/.lf/bin/lf-f5ef8d640340e9f8b9e36d17d84de83e14e905305c43e959fd00c49a322a527f`;
its Home is `/Users/jack/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea`.
That runtime with explicit ordinary and control Home/DB overrides reads the same
running Task, invocation and worker Run. Supervisor now uses this supported
pinned path for control. No authority bypass, restart, installation or store
conversion occurred. The actor causing the installation selection is unknown.

Approval contributor `run_e87c4b72f77e486e930ae264fbba2698` is terminal and its
final report was read in full. Preserve its precise ordering: the replay arrived
during B's resume, before A disconnected. An interval with no connected client
is not proven by this case. All bounded contributors are terminal again; main
continues executable ownership and the build slot.

Native pending-approval replay passes. Supervisor inspected
`.lf/tmp/approval-replay-probe/candidate-1/results.json`, both client transcripts,
the exact once-only marker, and the probe's assertion/cleanup path. Actual Codex
0.157.1 (SHA-256 `27ceb5f9b957b43a519efe4eaa3816a0bffb0a531a2c89af18840c0a3c016a7d`)
replays `item/commandExecution/requestApproval` on the second client's
`thread/resume`, retaining request ID 0 and the same thread/turn/item. Client A
never answers and disconnects; B answers once afterward. The selected turn
completes, the private file contains one `approved-once` line, and a held sibling
remains active before completing when separately released. The 1.086-second
experiment uses a synthetic localhost upstream and private Codex Home. Exact
owned engine PID/start/group cleanup exits 0. This resolves native replay
feasibility; public Loopflow stale-approval fencing, UI and completion/usage
ingestion still need their own proof. The report/reproduction belong to
`parallel-approval-probe.md` and `.lf/tmp/approval-replay-probe/probe.py`.

Failed ordinary Wave resolution is repaired on candidate SHA-256
`e224799dd21357a78af82d707bd3ea926d1d51fb6bdd1331fbd924e740b73931`.
Supervisor repeated the same two commands in a fresh private Home: inventory
records one succeeded Exec; the unchanged unknown-Wave error exits 1 and records
its own failed Exec with exit code 1. The copied binary stayed unchanged.
Receipt: `/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo298-entry-observation-repaired-1wsanv2y/receipt.json`.
The prior failing receipt remains below. `entry-and-native-safety.log` additionally
records four assertion passes in 4.827 seconds, with Nextest marking
`exact_process_evidence_distinguishes_a_live_exec_from_its_completion` LEAK.
That qualifier is unresolved; four assertion passes do not establish clean
process settlement. No additional build or behavioral rerun was requested.

Handoff-history contributor `run_28487ba852f6426cba061d64e7ac6437` finished.
Supervisor read `scratch/reviews/parallel-handoff-history-review.md` in full and passed its two
findings to main: native turns after reconnect have no completion/usage recorder
after the old receiver exits, and pending native approval replay is unproven.
The report separates the source counterexample from uncertain provider behavior,
names exact consumers and proposes one successful completion reference instead
of another Run/attempt product. No code or runtime proof occurred in that review.
A bounded native-protocol approval probe, `run_e87c4b72f77e486e930ae264fbba2698`, owns only
`parallel-approval-probe.md` and `.lf/tmp/approval-replay-probe/`, using a real
Codex engine with synthetic localhost responses and disposable Homes. It may
run no build or main-Home operation. Main retains all executable files.

`public-connect-2/results.json` passes at candidate SHA-256
`2bc50fcea68f2c8bda13c6d1089a751d9129c69a2861efd0267bf7f262119c69`.
Supervisor inspected the receipt and fixture: two actual public `lf session
connect` processes relay to the real Codex engine through controlled protocol
clients. The old client's interrupt is rejected, the selected turn and a sibling
remain active, and provider generation stays 1. This is public dispatch proof,
not rendered native UI, restart, old headless-driver teardown, or history/usage
preservation. Provenance currently checks the latest agent-issued Exec after
releasing both turns; both use the same synthetic tool command. Main must
distinguish the sibling command or correlate the selected turn's exact child
before claiming that particular post-handoff child. The narrower passing result
stands; no new product defect is established by this fixture ambiguity.

Actual ordinary-command failure remains unrecorded when explicit Wave resolution
fails before `with_runtime`. Supervisor copied candidate SHA-256
`f6f65c692cb3cdbee4db2fe3c350441cac13f19e91dda2b68efb2a9466e02129`
into a disposable directory, cleared inherited LF authority, and selected a
private Home/database. `session list --all --json` returned `[]`/0 and created
one succeeded Exec. `--wave fixture-wave-does-not-exist session list --all --json`
then returned the correct unknown-Wave error/1, but no second Exec existed.
The copied binary remained unchanged. Receipt:
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo298-entry-observation-kudhtuj3/receipt.json`.
Source matches: `bin/lf.rs` resolves explicit Wave and account selection before
entering the outer process lifecycle. Main owns moving ordinary fallible setup
inside the existing lifecycle. Installation/preflight and screenshot dispatch
have explicit independence requirements; do not make them require an ordinary
store merely to satisfy the logging rule. Their observation disposition must be
stated explicitly before claiming every command is covered.

Supervisor confirmed `native-socket-admission-2.log`: four focused tests pass
in 6.379 seconds, covering helper admission, exact authored Project content on
sync, conflicting statuses at the final Chapter inventory, and both directory
and append obstructions of the file journal. The compile reported an unused
`TempDir::keep` result; current source handles that result, but this receipt is
not a fresh Clippy pass. The earlier failed fixture receipts below remain valid
history and are superseded only for these exact repaired assertions.

`public-socket-parent/results.json` passes against candidate SHA-256
`4a869f0ab8bb6cabc5dc7e4da3ef3f29330a4e3e3d4c581de53bc1150fe7d444`:
normal socket launch, headless discovery, unchanged conversation on rename,
and nested `lf` ancestry under real Codex with synthetic local Responses.
Session `session_deacf009f1424cf5a50a254dfca135b8` retains provider generation 1;
child `3b4333d3-8aa7-46ee-ae09-4fde0a29277b` names its actual parent Exec.
This remains fresh-launch proof, not public connect/restart or Desktop evidence.

The previous stop/transfer race is being removed by detaching an established
managed engine at driver teardown. The source now puts startup termination under
the driver fence and omits the managed-engine signal hook. Actual race coverage,
startup failure cleanup, and completion after driver departure still need proof.
Read-only contributor `run_28487ba852f6426cba061d64e7ac6437` owns only
`scratch/reviews/parallel-handoff-history-review.md`, reviewing completion, usage, approvals and
Flow settlement across transfer. Main retains every executable file and the
sole build slot; the contributor runs no provider or build.

Supervisor reviewed the in-progress normal Codex WebSocket path. Its `stop()`
reads the current driver, awaits interruption, then kills the engine group;
the interrupt hook similarly checks before signaling without holding the transfer
fence. A replacement can claim between check and effect. This is a source-level
race, not an executed failure. Main must serialize the termination effect with
driver transfer while avoiding nested database locking through the interrupt RPC.
Keep the actual old-driver-stop/transfer and shared-engine-sibling proof in the
public lifecycle acceptance; a fenced socket send alone does not cover signals.

The supervisor replayed the exact file-journal directory obstruction after
repair, on candidate `9572e8e0b34c2737c284a699d2698c38eb54df3a50d6ba6c5f3fd12d6d027557`.
The command returned `[]`/0, stored one succeeded Exec with both start/completion
events, preserved the obstructing file and reported the unavailable journal.
Receipt: `/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo298-exec-journal-repaired-56z30yfy/receipt.json`.
This proves directory setup failure only; the authored append-failure case
remains part of main's integration batch.

`journal-h7-repair-2.log` ran six of eight selected tests: five passed, the
explicit-sync fixture failed after its exact provider-content comparison on
KR text (`Keep this KR Retain this closing note.` versus `Keep this KR`). The
unheaded closing paragraph is parsed as a KR continuation. Two cases did not run
because fail-fast canceled them. Do not claim eight passes or loss of the raw
authored content from that assertion. Main owns the fixture correction and the
remaining checks; retained two-Home mutation and legacy-adoption retries passed.

H7 repair contributor `run_e86b670c177442d3a8c9b4e5f8c0c3af` has returned.
`parallel-h7-repair.md/.patch` is ready, SHA-256
`ea77b0d35f74157b430472701033520cbea60b978574fa53de2b3ceb8d17900b`.
Supervisor reviewed production changes and all three regression setups; patch
applicability passes. Main must apply once and execute the regressions plus
affected retained retry coverage. Syntax/format and GraphQL shape passed in
private copies, not behavioral tests. All bounded contributors are now terminal;
main retains all executable files and the build slot.

Supervisor's next actual CLI counterexample retained the same `3847c4c3` binary
in a disposable Git repository and made `.lf/journal/runs` a regular file.
`session list --all --json` returned `[]`/0; debug output reported `Not a directory`
at Started, and the initialized writable database contained zero Execs. The
obstruction remained byte-identical. Receipt:
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo298-exec-journal-jon7jbqa/receipt.json`.
File-journal setup still precedes context/SQL admission; `append_event` also
precedes `ledger_insert`. Main must preserve error/input evidence while removing
this dependency from the database-owned lifecycle. A private fixture is not
installed acceptance, and the passing public-child proof does not close this
distinct file-failure case.

Public nested-command replay now passes at `public-headless-parent-6` with CLI
SHA-256 `3847c4c353b38841c70f23214667594b47f36a4057e0a80c7a54ebf01d25f451`.
Supervisor inspected its receipt: one actual agent-issued child, correct parent
Exec, unchanged AgentSession, provider generation 1, released driver after
completion, and successful rename. Native Codex is real; upstream Responses are
synthetic; Home is disposable. Parent-5 directly observed sandbox refusal of
`ps`, confirming why journal admission stopped. Command-entry time is now separate
from nullable OS process-start evidence; no missing process identity can grant
signaling authority. The sandbox was not widened. Public connect/handoff/restart,
physical owner conversion and import still remain; this pass closes only the
public fresh-launch/provenance portion.

Publication integration now has executed proof: `publication-preservation-3.log`
records both acknowledged-create/failed-read/retry and existing-PR/readiness
regressions passing in 3.284 seconds. The readiness case retains a differing
head-pinned request and proves it is cleared before saving the changed head.
The preceding failures were retained: the new fixtures still supplied old PM
snapshot shape without required `flow`. This is simulated GitHub/Linear failure
evidence, not a reproduced historical cause of PR #1296 or live publication.

Public nested-command proof remains failing at `public-headless-parent-3`.
Supervisor inspected its synthetic response output: `LF_AGENT_CALLER` is present,
and Home/DB name the private fixture, but SQL has no child Exec at all. That
narrows the missing row beyond a mere `via_agent` flag error. The source's new
required `process_started_at` query and file-journal append precede SQL writes;
workspace sandbox denial is a hypothesis to distinguish with exact debug output,
not an established cause. The successful direct native fixture explicitly used
a broader sandbox. Main must retain the public permission context while proving
admission, rather than change the fixture's sandbox to make it pass.

Both reviewers returned. Import mapping is in `scratch/reviews/parallel-import-review.md`;
main retains physical conversion and import implementation. H7 review is in
`scratch/reviews/parallel-h7-review.md`; the supervisor confirmed its two source counterexamples
(name normalization replaces authored content; final observed cancellations are
ignored before predecessor completion). A new bounded Codex contributor owns
only `parallel-h7-repair.md` and `.patch`, preparing a repair and focused tests
without touching executable files or using the build slot. Main keeps all source
ownership and must integrate/review/test the returned patch. The archived-Project
fixture's existing persisted-timestamp correction must survive integration.

Supervisor found a counterexample in the first per-thread Codex environment
change. `launch.env.clone()` followed by a `get_envs()` loop that ignores removed
values recreates explicitly supplied values that `configure_agent_env` or
`set_vendor_std_env` removed: bridge token, legacy writer identity, directive
path, and stale ordinary Home overrides on the release path. The existing
bridge-token regression demonstrates the explicit-config case. No real token
exposure was observed. Main was instructed to derive intended thread launch
values from the sanitized command changes, including removals and fresh
overrides, without copying account credentials wholesale. Public child provenance
and retained environment-removal behavior both need proof.

All previous implementation contributors have returned; main owns all executable
files and the sole build slot. Two new bounded Codex reviews run without builds
or patches: `scratch/reviews/parallel-import-review.md` maps historical evidence to the accepted
Exec/AgentSession/FlowSession model; `scratch/reviews/parallel-h7-review.md` checks the completed
Chapter/adoption source against retry and preservation counterexamples. Each
reviewer owns only its named note. Neither may modify implementation, duplicate
the schema work, invoke providers or inspect an installed Home.

Supervisor inspected `integrated-admission.log`: seven focused admission,
repository-before-pagination and Exec lifecycle checks passed in 17.144 seconds.
The later WAL repair passed three focused checks and the actual CLI replay at
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo298-first-home-execs-1dmf_kwt/receipt.json`.
That receipt pins CLI SHA-256
`d29d1d2573435a7271ca016b6d85756c54c40dccba6ff85dc0f2e0a7505d9c13`;
all six commands across three fresh Homes have start/completion events, success
rows, no warnings and no agent Runs. Earlier failing receipts remain retained.
This is isolated CLI proof, not general load or installed-Home acceptance.

Main integrated the publication patch with the reviewed ordering correction:
invalidate a known identity's stale merge request before changing its head;
persist acknowledged identity before Linear linkage. The supervisor inspected
that correction in source. Behavioral publication checks remain pending.

HEAD advanced to `d07e56933` (`lf pr open: prepare branch`) during supervision.
It checkpointed the integrated source and removed the prior supervisor note
`scratch/execution-model-review.md`, retained at `03330279f`. The supervisor did
not invoke that command; the commit title does not establish who invoked it or
whether remote publication occurred. Main was notified. The local-only review
boundary remains in force; current source history preserves the older evidence.

2026-09-28 · Jack authorized parallelizing as much as possible without reducing
quality. This allocation supersedes the earlier one-code-writer implementation
sequence, preserves the full accepted scope, and creates no second Task or
worktree. All contributions use Codex through installed lf.

## Ownership

- **Existing managed worker:** Exec/AgentSession/FlowSession conversion, native
  connection, public CLI, Session/Run history and import, shared store modules,
  Rust/Swift consumers, core docs and final integration. Retains the saved Flow
  claim. Do not edit the paths reserved below while their contributors run.
- **Schema performance contributor:** only
  `rust/loopflow/src/store/migrations.rs`, its tests in that file, and
  `scratch/parallel-performance.md`. Reduce repeated expected-schema construction
  without weakening actual schema, ledger, prefix or foreign-key validation.
  No installed-Home access or migration-history rewrite.
  A follow-up now retains this same file for the reproduced first-Home migration
  race. It owns `scratch/parallel-initialization.md` and an optional exact
  shared-file patch. Preserve the existing schema memo and its drift tests.
  Main retains journal/Exec admission policy; no build slot is granted.
- **Chapter contributor:** `rust/loopflow/src/pm/mod.rs`, `pm/linear.rs`,
  `ops/chapter.rs`, `ops/chapter_tests.rs`, `work/chapter.rs`, `work/project.rs`,
  `planning.rs`, `store/chapters.rs`, `store/sqlite/chapters.rs`, a uniquely named
  Chapter draft migration, `docs/waves.md`, and `scratch/parallel-h7.md`.
  Implement accepted status-based repository rotation and Project Flow behavior.
  Shared-file changes (CLI, store facade, Task/Flow integration, DTO/Swift and
  core docs) belong to the managed worker: leave exact edits as a patch/handoff,
  never silently modify those paths. Coordinate schema dependencies with the
  new execution model; retain Task identity and execution across transfer.
- **Existing-Task stacking contributor:** inspect the retained incident and
  current placement/delivery APIs; own only `scratch/parallel-stacking.md` and
  `scratch/parallel-stacking.patch`. Prepare a concrete repair and focused
  preservation proof as an unapplied patch. Shared Task/delivery code remains
  with the managed worker; this contributor applies no executable edits.
- **Incident audit contributor:** read-only source/history audit of the retained
  missing Task PR publication and cancellation/refused-start reports. Owns only
  `scratch/parallel-incidents.md` and an optional unapplied
  `scratch/parallel-incidents.patch`. No builds, executable edits, configured
  provider calls or duplicate stacking implementation. The managed worker owns
  any resulting repair and its proof.
- **Publication repair contributor:** after the incident audit, prepare the
  acknowledged-identity ordering repair as an unapplied patch only. Owns
  `scratch/parallel-publication.md` and `scratch/parallel-publication.patch`.
  Main keeps all Task/PR executable files and the build slot. No provider or
  remote Git operation is authorized by this contribution.

Contributors may inspect everything. Do not commit, stage, rebase, publish,
install, navigate the Task Flow, edit shared config or mutate configured Linear.
Preserve all supplied dirty work. A compile error in another contributor's
owned file is integration evidence, not authority to edit it.

## Verification and integration

Only one Cargo build/test batch runs at once; do not compete for the build lock
or run global formatting over another writer's files. The supervisor grants
build slots through Task comments and reads each contributor's ready note.
Read-only inspection, source edits and isolated non-building checks may overlap.
Each contributor records its exact command, input assumptions, result and
remaining integration before returning. Tests clear inherited LF/LOOPFLOW
authority and use disposable Homes per TESTING.md. Native/provider proofs remain
Codex-only. Resource checks must not clean another active contribution's build.

The managed worker checkpoints only coherent finished contributions, integrates
shared edits and runs the combined proofs. Partial passes do not satisfy the
code-complete concept review. Chapter retry must cover the partial-status
counterexample recorded in chapters.md. Performance retains the measured
empty-inventory baseline and validates deliberate schema drift rejection.

## Observed launches

- Managed execution worker: `run_8e4800ebef6045699e0be1fac668e6cb` remains at
  implement, iteration 9 of the saved feature Flow.
- Schema performance: `run_2bb88c9cf3604abaa992db98c3085143`, Codex; launch
  completed prompt preparation and began source inspection.
- H7 Chapters: `run_f328afde9d3e48bb86fef693dcd14506`, Codex; launch completed
  prompt preparation and began source inspection.
- Existing-Task stacking: `run_17b79f6272e848368b0f6f904486eb2e`, Codex;
  launched after the performance contributor finished. Its output is an
  unapplied patch and handoff, with no executable-file ownership.
- H7 preservation follow-up: `run_dd455dc0fe4947f5baa6b9e20430d7d5`, Codex;
  original H7 source ownership retained, no build slot.
- Incident audit: `run_f64d556882114e879409fd233a2007f9`, Codex;
  read-only source/history work, no source ownership or build slot.
- First-Home initialization: `run_5e039f52e291497cbb5ca8beded3756b`, Codex;
  owns migrations.rs and its ready note, no build slot.
- Publication repair patch: `run_06d28fdd07424f598ea79899c0d089fe`, Codex;
  patch-only contribution, no executable-file ownership or build slot.

Initialization contributor has returned successfully. Its source is released
to the managed worker; `parallel-initialization.md` owns the exact commands and
remaining fixture obligations. Supervisor source review checked that the read
snapshot ends before acquiring the writer, all migration decisions are repeated
under one transaction, released-draft adoption no longer nests a transaction,
and contention fixtures coordinate the waiting writer without sleeps. This is
source review, not compilation or behavior proof. Keep the actual first-Home
CLI reproduction and missing-start-event assertion in combined verification.

The supervisor strengthened `.lf/tmp/probe-first-home-execs.py` for replay:
`--lf PATH` selects the newly built candidate (default `target/debug/lf`),
copy hashes are compared before and after, and every round requires exactly
one started and completed event per succeeded Exec, matching recorded times,
zero agent Runs and no warning. Failure now exits nonzero with its receipt.
The original diagnostic script is preserved as `probe-before.py` beside the
failing first-Home receipt. Only argument parsing was checked after this harness
edit; the repaired candidate has not run through it yet.

H7 follow-up has now returned successfully. Its reserved source paths are
released to main. Read `parallel-h7-followup.md` and apply/reconcile only its new
shared patch; the original H7 sync patch was already integrated. Formatting,
populated SQL replay, eight GraphQL shape checks and patch checks passed in the
contributor; Rust behavior remains unexecuted. The build hold is released once
main integrates this last shared patch and passes resource preflight. The
publication contributor is patch-only and does not hold source or the build slot.

Combined initialization verification passed: `integrated-init-3.log` records
seven selected migration/cache/backup tests passing in 2.997 seconds after
28.95 seconds compilation. This includes all three new initialization regressions,
both expected-schema reuse regressions, backup transaction preservation and the
current-schema no-writer-lock check. Supervisor inspected the terminal Nextest
summary. Earlier invocation failed argument parsing (`-j` duplicated
`--test-threads`); the next compile exposed repaired shared fixture/import errors.
These are retained failed attempts, not passing tests. Actual CLI concurrent
admission replay, broader migrations/contention and materialized proof remain due.

The incident audit returned successfully. Its source-grounded handoff is
`parallel-incidents.md`: PR #1296 association has historical recovery evidence
but no proven original cause; current publication has an acknowledged-identity
recording window. Deletion/preflight retain their own proof, while released-claim
process settlement remains explicitly deferred. Main owns the narrow publication
repair and must not present Exec causal rows as process-death proof.

These bounded Runs are children of this supervisory conversation and carry
LOO-298 attribution. They do not own the Task Flow claim. The managed worker
acknowledged the allocation in `parallel-execution.md`; its preceding Cargo
batches finished before parallel editing began.

Stacking contributor returned successfully with its two assigned artifacts.
The patch is unapplied and has syntax/patch checks only. Its parent-selection
operation and publication-base reread under the existing mutation lock are
available for main-worker integration. The blanket `parent_pr_id IS ?18` change
in the generic update still needs the independent-PR-facts review below; its
test presently expects stale publication rejection, not preservation. Review
that expectation before applying it. No new contributor owns executable files.

## Current build slot

2026-09-28 integration update: the original H7 contribution returned with its
handoff; its source-only status does not satisfy acceptance. A bounded H7
follow-up retains the same reserved source paths to repair the four review
cases below (UUID contract, missing-local-Task uncertainty, archived history,
and initial legacy Project adoption). It owns `scratch/parallel-h7-followup.md`
and a separate shared-file patch; the original handoff remains evidence.
Shared store/CLI/DTO paths remain with the managed worker. No Cargo slot is
granted to this follow-up; the managed worker keeps the serialized build slot.

The managed worker has integrated the original H7 sync hook. Three shared
Project behavior tests, four Rust Project DTO tests and thirteen Swift DTO
tests passed. The stacking patch is applied with the independent-publication
observation correction; its focused behavioral proof is still due.

No Cargo batch is running. The supervisor released the performance slot after
two focused regressions passed. The wider migration command could not compile
because H7 changed ProjectFlowPlan and added Project flow/status while shared
callers still used the former API (`parallel-schema-migrations.log`). No tests
ran in that command. Candidate build/timing and wider checks wait for shared
integration; do not repeatedly compile the same intermediate source.

Performance edits are finished and available for integration; H7 and execution
edits remain active. The next slot belongs to the managed worker once it has
integrated the shared Project API. Record its exact next command and readiness
in `parallel-execution.md` before starting; no competing Cargo or global fmt.

## Review case for Chapter classification

Source observation during supervision: the old `ops/chapter.rs::disposition`
initializes `authored=Some(false)` when this Home has no Task row. A second Home
can lack that row even though the owning Home has attributed agent work while
Linear still says backlog. That absence alone cannot prove untouched work.
The H7 contributor should preserve this as an explicit two-Home classification
case and state what positive evidence authorizes expiry. Do not silently inherit
the old default or add a new shared owner merely to satisfy the fixture. This is
a review hypothesis to distinguish against actual admission/status behavior;
no cross-Home deletion was executed.

## H7 integration review — provider contract and existing Projects

The public [Linear schema](https://github.com/linear/linear/blob/master/packages/sdk/src/schema.graphql)
was read on 2026-09-28 and retained in `.lf/tmp/h7-review-schema.graphql`.
`ProjectStatus.teamId` and `statusId` are present. However, ProjectCreateInput
documents its supplied ID as UUID v4; the in-progress deterministic successor
helper sets version bits to 8. Match the provider's advertised input contract
while preserving deterministic retry identity, and exercise that shape in the
provider fixture. No live mutation was attempted.

The accepted chapters.md also requires existing non-archived Projects to remain
usable until the first rotation. The in-progress current selector accepts only
Started and the new content parser reads flow rather than recommended. Prove
the first upgrade with an existing Project in its actual prior status and old
content, preserving its default Flow and Tasks. Do not make the first rotation
require manual recreation or silently treat a deliberately Planned successor
as current. Any one-time conversion belongs in the existing transition path;
it must not become a permanent dual-format owner or an invented approval.

Static GraphQL validation passed for all seven affected operations against that
published schema: ListInitiativeProjects, CreateProject, IssueOwnership,
ProjectOwnership, ProjectStatuses, SetProjectStatus and CanceledWorkflowStates.
Receipt `.lf/tmp/h7-schema-validation.json` records schema/source hashes and
empty error lists; `.lf/tmp/validate-h7-schema.py` reproduces it. This proves
query shapes only, not runtime UUID constraints, authorization, fresh provider
state, retry behavior or configured-Linear acceptance.

Bounded contributors have not received the later Task comments as live input
(their event streams contain only initial user input). These findings remain
the supervisor and managed worker's integration responsibility until the
contributors explicitly incorporate them in their handoff. In particular, a
handoff that merely names the legacy Project conversion gap does not close it.

Legacy fixture must also include an archived predecessor. The pre-H7 writer
calls `archive_project` on predecessor Projects without changing their status.
The shared `checked_projects_with_store` now point-reads every retained Project
missing from membership, while `ProjectNode::into_pm_project` drops archivedAt.
An archived predecessor whose provider status is still Started can therefore
reappear as a competing current Project. Preserve the retained record and Tasks,
but do not promote archived history into current planning. This is a source
counterexample to exercise, not a claim about the configured Home's contents.

## Stacking integration review — independent PR facts

`ops/task.rs` records a successful GitHub publication through the general
`update_task_pr` writer after the remote effect and Linear link. A new blanket
parent-equality rejection in that writer could therefore lose a successful
publication observation when another command just changed the parent. Prefer
making the dedicated parent operation its sole writer and preserving fresh
parent state during unrelated PR-fact updates. Parent-dependent Git/GitHub
decisions still need the existing mutation coordination and fresh target facts;
do not mistake preserving observation for authority to publish against a stale
base. Include a stale publication-observation case in the patch review, alongside
parent-change and active-worker races. This directly relates to the retained
missing-Task-PR-link incident and should not recreate it.

## First-Home Exec admission counterexample — 2026-09-28

The supervisor ran `.lf/tmp/probe-first-home-execs.py` against copied candidate
SHA-256 `5dd1c368f1ed383821ec81eb4df3cbbef981771d7e3823fdbeb177767b59dda6`.
Two actual commands, `session list --all --json` and `usage --json`, began
together on a pristine disposable Home outside the repository, with inherited
execution authority removed. Both exited 0 and returned `[]`. Usage warned:
`ledger unavailable — runs are not being recorded`, followed by
`incompatible Loopflow database; delete loopflow.db and rerun the command`.
No database was deleted. The probe stopped after this first counterexample.

Read-only SQL found two succeeded Exec rows and zero agent Run rows, but only
three command journal events: Session-list started/completed, usage completed.
Usage has no started event; its Exec `started_at` equals completion time. Two
rows therefore do not prove preserved command admission. Receipt and copied
binary are under
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo298-first-home-execs-bf8h6n0w/`.
Free disk passed the 64 GiB check. This is an actual isolated CLI reproduction,
not a configured-Home incident or proof of a regression caused by the memo.

Source supports a race hypothesis: `apply_installed_development_sqlite` reads
ledger presence and validates the applied prefix/schema outside the subsequent
exclusive draft transaction. Another initializer may advance between those
reads. `journal::ledger_insert` warns and proceeds, so a later terminal event
can create an Exec whose start was never recorded. Distinguish the migration
snapshot race from the best-effort admission policy during repair. Preserve
real corruption/checksum rejection; neither missing-table-as-empty, suppressing
the warning, nor a terminal-row-count assertion establishes the contract.
The managed worker owns integration and the regression, including no fabricated
start time and no read-only-command Started side effect.

## Session pagination review — 2026-09-28

The in-progress `store/sqlite/sessions.rs::sessions` applies a global
`ORDER BY s.title, s.id LIMIT/OFFSET`; `lf/commands/session.rs::list` applies
`scope_to_repo` only after building those Session surfaces. Consequently a
page filled with another repository's titles can return an empty local list
despite matching local Sessions beyond that global page. This is a concrete
source counterexample, not an executed CLI result. The accepted query contract
requires relevant filters before pagination and before native/payload reads.
Prove with two repositories and a limit smaller than the foreign prefix;
preserve main-checkout/worktree equivalence and unavailable historical ancestry.
Also inspect Desktop's inventory caller: introducing a default limit of 100
must not silently remove the 101st open Session from a refresh or its panes.
Main owns the implementation and coordinated consumer proof.

The subsequent Swift paging loop must specify the page size it tests, rather
than depend on a second implicit copy of CLI's default 100. Its current
`ORDER BY title,id`/offset sequence also spans separate CLI calls: a rename or
insertion before the next offset can repeat an ID, while a completion can skip
one. The 101-static-row fixture alone does not cover that boundary. Retain
inventory identity and panes under such refreshes without adding another
durable snapshot owner. This is a source-level review case; no rendered
duplicate or pane loss was observed. Dense query-plan and latency proof still
must exercise the actual optional-filter/substring-search SQL, not infer indexed
selection merely because filtering moved into SQL.

## Public headless admission proof inspected — 2026-09-28

The supervisor inspected `public-headless-admission/results.json` and the
assertions in `tests/e2e/codex_connect.py::_launch_contract`. Copied CLI SHA-256
`4d72eed3084c5b3bff8f77bfb919697d49d9e2a36ed9012a0735a74f82765eb3`
returned launch exit 0, default empty inventory, one explicitly headless
conversation, and successful rename under the same Session ID. This uses actual
Codex and a synthetic localhost Responses server in a private Home. The receipt
lives at `.lf/tmp/execution-model/public-headless-admission/results.json`.
It predates repository-filter edits, and proves neither continuation/connect,
Flow settlement, final integrated bytes nor configured-model acceptance.

## Publication patch review — 2026-09-28

The patch contributor returned with `parallel-publication.md/.patch`; no working
executable files were changed. Its two proposed provider-failure regressions are
unexecuted. The supervisor found an integration issue in the proposed early
write: `attach_task_github_pr` replaces `publication.github.head_sha`, calls
`update_task_pr`, and only afterward invalidates a stale merge request. However,
`TaskPr::_validate` requires that request to name the current stored GitHub head.
A differing or unknown head with a retained merge request therefore fails the
new early save before reconciliation can run. This is a source counterexample,
not an executed failure or the historical cause of PR #1296.

Preserve acknowledged identity early without storing an invalid head/request
pair or relaxing the validator. Distinguish first acknowledged attachment from
enrichment of an already-recorded identity; retain the exact-head merge and
remote auto-merge revocation ordering. Main owns this repair while integrating
the patch and must add/retain the differing-head pinned-request proof alongside
the new read/readiness failure cases. The patch is not accepted as-is.


## 2026-09-28 — Public approval handoff proof

Supervisor private script `.lf/tmp/execution-model/supervisor-public-approval.py`
passes (handle 2848 terminal 0). Receipt:
`.lf/tmp/execution-model/supervisor-public-approval-1/results.json`, copied CLI
SHA d6ef27459f35fefa6e8540c7242fae373ce1c80bd47a61b26af7bd4bd7f8d317.
Actual Codex 0.157.1, private Homes, synthetic credential-free Responses and a
controlled native-protocol client through actual `lf session connect`.
Codex itself emits the escalated-command approval. A new public connection
receives the pending request. The old client's accept response followed by an
ordered read leaves waitingOnApproval and no marker; the new client's accept
runs the command once (`approved-once\n`). The held sibling stays active and
finishes after release. Public Session history contains exactly one successful
completion for the selected turn. Existing connect/provenance/history checks
also pass. No source files or installed Home changed; private script replaces
the separate driverless case with this approval case. This does not prove native
UI rendering, restart, Flow consumption or physical owner conversion.

Jack repeated the regular-rebase request. Main acknowledged checkpoint then
`lf rebase --plan` / `lf rebase` before the next owner conversion. At the read,
HEAD d07e56933 still lacked five commits from local origin/main; completion of
the rebase remains unobserved. Main alone owns integration.

## 2026-09-28 — Paginated native recovery and local rebase correction

Private supervisor script `.lf/tmp/execution-model/supervisor-pagination.py`
passed (handle90916 terminal0). Receipt:
`.lf/tmp/execution-model/supervisor-pagination-1/results.json`; copied CLI SHA
`d6ef27459f35fefa6e8540c7242fae373ce1c80bd47a61b26af7bd4bd7f8d317`.
After one normal headless turn, a direct native client executed 103 additional
synthetic turns while no lf receiver was connected. SQL still contained only
the first completion. Native `thread/turns/list` returned100 and a nextCursor.
Actual public `lf session connect` recovered all104 exact completion IDs;
103 recovered origins remained unknown, the original history stayed byte-equivalent,
and a second public connect produced identical history. No branch source/build
or installed Home changed. Real Codex/private Homes/synthetic Responses/controlled
protocol UI; this does not establish rendered UI, restart, Flow settlement or
configured-provider acceptance.

Observed usage boundary: reconnect produced ONE usage receipt for the latest of
103 missed turns, last20/5 and lifetime2100/525. The other102 turns had no usage
receipt. The last counter cannot become the last turn's whole spend, nor prove
per-turn cost for the gap. Preserve this counterexample in the usage conversion;
if recovering from additional native evidence, test it explicitly.

Supervisor corrected the earlier rebase spelling: installed `lf rebase --help`
says `--manual` keeps integration local; default rebase has a push path. Task
comment ed2223c9 directs `lf rebase --plan`, `lf rebase --manual`, then owned
`--continue` as needed. Regular rebases are authorized; publication is not.
Main remains sole integration owner. No rebase was performed by supervisor.

## 2026-09-28 — Busy turn/start after driver transfer is a continuation

Actual counterexample, copied CLI d6ef2745 (full digest in each receipt):
`.lf/tmp/execution-model/supervisor-busy-start-1/results.json` and
`supervisor-busy-start-release-1/results.json` both record B's public-client
reply timeout after A starts a held turn and B connects and sends turn/start.
The second releases the synthetic response after0.5s; native work completes
under the original turn ID while B never receives the reply. These diagnostic
scripts exit0 after storing the failure; exit0 is NOT a passing behavior result.
Handles74717 and51812 are terminal.

Direct native comparison `supervisor-busy-start-native-1/results.json`
(handle90223 terminal0) returns success with the exact original active turn ID,
status inProgress, startedAt/completedAt/durationMs null. Therefore turn/start
does not necessarily start another turn: it can add input to the active turn.
Source `codex_history::record` interprets every result.turn as new origin and
`record_session_turn_origin` rejects a different Exec. This explains the public
relay failure; the main worker received the comparison and must repair/prove it.
Retain A's origin, record B's input separately if needed, preserve unknown origin
for a previously unobserved active turn, and keep one completion/usage history.
Do not hide genuinely conflicting stored origins with a broad ignored error.
All probes use real Codex0.157.1, synthetic Responses, controlled native UI and
private Homes. No source/build/installed state changed; not rendered UI proof.

Bounded post-rebase DTO/Desktop review run_5d6a0b6e0a944924adc1933909944d14
(handle1569 terminal0) wrote `parallel-rebase-projection.md`. Reviewed in full:
only the stale Wave enablement projection was established as a merge defect;
main's dirty coordinated repair already addresses it, verification pending.
No extra Session/history or pane identity mismatch found. Existing history UI,
off-roadmap Task breadcrumb, physical owners and Run-removal gaps remain owed.

Strict busy-turn regression: `supervisor-busy-start-regression.py` under
`.lf/tmp/execution-model/` now exits1 on the same candidate (handle62602 terminal).
`supervisor-busy-start-regression-1/results.json` and `.log` retain exact reply
timeout. Model release is0.5s; secondary command is harmless printf. The assertion
requires B's reply, same active turn ID, original Exec/generation, one successful
completion and retained usage. Later assertions are unexecuted on this failing
candidate. This is the repair replay target; diagnostic exit0 above stays history.
