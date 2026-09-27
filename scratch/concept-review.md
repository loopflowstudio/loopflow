# LOO-298 concept review

2026-09-26 · Reviewed `654e2777cb036b956eeb5c1792165b486a6c7af8`
against `4cd64be3d0432aa04dd9256293eef7088db97ccc`, concentrating on the
Run constructor and the latest [slice review](data-model-slice-review.md).
Entry tree was clean. This pass changes only this review.

**Judgment: retain the amended model; the branch is not ready to publish.**
Session is the conversation Jack keeps; Run is an attempt within it. The
constructor and selected-attempt repair now have executed local proof. The
next work is the common owner/caller conversion, not another attempt to prove
that compilation is sufficient or another redesign of Session identity.

The [design](data-model-one-table-per.md), [review feedback](data-model-review-feedback.md),
[Session decision](session-runs-and-current-run.md) and final dated steers in
[questions](questions.md) govern. The participant whose name remains unresolved
approved taskless Invocations and Session owning Runs plus a current Run. Jack
subsequently selected write-once bind and a set-once Started timestamp. Those
corrections supersede the Task title, older Wave memory and this note's former
example moving a Run from Task X to Y. All eight Done When obligations remain.

## Usage first

Proposed end-state walkthrough, not an executed demonstration:

```sh
# In a checkout without a registered Task:
lf --interactive : "Review the parser"
lf session list --json
lf session rename <session-id> "Parser review"
lf session bind <session-id> --task INF-123 --json
lf session list --task INF-123 --json
lf runs --task INF-123 --json
lf session open <session-id>
```

Jack selects the unbound conversation, sees its name and the exact permanent
Task target, and confirms once. A landed or done INF-123 remains eligible.
Binding preserves the Session, terminal draft and Flow membership. Every reader
shows the current Run's same Task; there is no move-to-another-Task or unbind
interaction. A Wave-only Run can receive a Task within that Wave. A taskless
invocation's Run cannot independently receive a Task under nullable parent
equality; “orphan” does not mean “independent.”

Recovery uses the same Open action. Resume native history when recoverable;
when exact evidence permits replacement, append a Run and select it without
losing the Session's title, feedback or earlier attempts. Missing receipts
cannot authorize another provider launch. Stable Session identity preserves
navigation, not a dead provider's unsaved native state. Ready saves review
feedback; Complete returns it once to the waiting boundary. Provider exit and
pane closure do not complete a review.

Keep the updated examples and bind language in [CLI Sessions](../docs/lf.md#sessions).
The following clarification remains a draft for the final all-kind conversion:

> Use the Session ID for the conversation. A Run ID or Run prefix selects that
> attempt; a replaced attempt reports its Session and current Run without acting
> on the replacement. Open and Complete retain the attempt selected at entry.
> Bind retains the current attempt and exact target shown for confirmation; a
> concurrent change rejects the write rather than redirecting it. Renaming by
> Session ID names the conversation even if its current attempt changes.

This extends the accepted selected-attempt and confirmed-target contracts to
bind; it adds no recovery command or durable confirmation record. Confirmation
transport for headless/JSON callers belongs in the existing CLI/UI interaction
pattern during implementation. No new flag is specified here, and JSON output
alone must not count as confirmation.

Before shipping, reconcile `docs/lf.md:562`: its prepared-Run paragraph still
says listing fails until an old boundary is prepared and launch fills context.
The final all-kind reservation/import/publication behavior must own that wording.
The architecture reference owns validators; Waves owns Chapter history.
Builtin `LOOPFLOW.md:75` already separates Ready from Complete, and
`task/skill/review-design.md:112` separates decisions from assumptions. Keep
those instructions; storage details do not warrant a skill rewrite.

## Core model in one screen

| Action | Identity and owner | Transition |
| --- | --- | --- |
| Choose a workflow | Flow template, default on Project | Task may select another Flow |
| Execute or restart | Invocation: capture, cursor, returns, claim | Resume retains capture; restart retains the old execution as history |
| Enter a nested loop pass | Child Invocation | Runtime parentage, never template composition |
| Inspect an attempt | Run: nullable Invocation, Task, Wave and Session | Record outcome, usage and exact process evidence |
| Name or reopen a conversation | Session: title, feedback, completion, current Run | Replacement appends a member Run and compares the current pointer |
| Assign missing ancestry | Selected current Run | Confirm once, fill missing parents, preserve membership and previous Runs |
| Show Started | Task's `started_at`, maintained by the Run writer | Set once at first Task assignment, including bind; never recompute from Run ages |
| Complete a review | Selected Run and exact waiting boundary | Settle saved feedback once, then tear down the same attempt |
| Inspect past plans | Repository Chapter; Project at (Wave, Chapter) | Freeze old evidence and transfer active identity through one repository boundary |

This is the target model. Current SQL implements only part of it. A node and
launch-time iteration tuple locate a Run without an occurrence object. Loop
return, Run replacement and Invocation restart preserve different identities.
Session identity grants no execution or process-control authority.

## Findings and consequences

Source paths below are relative to `rust/loopflow/src/` unless stated otherwise.
Source observations and reused behavioral evidence are distinguished explicitly.

### 1. Write-once assignment removes a product lifecycle

The earlier reversible model required explaining rebind, unbind, moved usage
and a Task becoming unstarted. Jack's final contract removes those transitions.
Bind fills a null Task once, preserving an existing Wave. Usage cannot move
between Tasks. Authorized cross-Wave Task moves remain a separate operation.

The current constructor (`store/sqlite/runs.rs:60`) resolves Invocation's nullable
Task, fills Wave through Project, and rejects supplied mismatches within its
caller's transaction. Both review creation and replacement use it
(`store/sqlite/sessions.rs:84,313`); current/history reads share `read_run`.
Replacement inherits attribution and gets the Task's current worktree, while
older attempts retain their launch cwd. These are useful common owners to extend.

The update trigger in the `own_sessions_and_runs` draft validates matching
parents, but does not prohibit changing a Run between two otherwise valid Tasks.
There is no general bind API in `SessionCommand` (`lf/mod.rs:733`). Thus the new
write-once rule is an implementation obligation, not a passing constructor claim.
Keep the selected Run and resolved target through confirmation and the final
transaction. The existing selected-attempt race is the counterexample to
resolving again after confirmation. No generic action framework is needed.

### 2. Started has one meaning; its timestamp records assignment

Jack's final rule is any Run with Task X. The denormalized timestamp records
when X first received a Run, not the oldest Run's creation time. This permits one
reader without a second worker-claimed definition or a worktree-content definition.
A first bind uses bind time; later older/newer Runs leave the exact timestamp
unchanged. The writer checks timestamp presence iff a Task Run exists.

Current source still differs: `store/sqlite/chapters.rs:97` combines historical
events, worker generations and **published** review Runs. `record_task_start`
remains at `lf/commands/run.rs:832`. The shared INSERT neither sets a
`tasks.started_at` column nor validates its presence. Its passed reservation test
(`store/sqlite/durable.rs:1829`) correctly proves the transitional reader; its
reservation-is-false assertion must change with the final any-Run contract.
A reserved attributed Run will count as Started without proving provider launch,
liveness, successful output or Task completion.

No previous passing test is invalidated by this note. During conversion, replace
that transitional expectation and retain its history/status-read coverage.
Keep retirement's separate authored-work, PR, claim and unknown-evidence checks.
Import must populate Started for Tasks with Runs without inventing successful
Runs to explain older events; report any inferred timestamp provenance explicitly.

### 3. The local ownership proofs now run

The latest slice records **58 distinct library tests passing**, including the
constructor matrix, competing Project ancestry writer, repeated Session
replacement, selected-attempt races, publication recovery, stale actors,
corrupt-neighbor availability, 29 Task-controller tests and 16 durable-store
tests (overlap counted once). The retained results file includes the corrupt-
neighbor setup failures and subsequent pass. The repair supplies required Task
planning fields and distinct worktree/slug values; it does not loosen the domain.

The populated projection, invocation and Session migrations and schema fixture
also pass after canonical materialization in a disposable source copy. Two
actual CLI status/roadmap integration tests pass against isolated current and
prior-release stores. Inspected the retained result ledger and the materialized
Session/CLI log summaries in `.lf/tmp/loo298-proof/` alongside the source diff.
The prior concept review's unexecuted-proof and resource-blocked conclusions
are superseded for these named checks. The design's `Measure` paragraph at
`:1131` still repeats the old blocker; use its current ledger and slice review
for evidence, and correct that stale paragraph at the next design update.

The selected-attempt matrix (`controller/task/mod.rs:2338`) now supports the
repaired API: a superseded Run cannot redirect Complete/Open/rename; Session-ID
rename intentionally stays conversational. Native handoff uses an owned shell
stand-in. These proofs do not establish configured provider recovery, all Session
kinds, taskless CLI recovery after source removal, desktop retention or Home import.
The earlier P1 is repaired and locally exercised; another review loop over that
same repair is not the remaining work.

### 4. The one-conversation experience still crosses live owners

| Remaining experience gap | Current source | Required continuation |
| --- | --- | --- |
| One Session list | `ops/human_session.rs:575` combines four sources | Query Session/current Run for every kind after import |
| Retain Ask feedback/name on replacement | `human_session.rs:1204,1208` resets feedback and copies names | Stable Session row; remove the copy operation |
| Taskless execution and review recovery | `ops/flow_run.rs:108,136` reads/writes `position.json` | Common captured invocation owner/driver and runtime nesting |
| Task Run history and usage | `lf/commands/runs.rs:77,101` scans manifests | Indexed typed Run ancestry shared by consumers |
| Stable pane/history and correct grouping | Swift `SessionRecord.swift:134`, `WorkspaceProjection.swift:43,107` | Distinct Session/current Run IDs, typed parents/history and cached projection |
| One repository planning clock | `store/sqlite/chapters.rs:44` clears current by Wave | Repository activation and frozen predecessor plans |

`WorkspaceProjection` still groups through visible roadmap matches and reverse
searches them. A bound Session missing from that roadmap must remain bound and
reachable; Wave-only Sessions are orphans under the accepted null-Task definition.
Run's current Rust type also lacks the general lifecycle/process/location facts.
Deleting these remaining adapters before replacement would lose capability.
Adding table-only readers over an unimported Home is equally unacceptable.

## Remaining decisions and scope

Current-Run-only bind remains an implementation assumption. The valid example
is now: A and replacement B began unbound; Jack binds B to X; A stays unbound
and B's usage belongs to X. Later replacement C inherits X. If A already belonged
to X, B inherits X and cannot bind to Y. Stable Session approval did not authorize
bulk binding of old Runs. Keep that boundary; an all-history assignment would
need a separate product decision before expanding it.

Nullable invocation Task equality still prevents individually binding a taskless
invocation Run to a Task. Keep the explicit explanation at the bind surface.
Relaxing equality or binding an entire invocation would change the accepted
model; neither is selected or necessary to continue. No Ask is required here.

The supplied PR #1296 publication-record and existing-Task stacking reports stay
unreproduced scope. The cancellation steer also remains: `lf pm task cancel --id`
should use the existing Linear client's Team canceled-type state; abandoning
local Task Work should cancel its issue; a refused Task start must not leave a
new issue behind. Cancel must not report completed work. `PmTaskCommand`
(`lf/mod.rs:1499`) has create/update/comments/done, without cancel. This review
checks that surface only; it neither reproduces the remote incidents nor changes
issues, credentials or delivery state. No new planning object is warranted.

## Smallest next action and proof

Continue the approved common Task/taskless invocation owner and driver, extending
the existing Run writer to general launches and all Session kinds. Include
write-once bind and set-once Started in that conversion. The accumulated local
proof debt has been repaid; preserve those receipts instead of rerunning the
same suites merely because this review began.

The first new bind/Started proof should reserve an attributed Run, assign a
previously unbound older Run to another Task at bind time, and compare timestamp
presence with actual Run existence. Later launch and older/newer binds must
preserve the timestamp bytes. Competing targets or replacement during
confirmation must fail atomically without changing ancestry, Session feedback,
artifacts or the losing Task's timestamp. Include same-Wave assignment, conflicting
Wave, done/landed Task and nullable invocation mismatch cases. Prove import
presence and timestamp provenance separately from observed runtime assignments.

For the common execution owner, run an isolated actual taskless CLI Flow, remove
its template, recover its review, replace the Run twice and complete saved
feedback once. Preserve exact claims, stale-result rejection, uncertainty after
publication and native process authority. Re-run affected preservation proofs
when those executable bytes change, including canonical materialization when
migration inputs change. This is the next behavioral distinction, not a new
acceptance substitute.

All eight design obligations remain: complete ancestry/structure validation;
common CLI/usage/bind readers; captured execution preservation; repository
Chapter operation; populated filesystem import; Rust/Swift DTO and retained-pane
proof; backed-up exact-writer real-Home conversion with configured CLI/app
acceptance; deletion research followed by deletion and final documentation
consistency. Comparable measurements, the recorded **32/33** owner inventory gap
for `wave_chapters`, historical branch whitespace and the three added reports
remain outstanding. No intermediate publication is selected.

## Evidence boundary

This pass inspected the latest diff, constructor and Session transactions,
ancestry triggers, Started/retirement readers, live alternate owners, Swift
projection, usage/skill guidance and retained proof logs. No new blocking source
defect was found in the constructor cut. The final bind/Started obligations are
accepted but unimplemented requirements, not failures concealed by existing tests.

Only this note changed. Its whitespace and local-link checks pass. No product
tests were rerun; the latest slice's formatting, Clippy, migration, HTML and
behavioral receipts retain their stated scope. That slice's post-proof resource
preflight passed at 102.8 GiB free and 3.1 GiB for this checkout; this review did
not resample resources. No Home, provider, PR, Task disposition or Flow navigation
changed. This review supplies evidence and chooses no next Flow edge.
