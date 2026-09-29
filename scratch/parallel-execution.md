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
