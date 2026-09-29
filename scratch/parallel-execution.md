# Main implementation handoff

LOO-298 · 2026-09-28. Main owns executable edits, builds, Git and this handoff.
Supervisor owns the other compact scratch documents. Read the full
[contract](data-model-one-table-per.md), [remaining scope](remaining-work.md),
[import obligations](import-preservation.md) and [evidence](evidence.md).

## Implement iteration 10 · 2026-09-28

Preserved the complete returned edits and review notes in a local checkpoint after
`implement-preserve-static.log` passed formatting and all-target Clippy (20.18s).
Pinned installed `lf rebase --plan` selected direct rebase; `lf rebase --manual`
completed without conflicts. HEAD is `884b90be9`, base `b42d53205`, zero behind
observed origin/main. No publication or Flow navigation followed. Session inventory
refresh succeeded through the captured installed executable/data pair.

The integrated materialized diagnostic completed: 1,925 passed, 33 failed,
15 skipped. `implement-materialized-matrix-2.log` retains the complete first
matrix; `implement-matrix-source.json` identifies its unfinished-owner snapshot.
Its ordinary source copy lacked Git metadata. The corrected isolated checkout,
created through `lf wt`, and removal of inherited `LF_DB_PATH` cleared 13 of 15
setup-sensitive cases. The five account-first cases passed together; the evidence
does not isolate which setup correction mattered. No full matrix was repeated.

Focused repairs now pass the three populated canonical migration cases, all 11
status tests returned by the contributor, and the affected command, import,
publication and fixture cases. The latest three-case check has three assertion
passes but one Nextest LEAK on failed draft promotion; process settlement is
unproven. The managed topology/restart test passed in its disposable Docker OS
fixture. Logs are `implement-setup-recheck`, `implement-quoted-output-red-2`,
`implement-focused-repairs`, `implement-final-fixtures` and
`implement-managed-topology` under `.lf/tmp/cut-i/`. No configured provider,
installed-Home migration or full-green claim follows.

The observed quoted-output case first reproduced the Task capability blocker.
Task settlement now consumes provider/command outcomes from their owners and
preserves tool events without interpreting quoted command text as authority.
The existing driver proof passes with the failed inspection and failed/retried
Git command retained. Genuine provider failure and the actual failed CLI commit
remain separate passing checks. Existing draft promotion now succeeds before
publishing ready copy; failure retains both local and remote draft copy.
Production delta against `884b90be9` is +8/-85 (net -77), excluding tests/docs.
`implement-repairs-static.log` passes fmt, all-target Clippy, Ruff and diff checks.

Three original matrix failures remain in native execution: OpenCode decisions
lack original-turn attribution (two cases), and managed retry lacks confirmed
engine exit/connection evidence (one). The retained valid Codex decision retry
and ordinary zero-Run CLI proofs remain red independently. Next work is the
selected complete AgentSession admission/publication and SQL-reader conversion
through ordinary launches, TaskLauncher, reviews and Asks. Native transport is
separate. These diagnostic repairs do not complete the implementation slice.

### Admission checkpoint and remaining conversion

The new `agent_session_admission` draft moves captured input/publication metadata
onto AgentSession, removes its Run FK and its SQL reader's Run join, and retains
immutable input references as subordinate history without outcomes or ordinals.
Ordinary capture and Flow reservation/publication write the conversation directly;
review/Ask consumers now read it. This is retained unfinished implementation,
not a completed slice. Historical Run import and test consumers still need
conversion, populated preservation and saved-answer/refused-publication proof.
The new empty schema replay passed; it is not populated migration proof.

`agent-admission-build.log` builds the real CLI. Candidate SHA256
`d2cdd28d5bc0ec3bee424d1b40ee598827afee7fa54f9a15713c9ee95520286e`
passes the formerly red ordinary automatic-retry target in
`agent-admission-native-zero-run-2.log` and
`.lf/tmp/execution-model/agent-admission-native-zero-run-2/results.json`:
zero Run rows, failed then successful native turn, retained 40/10 token usage,
one Session/thread/provider generation/Exec, selected events 1/3 and consumed
successful event 6 exactly once. Real Codex uses synthetic Responses and a
private Home; no configured-provider acceptance follows. The first harness
invocation lacked its `websockets` dependency and never ran the fixture;
`uv run --with websockets` corrected setup. Native decision retry remains red.
Later source edits have removed the unused Run selector and dead-driver outcome
fabrication; those edits postdate the candidate and need focused validation.

Admission now has populated source and canonical preservation proof. The four
checks in `agent-admission-canonical.log` pass after all 26 drafts are materialized
in an ordinary disposable source copy; `canonical-admission-source.json` records
the exact source hashes. They preserve existing Session fields, native/Flow
history, immutable input/caller references and historical Run evidence. That
copy has no Git metadata and proves only the selected migration/store cases.

The focused 73-case run passed 71 and exposed two obsolete outcome assertions
(two passing cases also leaked). The separate 28-case store/migration run passed
21 and exposed seven issues, including real Flow provenance loss. Correcting
inherited provenance and preserving unknown driver outcomes, plus repairing
fixture construction, produced 2/2 then 5/5 focused passes. All Chapter cases
passed, including exact non-null Started and second-store sync-only adoption.
Those use operations and simulated provider data, not public CLI/default launch.

The first CLI recovery run retained 17 passes, 12 assertion failures and one
owned Ask fixture interrupted after 158 seconds. Its SQL poll suppressed a
removed-column error; the corrected helper retries only absent rows/busy locks
and terminates its owned child on other failures. The five affected cutover
cases now pass in `agent-admission-cutover-fixtures.log`: interactive reopen,
Ask answer recovery, saved review replacement, SQL-only review discovery and
scoped inventory. The earlier Session CLI suite passes 6/6. The current exact
stale/current Ask status proof also passes using SQL caller input identity, with
no artifact payload prerequisite. Review completion now checks the selected
input directly on FlowSession; it no longer calls the Run module's fence.

Still incomplete: import comparison/preservation and legacy Run discovery,
usage/activity, final wire/history conversion, all-provider recovery and native
decision retry. The CLI failures in these areas remain evidence, not fixture
successes. The returned Desktop changes pass all 25 WorkspaceNavigation tests and the
one mounted named-Session terminal proof (4.590 seconds). The latter removes and
restores planning while retaining surfaces, focus, siblings, layout and draft
text; Ghostty used timer rendering because CoreVideo display-link creation was
unavailable. This is fixture-mounted behavior, not configured Desktop acceptance.
Active-process attribution now reads the owning AgentSession, avoiding a missing
Task after zero-Run admission. Review completion and corrupt-neighbor/replacement
checks pass after their last Run-module settlement fence was deleted. Ask launch
also reads its caller input from SQL rather than reopening its own input payload.
The checkpoint candidate `88b2b834d230a644fb15ddd01c7de482645c9753992218bf87d467002bec7eae`
repeats the public automatic-retry proof with zero Runs, retained failure/success,
40/10 usage and exact single consumption in `agent-admission-native-checkpoint`.
Formatting and all-target Clippy pass (`agent-admission-static-2.log`, 15.50s);
the earlier static run caught two test-only manual-contains lints, now corrected.
The pinned rebase plan selects direct rebase. No full matrix repeat or completed
slice follows. Historical discovery/import and all-provider/native decision
recovery remain implementation work after this admission checkpoint.

Admission checkpoint `ddf4a2ddc` was rebased through pinned `lf` onto main
`5402d93974dba3b6e94bea1d411951a9dfeb20d9`, producing `95b1dd685`.
All returned Chapter and Desktop edits are preserved. Integration retains
upstream Task file operations, the current skill catalog and compact Wave
operation guidance, while keeping the accepted status-based Chapter contract.
Deleted thread/Wave-host readers remain deleted. The recent historical SQL
reader now filters the inclusive time window before hydrating payloads; its
regression retains boundary usage and partial evidence and excludes an older
corrupt payload. This remains a historical Run reader, not final owner deletion.

Post-rebase checks pass: one historical-window case, 25 Rust DTO/context cases,
eight Wave-detail cases, the mounted named-Session terminal-retention proof,
and 25 Python architecture/skill checks. The latter first found an omitted
`agent_session_inputs` map entry and archived rebase transcripts scanned as
active guidance; the map and archive exclusion are corrected, with rejection
of retired vocabulary in active skills retained. Logs are
`admission-rebase-{window,contracts,wave-detail,native,docs-2}.log`.
Formatting, Ruff, whitespace and all-target Clippy pass (15.76 seconds,
`admission-rebase-static.log`). No full matrix repeated. The zero-Run native
candidate predates rebase; its admission/authority source remained identical,
but it is not a fresh integrated native binary proof.

The next complete owner boundary is historical input import and indexed
conversation/history discovery, preserving original attribution and replay
conflicts before removing historical Run rows. Native transport stays separate:
retain paired stale-child rejection and valid-retry failure, and the smallest
concrete proposal requirement in `native-turn-retry-tradeoff.md` before expanding
provider machinery. This checkpoint does not complete iteration 10 or LOO-298.

Publication reached `f60fcb3ded` on base `5402d9397`, with matching GitHub/Task
head and no auto-merge. Main then advanced to `7c2f53b9e`; pinned `lf` rebase
integrated its ownership-before-payload behavior without restoring manifest
selection authority. The current SQL query already selects Work and exact caller
before hydration. Focused proof retains inherited Task identity after Project
rename, 56 exact children beyond the presentation cap, inclusive dates, usage,
partial evidence and activity's start-or-finish window. Two checks passed first;
the third exposed reliance on ambient database initialization in its fixture.
Explicit ephemeral-store creation fixes that setup and the single rerun passes
(`admission-rebase-1337` and `admission-rebase-1337-window`). Historical readers
remain transitional; these results do not cover newly admitted zero-Run history.

## Decision pass 10 · 2026-09-28

Reassessment after the completed operational unblock in
[unblock-quoted-output.md](unblock-quoted-output.md) retains Iterate. The corrected
inspection path and completed review are readable; the pinned installed Session
inventory succeeds and retains the control conversation's `loopflow` title.
All 24 source hashes in `review-slice-source.json` still match. This is the same
decision boundary, not another failed implementation pass or new product approval.
The classifier repair remains required; repeating the failed inspection or opening
another Ask would supply no new evidence. No build or behavioral suite ran here.

Iterate to implement under the current three-owner contract. The older incoming
direction's Run-based fences and archived note paths are superseded by that
contract and the compact notes. Meaningful progress includes native-selection
authority moving to Session/Exec evidence and the negative failed-thread reload
experiment, which isolates a transport limitation; it does not establish a working
retry repair. This decision reread the three slice receipts and verified all 24
source hashes unchanged. Valid decision retry and zero-Run agent admission remain
red. No build or behavioral rerun was needed to judge this boundary.

Next complete admission/publication and Session readers on their final owners,
including TaskLauncher, ordinary launches, saved reviews and keyed Asks. Require
the retained ordinary automatic-retry CLI proof to preserve failure/success,
usage, conversation and exact consumption with zero Runs; pair it with failed
publication before provider effects and saved-answer/review recovery. Preserve
the full remaining-work/import/Chapter matrix. Separately repair the obsolete
Project-status assertion, preserving the Task-status and obsolete Project
reason/time checks. Supervisor refinement `ac8e73dc-b16f-4541-b1f8-3ea8187e56c9`
replaces the separate migration-module rerun: after owned upstream integration,
run one Rust matrix without fail-fast in a disposable materialized source snapshot,
including migration coverage. Record whether that snapshot includes unfinished
owner edits, retain every failure, and avoid repeating the full matrix after
each small repair. This diagnostic cannot close the public native or zero-Run
failures by itself. Preserve local work through the queued
serialized lf integration; publish only coherent verified changes. Native
expansion still requires the concrete cost/proof review in
native-turn-retry-tradeoff.md; no retry-contract change or provider fork is selected.

Supervisor comment c9c25a86-051f-4314-8054-194180846866 adds an observed settlement
counterexample. This decision directly read retained Run
run_fe7ae2c0bf9941caa2550d950e4f3d62 events.jsonl, line 190 / seq 189:
ItemCompleted command exec-2514c49a-b870-45a2-9574-7548275b6317 failed with exit 1
on `AttributeError: 'list' object has no attribute 'get'`. Its earlier stdout
quoted saved fixture output containing `operation not permitted: ps`.
controller/task/mod.rs::completed_boundary_failure searches that entire output;
execution_blocker_at_handoff turns the collected match into a capability blocker.
The supervisor reports the resulting Task stop and successful corrected-reader
resume; current source corroborates the classification path. During this decision,
an initial read also exited 2 solely because the Run directory was addressed
without its `fe/` shard; the corrected exact-path read succeeded. Neither read
failure establishes unavailable process-control capability.

Carry this quoted-log case into the existing Task/Exec settlement conversion:
retain the actual command failure, but derive capability/control consequences
from their owning outcomes, not arbitrary quoted output. Prove that this exact
inspection failure cannot block a completed review as a capability failure,
while genuine delivery/provider failures remain represented. Do not add a
command-name allowlist, string-exception parser, lifecycle or separate Task.
This decision retains evidence only and changes no executable source.

Compression iteration 9 removes the Run-store location helper and two navigation
write helpers: Flow transactions reuse their loaded capture/cursor, preserving
the shared caller check. Three history paths and two navigation paths no longer
reread captures; dead Op branches in agent reservation are deleted (+77/−94,
net −17 production lines against saved pre-compression bytes). `compress-flow-owners.log`
passes six owner checks; `compress-flow-consumers.log` passes stored nested/post-XOR
membership and both mechanical CLI checks (three). No schema, wire or native
transport changed; valid native decision retry and zero-Run agent ownership
remain RED, and the full remaining-work matrix still applies.
Formatting, diff checks and all-target Clippy pass (`compress-flow-clippy.log`).

The requested isolated `ops::pm::` no-fail-fast pass reused
`loo298-completion-fixtures-h40t1al7` and its build: `compress-pm-module.log`
records 44 passed in 17.313s, including one Nextest LEAK on
`linear_oauth_cancelled_lock_waiter_never_exchanges`. No assertion failed; this
does not prove clean process settlement or classify the leak as the known
SQLite-contention flake. Its source remains published `0131ed763f` plus the
completion-fixture overlay recorded in `completion-fixture-source.json`.

The later CI70e8 hierarchy fixture now assigns its second Project to its second
Wave, retaining matching/mismatched selector assertions. After verifying the
isolated Rust files against `70e8db9c6`, only `ops/run.rs` was overlaid;
`compress-run-fixture.log` passes all six `ops::run::` tests without fail-fast
(2.825s), formatting and all-target Clippy (17.41s). Source receipt:
`compress-run-fixture-source.json`. The fixture fix alone was subsequently
published as `fb100f486` during the review below. Compression and the native
conversion remain uncommitted; no installed-Home change or native acceptance follows.

## Current failing assertion and retained edits

Published HEAD is now `70e8db9c6`, based on main `1dce02734`. The prior rebase
at `594c7c319f` changed only upstream delivery/schema-cache files. Two schema-cache checks passed.
The mechanical implementation and native recovery bytes were unchanged.
Hosted job109231281578 passed all five disposable-OS tests, including the
managed operation Started proof (6.52s), closing the earlier local Docker gap.
This does not prove configured agent recovery. Supervisor owns the full ledger.

Jack requested bounded original-turn attribution and one shared navigation check.
The current edits carry one harness-created caller token through AgentCaller into
the child Exec and selected Flow event, replacing navigation's current-Run check
for decide/route/blocked. No new attempt object or runtime supervisor is introduced.
Native input remains the existing thread config, but that transport is inadequate:

- `native-late-decision-red.log` reproduces acceptance of a failed turn's delayed
  child on the prior candidate. The tracked fixture retains that public assertion.
- Candidate `d2791ad2264bde9389b3a9f66b6d44d1cde4d01ac54965b4e7071667b0b6413e`
  passes `native-turn-caller-late`: old child exits1, retry has no verdict and blocks.
- **RED** `native-turn-caller-replace`: the same candidate also rejects the valid
  successor Advance; its tool environment retains the first turn's token. The
  Flow blocks instead of finishing. The earlier late-child pass is insufficient.
- Local Codex0.157.1 experimental schema exposes no shell-environment override on
  TurnStartParams. ThreadResumeParams accepts config but the actual retry did not
  update its loaded thread. `native-turn-environment-1` observed CODEX_THREAD_ID
  and no CODEX_TURN_ID; resolving a child against today's turn would repeat the bug.

Logs are `.lf/tmp/cut-i/<name>.log`; native raw results are under
`.lf/tmp/execution-model/<name>/`. Current test callers use Exec authority. `native-navigation-store.log` passes
nine selected store/Task driver tests. `native-selection-session.log` passes
exact native selection and managed recovery after replacing FlowTurnSelection's
Run id with the reserved Session id and deleting select_flow_turn's Run join.
`native-navigation-clippy.log` passes all-target Clippy. These checks do not
repair the native retry environment.

The consultation `ask_2e735a344f6c42e28a9b92d670b2d822` is completed. Supervisor
selected bounded investigation under Jack's existing autonomy; no new Jack
approval or retry-contract change follows. The experiment is negative:
`native-idle-resume-config-1` observes systemError after failure and retains the
old tool environment after unsubscribe/resume; `native-interrupt-resume-config-1`
stops at no active turn. The successful-idle control
`native-success-resume-config-1` refreshes the environment with the same thread
and engine. Pinned Codex source permits this reload only for Idle. No shared
sibling preservation or repaired failed-turn retry has been proved. Keep both
stale-child rejection and valid retry acceptance as integration requirements.
No Run fallback, engine replacement or Flow navigation is selected.

The smallest native proposal is to extend Codex's existing unloaded-subscriber
reload eligibility to a completed failed thread, while preserving its no-active-
turn and no-subscriber requirements. This uses its existing shutdown/resume
operation, not another Loopflow lifecycle. It requires an upstream provider
change and a same-engine sibling proof; neither is implemented here. A Loopflow
shared-engine replacement is not selected. Independent store conversion continues.

Independent CI fixture repair: four files supply realistic started status and
current Flow content in Project payloads, retaining legacy reteam content and
comment-without-placement assertions. `project-fixture-contract.log` passes five
focused tests against published HEAD plus only those fixture overlays. It does
not validate unfinished navigation. Static validation and the separate fixture publication are complete below.

## Published checkpoint and next dependency

Docs-first, metric-planning, native recovery and mechanical-owner work are in
published history; their scoped proofs and unresolved obligations live in
[evidence](evidence.md) and [remaining work](remaining-work.md). Reuse unchanged
proofs. Mechanical operations retain unknown effects without inventing outcomes;
hosted disposable-OS coverage closes the local Docker gap, not configured agents.

Fixture-only checkpoint `0131ed763f38a687e3bd473f22e9500aa5c96ce6` is pushed to
PR1296. It changes four fixture files only. The exact published-head source copy
plus those overlays passes five focused Rust tests, cargo fmt, all-target Clippy
and Ruff. GitHub readback reports that head, MERGEABLE and auto-merge null;
CI36515601936 is queued at observation. This is not a hosted passing result.
`lf commit --no-add --push` preserved unfinished navigation and contributor files;
`lf pr publish` would have staged them automatically. Existing PR copy is unchanged.

Current focused command: `native-navigation-canonical` materializes the full
migration graph in an ordinary disposable source copy and runs four selected
checks: populated caller-origin preservation, exact native selection, managed
recovery and nested route/decision authority. All four pass; source hashes live
in `.lf/tmp/cut-i/native-canonical-source.json`. No additional worktree is created.

The public new-contract fixture `native-agent-owner-red-2` fails exactly at
`run_rows == 0`: actual CLI exit0, Flow completed, selections1/3 and consumption6,
failed→successful history in one Session/thread/generation/Exec, but one Run row.
Candidate SHA256 `78feced5b868b753557251a116a0eba3f871ec494ac1c56b5ead51aaa028aa67`.
This real Codex/private-Home/synthetic Responses proof establishes preserved
ordinary retry and a failing ownership requirement, not valid decision retry.
The first invocation stopped before product execution because the Python
websockets dependency was missing; `uv run --with websockets` reached the result.

Current production delta from `0131ed763`: Rust +112/-84, SQL +9, net +37;
`.lf/tmp/cut-i/native-navigation-line-counts.json` excludes test modules/helpers,
docs and fixture scripts. No path is deleted. Navigation's Run authorization
helper calls, CLI active-Run lookup and selection's Run join are replaced by
original-turn Exec attribution and the existing Session/Flow fences. Run launch
publication, creation and history readers remain; this is unfinished conversion.

Next replace Run-backed launch admission/publication and Flow current_attempt
projection through the shared driver. The navigation writes and selection check
now use Exec/AgentSession/FlowSession, but capability creation still reads the
published Run and Flow selection still projects its publication flag. Preserve
native recovery, completed keyed feedback and mechanical unknown effects while
removing those remaining writers. Keep valid native retry RED; no accepted repair
or complete owner removal follows from these local store passes.

Supervisor reviewed and returned the eight builtin Chapter Markdown edits;
main now owns integration. The contributor ran no builds or live planning writes.
CI0131ed exposed duplicate Project insertion in two completion fixtures. The
repair reuses the Project synced by task_create. The first no-fail-fast batch
passes ten/twelve and exposes an unsupported ProjectOwnership read in deletion
recovery. The fixture now retains the omitted predecessor as Completed and names
the successor separately. `completion-fixture-contract-2.log` passes all twelve
planning tests, cargo fmt and all-target Clippy on published source plus the one
fixture file. No production uniqueness or parser change. This is published as
`70e8db9c6c5a717862fa5927ac133e11fb721fa6`; GitHub confirms MERGEABLE, auto-merge
null and CI36516387300 queued. No hosted green claim. The retained ops/task.rs
restart hint now requests planning sync rather than a deleted chapter command;
that text edit is not included in the fixture-only publication.
Main retains executable/test/build/Git ownership; scratch ownership is unchanged. No
landing, auto-merge, promotion or saved Flow navigation is authorized.

## Control and proof discipline

Use captured installed control in [parallel-work](parallel-work.md). Never run
source drafts against installed Home. Saved feature invocation remains
`1f9ba70e-f0e9-41d4-9722-948d7bc4ce8c`, compress iteration9; no restart selected.
Single build slot; disposable proof Homes, inherited LF/LOOPFLOW authority scrubbed,
four Cargo workers/nice +10. `.lf/tmp/cut-i/run.py` owns proof environment/logs.
Canonical tests materialize only a disposable source copy. Native fixtures copy
the candidate before executing. Do not restore recursive archived scratch logs.

## Slice review · 2026-09-28 · iteration 9

**Disposition: incomplete; not accepted as a cutover.** Review covers the working
navigation/caller conversion, compression and returned Chapter guidance against
[the accepted contract](data-model-one-table-per.md). The full remaining scope in
[remaining-work](remaining-work.md) and [import preservation](import-preservation.md)
still governs. This review chooses no Flow edge and grants no Task completion.

The invariant is: a navigation command must retain the identity of the native
turn that issued it, and only that selected turn's successful completion may
supply the Flow's decision. The shared store check enforces this when supplied
correct caller evidence; the actual failed-thread retry does not transport fresh
evidence to its tools.

### Fresh public evidence

Resource preflight passed with 65.5 GiB free. A current-tree CLI build passed in
23.15s. All three tracked native probes below copied the same candidate,
SHA-256 `66f410dd9019612fa163fdd41c56219a594d32660d67798b2f70d9c7677d0280`.
They use actual Codex 0.157.1, synthetic localhost Responses and private Homes;
they establish neither configured-account nor Desktop acceptance. No fixture
reported a cleanup error. Logs are under `.lf/tmp/cut-i/review-slice-*.log`;
raw results are under `.lf/tmp/execution-model/review-slice-native-*/results.json`.

Command shape (each executed separately through the isolated proof runner):

```sh
uv run --script tests/e2e/codex_connect.py \
  --codex /Users/jack/.codex/packages/standalone/releases/0.157.1-aarch64-apple-darwin/bin/codex \
  --lf target/debug/lf --launch <case> --output <private-result-directory>
```

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Reject failed-turn descendants | Old child cannot supply the retry's decision | Child exits 1 with original-caller authority error; Flow remains unresolved, only earlier work consumed | `--flow-decision-retry late`, suffix `late`, harness exit 0 | Pass within fixture scope |
| Accept legitimate automatic retry | Retry chooses Advance and completes once | Retry tool exits 1 with the same authority error; provider succeeds but Flow exits 1 requiring a decision; selections 5→8, no consumption of 11 | `--flow-decision-retry replace`, suffix `replace`, harness exit 1 | **Gap: reproduced** |
| Remove Run from agent execution | One Exec and AgentSession history own the work | Ordinary retry completes, selects 1→3 and consumes 6; failed/successful history and usage survive, but SQL still contains one Run | `--flow-automatic-retry`, suffix `owners`, harness exit 1 at zero-Run assertion | **Gap: reproduced** |
| Preserve mechanical execution | Several operations share one Exec; uncertain effect requires inspection/retry | Compression changes occurrence lookup, retaining Flow history writer | Prior three consumer/six owner compression passes; canonical, crash and hosted managed proofs in evidence.md | Scoped passes reused; no fresh mechanical claim |
| Consolidate runtime owners | No reachable replaced writer/reader | `FLOW_SELECT`, `flow_turn_selection`, `publish_attempt`, Session inventory and constructors still use Runs | Source inspection of sqlite/flows.rs, sqlite/sessions.rs and store/sessions.rs | **Gap** |
| Preserve historical imports | All origins and conflicts retain exact evidence | ID-only Session no-op remains; non-review/finished taskless captures are skipped; `declared_work` still turns resolver errors into absence | session_import.rs `store`, `flow_review`, `declared_work`; import-preservation.md | **Gap; source findings retained** |
| Fast complete discovery | Indexed summaries before payload IO, measured dense fixture | Session SQL filtering exists but joins Run; no complete public Exec summary/detail path or final dense receipt | sqlite/execs.rs, sqlite/sessions.rs; retained discovery audit | **Gap** |
| Desktop and wire | Bound off-roadmap Task and pane/history survive | Breadcrumb resolves through current Wave/Task projection; unmatched Session returns nil Task; final history/numeric DTO conversion remains | WorkspaceProjection.swift:80; remaining-work.md | **Gap; no rendered environment** |
| Chapter/default Flow | Public second-Home sync, exact Started retention, actual default launch and final populated upgrade | Returned eight skills now describe Project/status rotation; these four proof obligations remain | Source/skill review; fourteen hosted Chapter passes retained in chapters.md | Partial proof; **gap** |
| Integrated verification | Affected final-tree checks, canonical import and hosted matrix pass | Current native build passes; prior scoped compression/static checks remain valid within their recorded source; hosted Rust has not completed green | Build log, retained receipts, new hosted run below | **Gap** |

### Architectural findings and next complete change

1. **P1 — valid decision retry is broken.** The harness creates a new token at
   codex.rs:1098 and puts it in thread configuration. The retained failed thread
   executes tools with its prior token; flows.rs `require_turn_authority` correctly
   rejects that evidence. Accepting today's token for the old child would restore
   the reproduced stale-write bug. Keep the paired late/replace public tests.
   The negative native lifecycle experiments and smallest upstream proposal remain
   in native-turn-retry-tradeoff.md; no provider fork, shared-engine replacement,
   retry-policy change or additional lifecycle is approved by this review.
2. **P1 — the agent ownership cut is unfinished.** `publish_attempt` still writes
   `runs.published`; `flow_turn_selection` still obtains its capability through
   that row; Session list/detail still join `current_run_id`. Removing a Run join
   from selection alone cannot satisfy the zero-Run consumer proof. Next complete
   boundary is agent reservation/publication through TaskLauncher and saved launch
   callers, then Session projection/history on their owners, deleting those Run
   dependencies together while preserving immutable launch input and exact history.
3. Import, discovery, Desktop and Chapter gaps above are retained acceptance
   obligations, not waived by this smaller review. Do not delete historical inputs
   before conflict/idempotence/interruption and final canonical preservation proof.
   Existing publication/stacking repairs keep their scoped evidence; cancellation
   remains disposed through LOO-305 with process settlement separately unproved.

Compression deletes `location_in`, `record_verdict_in`, `record_route_in`,
`require_attempt_authority` and CLI `active_run_id`; their responsibilities move
to captured-graph location and one shared native-turn check. No source path or
Run product owner is deleted in this pass. Against published `70e8db9c6`, the
current Rust production prefixes plus the untracked caller SQL draft are
**+193 / −173, net +20**. Tests (including the test-only impl), docs and generated
files are excluded. Receipt `.lf/tmp/cut-i/review-slice-source.json` records paths,
method and hashes. This is the entire working conversion since that checkpoint,
not the narrower compression's separately verified +77/−94, and not a new
whole-branch deletion claim. The mechanical pass replaced an owner; the current
agent pass has not. This is progress toward the design, not a coherent finished slice.

### Authorized fixture-only publication

Jack's relayed direction in comment `be418650-8459-4be9-ba12-055080ae24df`
authorized publishing only the hierarchy fixture repair after review. Its exact
file SHA matches `compress-run-fixture-source.json`; the recorded six module
tests, formatting and all-target Clippy pass on published source plus that overlay.
The matching/mismatched selector assertions and production uniqueness remain.

Only `rust/loopflow/src/ops/run.rs` was staged. Captured installed
`lf commit --no-add --push` produced **`fb100f486cf69d03bd87a6fda748c2c75588a906`**
(one insertion/one deletion). Remote branch, GitHub PR1296 and the refreshed Task
publication agree on that head; GitHub reports MERGEABLE and auto-merge null.
CI **36517146691** was in progress at readback; no hosted pass is claimed.
The native conversion, caller draft, Chapter handback and scratch remain local.
No source binary accessed the installed Home. No rebase, PR-copy mutation,
installation, promotion, landing or Flow navigation was performed by this review.


## Import implementation continuation · 2026-09-28 · after b884feeef

Main retains the complete remaining-work contract. The imported-evidence draft
now preserves `session_events` sequence numbers, selected/consumed Flow references,
and all three affected Started/history triggers. The first populated proof found
SQLite rejecting a rename while `validate_task_started_update` referred to the
removed table; that trigger is now recreated too. The next canonical pass reached
a duplicate-worktree fixture seed. An attempted overlay then erased the generated
migration registry: its missing `execs` failure is fixture setup, not production.
Recreated exact-source materialization passes the populated test (one test,
`historical-materialized-final.log`, source receipt `canonical-history-source.json`).
The preceding complete-chain test passed in `historical-materialized.log`.

`historical-attributed.log` passes the observed-history Rust DTO and completed
keyed-Ask recovery; the multi-origin case exposed the import writer supplying a
redundant Task Flow cwd. That writer now uses the existing Task-owned cwd contract.
The public multi-origin proof is still under repair; its dry run now resolves
same-batch captures from parsed input without durable writes. Do not claim its
membership, Started/report equality or replay assertions passed yet.

The returned Chapter CLI fixture now requires a nonempty PR list containing
GitHub number 17. Ruff passes after formatting (`chapter-static-final.log`). It
has not executed on Linux; no public Chapter/default-launch proof follows.
The native valid-decision retry, legacy multi-member import, final owner readers,
and other obligations in remaining-work/import-preservation remain open.

The later `historical-attributed-4.log` passes the public multi-origin proof.
`historical-replay-final.log` repeats only that changed case with the additional
conflicting-terminal assertion: pass, retained failure history unchanged.
The proof compares fresh-Home dry-run/applied classification, no preview writes,
exact existing Started, same-batch capture/member resolution, retained past
Flow/node/iteration after cursor advancement, seven unchanged replay inputs,
zero Run rows, both Session inventory modes, and no provider launch. It removes
mutable source sidecars and retains public lists. This replaces the pending
judgment above without erasing its failed attempts. Historical membership is
checked through Session/Flow owners; legacy `runs` reader conversion remains open.

`historical-history-swift.log` passes one actual Swift Testing observed-history
DTO test. `historical-import-static.log` passes all-target Clippy in 15.99s;
`cargo fmt` and scoped diff whitespace pass. Unknown provider thread/turn remains
nullable in the shared Rust/Swift fixture. The supervisor approved publishing this
coherent import checkpoint and the three setup repairs under existing authority
(comment c3c6db65); this is supervisor direction, not new approval attributed to Jack.
Next boundary: historical multi-member inputs on one AgentSession and final SQL
readers, retaining every original failure, usage owner and exact Flow reference.


Checkpoint `275589beeae2f6e2d439cc3642c10b95c02942f1` is published on PR1296;
GitHub and Task publication agree and auto-merge is absent. The Task base remains
7c2f53b9e; GitHub's base branch had advanced to 7e5f32ce6 at readback.

Next member proof `historical-members.log` passes through public CLI import:
one stable conversation/two immutable input references retain old failed and
current completed receipts, independently namespaced equal attempt keys, distinct
account/usage payloads, and null versus zero. Pre-bind history remains unassigned;
current history has the recorded Task. Import preserves current input/name/feedback,
existing Started, dry-run/report agreement and repeat sequence/payload identity;
zero Run rows and no provider launch. This exercises retained input references
using final admission APIs, not a newly demonstrated populated released upgrade
or receipt-based usage aggregation. SQL rows with unavailable artifacts and
finished/unopened review preservation still need completion.


## Session history readers · iteration 10 continuation

Base `dd97558752f1262634ef9edba31857792a99c6e7`; Task base `7e5f32ce6`.
Hosted CI36527817576 stopped at telemetry after 885 passes, one failure,
15 skips and 1,077 unrun tests. Admission created no Run row; the old telemetry
reader still selected `runs`. This was a production reader boundary.

Capture now retains launch, generic usage/account/lifecycle and terminal
observations in AgentSession history through the same writer as offline import.
Observed evidence cannot select or settle a native Flow turn. SQL input/history
selection replaces Run reads for lists, usage, telemetry, activity and landing
conclusions. Diagnostic artifacts remain available; these readers do not hydrate
them or create Run rows. Existing wire names remain transitional.

Review found a concrete error in the first projection: aggregating a conversation
labeled pre-bind usage with its current Task and excluded recent continuations
of old conversations. The revised projection uses each retained input's recorded
Work, provider, start and terminal evidence. No input acquires mutable state or a
new lifecycle. The replaced-input proof retains the unbound 21-token failure and
bound zero-token success in both unfiltered and Task-filtered reports; seven-day
selection includes only the recent continuation. Exact/prefix parent selection
is uncapped, and unavailable artifact files no longer erase these rows.

`conversation-telemetry.log` passed the original test. `conversation-attribution.log`
passed telemetry and the public replaced-input case, but the window fixture's
partial observation lacked its input receipt namespace. `conversation-readers.log`
passed three cases and exposed the landing fixture inheriting the runner database
while its landing reader used another store. Correcting the fixture's environment
with the existing TestLedgerGuard retained the original assertions.
`conversation-readers-final.log`: four passes (telemetry with artifacts removed
and zero Runs; inclusive window/partial history/excluded corruption; landing
conclusion exactly once; public replaced-input attribution/window/import replay).

`conversation-native-usage.log` completed native retry but the added Python
inspection omitted its timeout argument. The corrected `conversation-native-usage-2`
passes on candidate `a1397de2c6f386b7afc362c3d152c48412ac6f3545cc5ba72555fd97b270597a`:
real Codex 0.157.1 with scripted Responses/private Home, zero Run rows, retained
failed/successful native turns, exact once-only consumption, and public usage
40 input/10 output, unknown cost, one final stream. No configured-account,
native-only driver-loss usage or valid decision-retry claim follows.

`conversation-static.log`: all-target Clippy passes in 15.91s. Formatting and
Ruff pass after wrapping the new fixture call. Production-prefix measurement
versus dd9755875 is +363/-97, net +266 Rust lines; excludes cfg(test) tails,
integration tests, Python fixtures, docs and scratch. Receipt:
`.lf/tmp/cut-i/conversation-reader-lines.json`. The retained full obligations
remain: native-only history usage, remaining detail/import consumers, complete
released multi-member/SQL-only import and final Run deletion, dense measurements,
all-provider recovery, public Chapter execution and configured/installed proof.
The existing valid native decision retry failure remains separate.


Reader checkpoint rebased and published as
`190455f062fa95d31f31f46e8ebc3ec97a4c5fcd`, Task base
`1e163d20931efc7d92b54760877a62a6308143d6`. The two cache-recovery conflicts
retain upstream non-waiting uv lock acquisition and the existing subprocess bound.
The busy-cache integration assertion now expects uv's immediate lock failure;
cache preservation, continued build cleanup and later idle pruning are unchanged.
`conversation-rebase-cache.log` passes all three selected reconciliation cases;
Ruff and formatting pass. No unrelated Rust suite was repeated for the rebase.
GitHub and Task publication head agree; auto-merge is absent and the PR is mergeable.
GitHub main advanced to 566fb3981 after integration; this does not change the
recorded Task base or invalidate the local focused results. No acceptance or
Flow navigation follows. Next owner boundary remains complete historical import
and remaining reader/deletion work, with native transport scope kept separate.

## Saved review import and command admission · iteration 10 continuation

Base `190455f062`. The focused public import proof first reproduced loss:
`historical-review-red.log` reports only one review, two generic inputs and two
captures for three saved review boundaries. Import now preserves the boundary ID,
feedback, completion and recorded node/iteration after an autonomous successor
or finished Flow. Unopened reviews use deterministic immutable input references;
import no longer calls runtime review reservation. Only the matching current
human occurrence can become pending, and existing progressed Flow state remains
owned by its row. Preview rolls back the same import transaction.

Supervisor identified that an active boundary also describes autonomous agents
and operations. Classification now reads the captured node's human policy;
non-human captures proceed through ordinary headless import, and an operation
creates no AgentSession. The proof includes both cases. Historical reviews with
insufficient occurrence evidence, complete SQL-only member import and mechanical
outcome preservation still belong to the full import contract; this fixture does
not establish those remaining cases.

`historical-review-repair.log`: existing multi-origin import passed; the new
fixture failed because deleting only position.json left an empty Flow directory
which the next import correctly reported as missing input. The corrected fixture
removes its retired directory. Closed inventory uses `--history --all`; default
inventory behavior is unchanged. `historical-review-final.log` passed the first
review matrix. `historical-review-and-auth.log` passed the strengthened active
agent/operation matrix, existing multi-origin import and both cached-auth cases
(four passes, 6.728s). No provider launches, runtime Run rows or native successful
turns are manufactured by these imports. These are public CLI/private Home proofs
with authored historical inputs, not released populated multi-member conversion.

Hosted CI36530104314 on 190455f06 stopped after 1,578 passes, two auth failures,
15 skips and 383 unrun tests. Both cached inspections deliberately omitted Git
from PATH; unconditional repository discovery before Exec admission prevented
auth dispatch. Exec admission now takes the actual process cwd; repository
operations resolve their repository in their existing dispatch paths. No auth
exception or global Git-error fallback was added. `auth-exec-scope.log` passes:
five actual auth commands retain completed successful Execs with no broker
contact, and `rebase --plan` still reports missing Git and records a failed Exec.
No credential or installed-Home mutation was needed.

Production measurement versus 190455f062 is retained in
`.lf/tmp/cut-i/review-import-lines.json`; excludes test tails, integration tests,
docs and scratch. All full-scope obligations remain, including the valid native
decision retry red and bounded transport proposal, native-only driver-loss usage,
released SQL-only/multi-member import, final readers/deletion, typed wire cleanup,
all-provider recovery, Chapter CLI execution and configured/installed acceptance.

Pre-commit inspection exposed five generated journals below rust/loopflow/.lf:
using actual cwd for Exec admission had also moved the optional file journal.
Their exact files were retained at `.lf/tmp/cut-i/subdirectory-journal-counterexample`.
The journal now resolves its checkout root independently, with the existing
ledger-only behavior when Git/file journaling is unavailable. Exec retains actual
cwd. `auth-exec-directory.log` passed all six selected auth/Exec tests but reported
a LEAK on the interruption case; assertion success is not process-settlement
proof. The dedicated subdirectory proof first failed only on macOS /var versus
/private/var spelling (`exec-subdirectory-journal.log`); compare the canonical cwd,
without changing the recorded process directory. No timeout or cleanup assertion
was weakened. Static pass `review-auth-final-static.log` took 15.49s before this
one-line fixture correction; final static receipt follows below.

`exec-subdirectory-journal-final.log` passes the scoped journal/cwd proof in 3.309s.
`review-auth-commit-static.log`, cargo fmt and diff whitespace checks pass.
Final production-prefix measurement: +159/-82, net +77;
trailing cfg(test) modules, integration tests, docs and scratch excluded.

## Interrupted scorecard child · observed recovery counterexample

The next checkpoint rebased conflict-free onto `566fb3981` through pinned lf;
no behavioral suite was repeated just for integration. Before publication,
Supervisor requested diagnosis of the retained interruption LEAK. The fixture
script loops on a stop file, but Driver::drop waited only for lf, then TempDir
could remove the stop directory before the orphan script observed it.
Production telemetry used Command::output without interrupt child ownership.

`exec-interruption-child-red.log` records the exact reproduced child PID 4142
and its unique fixture script path surviving five seconds after lf exited 130.
The existing interrupted/130/unknown-signal Exec assertion still passed. The
fixture separately wrote that child's stop file and observed its exit before
asserting failure. This cleanup is not counted as production settlement. An OS
inspection also found older similarly named orphans, including a process whose
start matched the earlier leaking test window; without retained exact launch
identity those older processes were not signaled. No fleet cleanup is claimed.

Telemetry now places its scorecard in a fresh process group and uses the existing
ProcessGroupGuard for interrupt cleanup, disarming after output completion.
No new process registry, lifecycle or fixture-only production branch was added.
`exec-interruption-child-fixed.log`: two passes in 3.163s, including the original
telemetry usage/output contract and exact fixture-child disappearance after
interruption, with no Nextest LEAK. The initial build warned about an accidentally
added test-module import; it was removed before the final static check. Existing
signal missingness remains unchanged. This proof is the owned synthetic scorecard
child, not provider-engine death, historical orphan cleanup or general settlement.

## SQL-only conversation members and selected-caller fixture

The importer now retains historical `runs` rows joined by existing conversation
input references as source observations in Session history. Public runs/usage
read the imported provider, original timing, publication and outcome when payload
files are missing. Removing those old SQL rows after import no longer removes
that evidence. Replay compares each source receipt; changed SQL evidence fails
without replacing retained history. Import creates no provider turn or historical
Exec and does not borrow the importing command's Work. A NULL historical Session
ID is unknown; a positively different ID remains a conflict.

`historical-sql-members-red.log` failed on an invalid synthetic attempt ordinal;
`historical-sql-members-red-2.log` then reached the missing import (zero versus two).
`historical-sql-members.log` passed the SQL-only and existing replaced-input proofs.
The subsequent `historical-sql-partial.log` failed during setup: this schema requires
outcome/end presence together. Supervisor identified the same constraint; the
fixture now uses an unpublished input with both absent. No constraint was bypassed.
`historical-sql-unpublished.log` passes in 5.122s: three preserved inputs, two public
usage rows, unknown counters and missing-payload gaps, stable current name/feedback,
conflicting replay rejection, and identical usage after old rows are removed.
This is final-schema/public CLI evidence, not a released populated upgrade. The
separate SQL/terminal-receipt discrepancy obligation remains; the impossible pair
was not a production failure.

CI36531960032 on da19cf0b5 stopped at the checkout identity fixture after 1,656
passes, one failure, 15 skips and 307 unrun tests. Its direct decision supplied
only the old Run/Flow environment. The repaired fixture uses the shared Codex
harness with a scripted provider to record/select the native start, claims its
conversation driver through the existing API, and supplies the emitted thread's
caller to the real CLI. Main/parent upstream variants, child-only Iterate, unchanged
parent and nested checkout assertions remain. `checkout-selected-native-caller.log`
passes in 7.636s without Nextest LEAK. No production navigation check changed;
this proves fixture-authorized navigation, not delayed-child or valid-retry behavior.

Review retained one SQL-to-history importer and the existing history projection;
no alternate runtime writer or generic attempt object was added. Production
prefix measurement versus da19cf0b5: +120/-12, net +108, excluding tests/docs/scratch;
receipt `.lf/tmp/cut-i/historical-sql-lines.json`. Historical rows remain input only
until complete released/member preservation permits their deletion. Sessionless
rows, captured membership and all remaining-work/import-preservation requirements
remain open, including valid native decision retry and Linux Chapter execution.

`historical-sql-static-final.log`: all-target Clippy passed in 15.30s; formatting
and diff whitespace checks passed before this checkpoint.

## Populated release and development upgrades (2026-09-29)

The released-frontier proof starts at 0.12.24 with populated Task Flow positions,
not a fictional released Session/Run schema. It retains human and autonomous
captures, cursor, iteration, version/generation, saved answer and original Task
Started events. `released-position-upgrade-2.log` exposed a NULL Task title that
cannot populate the old Session title; the normal fixture now supplies its title,
but preservation of a supported missing-title Task remains an unresolved input
limit. `released-position-upgrade-3.log` then rejected the fixture's assumed
Started=position.updated_at. Jack's accepted rule assigns inferred Started during
conversion. The proof now bounds that assignment by the migration window and
preserves the original Started event at 50 separately.

`released-started-evidence-red.log` reached a real loss: an autonomous Task with
that recorded Started event and no mapped Run remained unstarted. The forward
retain_task_start_evidence draft backfills only NULL assignment times from
existing Started evidence at conversion time, retaining original events and
immutable non-NULL assignments. It does not manufacture a conversation or Exec.
`released-started-evidence.log` passes (0.737s); the subsequent materialized
released-position check also passes. The trigger still references historical
Runs and must join the eventual forward table-removal conversion.

The separate pre-admission development proof retains two SQL members, exact
Flow/node/iteration, failed/successful native history, current input/caller,
title/answer, selected_start and Flow event seq references. Preview, application
and repeated import retain both members; a changed source outcome rejects replay.
A preexisting Task Started=17 remains exactly 17 after the upgrade/import. No
historical Exec is fabricated. This is a populated development schema proof,
not the public filesystem importer or configured installation acceptance.

`materialized-sql-upgrade.log` found canonical adoption rejected a valid shorter
applied draft prefix at agent_session_admission. The existing migration
transaction now skips only verified applied bytes and applies the remaining
release suffix. Divergent recorded names/checksums still fail. The populated
proof injects an unexpected schema column, verifies failed adoption rolls back
schema, both ledgers and native history, removes that fixture column, then proves
successful replay. No historical migration bytes changed. The first repair log
passed the three existing adoption checks but failed a stale history count after
adding the Started event (7 versus 6); that assertion is corrected, not a product
failure. `materialized-sql-upgrade-rollback.log` passes all four focused checks in
0.955s, including existing checksum/schema rejection and populated suffix rollback.
Snapshot scope is current source plus these migration edits, canonicalized to
0.12.25.001 in a disposable copy; its exact receipt is
`.lf/tmp/cut-i/canonical-sql-upgrade-final-source.json`. No installed store was read
or migrated. Full remaining-work obligations and native valid-retry red remain.

`source-development-upgrade.log` also passes the source-draft path (0.739s).
`populated-upgrade-static.log` passes all-target Clippy (16.06s); cargo fmt,
whitespace and migration-history checks pass, with 55 shipped migrations unchanged.
Review keeps the repair in the existing adoption transaction and one forward
Started draft; no new migration framework or execution owner was introduced.

## Direct-provider continuity and Session fixture reconciliation

The populated-upgrade checkpoint and released notes were pushed selectively as
29d5c5229 on base d4b283a87. The first pinned rebase stopped before creating a
sequencer and preserved HEAD; dirty tracked supervisor notes were still present.
After their explicit handback/checkpoint, the same lf rebase succeeded with no
base change. The wrapper did not expose Git stderr, so dirty-state rejection is
the supported explanation rather than a captured exact diagnostic. Selective
`lf commit --no-add --push` excluded the then-active research artifact.

`session-flow-current-inputs.log` preserves both CI counterexamples after the
Task Flow fixture stopped querying Runs: OpenCode continuation now reaches
“Conversation has no connection and no confirmed engine exit”; both decision
calls still lack selected native original-caller authority. Direct CLI spawn
now records its actual PID/start identity in the existing Session provider
fields, just as the native harness records its process. A failed registration
stops and waits only that spawned child. No new process inventory or native turn
is created. `session-flow-provider-process.log` passes the same renamed Session,
failed/successful inputs and exact review in 7.426s. This is scripted OpenCode
process continuity, not native OpenCode decision/history integration. It neither
fabricates native completion from recorder success nor repairs Codex retry.

The once-requested affected suite in `session-cutover-provider-repair.log` finishes
20 cases: 15 passes, five failures, zero skipped. The Task Flow case also proves
zero Run rows and repeated completed resume without another launch. One red is
the retained taskless OpenCode decision. Three others share run_parents reading
the conversation's new binding instead of the input's original observation; the
fourth still selects legacy Runs throughout its launch/listing proof. Fixture
queries now read immutable input references and per-input Session observations,
retaining original ancestry, missingness, completion, caller, scope and listing
assertions. Only those four cases are being repeated; no broad matrix is claimed.

`session-cutover-history-fixtures.log` passes import-after-bind and prospective
binding, then retains two source-field assertion failures. An inherited child
has SQL admission ancestry without authored manifest subjects. The fixture now
uses Session provenance only when its Task/Wave still exactly match the original
history ownership; an earlier unassigned input cannot acquire a later binding.
`session-cutover-inherited-fixtures.log` passes both remaining cases (6.771s and
8.866s), including parent drill, exact list/usage/activity inventories and original
bound history. No assertion was removed. The taskless OpenCode decision remains
red and is an integration obligation, not a fixture pass. No new full suite ran.


Supervisor's follow-up exposed a separate public-reader regression after the
fixture reconciliation: a fresh inherited input with no authored manifest
subjects became Declared after replacement. `inherited-replacement-red.log`
first rejected an unsuitable Ask setup (that manifest already had subjects).
`inherited-replacement-red-2.log` then reproduces the actual public `runs`
misclassification after replacing the child input through the Session API.
The shared reader now deduces inherited source from unchanged admission source
and matching immutable history ancestry; explicit manifest/SQL provenance wins.
Binding changes source to Bound, so it cannot lend inherited provenance to old
unassigned inputs. The extension checks public runs/usage before and after input
replacement, with and without Task filtering, and retains unassigned old history.
`inherited-replacement.log` passes that case and the replaced historical-input
proof; this does not claim a second provider turn or configured native recovery.
Direct-provider Clippy passed in 16.39s before this reader addition; final static
checks follow for the combined checkpoint. The decision-interface question is
pending in Jack's existing conversation; no interface change is selected here.

Combined all-target Clippy passes in 16.67s (`inherited-provider-static.log`);
formatting and diff whitespace pass. Review keeps process evidence in Session
and source inference constrained by immutable ancestry and write-once assignment.
No native outcome or new execution owner was added.


## Remove obsolete Run CRUD

Checkpoint 325e0e13e was published to PR1296 after a conflict-free pinned rebase.
This next boundary deletes Run/RunEnd/ListedRun, their store writers/readers and
unused captured-cursor lookup. Session/Flow constructors retain their shared
Task-to-Wave lookup under the existing durable store module. Historical SQL and
its import/trigger dependencies remain pending their preservation conversion.
The old selector fixture now exercises Session input history, retaining all Task
selector forms, renamed Project ancestry, scope rejection and 56 uncapped causal
children. Causality alone no longer fabricates helper assignment; the existing
public child-admission proof owns that behavior. Reservation/Started/racing-bind
fixtures now exercise AgentSession writes and preserve atomicity assertions.
Zero-Run assertions use a test-only row count rather than the deleted public API.
Two compile diagnostics exposed a mistaken Wave accessor edit, remaining old
reservation fixtures and the now-unused capture lookup; all are repaired in this
boundary. Rust API removal is documented without claiming external migration.

`remove-run-crud-proof.log` passes four ancestry/Started/selector cases;
`remove-run-crud-cli.log` passes both converted inventory and multi-origin import
fixtures. Final all-target Clippy passes in 15.83s; formatting and whitespace
pass. No table is dropped and no historical migration bytes change. Next is the
reported Linux interruption failure: capture its discarded child diagnostic
before attributing the exit-code mismatch or changing production behavior.


## Coordinate interruption with command return

Supervisor reported hosted 325e0e13e returning OS exit 1 instead of 130 in the
owned scorecard interruption proof. The old fixture discarded the diagnostic.
Output capture and joint OS/Exec assertions now retain it. The first isolated
Linux run (`linux-interrupt.log`) passes, so it does not reproduce hosted timing.
A separate controlled probe delays return from the real group-kill syscall in
the signal hook. `linux-interrupt-ordering-red.log` reproduces OS exit 1 alongside
Exec interrupted/130, with stderr “telemetry scorecard exited with signal: 9
(SIGKILL)”. This establishes the supported race mechanism under fault injection,
not the unavailable stderr of the original hosted failure.

Ordinary command return now waits on the existing interrupt-hook mutex. The
signal handler holds that mutex through its cleanup, child termination and exit
130, so the child error cannot overtake cleanup. No new interrupt state, process
owner, accepted exit code, production sleep or retry is introduced. The Linux
fixture retains that controlled ordering and requires both truthful Exec outcome
and no surviving owned scorecard. Source snapshots and diagnostics are retained
under `.lf/tmp/cut-i/linux-interrupt*-source.json` and matching logs. Docker uses
the existing isolated-account/build-cache pattern; no host Home is mounted.

`linux-interrupt-final.log` passes the controlled ordering in 1.17s with
matching source hashes. Both ordinary exit/journal proofs pass in
`interrupt-ordinary-exit.log` (2.491s/3.415s). All-target Clippy passes in 15.67s;
formatting and whitespace pass. Each disposable Linux container was removed.
The Run API deletion alone is +29/-513 production lines versus 325e0e13e
(supervisor receipt `status-counts-1d86cd4f3.json`), excluding tests/docs.
Neither checkpoint closes native decision, import/table or reader obligations.

### Interruption checkpoint integration (2026-09-29)

Checkpoint `39985424d` retains the bounded interruption repair after Run API deletion.
Rebase onto `359c9a6c3` conflicted only in CLI documentation while replaying the
early spec rewrite. The final text retains the accepted owner model and upstream
single-linear-commit preservation; upstream executable changes apply unchanged.
No native decision or table-removal acceptance follows.

### Unmapped SQL agent inputs (2026-09-29)

Publication `9887f8c84` includes the interruption repair and obsolete Run CRUD
removal. The next public import regression removed old input links and added a
standalone SQL agent input with no artifacts. `historical-unmapped-red.log`
reported only one of four retained inputs. `historical-unmapped.log` now passes
(5.617s): preview writes nothing, applied/repeated imports retain all four,
current selection/title/feedback survive, caller references and nullable native
identity survive, conflicting SQL fails without rewriting history, and public
usage remains after legacy rows are deleted. No provider launched.

Import now restores missing references from recorded SQL ownership and creates
standalone agent conversations using the existing input selector contract. It
retains original SQL evidence and refuses conflicting Session/caller mappings.
Unknown or mechanical rows do not become invented agent conversations.

`historical-unmapped-canonical.log` passes the expanded populated pre-admission
development proof (0.617s), including rollback on schema discrepancy, exact
native/event references, Flow selection, existing Started=17, standalone agent
attribution and repeat/preview. A fresh exact-source disposable copy owns the
canonical registration; `canonical-unmapped-source.json` names its source hashes.
The intentionally unclassified caller row remains present with no Session. This
is a concrete remaining preservation boundary before table removal, not a pass
for all SQL origins. Released Task-position and filesystem proofs retain their
separate earlier receipts. Native retry and final readers remain outstanding.

Indexed-discovery research is returned at `research-indexed-discovery.md`; its
private schema measurements and nullable-membership counterexample do not prove
full-schema density or authorize narrower discovery. No contributor remains active.

All-target Clippy passes in `historical-unmapped-static.log` (16.38s); formatting
and whitespace pass. This cut versus `9887f8c84` adds 91/removes 3 production
Rust lines in the importer/Session store, excluding migration test code, CLI
tests and docs. Review retained the existing history owner and input links; no
new process or attempt owner. The Run table still holds unclassified/mechanical
evidence and remains protected by its historical triggers.

### Mechanical classification and public detail (2026-09-29)

Supervisor source evidence disproved the skill-name predicate at `4682d662a`:
old Task operations also stored their step name in `skill`. Import now excludes
`provider=loopflow`; a provider-null standalone input needs a captured Skill/XOR
node, not a label. Existing Session identity remains separate recorded evidence.
The expanded CLI case retains a named mechanical row and a provider-null label
without constructing either AgentSession. The earlier assumption is superseded.

The populated development fixture then found zero Flow history entries for that
completed named operation (`historical-named-operation-red-2.log`). A forward
`retain_named_operation_history` draft preserves both entries plus complete SQL
bytes, exact node/iterations/outcome and current operation selection. Reservation
time remains payload, observed start/Exec stay unknown. The materialized proof
passes in `historical-named-operation-canonical.log` (0.599s), retaining original
Session event references, Started and schema/ledger rollback checks. Prior drafts
are unchanged. Two preceding compile failures were fixture/module-name errors,
not behavioral failures (`historical-mechanical-classification.log`,
`historical-named-operation-red.log`).

Public detail initially failed for an imported SQL-only input missing its manifest
(`historical-detail-red.log`). Default `lf runs ID` now uses the same retained-input
projection as usage, adding its exact input predicate before payload hydration.
Exact Session selectors select current input; full and stripped input prefixes
retain ambiguity rejection. Text replay availability still checks the selected
immutable manifest. Final/events, native resume and active readers retain their
separate conversion obligations. `historical-detail-and-kind.log` passes (7.583s),
including exact/prefix detail equality, retained SQL data, import replay/conflict
and no provider launch. This does not establish table removal or dense paging.

Hosted CI on `9887f8c84` passed interruption in 1.576s. Rust stopped on the known
OpenCode taskless decision failure after 1835 passes and 15 skips; 133 did not run.
The supervisor retained `ci-9887f8c84-rust.log`. No native decision contract change
is selected. The active read-only Exec admission contributor owns only its
unreturned research artifact; executable/build/Git ownership remains with main.

Prepared Ask detail and replaced-input history both pass in
`historical-detail-callers.log` (3.519s/5.533s). All-target Clippy passes in
`historical-classification-detail-static.log` (17.14s), and migration verification
retains all 55 shipped checksums. The canonical operation snapshot is recorded
in the latest `canonical-unmapped-source.json`; its earlier agent-only snapshot
is retained by local checkpoint `4682d662a` and the first disposable build path
in `historical-unmapped-canonical.log`. The receipt filename was reused, so it
is not an exact-source receipt for both runs. No extra matrix was run.

This follow-up versus `4682d662a` adds 136/removes 17 production Rust/SQL
lines, excluding migration/CLI tests and docs. It replaces the skill-name
classification and manifest-dependent detail projection; table deletion and
final/events/resume/active conversions remain open. Review preserved operation
uncertainty and the existing Flow history owner without adding process authority.

### Session-owned final/events reader (2026-09-29)

Publication `6f927ea5f` includes the returned Exec admission research and the
verified historical classification/detail cut. The next complete consumer boundary
is public final/events plus ordinary launch and landing repair conclusions.
Currently the recorder copies usage/provider evidence into Session history but
leaves conversational text only in JSONL. Move that writer and its conclusion
readers together. Proof must remove the artifact directory after settlement/import,
retain completed versus failed/incomplete turn distinctions and exact versus
streamed conclusions, and preserve once-only landing reporting. File presence or
a summary-only import does not pass. Native-only recovery, table removal and dense
query costs remain separate unfinished requirements; this adds no decision writer.

`session-final-red.log` reproduces both exact and streamed conclusions becoming
absent after artifact removal. The recorder now retains all observed envelopes
in existing Session history; public final/events, ordinary launch return and
landing repair conclusions consume that history. The file-backed final reducer
and landing's artifact lookup are removed. Existing JSONL writes still serve the
unconverted native/active readers and are not claimed deleted.

`session-final-owner-2.log` passes five behavioral checks: exact conclusion,
honestly labeled streamed prose, once-only landing output after artifact removal,
the retained window/usage/partial-evidence regression, and public imported final/
events with failed and unfinished later turns excluded. Its summary-query fixture
failed on invalid `run_fixture` setup and reported LEAK. After correcting that
input ID, `session-summary-fixture.log` passes (0.411s) without a leak report.
This focused fixture proves transcript exclusion before Rust payload hydration,
with usage/unknown evidence and complete event detail retained; it is not dense
latency, paging or native-only recovery proof. The initial implementation check
`session-final-owner.log` failed compilation on a statement lifetime and unused
Home variable; no behavioral result follows from it.

Summary selection now excludes known non-summary event kinds in SQL. Native
Session history remains complete and unknown/schema evidence stays visible.
Review found the prior all-event summary hydration would load transcripts after
this writer expansion; the query repair addresses that concrete cost. Existing
manifest payloads and unlimited inventory remain part of indexed-discovery work.
This consumer cut versus `6f927ea5f` adds 62/removes 35 production Rust lines,
excluding test modules, integration fixtures and docs. No lifecycle owner, native
decision transport or Flow navigation change was introduced.

All-target Clippy passes in `session-final-static.log` (16.16s); formatting and
whitespace pass. Supervisor independently reviewed the writer/readers and proof
limits. Hosted `6f927ea5f` retains the known OpenCode decision failure (1834 passes,
one failure, 15 skips, 134 unrun); it predates this final/events cut.

### Retained provider identity (in progress, 2026-09-29)

The final/events checkpoint rebased conflict-free onto `7d1158dac` and published
as `4ced9467f`; upstream Xcode cache behavior applied unchanged. No behavioral
repeat was added for that integration. Next, replace provider-session sidecar
publication/lookup with the existing observed Session history. Preserve the exact
recorded thread/account through retry and legacy import; no current account
selection may stand in for old history. SQL history gives no process or Flow
authority. Saved continuation still has separate immutable-input/client dependencies;
removing one sidecar does not prove all native-only recovery or complete connect.

`native-history-owner-red.log` reproduces native identity loss after artifact
removal. Session observations now own synchronous provider identity publication;
the sidecar writer and ordinary file reader are deleted. Five initial focused
checks passed in `native-history-owner.log`. The expanded public import then
failed in `native-import-sidecar-removal.log`: classification queried SQL before
import, reporting zero interactive inputs instead of one. Classification now
uses the parsed legacy history through the same reducer.

Supervisor identified insertion-order loss in the fallback. Original JSONL
ordinals now order thread/account reduction; fresh publications outrank a late
imported sidecar, whose import time is not native chronology. Unknown accounts
remain unknown. `native-history-order-and-import.log` passes all three selected
checks (7.253s): reversed observations/fresh-versus-imported identity, transcript
exclusion with full detail retained, and public multi-origin import/replay after
removing the provider sidecar. This proves retained source ordering, not a
configured incident or native-only recovery.

`native-history-final.log` repeats the five native writer/recovery checks after
the reducer change: all pass (9.099s), including pre-start durability, retry
account changes, late output after client exit and public saved-Ask resume.
All-target Clippy passes in `native-history-static.log` (15.86s); formatting,
whitespace and two portable-documentation checks pass. Architecture rendering
initially used the root Python environment and failed for missing fasthtml;
using the website environment generated the artifact and passed both checks.

This cut versus `4ced9467f` adds 107/removes 38 production Rust lines,
excluding test modules, integration tests and documentation. Review retains one
Session history owner and removes native sidecar publication/read authority;
import is the sole legacy source consumer. This does not remove process/client
receipts or immutable input dependencies, prove all-provider continuation, or
resolve native-only usage, Run-table removal or dense-query costs. Hosted
`4ced9467f` retains the same OpenCode decision failure (1835 passes, one failure,
15 skips, 135 unrun); it predates this cut. Supervisor's bounded-investment
recommendation is not Jack's approval for a new decision interface.

### Continuation and active input readers (2026-09-29)

Jack's retained continuation requirement now has a public missing-manifest
counterexample and repair. `native-continuation-without-manifest-red.log` showed
Open replacing the saved Ask input after its manifest disappeared. Saved native
continuation, prefix selection, failed-review outcome checks and active metadata
now use AgentSession/input history; exact process receipts remain the authority
for client liveness and stop. Current Session Work is projected separately from
historical input attribution. `read_run_snapshot`, `context_ref_is_valid` and
file `reduce_usage` have no remaining callers and are deleted. Initial prepared
launch, immutable payload and process/client receipt dependencies remain.

The first compile failed on an unnecessary RunId ordering requirement; string
keys removed it. Subsequent focused runs exposed non-admitted fixture inputs,
ambient database selection and the existing stop error assertion. Those failed
receipts remain. `native-continuation-repair.log` passes five cases (9.180s):
retained stop identity, stale review rejection, keyed Ask retry, replaced-Home
discovery and public saved-Ask resume with its original input/prefix. The latter
also proves active display without a manifest and fixture client settlement.
`native-retired-reader-removal.log` passes eight usage/format/context/decision
cases; `native-immutable-input-preservation.log` separately passes rejection of
conflicting publication and altered context. No immutable-input acceptance check
was replaced with a successful empty read.

Published db60d9896 CI stopped at three Task-review tests opening a path-derived
missing database (10 passed, three failed, 15 skipped, 1960 unrun). Review state
and native history now use the supplied SharedStore. All three assertions pass
in `native-continuation-and-review-store.log`, whose 15-case result still retains
two subsequently repaired failures and one concurrent-completion LEAK. The
isolated `native-review-settlement.log` traced that exact test: short-lived Git
children, exit zero, pipe EOF and no remaining process group by 0.569s.
`native-review-settlement-nextest.log` passes cleanly in 0.512s. This is clean
local settlement evidence; it does not identify the earlier leak's cause.

All-target Clippy passes in `native-continuation-static.log`; formatting,
whitespace and two generated-documentation checks pass. Production delta against
`db60d989665f3c1787c36cd019a6f33dc6e30020`: +142/-210 Rust lines (net -68),
excluding test modules, integration tests and docs, using the retained corrected
prefix method (`native-continuation-count.json`). Review found and removed the
last file-outcome consumer instead of keeping a second history reader. The old
file-age active fixtures do not separately establish old SQL timestamps; active
queries have no recency cutoff, but dense/history-age proof remains with discovery.

Returned Session-wire research is retained for the coordinated next consumer
boundary. Complete SQL evidence/table removal, native-only usage, Exec entry
coverage, final typed DTOs and Chapter CLI proof remain required. The pending
decision-interface choice is unchanged; neither valid Codex decision retry nor
OpenCode decision integration is claimed passing. No code-complete or shipment
claim follows from this checkpoint.

### Explicit unresolved historical SQL (2026-09-29)

Continuation checkpoint `dca1d617d` rebased to `3280c7524` on `567ac07df` and
published. Integration changed only migrations.rs: the adoption loop reconciles
upstream prefix hashing with the branch's transaction structure. It is not an
exact byte transplant. `rebase-migration-contention.log` passes the one selected
initialization/append/adoption regression (1.021s); no broader repeat followed.

The next public preservation proof reproduced silent omission:
`historical-unclassified-report-red.log` reports `failed=[]` despite two retained
SQL inputs with no established conversation or operation destination. The
existing reader now returns those observations without an invented AgentSession;
import reports each original input in its existing failed list and retains its
SQL row. Known operations are omitted from pending import only when their full
source JSON, Flow/node/iterations and recorded completion match Flow history.
No new registry, lifecycle object or migration archive was introduced.

The first repair run passed public preview/applied/replay but exposed the
populated fixture's old three-member count: its unknown caller now also appears
explicitly. The expanded proof then exposed its positional `remove(0)` assumption;
conflicting replay now selects the exact prior input. Both failed receipts remain.
`historical-unclassified-canonical.log` passes the populated pre-admission upgrade
in a freshly materialized disposable source snapshot (0.601s). It retains unknown
caller/Started/native/Flow references and rejects missing, partial or conflicting
operation payload, changed boundary, wrong completion time and absent completion;
restoring the exact evidence resolves that operation. Snapshot hashes and root
are in `canonical-unmapped-source.json`. No historical draft bytes changed.

Hosted `3280c7524` also exposed an old publication fixture using an arbitrary
temporary directory without Session admission. The test now creates a real
isolated admitted input; it retains no-history/no-owned-client refusal and exact
child cleanup. `historical-unclassified-and-admission-2.log` passes that test and
the public SQL-only import (7.614s), while retaining the intervening populated
fixture failure above. The earlier compile receipt records an IO/anyhow error
conversion mistake, subsequently fixed; it was not a production behavior failure.

All-target Clippy passes in `historical-unclassified-static.log`; formatting and
whitespace pass. Production delta versus `3280c7524`: +42/-11 Rust lines, excluding
test modules, integration tests and docs (`historical-unclassified-count.json`).
Review tightened exact operation preservation instead of treating a matching old
ID as sufficient. Reporting does not dispose of unknown rows: `runs` and its
remaining triggers cannot be dropped until every row has a preserved destination.
Complete import/table removal and every remaining-work requirement stay open.
The separate Swift active-client fixture failure is under bounded research;
its unpublished proposal is excluded until supervisor handback.

### Admitted Swift observation fixture (2026-09-29)

Supervisor returned the bounded proposal in `research-active-swift-fixture.md`;
main applied and formatted only ActiveRunsObservationTests.swift. Each historical
input now enters through public `session import` before the reader observes its
live cat client. Import must report one interactive conversation and no failures;
the private binary/Home/database are pinned with inherited authority cleared.
Historical input time remains 2020 while the exact client receipt records its
actual start. Throwing paths await the owned reader and clients before deletion.

`active-swift-fixture-build.log` records the current source CLI build.
`active-swift-fixture.log` records one actual Swift Testing realCLI pass in
10.370s (the preceding XCTest zero count is not the result). Automatic first and
second discovery, rescan, client exit, no gaps and reader cancellation retaining
the first client all pass with the unchanged 12-second deadline. This is private
Home/synthetic cat transport proof, not configured provider or Desktop acceptance.
The existing ignored manifest-only density benchmark also needs admission
conversion before its scale results could count; it was not run here.

Review retained the production SQL reader and repaired fixture admission, without
a manifest fallback or timeout change. Production delta is zero; only the Swift
test and notes changed after c1f1b8c82. Rust bytes retain that checkpoint's Clippy
result. Whitespace and Rust formatting checks pass. Historical unresolved rows,
final table removal and the complete remaining-work contract remain open.

### Retire the Run table without discarding unknown inputs (2026-09-29)

Published `003fa792f` includes the admitted Swift transport repair and explicit
unresolved-input reporting. The next forward draft moves every old SQL column
into the existing immutable-input catalog, with nullable conversation attachment,
then drops `runs`. Unknown inputs remain import failures rather than invented
Sessions/Execs. Original source payloads cannot change or disappear; generated
foreign-key fields retain Task/Wave/Flow/Session relationships without a second
lifecycle writer. Surviving Started and ancestry triggers use that evidence.
Historical drafts are unchanged. Public import uses the same classification and
exact Flow-operation comparison over this retained source; repeated import works
with the old table absent.

Supervisor review found that shared historical attachment had broadened ordinary
replacement to allow an old input again. Ordinary replacement now keeps its
original unique INSERT; the existing binding proof rejects A→B→A even with changed
cwd/provider/model and preserves B. Historical attachment alone can fill unknown
membership. This was a source counterexample, not a claimed executed race.

`historical-table-removal.log` passes the first populated development conversion.
`historical-table-public.log` retains an intervening compile error from using a
nonexistent SharedStore convenience method in the hosted fixture repair. The
repair instead keeps its actual database pin through the final history read.
`historical-table-public-2.log` passes six selected checks: public SQL-only import
and replay, prior-input rejection/binding, serialized ancestry, interrupted Op,
reserved Started status, and deleted-Task refusal/history (7.618s total).
Current-schema fixtures now assert table absence, not zero rows in a retained
lifecycle table. The native Python probe's assertion was converted but not rerun.

`historical-table-canonical.log` passes two distinct populated upgrades in one
freshly materialized disposable source copy: released Task positions (0.881s) and
pre-admission development members (0.905s). It compares all original SQL columns,
retains exact Started17/native sequence/selected-success references, preserves
unclassified callers without Execs, checks immutable evidence and post-drop
ancestry/foreign keys, permits same-Wave transfer, rejects changed Wave/Task
ancestry and preserves schema/ledger rollback on an injected discrepancy.
Snapshot scope is `canonical-input-evidence-source.json`; the copy lacks Git
metadata and the selected SQLite proofs require none.

All-target Clippy passes (`historical-table-static.log`), as do formatting, Ruff,
migration history and generated-doc checks. Architecture initially rejected
duplicate ownership of the input/history tables in the new projection row; the
row now references the AgentSession owner rather than remapping its tables, and
the checker passes. Two website checks pass. Production delta versus003fa792f,
including the new draft and excluding tests/docs with the retained corrected
prefix method: Rust+56/-42, SQL+81/-0 (net+95); receipt historical-table-count.json.

The Run lifecycle table is removed on this local tree. This does not complete
Run product/wire removal: selected input column/API naming, immutable launch and
process receipt consumers, native-only recovered usage, Exec entry coverage and
dense discovery remain. The valid native decision retry/OpenCode failures and all
remaining-work/import/Chapter obligations are unchanged. No configured-provider,
installed migration, code-complete or shipment claim follows.
