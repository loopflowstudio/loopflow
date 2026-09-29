# Evidence retained for the finish line

LOO-298 · Consolidated 2026-09-28. Results describe their recorded bytes only.
Detailed reports, earlier failures and source hashes remain in the
[archive](parallel-work.md). No new behavioral suite ran for this consolidation.

## Published CI and retained red results

Implement iteration10 checkpointed the unfinished caller conversion and review
evidence, then rebased without conflicts onto `b42d53205`; local head is
`884b90be97836c2bc0c73ed5e84e81b7425818d0`. Fresh Task and Git reads agree on
the base; Task publication still names `fb100f486`. Five upstream commits add
delivery/check caching, release-test cleanup, SQLite build optimization, Swift
build reuse and armed-PR queue handling. No conflict reconciliation test is
needed for that rebase. The one-line migration-test correction removes only
Project `status` from its retired-field prohibition, preserving Task status and
Project reason/time checks. It passes in the materialized diagnostic below.

The materialized matrix's first setup failed before tests because its file-copy
loop treated the `vendor/ghostty` Git submodule directory as a file. Retain this
as fixture setup failure, not a product test failure or executed Rust matrix.
The corrected setup's `implement-materialized-matrix-2.log` records compilation
in 58.78s, then **1,925 passed / 33 failed / 15 skipped**, all 1,958 selected tests
executed across 45 binaries in 263.361s. Exit100; no LEAK or timeout reported.
`implement-matrix-source.json` records head/base, command, source hashes and
private source root. It includes the Project-status assertion repair, which
passes, but predates later quoted-output regression/repair edits.

The diagnostic copy has no Git metadata. Two failures explicitly report this:
`watched_landing_completes_task_only_from_merged_pr_evidence` fails `git rev-parse
HEAD`, and `run_attribution_classifies_absent_context_and_hand_set_names` fails
repository discovery. Verified supervisor comment
`04354d7f-063f-4fe2-a797-3fc1aee16b41` requests correcting the isolated Git
context through the supported path before rerunning affected failures, without
weakening production behavior. Main acknowledged this distinction. The five
account-route failures sharing that cause remains a hypothesis. Three historical
migration fixtures insert into `wave_chapters` after their helper has applied the
full now-materialized tail that drops it; those are separate seed failures.
The Session native-client test changes its DB path after capture and fails to
open that file on resume. Other failures cover landing output, branch-data
selection, active-Run attribution, native-provider fixture launch, command
admission, Flow/cutover/Chapter fixtures and command-matrix inventory. Keep all
33 results until each is explained and repaired; this is a complete diagnostic,
not a passing matrix or native/provider acceptance.

Main created the isolated `loo298-matrix-fixture` through `lf wt` and copied the
same materialized source, removing tracked files absent from the snapshot. With
Git context and the runner's ambient `LF_DB_PATH` removed,
`implement-setup-recheck.log` records **13 passed / 2 failed** in 6.203s across
the 15 setup-sensitive failures. All five account-route checks, the native-client
publication test, landing repair-output read, attribution and retained branch-data
checks now pass without source edits. The two remaining global-command failures
are `task_origin_prevents_promotion_but_allows_read_only_candidate_preflight`
and `missing_repository_and_missing_home_are_distinct`. This removes 13 from the
original unresolved set, leaving 20; it does not prove which of the two fixture
changes caused each recovery. Receipt: `implement-setup-recheck.json`.

The next five-check batch, `implement-quoted-output-red.log`, overlaid the source
migration registry onto that materialized copy, accidentally unregistering the
generated batch. Two tests then stopped at missing `projects.flow`/`execs` before
the intended behavior. Its three migration passes are not final-schema proof.
Main identified the mistake and restored the generated registry; supervisor
confirmed both original and focused copies include `0.12.25.001_release.sql`.
The corrected repeat, `implement-quoted-output-red-2.log`, reaches the intended
classifier rejection of the completed review (including the recovered delivery
failure). Its other four checks pass: the three migration-preservation scenarios
and the public CLI command-failure/retained-lock proof. This establishes the red
regression and restores the migration proof's final-schema scope; the classifier's
green repeat belongs to the subsequent focused repair batch, described below.

Bounded Codex status-test contribution completed with exit0 and ownership returned
to main in verified comment `c063210e-67fe-499b-acc3-78f57cb83b68`. Only
`rust/loopflow/tests/status_tests.rs` changed: Project-owned KRs/targets and exact
current planning replace retired Chapter expectations, while stranded Task
evidence and retained historical credential failure remain asserted. Supervisor
reviewed the diff against `ProjectSummary`/`project_planning` and fixture seeds;
whitespace passes. No builds/tests ran in that contribution. Main acknowledged
handback and ran the complete status suite on the corrected materialized fixture:
**11 passed**, 10.434s after 21.95s compilation in `implement-focused-repairs.log`.
Supervisor confirmed current and fixture status-file hashes match
`implement-repairs-source.json`. The three status failures are resolved within
that proof. Contributor log: `status-fixture-contributor.log`.

The second command in `implement-focused-repairs.log` records **12 passed / 1
failed** in 32.267s. The completed-review regression now passes after deleting
the output scanner; command events remain, actual provider failure releases its
claim, rejected credentials stay explicit, and a public failed `lf commit`
retains its failed Exec result and lock bytes. Failed PR draft promotion also
passes after moving readiness before copy update: rejected readiness preserves
the prior draft copy, and retry publishes the ready copy. Supervisor inspected
the production diff and verified the controller/PR/Exec-test hashes still match
the receipt. These are local proofs, not an installed control repair.

The cached-auth failure exercised empty PATH from the Git source checkout and
stopped before its assertion. Main retained empty PATH and set the intended
ordinary disposable cwd. `implement-final-fixtures.log` then records three
assertion passes in 10.372s: cached auth, failed draft promotion/retry, and fresh
launch inventory. Nextest reports **one LEAK in the PR test**; this is not clean
process-settlement proof. `implement-final-fixture-source.json` records the four
overlaid fixture hashes. No full-green claim follows.

`implement-managed-topology.log` records the topology/return-count/bad-restart
test passing in 1.99s in the existing disposable OS installation harness; its
Docker container was removed afterward. This uses source 0.12.24 with drafts,
not the materialized 0.12.25 fixture or Jack's installed Home. The assertions
prove the managed fixture path, not configured-provider or Desktop acceptance.
`implement-repairs-static.log` records all-target Clippy finishing in 15.88s and
the static runner reporting success. Supervisor read these logs directly.

Main acknowledged verified sequencing comment
`c460c719-6374-4ed5-8eef-9f821479e05d`: continue from this repair checkpoint into
AgentSession reservation/publication and readers, retaining the zero-Run CLI
failure as the next executable target. Native decision retry remains a separate
failing requirement. Neither these fixture passes nor the static checkpoint
complete the owner conversion.

Main checkpointed the repairs as `f48606e8c`. Supervisor reconciled all 33 original
failures against the retained reruns in `supervisor-matrix-dispositions.json`:
**29 have focused passes, one has passing assertions with a process leak, and
three remain unresolved**. Renamed tests are mapped explicitly; the topology
pass retains its disposable-OS scope. These are mixed recorded snapshots, not
one passing matrix. The unresolved cases are checkout identity with main/parent
upstreams, Task Flow failure/retry/review, and taskless decision recording. Their
actual failures concern original-caller navigation authority or absent native
connection during OpenCode continuation. Keep all-provider admission/recovery in
the owner conversion; a Codex-only successful launch does not settle these cases.

Latest published head is `fb100f486`, still based on `1dce02734`, mergeable with
auto-merge off. GitHub and Task publication both name that head. It contains only
the one-line hierarchical-selector fixture correction described below; subsequent
local checkpoints and working changes remain unpublished. CI36517146691 is terminal. Rust
job109241873536 reports **1,304 passed / 1 failed / 14 skipped / 643 unrun**:
`a_fresh_database_applies_the_whole_chain_once` still forbids `projects.status`
alongside retired controller reason/time columns. The Chapter model deliberately
stores Linear Project status. Supervisor inspected the test and sent the narrow
assertion correction plus a materialized migration-module proof request in verified
comment `cb67b4d4-ef13-4728-82c0-0c765b399bd0`. Keep Task status absence and obsolete
Project reason/time checks; retain the positive Project-status schema/preservation
proofs. This test applies canonical MIGRATIONS, so its source-only pass cannot
establish the materialized result. Raw log `.lf/tmp/execution-model/fb100-rust-ci.log`.
No full hosted pass is claimed for this head. Verified supervisor comment
`ac8e73dc-b16f-4541-b1f8-3ea8187e56c9` supersedes the separate migration-module
run: after this correction and the owned rebase, run one no-fail-fast Rust matrix
on an isolated materialized snapshot, including migration coverage. Four successive
hosted stops across different modules justify that broader diagnostic. Record all
failures and exact snapshot bytes; do not run a redundant module first or repeat
the whole suite after each small repair.

Preceding head `70e8db9c6`, CI36516387300:
Rust job109239490288 failed: **1,054 passed / 1 failed / 14 skipped / 893 unrun**.
`ops::run::tests::hierarchical_work_selectors_must_match` creates two Started
Projects under the same Wave, then fails in Task creation before its selector
assertions: `cannot prepare a new Task in chapter history`. The fixture creates
`other_wave` but passes `wave` to its other Project constructor. Supervisor sent
the exact finding and a focused module proof request in verified Task comment
`1199b96b-fae4-4ca2-9824-2011d63ba4e0`. Raw log:
`.lf/tmp/execution-model/70e8-rust-ci.log`. CI is terminal: all substantive jobs
except Rust pass; scratch-clear fails with active notes. No full hosted pass follows.
The one-line correction assigns the other Project to `other_wave` and preserves
both selector assertions. `compress-run-fixture.log` records six `ops::run` tests
passing in 2.825s, formatting and all-target Clippy (17.41s) on published source
plus that fixture overlay. Supervisor inspected the diff and log; the correction
is now published as `fb100f486`. Verified review handoff `be418650-8459-4be9-ba12-055080ae24df`
requests a selective fixture checkpoint while preserving incomplete native edits.
Review-slice iteration9 completed with an explicit incomplete disposition in
`parallel-execution.md`; the saved Flow moved to its intermediate concept review.
The unfinished owner conversion and native failure are retained. This does not
establish the requested code-complete review or authorize a model/retry change.

Concept review `run_fe7ae2c0bf9941caa2550d950e4f3d62` wrote its retained judgment
and final answer, but Task execution blocked on a failed inspection command:
`AttributeError: 'list' object has no attribute 'get'`. Its code treated saved
`flow_events` row arrays as dictionaries. Supervisor reran the reads successfully
with row indexes, verified worker917/provider16037 absent through fresh `lf ps`,
and used `task resume --reason` to retry the same saved boundary. Fresh status and
OS evidence show `run_2cd37ffce47843dc8d9d27c132dd5ed9`, worker19438/provider20411.
This is recovery from a script error; no missing filesystem/control/network
capability was demonstrated by the blocker wording. The artifact and original
failure remain evidence; no accepted model or Flow edge was changed.
Further inspection of that Run's `ItemCompleted` event establishes the trigger:
the reader printed saved native fixture tool output containing `operation not
permitted` before its Python exception. `completed_boundary_failure` searches
the entire failed command output for that phrase, and
`execution_blocker_at_handoff` turns it into a Task capability blocker. The
permission text described a prior fixture's `ps`, not this command's failure.
Verified supervisor comment `c9c25a86-051f-4314-8054-194180846866` carries this
observed counterexample into the existing Task/Exec outcome conversion. The
reader correction recovers this instance; the classifier is not repaired.

The resumed concept review completed. Its following decision,
`run_232d082cf8da4b26b1753affd8190c8f`, recorded Iterate but was then failed by
the same classifier. This time source inspection itself printed the marker;
the command's actual exit2 came from `rg` using a guessed unsharded Run path.
Public Run inspection succeeded. The failed Run's verdict was not consumed.
The existing automatic unblock Ask
`ask_once_3b65e08317dfd16e8a83b89cb9a2fd0b0b49f4a48ca7e37fa05b0b56edc8619e`
was opened through supported Session controls; its agent verified the sharded
path, retained [the operational resolution](unblock-quoted-output.md), and marked
ready. Supervisor read the summary, completed that Ask under existing autonomous
management authorization, refreshed the Session list, and resumed the same saved
decision. Fresh Task/OS reads show `run_c94415c9f4554764bb7c41d78e615139`,
worker87749/provider88434. This is operational recovery, not new Jack approval,
a Flow verdict, or a classifier repair. Native retry remains separately failing.
The resumed decision subsequently completed and recorded Iterate. Fresh Task/OS
reads establish implement iteration10, `run_4f129189186948279f5cc9416d7897e6`,
same worker87749 and new provider18549. Its opening retains owner conversion,
queued integration and materialized diagnostics. No implementation result follows
from this successful navigation.

Review-slice replayed the legitimate native decision retry on the current source,
candidate `66f410dd9019612fa163fdd41c56219a594d32660d67798b2f70d9c7677d0280`.
`review-slice-native-replace/results.json` still records command exit1, a successful
successor provider turn whose decision command is rejected, and no consumption of
that successor. The Flow stays blocked for a missing decision. Supervisor inspected
the result: failed completion7, successor start8/completion11, selections5/8 but
no consumption11. This is real Codex with scripted Responses and a private Home;
it is a current-source regression result, not configured-provider acceptance.
The same candidate's two companion results are also inspected:
`review-slice-native-late` rejects the old child (exit1) and retains the unresolved
decision; `review-slice-native-owners` completes ordinary automatic retry with
one Session/thread/generation/Exec and consumes successor completion6 once, but
still records **one Run row**, failing the zero-Run assertion. The three results
together preserve both failing requirements instead of mistaking stale-child
rejection for a complete retry repair.

The current review receipt `review-slice-source.json` compares published
`70e8db9c6` with the working source at head `fb100f486`, including the untracked
caller draft: **+193 / -173, net +20 production lines**. It excludes tests,
Markdown and generated files and includes nine SQL lines. All 24 recorded source
hashes matched at supervisor inspection. This is the current bounded conversion
delta, not the whole-branch reduction; no final owner table is deleted by it.

Preceding head `0131ed763f` failed Rust in CI36515601936. That commit changes
only four Project fixture files (+11/-4), preserving the unfinished navigation diff locally.
Its isolated source copy passed five focused tests, formatting and all-target
Clippy (17.35s); `project-fixture-static.log` and its runner retain that scope.

Rust job109237049839 reports **932 passed / 2 failed / 14 skipped / 1,010 unrun**.
Both `task_completion_reconciles_lost_registered_response` and
`task_completion_reconciles_provider_outcome_after_landing` fail at
`ops/pm/task_planning_tests.rs:834`: `UNIQUE constraint failed:
projects.external_project_id`. Supervisor inspected the shared fixture: after
real `task_create`, registered cases unconditionally insert another `project-1`
with a fresh local ID. Reuse of the synchronized Project is the leading repair;
production uniqueness and actual completion/retry assertions must remain.
Raw log `.lf/tmp/execution-model/0131ed-rust-ci.log`. Task-installation passes
on this head; remaining hosted results keep their own scope.

The isolated completion-fixture repair first passed ten of twelve tests and
exposed two deletion cases whose synthetic provider omitted ProjectOwnership
for the retained predecessor. The final fixture reuses the synced Project and
returns the Completed predecessor separately from the named successor.
`completion-fixture-contract-2.log` passes all twelve planning tests (7.692s),
formatting and all-target Clippy (16.85s); the first red log remains. No product
uniqueness constraint or outcome assertion was relaxed. Supervisor inspected
the diff/log; this does not establish the unfinished navigation tree or hosted CI.

The broader PM check reused that isolated completion-fixture source tree:
`compress-pm-module.log` records **44 passed (one leaky)** in 17.313s, with
`--no-fail-fast`. The leak is `linear_oauth_cancelled_lock_waiter_never_exchanges`;
its assertions passed, but process settlement is not clean. The known SQLite
contention test passed in this run. No further PM fixture failures appeared;
this result does not verify subsequent navigation/compression edits.

Supervisor reviewed compression against the three saved source files under
`.lf/tmp/cut-i/compress-before/`: **+77 / -94, net -17 production lines**,
excluding trailing test modules. The location helper now derives from the
already-loaded capture, and decision/route writes reuse the Flow read in their
transaction. Five duplicate capture reads and obsolete operation branches in
agent reservation are removed; the shared original-turn check stays. The
remaining `runs` owner is not deleted by this reduction. Inspected logs record
six owner tests (2.861s), three Session/mechanical consumer tests (3.993s), and
all-target Clippy (17.23s), all passing. These retain the synthetic/native proof
boundaries; the valid native decision retry and zero-Run public proof remain red.

Chapter skill contributor `run_cb8561c9f2d9443897f282a50e52d0ad` completed eight
Markdown edits without builds or live operations. Supervisor reviewed the diff
against parser/data/writer source and repeated the obsolete-command/storage
scan plus scoped whitespace check. Guidance now uses repository rotation,
required Project Flow, retained canceled issues, current-plan-only updates and
dated historical evidence. Review gates remain. The files are uncommitted and
returned to main; no installed skill synchronization or live Chapter proof follows.

Preceding head `594c7c319f` integrated upstream. Reconciliation changes only seven files relative to
the pre-rebase mechanical checkpoint; operation/native source and draft bytes
are unchanged. `rebase-schema-cache.log` passes both focused checks: distinct
prefix/changed-SQL keys and rejection of schema/ledger/data drift.

Hosted run36513715591, Rust job109231281571: **930 passed / 1 failed / 14 skipped /
1,013 unrun**. The previous OAuth regression passes. The new failure is
`ops::pm::task_comments_tests::task_comments_read_and_publish_without_placement`,
at task_comments_tests.rs:201: `failed to decode Linear response: missing field
status`. Its IssueOwnership fixture omits Project status and carries empty
content; the adjacent planning fixture supplies started status and `flow: feature`.
Raw log `.lf/tmp/execution-model/594c7-rust-ci.log`. Do not relax required provider
fields to repair a stale fixture. Supervisor relayed the exact failure and sibling
fixture audit in verified Task comment `77e10d11-aee9-4341-8ac2-5a0522193640`.
No full Rust pass follows.
Local `project-fixture-contract.log` subsequently passes five focused tests:
comments without placement, PM snapshot, OAuth recovery, reteam preservation
and interrupted reteam. It tests an archive of `594c7c319f` with four fixture-file
overlays, recorded in `project-fixture-source.json`. Supervisor verified the two
Rust overlay hashes still match. The run executes Rust only; it does not verify
the two edited Python CLI fixtures (one Python hash changed afterward), the
unfinished navigation diff, or a fresh hosted result. No production parser changed.
Swift job109231281996 and UI job109231281715 subsequently passed; all other
substantive jobs are also green. Scratch-clear remains red with active notes.
Those hosted results do not establish configured Desktop acceptance.

The same job passed all **14 Chapter tests**. Supervisor inspected
`ops/chapter_tests.rs::every_provider_mutation_recovers_on_the_same_or_a_second_home`:
24 recovery cases cover twelve interruption points across two stores, a two-Wave
read-only preview, final statuses, repeat without extra provider mutations, and
preserved Task identity/worktree/plan/PR/Flow in the retained transfer case.
The fixture invokes Rust rotation, including on the second Home; it does not
prove adoption by public sync alone and does not compare Started timestamps.
Adjacent tests cover legacy content, response loss, conflicting/archived statuses
and unobserved work on another Home. The updated Chapter note distinguishes
these fresh hosted passes from remaining public-path and final-schema proofs.

**The mechanical managed disposable-OS proof now passes in hosted CI.** Job
109231281578 ran all five tests through `scripts/test_task_installation.py` in a
container with a fresh OS account and no host Home mounted. Exact named test
`task_operation_starts_with_durable_history_after_claim_only_failure` passed in
6.52s, zero ignored. Supervisor inspected the test: claim/release leaves Started
unset, roadmap agrees, removal of the template does not prevent the real task
worker executing its captured operation, durable history sets Started, and Run
inventory stays empty without changing that timestamp. The claim is seeded
through the store before invoking `task __worker`; this is not a configured
agent or normal Task-launch admission proof. Other four installation proofs
also pass. Raw log `.lf/tmp/execution-model/594c7-task-installation-ci.log`.
The earlier Mac Docker timeout remains an observation, but no longer blocks
this particular proof obligation. Shared result recorded in Task comment
`bf673e2e-3df0-48e1-9336-71faa89ba7f6`.

Historical published head `81b57c91f`, rebased onto `aab595e6a`: GitHub reported
mergeable; Task base matches Git and auto-merge is off. CI36510330745, Rust job
109220794661, reports 928 passed / 1 failed / 13 skipped / 1,007 unrun.
`ops::pm::oauth_tests::pm_read_linear_oauth_recovers` fails decoding its synthetic
Linear Project: missing required `status` at oauth_tests.rs:174. The response
fixture lacks the status already present on its local ProjectPlan. This is not
the separately recorded SQLite-contention flake. Raw log:
`.lf/tmp/execution-model/rebase-81b57-rust-ci.log`. Other substantive hosted jobs
passed; scratch-clear and aggregate result failed. The preserved mechanical
ownership proof had not yet passed: its local RED in `flow-operation-owner-red.log`
observes two Runs, zero AgentSessions and one Exec for two in-process operations.
CI stopped before that test. The first local `flow-operation-owner-green.log`
passes the new actual-CLI mechanical assertion (two operations, one Exec, zero
Runs/AgentSessions, separate Flow events), but OAuth still fails after adding
status: its whole-Project equality expects the old plan even though H7 refreshes
planning fields. Retain durable identity/progress/events while checking refreshed
planning; its synthetic content also lacks the required Flow. Source review
comment `829b1dc0-bc67-44fb-b0bc-0194a7a1e131` names a separate unfinished Started
gap: a claim-only mechanical reservation can lose its sole work evidence when
released before operation start. No full-green or complete owner-removal result
follows from the one mechanical pass.

`flow-operation-boundaries.log` then passed operation interruption/retry, OAuth
recovery and the taskless mechanical CLI (three tests). The managed Started
proof failed before worker execution: OS-account installation selection ignored
the fixture's HOME override and correctly refused the branch worker. That is
not a managed execution pass. The Started source writer/claim exception are
removed; the proof still needs the disposable account harness specified in
TESTING.md. Comment `01be4f1e-fbe1-46d6-a811-b4359bf63c26` records the handoff.

`flow-operation-managed-os.log` records Docker's 10-second preflight timeout;
no proof container was created. Local work continues. In `flow-operation-store.log`,
the historical fixture first violated the existing positive-attempt constraint;
main corrected its old Run inputs. `flow-operation-store-2.log` then passes
populated operation migration, exact native completion, interruption/retry and
OAuth recovery: four passed, two failed, one unrun. The failures are the managed
reducer still expecting two Run rows after both Ops complete, and the new
later-operation-failure CLI fixture asserting a missing-generator error. Source
inspection finds `telemetry-scorecard` in that fixture while the actual parser
names `__telemetry-scorecard`; actual stderr was not printed by its failed
assertion. Comment `dee0eb8f-ce65-43b0-9959-bef47dda4bbd` records both findings.
The subsequent `flow-operation-error-observation.log` prints the actual failure:
the first operation ran, then `telemetry-scorecard` was rejected as an unsupported
Op. This confirms the fixture never reached the intended missing-generator error.
The migration pass preserves selected history, old operation selectors/results
and unknown start/Exec evidence; it is not a canonical-materialized repetition,
complete filesystem import or installed-Home proof.

`flow-operation-store-3.log` passes all seven selected tests (29.243s after
3m04s compilation). The reconciled Task-driver proof checks claim/release leaves
Started unset, real operation history sets it, both Ops finish without Runs,
and the first-assignment time cannot change. The corrected CLI failure fixture
reaches the missing-generator failure and retains earlier success in the same
failed Exec. These results supersede the two failing assertions above without
erasing their diagnostic logs. Canonical repetition and disposable-OS managed
execution remain distinct outstanding checks.

Supervisor's actual-CLI interruption proof also passes on copied candidate
`33be09c872e4c82c786237a1b21f23d59c024c33a56a1be247fe95f548182467`:
`.lf/tmp/execution-model/supervisor-mechanical-crash-1/results.json`, with script
and log in its parent directory. In a private Home/repository, a synthetic
telemetry script writes one effect and waits. Killing only the fixture's owned
process group leaves the operation and original Exec outcomes unknown. Remove
the Flow template: plain resume exits1 with inspect-before-retry and still one
effect; explicit retry exits0 with exactly one further effect and completes the
captured two-Op Flow. Five history events retain the original unknown start and
two successful operations under one new Exec; zero Runs/AgentSessions. No SQL
mutation, installed Home, provider or remote effect. Preflight passed72.5GiBfree.
Comment `f098395e-c8b9-4040-a97b-996d65706dda` records the reviewed proof and limits.

`canonical-operation.log` materializes all24 drafts as `0.12.25.001_release` in a
disposable source copy and passes six checks (5.755s after1m39s compile): populated
operation history, exact native completion, Task mechanical driver/Started,
agent reservation/retry continuity, interrupted operation recovery and empty-draft
initialization/upgrade. This closes the focused canonical repetition above;
complete historical import and disposable-OS managed execution remain unproven.

Earlier integrated checkpoint `26a0270af`, based on main `d9632d833`, was
published to PR1296 on 2026-09-28. GitHub and the Task record agree on the head;
the Task base agrees with Git. Rebased recovery commit `e9597b75e` preserves
the recovery source and compact scratch from `b352cc691`; the intervening
diff is the eight files from upstream PR1317. The final checkpoint updates
the upstream Ask retry assertion to read its retained SQLite-backed public
Session instead of the retired Ask file. Main reports the focused Session
launch/retry proof passed; that is not a new recovery-suite execution.

Hosted run **36507832289**, Rust job **109213159143**, now reports **882 passed /
1 failed / 13 skipped / 1,050 unrun**. `ops::flow::tests::telemetry_flow_persists_the_portfolio_reading`
prints `product/task-loop-trust: accepted`, then panics at `flow.rs:518` indexing
an empty portfolio. Raw log: `.lf/tmp/execution-model/recovery-26a027-rust-ci.log`.
Source observation: the fixture creates a Wave but no PM snapshot; the current
portfolio reader returns an empty list plus `ChapterUnavailable` when current
Project resolution fails. Three neighboring metrics tests also omit that snapshot
and were not reached by that CI run. The initial source trace was not a local
reproduction or an accepted repair.
Resolve the contract without confusing unknown target planning with no target,
or erasing retained observations. Main received the exact evidence in Task comment
`30db73b7-3e6b-4400-87ec-dded72424d47`; docs-first work continues before the repair.

After docs checkpoint `4bb44b999`, main reproduced all four affected failures in
`.lf/tmp/cut-i/metric-planning-red.log`: one pass, four failures, 1.506s after a
30.20s compile. The command selected `ops::metrics::tests` plus the telemetry Flow
test, with isolated Home/authority. Each affected portfolio loses its expected
rows. Source review also found Desktop's unavailable-plan headline depended on
an empty metric list, and row targets said unset for any null target. Task comment
`2059afb5-64b7-4095-99d8-da1ce9a66540` records that consumer dependency. The repair
must preserve the reading and distinguish unavailable planning from known empty
targets in Rust, mirrored DTOs and Desktop presentation. The next compile found
a missing CLI enum match; review also found wildcard value rendering and null
target rendering would hide the reading/uncertainty. Main fixed both, including
Wave-specific target availability. `metric-planning-green-2.log` records 24/24
Rust tests passing (1.540s), including all four reproduced failures and CLI
fresh/stale/never-observed formatting. The first `metric-swift.log` records
21/22 passing: the summary fixture still expected a normal target headline in
the presence of `ChapterUnavailable`. Main is reconciling that expectation while
retaining a separate known-plan assertion. The final `metric-swift-2.log` passes
22/22 (0.018s), covering mirrored DTO decoding and row/summary presentation;
unavailable-Wave matching keeps other Waves' row targets intact. This is a Swift
presentation test, not rendered Desktop acceptance. `metric-final-rust.log`
passes 26/26 (1.520s), adding the two shared DTO assertions to the focused Rust
checks. Static checks and a new hosted result remain pending at this observation.

Hosted run 36507832289 is complete and failed. Rust lint, migration, architecture,
Python, website, smoke, Task installation, Swift and UI compile jobs passed.
The failing jobs are Rust, scratch-clear (active design notes remain), and the
aggregate tests-result. No full-green gate, auto-merge or landing is claimed.

Earlier published `7e2101b41`, base `a2b59ed50`: CI **36494751569** failed:
Rust job **109171585133**, 39 passed / 2 failed / 13 skipped / 1,890 unrun.

- `task_decision_driver_failures_open_one_unblock_and_reassess_feedback`:
  SQLite foreign-key failure, `controller/task/mod.rs:1842` at that checkpoint.
- `task_decision_public_resume_preserves_feedback_after_adoption_refusal`:
  “Conversation has no connection and no confirmed engine exit,” line 2044.

Log: `.lf/tmp/execution-model/flow-checkpoint-rust-ci.log`.
Migration, Rust lint, architecture, Python, website, smoke, Task installation,
Swift and UI compile jobs passed. Scratch-clear and aggregate test result failed.
These passes do not close the unrun Rust tests or establish a full gate.

The current focused reducer test first failed on absent Run outcome:
`.lf/tmp/cut-i/flow-native-completion-red.log`, one failed, 1.283 seconds.
A future green assertion must distinguish selected from earlier success, advance
once, reject stale claims/versions and retain unknown command outcome.

**Public driver-loss counterexample:**
`.lf/tmp/execution-model/supervisor-flow-completed-after-driver-1/` contains
results, source hashes and logs; script `supervisor-flow-completed-after-driver.py`
in the parent directory. Copied CLI SHA
`709b1ca131aa7c39f7b26ddf3fea8f5139a42c1122a5dc6a5c02d7f22efabdae`.
Actual Codex 0.157.1, synthetic local Responses, private Home. Kill only fixture
lf process group, retain separately owned provider, release response. Native
`thread/read` observes turn `01a0ea3c-8e9b-7590-943e-1050518b0fde` completed
**before** plain public `flow resume`. Resume exits 1 waiting for old Run end;
SQL history still has Started only. Original Exec outcome/exit remain NULL.
No SQL seeding/mutation created this result; exact fixture engine was cleaned.
This proves the failure after native completion, not merely a still-busy turn.

## Working recovery proof reviewed after consolidation

The supervisor inspected main's test source, raw results and logs. Final copied
candidate `162dd96586ed88470fbf974bc467a91fd2cf8eba19ad92bae525ca9f12be0c92`
passes `native-selected-{running,completed}-final/results.json`: public resume
consumes the exact selected completion once, completes the Flow, retains Session/
thread/generation, and leaves killed Exec outcome/exit NULL. Both check public
history's original Exec/generation. Completed mode observes native completion
before public resume; repeated resume consumes nothing twice. The same candidate
passes `native-selected-{retry,engine-loss}-final` for retry after a recorded
failure. Earlier c718 candidate results remain history, not additional coverage.
These repair the retained driver-loss counterexample within taskless fixtures;
account continuity, managed Task recovery and full owner removal remain unproven.

Cut `flow-selected-history-2.log` passes two checks (selection and busy-turn origin).
Cut `flow-selection-and-decision-fixtures.log` passes five checks, including both
published CI failures, exact selected-turn consumption and final cursor fences.
The selection test rejects earlier/unrelated success, old version/claim and retry
cross-talk, then consumes one exact success while Exec outcome stays unknown.
Cut `canonical-flow-history.log` passes four checks after 23 drafts materialize
as 0.12.25.001_release in a disposable source copy: selected-turn consumption,
both CI regressions and empty-draft initialization/upgrade (3.893s,31.25s compile).
This is not all-kind populated import proof. Cut `flow-history-clippy.log` records
all-target Clippy passing, 17.06s. These local passes do not replace hosted CI.

### Remaining failure: driver and engine both lost

Supervisor probe `supervisor-flow-both-loss-3/results.json` (same parent directory)
fails public `flow resume FLOW --retry`: exit1, Connection refused (os error61).
Candidate SHA `162dd96586ed88470fbf974bc467a91fd2cf8eba19ad92bae525ca9f12be0c92`.
Selected native turn is held; fixture lf is killed, surviving engine identity is
checked, then exact fixture engine is crashed. `ps` confirms it absent before
retry. Flow remains current without a failure; old Exec outcome/exit remain NULL.
No SQL manipulation. Source routes pending selection through old-socket readback
before replacement. Existing stopped-engine proof starts with a completed failure
receipt and does not cover this case. Fix within existing recovery owners, retaining
unknown prior turn/Exec evidence and same native conversation.

Earlier -1/-2 probes stopped at fixture graceful shutdown waiting on held work;
they are not product failures. -3 deliberately injects SIGKILL after exact identity
checks and reaches the advertised retry command. Later assertions inherited from
the surviving-engine fixture must be adapted to generation replacement before a
future green run can establish full recovery. The failing retry assertion itself
is direct CLI evidence. No configured account or installed Home was used.

### Automatic retry: retained failure and repaired public path

`supervisor-flow-automatic-retry-1/{probe.py,results.json,source.json,probe.log}`,
same162dd965 candidate, actual Codex/private Home/synthetic Responses. One public
headless Flow command receives one transient-unavailable response, then success.
The lf retry loop logs attempt1→2, resumed=true, no account failover. Session
history retains failed and successful distinct turns under one native thread and
generation, with successful cumulative usage40/10. The command exits1 waiting
for its selected native completion; Flow history contains only selected seq1,
which names the failed turn, and no consumption. No explicit Flow retry or SQL
mutation. This reproduces the source concern in remaining-work.md; existing
explicit-retry passes do not close it. Preserve both outcomes and select only
the authorized successor, without replacing Session identity or inventing Execs.

Supervisor inspection of `native-selected-automatic-1/results.json` and the
tracked fixture on 2026-09-28 confirms the repaired candidate
`84fb1f7d744de2409c8cedc1153c0fed6eb2c1d043e7875039e84c10ed68f0b3`
exits0 and completes the Flow. One command/Exec, AgentSession, native thread and
engine generation contain failed then successful turns; selection records seq1
then seq3 and consumes only success seq6. Successful cumulative usage40/10 is
retained; the failed response reports no usage. The fixture asserts one engine.
This closes that public counterexample with real Codex and synthetic Responses;
it is not configured-account or full managed-Task acceptance.

### Both-dead recovery: repaired standalone path

Supervisor inspection of `native-selected-both-loss-1/results.json` and the
tracked fixture confirms candidate
`6823ef083d531ae4719bd03d2403bc23c0794246b1c248d8edd8dd4f7384069c`
recovers through public `flow resume --retry` after exact fixture driver and
engine SIGKILL. Session/thread remain, generation1→2, old Exec outcome/exit stay
null. Native readback reports the earlier turn interrupted; the retry's success
is consumed once and repeated resume consumes nothing further. Public history
attributes the earlier interruption to its original Exec/gen1 and success to
the new Exec/gen2. These are real Codex/private-Home/synthetic-Responses results.

The preceding Connection-refused result remains the pre-repair observation.
Current source shares `prepare_native_retry` between explicit Flow retry and
Task resume preparation, before replacement claim acquisition. Managed-Task
policy now has focused evidence in Cut `flow-final-recovery-tests-2.log`: four
tests pass (4.291s), including both prior CI failures, exact native selection and
`managed_flow_retry_releases_only_dead_native_selection_and_consumes_its_successor`.
Supervisor inspected the test: public managed resume prepares the dead selection
then preserves the existing branch-adoption refusal; a manually claimed successor
with synthetic native completion is settled through `drive_task`, preserving
history, Session identity and unknown old Run result. Its owned dead process is
real, provider/claim/completion are simulated. This proves dispatch preparation
and reducer policy, not a full configured managed Codex launch. Final combined-byte
native verification remains due. Earlier compile failures are retained in the
unsuffixed test log; `-2` is the completed passing command.

### Failed-turn decision: reproduced and repaired

Supervisor probe `supervisor-failed-decision-retry-1/{probe.py,source.json,
results.json,probe.log}` runs candidate84fb1f7d (the automatic-retry pass above)
through real Codex, synthetic Responses and a private Home. Resource preflight
passed with73.1GiB free. No SQL mutation, installed-Home access or source build.

The two-step Flow's first decision turn calls public `lf flow decide advance`
successfully, then receives a transient error. The same-command automatic retry
completes without calling decide. The command incorrectly exits0 and the Flow
completes. Six HTTP requests retain exact tool output `Decision recorded`; native
history records failed turn seq7, successor start8 and successor success11.
Flow events select5, select8, consume11. Assertion fails: `Flow consumed a decision
written by a failed native turn`. Fixture cleanup completed without an error.

Source explanation: `select_flow_turn` replaces selected_start while
`record_verdict_in` and `record_route_in` persist cursor candidates under the same
Run. Automatic retry retains that Run, so the failed turn's candidate survives.
Correlate or clear navigation on authorized native-turn replacement; keep history,
same-turn contradictory-choice rejection and stale claim/version checks. Router
leakage has the same source shape but is not yet separately reproduced. The
earlier automatic-retry pass remains valid for an ordinary nondecision skill.

Source now clears the failed candidate during native reselection. Supervisor
inspected the tracked fixture and `native-recovery-decision-{missing,replace}-combined`
results on candidate
`894aded0d8c465a971d4006ed6016e2c3ae38bbd1d6eb175100c7cadd473d701`.
Missing-decision retry exits1 with `requires a decision`, retains the cursor with
no verdict, and consumes only the preceding work step. Replacement-decision retry
changes failed Iterate to successful Advance, exits0 and consumes only the first
work completion and successful retry completion. Both retain completed/failed/
completed native history. These public CLI checks close the original decision
counterexample; they do not separately prove XOR router replacement.

`native-recovery-{automatic,both}-combined` also pass on that identical candidate:
ordinary same-command automatic retry and explicit retry after both processes
die. The both-dead proof retains null old Exec outcome/exit and exactly one
consumption. Its fixture server logs a BrokenPipe after deliberate client/engine
loss; the command completed0 and the final recovery assertions all passed.
Real Codex plus synthetic Responses/private Homes remains the proof boundary.

Final source checks: Cut `flow-navigation-tests.log` passes the four focused
recovery/decision tests (4.347s). Cut `flow-recovery-clippy.log` completes all-target
Clippy (16.78s). Cut `canonical-recovery.log` materializes23 drafts in a disposable
source copy and passes five tests (3.998s,32.38s compile): those four plus empty-
draft initialization/upgrade. `canonical-recovery-source.json` retains exact source
hashes. This is focused materialized-schema proof, not populated all-kind import.

Two additional probes
`supervisor-late-decision-retry-{1,2}` tried a delayed child from the failed turn.
Neither reached the intended late decision write: the first hit zsh background
nice refusal; the second left an empty child log. Both correctly stopped for
missing decision. These are inconclusive fixture results, not another product
failure or proof that late commands are fenced. Their original logs remain.

**Late-child counterexample now reproduced.**
`supervisor-late-decision-retry-3/{probe.py,results.json,probe.log}` uses the copied
mechanical candidate `33be09c872e4c82c786237a1b21f23d59c024c33a56a1be247fe95f548182467`,
real Codex and synthetic Responses/private Home. The first invocation stopped
on missing Python websockets before launching; `uv run --script` supplies the
declared environment. The definitive run starts a Python child from the first
decision turn; the child waits for a file written by the retry's tool call.
The first turn fails. After retry selection, the old child calls public
`flow decide advance`, which returns0 and `Decision recorded`. The retry waits
for that child and completes without deciding itself. Flow wrongly completes
with command exit0: native completions are successful4, failed7, successful11;
Flow selects5, then8 and consumes11. The assertion rejecting completion fails.
The child finishes before retry returns; the existing cleanup handles the exact
fixture engines. No SQL mutation, installed Home or configured model service.

Source explains the gap: `record_flow_decision`/`record_flow_path` compare Flow
version and current Run, both retained across automatic native retries. Clearing
the candidate at selection repairs earlier writes but cannot reject this later
old-turn write. Agent publication/navigation conversion must carry the original
native-turn authority to children and compare it to the selected turn; looking
up today's turn for a delayed child would preserve the defect. This is existing
late-writer scope, not a new product object. Verified Task comment
`cae3e281-31e1-4ba0-a757-3dd2cb00c4ee` gives main the reproduction. Earlier passing
native/migration tests keep their scope and do not prove late-child rejection.

**The first caller-token repair rejects valid retry too.** On candidate
`d2791ad2264bde9389b3a9f66b6d44d1cde4d01ac54965b4e7071667b0b6413e`,
`native-turn-caller-late/results.json` rejects the old child and leaves the Flow
waiting for a decision, retaining both outcomes. But
`native-turn-caller-replace/results.json` also rejects the replacement turn's
valid Advance. Main observed that thread resume retains the earlier tool
environment. Supervisor inspected both results: this is not a passing repair.

Supervisor exported the installed Codex0.157.1 experimental protocol schema to
`.lf/tmp/execution-model/supervisor-codex-schema/`. `TurnStartParams` and
`ThreadSettingsUpdateParams` expose no general config or shell-environment
override; `environments` holds environment identity/cwd/workspace roots only.
`ThreadResumeParams` accepts config. Current
[official app-server documentation](https://learn.chatgpt.com/docs/app-server#unsubscribe-from-a-loaded-thread)
describes delayed unload after the last subscription, with a 30-minute inactivity
period; unsubscribe is not documented as immediate reload. The schema is pinned
to the local binary; the webpage is current documentation. Main must distinguish
a native lifecycle solution from added provider machinery before widening this
repair under Jack's simplicity constraint. Original-turn attribution remains the
required behavior; no new product object or alternate success criterion follows.

**Existing native lifecycle experiment is negative.** Supervisor inspected
`native-idle-resume-config-1`: after the failed turn, the thread reports
`systemError`; unsubscribe succeeds and a new client resumes the same thread
with generation 2 configured, but the next tool still prints generation 1.
The assertion fails. Its filename does not establish an idle thread.
`native-interrupt-resume-config-1` then receives `no active turn to interrupt`
when interrupting the completed failed turn, and stops before resume. These
real Codex0.157.1/synthetic Responses/private-Home probes establish those two
failures, not impossibility of every handoff. Their original scripts/results and
Cut logs survive. See [tradeoff review](native-turn-retry-tradeoff.md); passing
store authority tests cannot stand in for the still-failing valid native retry.
The subsequent control `native-success-resume-config-1` passes: after success,
the observed thread is idle and unsubscribe/resume preserves thread/engine while
the next tool prints generation 2. Supervisor inspected the actual results.
This isolates the demonstrated failure to the failed-thread path; it does not
prove a fix, sibling preservation, or configured account continuity.

`native-navigation-store.log` passes nine selected store/Task-driver tests
(6.422s after 32.08s compilation) following the Exec authority conversion.
Supervisor inspected the log and changed fixtures: they supply synthetic native
history and correctly attributed caller Execs. Coverage includes old-caller and
conflicting-decision rejection, exact completion, nested routing and unblock
feedback recovery. These passes precede further selection-capability edits;
they do not establish the real provider's environment transport or all-target
static success for the unfinished conversion.

`native-selection-session.log` passes two focused checks (1.711s): exact native
completion and managed recovery after FlowTurnSelection takes the reserved
AgentSession ID instead of Run ID. Source inspection confirms `select_flow_turn`
no longer joins Run for authority; capability creation and launch publication
still do. `native-navigation-canonical.log` passes four checks (1.591s) after
materializing all 25 drafts into disposable 0.12.25.001_release: unknown caller
origin/selected-history preservation, exact selection, managed recovery and
nested route/decision authority. Source receipt: `native-canonical-source.json`.
`native-navigation-clippy.log` passes all-target Clippy (17.03s). These are
bounded source/store proofs; the caller-environment retry failure remains open.
The source receipt covers 316 files; supervisor comparison found only the later
completion fixture and restart-hint edits changed at inspection. The latter is
wording only. Further implementation needs its own affected verification.

`native-agent-owner-red-2` on candidate
`78feced5b868b753557251a116a0eba3f871ec494ac1c56b5ead51aaa028aa67`
records command success and the automatic-retry/history assertions passing,
then fails its new zero-Run assertion with one retained row. This is a concrete
launch/publication owner-conversion target, distinct from the already-failing
decision retry environment. Supervisor inspected the result and assertion log.

## Retained local proof

**Agent admission: ordinary automatic retry now has zero Run rows.** Supervisor
read `agent-admission-native-zero-run-2/results.json` and the fixture assertions
in `tests/e2e/codex_connect.py`. Candidate SHA-256
`d2cdd28d5bc0ec3bee424d1b40ee598827afee7fa54f9a15713c9ee95520286e`
exits0, completes the standalone Flow, records one failed and one successful
native turn under the same Session/thread/generation/Exec, retains reported usage,
selects history 1→3 and consumes success 6 once. `run_rows=0`; both automatic
retry and owner assertions pass. Real Codex with synthetic Responses and a
private Home; not configured provider, managed Task, Ask/review, populated import
or decision-retry proof. The first invocation stopped before execution because
`websockets` was missing; main repeated with the script's dependencies. Logs:
`agent-admission-native-zero-run.log` and `agent-admission-native-zero-run-2.log`.
The candidate build passed with one unused old helper warning. The conversion
is still dirty and incomplete; later bytes require their own affected proof.

`agent-admission-session-cli.log` first passed one test, then its five Ask-based
cases waited in a fixture query still selecting removed `current_run_id`.
Supervisor identified the shared polling query and sent verified comment
`38689def-dc29-45e6-a8f5-07843aeaa3fe`; main acknowledged it and stopped that
owned fixture group. That run ends with five SIGTERMs, not recovery passes.
After changing the query to `input_id`, `agent-admission-session-cli-2.log`
records **six passed** in 11.796s after 20.78s compilation. Supervisor inspected
the result and test diff: resolution, naming/replacement, saved executable/Home,
waiting-provider launch/resume and prepared Ask checks remain; the Ask adds zero
Run rows and one captured-input reference. These use fixture providers, not a
configured account or complete all-kind migration. A proposed fail-fast change
to the polling helper was not part of the inspected passing diff.

The bounded Task initialization fixture conversion returned partially complete
with exit0 and no builds/tests. It changed review publication/readiness to the
AgentSession input fields, preserving assertions and main's starting dirty edits.
The stale/current Ask fixture remains red at compilation rather than inventing
caller evidence. Supervisor confirmed `task_waiting_unblock` now reads
`manifest.parent_run_id`, propagating a missing-payload error, while admission
and input replacement no longer retain that relation. Verified handback
`9464adc8-3045-4b71-82a0-22b9eaaa1a8d` returns ownership to main and requests
preserving the exact caller relation through the accepted owners. No new product
object or weaker stale-Ask criterion is selected. Log:
`task-initialization-contributor.log`; the source finding is not a runtime repro.

Paths without a directory are under `.lf/tmp/execution-model/`; Cut logs are
under `.lf/tmp/cut-i/`. Native tests use actual Codex with synthetic upstream and
private stores, not configured model/accounts or rendered Desktop.

Later admission inspection confirms the caller relation is now stored as
`agent_session_inputs.caller_input_id`, imported from recorded Run caller IDs;
`task_waiting_unblock` reads it through the Session SQL query. The converted
Task initialization fixture retains stale/current Ask, completion without cursor
movement, and wrong-boundary checks. Its execution remains pending.

`agent-admission-focused.log` completes 73 tests: **71 passed (two leaky), two
failed**, 49.651s after 28.09s compilation. All 15 Chapter tests pass, including
the returned exact Started and `pm_sync`-only second-store adoption proof. Ask
and saved review checks pass within the selected modules. The two failures are
`managed_flow_retry_releases_only_dead_native_selection_and_consumes_its_successor`
and `task_decision_recovery_requires_the_original_successful_run`. Main removed
their obsolete synthetic Run-outcome expectations, retaining selected native
history, cursor preservation and zero Run rows. Supervisor inspected the diff;
this does not establish that native driver-loss recovery works. The first rerun
(`agent-admission-store-recovery.log`) failed compilation on a test-only Store
method call and exercised no behavior. The corrected
`agent-admission-store-recovery-2.log` completes **21 passed, seven failed** out
of 28, with one failed test also leaking. Both preceding controller failures now
pass. This uses store fixtures, not the still-failing real native decision retry.
The seven remaining failures cover populated admission upgrade, claim reservation,
Task invocation selection, headless history, review provenance, constructor
fixtures and stale-driver recovery. The upgrade fails during fixture setup on
the removed `flow_sessions.ready_summary` column, before migration execution;
no populated migration result follows. Main owns their repair and rerun.
The leaky tests are `active_task_invocation_ignores_later_flow_and_skill_edits`
and `comment_sync_recovers_history_and_deduplicates_edits_and_webhooks`.

The seven-case repair (`agent-admission-repaired-store.log`) passed two and failed
five. It exposed an actual Flow-conversation provenance regression; main corrected
the writer to inherit attribution. The migration fixture needed the existing
migration transaction wrapper for table rebuilds, retaining foreign-key validation.
`agent-admission-repaired-store-2.log` then passed all five remaining cases in
0.609s after 29.39s compilation. Supervisor inspected the preservation assertions:
conversation fields, both input/caller references, native history/usage, Flow
history/selection and connection identity retain their recorded values. Historical
Run rows remain import evidence. This proves the populated draft boundary, not
complete filesystem import, canonical repetition or installed adoption.

`agent-admission-cli-recovery.log` is the subsequent CLI pass. The exact
stale/current unblock integration test passes, covering the caller-relation fix.
It completed **17 passes, 12 assertion failures and one SIGTERM** in 158.433s
(five unselected tests skipped). Main stopped the owned Ask fixture group after
29 cases finished because it was polling a removed
`agent_sessions.current_run_id` column while suppressing the SQL error. Verified
comment `a86f9710-5975-4c3c-8a3d-b1066952ba99` gives main the source diagnosis and
bounded fixture correction; `agent-admission-cli-recovery-interruption.txt`
records the stop. This is not evidence of a hung provider. The failed cutover
cases still include launch/inventory/import/bind and the retained native Flow
failures; main is converting their fixtures and preserving the behavioral
requirements, not treating the 17 passes as completion.

`agent-admission-cutover-fixtures.log` now passes all five selected repaired
cases in 11.566s after 20.39s compilation: interactive lifecycle, Ask answer and
replacement, taskless review replacement/completion, SQL-only review inventory,
and repository/task filtering before paging. Supervisor inspected the diff:
retained identity, name/feedback, stale-action and retired-file assertions remain;
fresh admission asserts zero Run rows and uses immutable input references.
Providers are scripts. Other discovery/import/bind and native failures are open.

The bounded Desktop contribution completed exit0 and returned ownership in
comment `8ff3937a-f2a9-4675-af30-657c13cf687a`. Supervisor inspected the five-file
diff and verified handback hashes/whitespace. It preserves typed bound ancestry
and selection without roadmap records, excludes bound Sessions from the orphan
section, and adds unit plus existing mounted terminal-proof coverage. It ran no
build/test. Production delta +70/-11, net +59, excludes tests. Handback with exact
commands and source hashes: `.lf/tmp/cut-i/desktop-ancestry-handback.md`.
Mounted proof execution is required: an unrelated unit layout object staying
unchanged cannot establish terminal retention. Full DTO/history/numeric graph
conversion and configured Desktop acceptance remain separate.

Main has now executed both requested Swift checks on the handback bytes:
`agent-admission-desktop-unit.log` passes **25 tests** (including four planning-loss
variants) in 4.452s after an 11.92s build. `agent-admission-desktop-native.log`
passes the one mounted `namedSessionDrillDownRetainsTerminal` proof in 4.590s.
That proof removes all Tasks and then all Waves before restoring planning,
asserting typed ancestry/siblings, selection, identical surfaces, focus/layout,
and the retained draft/PTY responses. It uses fixture readings and owned test
terminals; no configured live Desktop acceptance follows. Supervisor verified all
five source hashes still match the handback; receipt
`desktop-ancestry-verified-source.json`. CoreVideo was unavailable and the native
proof used its timer-rendering fallback. No extra layout-only claim is needed.

`agent-admission-canonical.log` also passes **four selected checks** after all
26 drafts are materialized as `0.12.25.001_release` in a disposable source copy:
populated admission preservation, fresh schema, retained retry conversation and
exact selected native completion without a Run outcome. Source receipt:
`canonical-admission-source.json`. The admission SQL still matches at inspection;
subsequent formatting and source edits mean this is not a full current-tree gate.
The first static pass (`agent-admission-static.log`) fails on two test-only
`manual_contains` Clippy findings; a final static pass remains required.

| Evidence | Observed result and limit |
| --- | --- |
| `native-flow-live-final/results.json`, `native-flow-loss-final/results.json` | Same Session/thread/history across retry. Live generation 1→1, dead engine 1→2; nested child attribution and one engine. Candidate `4ac6ec1310b509c3ba97884902a5a1f5dc7982da6c08ea540b216a68e98835f3`. No driver-loss or account continuity proof. |
| `native-flow-checkpoint/results.json` | Final checkpoint engine-loss retry: failure then success, same thread/Session, old history, one replacement. Candidate `709b1ca…abdae` above. |
| `supervisor-flow-recovered-origin-1/results.json` | Deliberately remove SQL completion after real failed turn, terminate engine, retry. Old failure restored under original Exec/gen1; new success under retry Exec/gen2; idempotent history. Simulated missed receipt, not actual write failure or recovered usage proof. |
| `supervisor-flow-driver-loss-1/results.json` | Earlier killed-driver/held-turn test also failed missing Run completion. Later completed-before-resume result above is stronger; neither is fixed by engine-loss retry. |
| `supervisor-busy-start-regression-2/results.json` | New client receives existing active turn, preserves original Exec/generation and one completion/usage; strict prior candidate failed. A diagnostic timeout with exit 0 is not a pass. Native turn/start can add input to an existing turn. |
| `supervisor-public-approval-1/` | Old client approval rejected; selected client executes one marker; sibling survives. Native A-close/B-connect replay had another sibling client alive; not zero-client approval continuity. |
| `supervisor-pagination-1/` | 104 exact completions across a 100-item page; 103 unknown origins retained; idempotent. Only latest missed turn has 20/5 usage; lifetime 2100/525 is not allocated to missing turns. |
| `supervisor-zero-clients-1/` | Both lf clients and inspector close; marker written before fresh inspector. Reconnect records one completion and 120/30 lifetime usage; old Exec remains interrupted. Marker proves tool execution before inspection, not whole-turn completion before inspection. |
| `public-live-history-2/` | Exact selected-child ancestry after handoff; stale start/steer/interrupt rejected; old headless SIGINT/130 leaves selected/sibling turns alive. |
| Cut `flow-conversation-and-ci.log` | Four CLI checks pass: failed/retried conversation identity/title, ad hoc/replay writable private stores, refusal before provider launch for inaccessible store. |
| Cut `stored-session-graph-{red,green}.log` | Nested stored node 3 was wrongly emitted as structural key 3; mapper now returns 1/fix/1. Green one test covers nested/post-XOR current and prior occurrences. Numeric Rust/Swift conversion remains due. |
| Cut `canonical-engine.log` | Four passes after 22 drafts materialized as 0.12.25.001_release in disposable source: graph, history/fences, copy authority exclusion, initialization/upgrade. Source fingerprint `canonical-engine-source.json`. |
| Cut `flow-native-final-clippy-2.log` | All-target Clippy passed 14.92s; prior test held async environment guard across await. Not a behavioral suite. |
| Cut `shared-stacking.log` | Three passes, 5.840s: existing-root parent selection retains fork/history/publication, held claim exclusion, local rebase. GitHub simulated. |
| Cut `publication-preservation-3.log`, `integrated-publication-env.log` | Acknowledged identity retained after follow-up read/readiness failure; merge intent ordering and independent parent/PR facts preserved. Original PR1296 incident cause remains unknown. |
| H7 integrated and Cut `native-socket-admission-2.log` | Twelve-mutation two-Home matrix and legacy response-loss matrix pass; archived predecessor comparison fixed timestamp precision and passed; authored-content and fresh-status-conflict regressions pass. No configured Linear rotation or remote-work absence proof. |
| Cut `wal-admission.log`; first-Home receipt `loo298-first-home-execs-1dmf_kwt/receipt.json` under OS temp directory | Three focused checks plus three fresh concurrent CLI pairs pass: each command has start/end, two succeeded Execs, zero agent Runs, no warnings. Earlier pair lost start on concurrent WAL negotiation; retained failure remains history. Six commands do not prove arbitrary fleet load. |
| Cut `canonical-materialized-repair.log` and related `canonical-{init-red,init-repair}.log` | Canonical nested-transaction RED repaired; seven materialized tests include CI failures and populated backup. Original receipts in archive. |
| Cut `import-started-and-rebase.log` | Three checks pass, including import/dry-run retaining Started for originally unbound Runs and original ancestry/bind timestamp. Synthetic history; not complete importer proof. |
| Rebase DTO/Swift batch | Corrected obsolete H7 Project-history rejection; ten DTO/history and 47 Swift checks passed, including 101-Session inventory. No rendered pane/draft retention proof. |

## Measurement limits

Old empty-inventory paired CLI measurement: first initialization 3.668→2.370s;
subsequent fresh-process median 2.354→0.704s (four repeats, interleaved order).
Other integrated changes differ, so no isolated causal claim. No dense discovery,
Desktop latency or final-tree performance acceptance follows. Script
`.lf/tmp/measure-schema-cli.py`; exact receipt path and binary hashes in archived
`parallel-performance.md`.

Parsed-command probe `command-observation-cpxkch7g/receipt.json` records succeeded
list Execs and failed rename Exec; help/version/argument errors emitted no Exec.
Early paths 9–11ms, empty listing about 752ms in one diagnostic sample. Admission
coverage remains unfinished. A separate four-test batch had a Nextest LEAK;
passing assertions are not clean process settlement.

Current production count is in [control](parallel-work.md); it excludes working
edits. Preserve historical failed attempts and measured missingness when updating
this ledger. Passing an isolated replacement check never upgrades unchanged gaps.


## Command/provider and mounted CI repair (2026-09-29)

Supervisor's hosted 59ed94ea5 failures exposed two generic fixtures still naming
OpenCode while emitting one-shot Claude result JSON. Generic batch/research
fixtures now select Claude; replay uses the existing native OpenCode HTTP/SSE
fixture and records the actual submitted body, input and parent. Input roundtrip,
account-free replay, environment/admission/usage and research artifact assertions
remain. Interactive OpenCode fake scripts retain their separate TUI contract.
No production provider path was restored or changed.

The requested no-fail-fast command/provider run collected 54 cases: initially
52 passed, with replay's missing recorded managed account after a trial Claude
conversion and a native-stop fixture switching DB after Session admission. A
second run passed 53, retaining replay's absent private account Home failure.
Replay then returned to the existing account-free native fixture: 1 pass/1.105s.
The stop fixture now retains its original store. Shared native decision and
ordinary automatic retry proofs pass 2/2 in 7.536s. Logs are
`.lf/tmp/cut-i/command-provider-fixtures{,-2}.log`,
`command-provider-replay-native.log`, and `command-provider-native-shared.log`.
The initial run's LEAK annotations remain recorded; the second run had none.

SessionChrome now checks the selected Session workspace chip and the Task-only
location after clearing selection, retaining both mounted sizes, controls and
real terminal/draft/companion assertions. The unchanged Monitor test passed
locally. Source shows its initial asynchronous observation was assumed after a
100ms delay; the test now waits for the expected attributed snapshot before
Chapter transfer, retaining the history assertion during both transfer stages.
This explains a possible initial-observation race, not a reproduced production
loss. Final focused mounted run passes both tests (two chrome sizes), 4.683s,
`desktop-ci-proof-final.log`. No production Swift change was needed.

Review: removed fixture-only database drift; avoided expanding replay into
managed credential setup; kept protocol-specific lifecycle proof in the native
fixture. This cut changes zero production lines. The preceding local resume
entry deletion is included in publication. Supervisor's metadata proposal is
released but unapplied; its dense correlated-scan finding and importer
surface/closure cases remain the next implementation work. No full CI success,
configured acceptance, promotion or completion follows.
