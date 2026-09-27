# Data model storage slice review

## Current review: selected-attempt propagation

2026-09-26 · LOO-298 · review-slice

**Not ready to publish.** Reviewed implementation `830e9af7b` against active
base `4cd64be3d`, including the complete branch change and the narrower
selected-attempt diff since `cb4ee961f`. Preserved the supplied uncommitted
compression review in `3b86c9b6b` before editing. This review also covers the
bounded Task-start reader correction below. The
[amended design](data-model-one-table-per.md), [review feedback](data-model-review-feedback.md)
and [Session decision](session-runs-and-current-run.md) govern: taskless
Invocations, Session owning Runs plus its current Run, and Flow as template.
Current-Run-only bind remains an assumption; no historical reassignment was
approved. All eight Done When obligations remain.

The prior selected-attempt P1 is repaired at source level. Complete carries the
lookup's original position into settlement; Open and rename retain the supplied
Run selector across lock waits. The new deterministic race matrix addresses the
reported interleavings. It has compiled but has not executed. This review found
and corrected a separate reachable query against the deleted Task table.

### Corrected here: Task detail still queried the deleted table

**P1 — Task status/roadmap could fail after schema migration.**
`rust/loopflow/src/lf/commands/waves.rs:1090` calls `store.task_started` while
building a registered Task's detail. Its SQL in
`rust/loopflow/src/store/sqlite/chapters.rs:97` still referenced
`task_flow_positions`, which the `retain_flow_invocations` draft drops.
SQLite must prepare the whole query even when another EXISTS arm could be true.
This is a source-established missing-table failure, not an executed CLI result.
The preceding review's negative inventory missed this reader.

Changed that query to retained `flow_invocations`, without a current-only filter,
and included published Run rows as the chapter evidence reader already does.
Kept historical Started/progress/completion events; reservations alone remain
insufficient. No compatibility view, new writer or schema edit was added.
`task_started_tracks_published_review_history_not_reservation` exercises a fresh
ephemeral migrated store, unstarted Task, unpublished reservation, publication,
and completed review with no current invocation. It asserts the displayed fact
through the production reader. The fixture is compiled, unexecuted.

### Selected-attempt repair: source findings and proof limits

Paths in this section are relative to `rust/loopflow/src/`.

- Complete (`ops/human_session.rs:724`) holds the Session launch lock through
  settlement, continuation request and teardown. Controller completion
  (`controller/task/mod.rs:764`) accepts the original expected position instead
  of reloading a newer one. Existing immediate transactions in
  `store/sqlite/children.rs` compare the full position and Session current Run
  before writing feedback, cursor and events. Teardown receives that same
  selected position. Continuation remains requested before provider teardown.
- Open (`human_session.rs:1035`) re-resolves the original exact/prefix selector
  under exclusion and compares the opening snapshot. Native history lookup
  precedes transfer; transfer and client receipt publication share the lock.
  `lf/commands/util.rs:616` releases it before waiting for provider exit, so
  Ready/Complete can proceed during the conversation. Replacement returns the
  Run it launched rather than rereading a newer current pointer after exit.
- Rename (`human_session.rs:1505`) preserves the original selector and passes
  an expected Run for exact/prefix requests into the immediate SQL transaction
  (`store/sqlite/sessions.rs:214`). Stable Session-ID rename remains a
  conversation operation and can name its newer current Run.
- Public CLI dispatch in `lf/commands/session.rs` passes the original ID
  through Open and Complete. Completion's separate worktree lookup selects
  journal scope; it does not rewrite the action's selector. JSON Open prepares
  an action and does not demonstrate native resume.

`review_actions_preserve_selected_attempt_across_replacement`
(`controller/task/mod.rs:2338`) pauses all three actions after lookup, publishes
B with retained A feedback, then resumes. It covers exact IDs, prefixes and
Session IDs with real lookup/SQLite transactions. Rejections preserve B,
cursor, feedback and simulated clients; stable Session rename is the deliberate
exception. Subsequent Open/Complete of B leaves A's client intact, stops B and
records the answer once. Notify barriers synchronize the race, not sleeps.
The native handoff fixture `intentional_session_move_exits_cleanly` separately
uses the real launch lock and an owned shell stand-in to observe publication
before stopping that client. Both fixtures are unexecuted here. Simulated native
effects in the action matrix are not configured provider or actual CLI proof.

### Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Slice: selected attempt | Replacement cannot redirect Complete/Open/Run rename | Original expectation reaches CAS and native exclusion; stable Session rename stays conversational | Source trace above; deterministic action matrix | source repaired; behavioral gap |
| Slice: Task started reader | Task detail survives schema change and retains execution history | Query uses invocations and published Runs plus old event evidence | Bounded correction and new lifecycle fixture | compiled; behavioral gap |
| Done 1: ancestry/history | Nullable parent and structural constraints; stable replacement history | Task review parent/current-Run fences present; general Run/node/tuple/runtime-parent model incomplete | SQL drafts, Session transactions, retained replacement fixtures | gap |
| Done 2: one CLI reader/bind | Session/Run/usage/sidebar agree through typed parents | Task review naming uses SQL; general bind and reader conversion unfinished | CLI dispatch and negative inventory below | gap |
| Done 3: execution | One Task/taskless driver, captured recovery, exact settlement | Task capture and selected-attempt fences retained; taskless driver remains file-backed | Controller/Open source and authored recovery/race fixtures | gap |
| Done 4: Chapter | One repository clock, frozen history and active transfer | Current activation still scoped by Wave; stale Task-start query repaired | `store/sqlite/chapters.rs` | gap |
| Done 5: import | Populated SQL/files, idempotence, interruption, canonical rehearsal | Task-review draft preserves conversion inputs; complete offline importer absent | Three migration drafts and preservation fixtures | gap; no materialized rehearsal |
| Done 6: desktop | Stable Session panes/history and cached typed grouping | Existing DTO still exposes old projection; current-Run/history contract unconverted | Rust/Swift Session model and branch diff | gap |
| Done 7: configured acceptance | Backed-up actual Homes, CLI/app demo, retained drafts and measurements | No conversion, configured provider/app demo or new latency sample | No live acceptance attempted | gap |
| Done 8: deletion/consistency | Alternate authorities gone; final docs/checks agree | Old Task table reader now gone; other Session/Run/Flow owners still reachable | Negative search and static checks below | gap |

### Verification and negative architectural proof

Fresh resource preflight and safe `--recover` both failed: active
`main-view-task` **15.3 GiB / 12 GiB**, this checkout **332.9 MiB**, free disk
**97.2 GiB**. Recovery preserved the foreign active build. TESTING.md stops
product tests under unresolved pressure. No behavioral tests, isolated actual
CLI demo or materialized migration rehearsal ran; an older installed CLI would
not prove these bytes. Source tracing is the available evidence, not a demo.

After the bounded correction, formatting, working-diff whitespace and isolated
`cargo clippy --all-targets -- -D warnings` pass; Clippy took **48.11 s** and
compiled the new fixture. Migration validation passes: three ordered drafts,
52 shipped migrations unchanged. Architecture still fails only SQLite owner
coverage, **32/33**, missing `wave_chapters`; seven other inventories pass.
Generated architecture HTML consistency passes from the website environment.
The initial root-environment HTML command failed on missing `fasthtml`; rerunning
the documented website command resolved that invocation error.

Whole-branch whitespace still flags the unchanged blank draft dependency header
and copied historical patch context. Neither applied migration bytes nor copied
evidence was rewritten for cosmetics. The local diff pass clears neither.

After this correction, searching current Rust outside migration history finds
no `task_flow_positions` reference. No replacement compatibility view exists.
The remaining forbidden end-state paths are still reachable: four-source Session
list (`human_session.rs:575`), Ask feedback reset/name copy (`:1204,1208`),
taskless `position.json` reads/writes (`ops/flow_run.rs:108,136`), Run manifest
scan/WorkCatalog (`lf/commands/runs.rs:77,91`), explicit Started writers, and
name/resolution/native-client sidecars. Swift's Session DTO still lacks the
target current-Run/history contract. Repository Chapter activation is still
Wave-scoped. These are unfinished cutover dependencies, not accepted alternate
implementations. The slice advances the common owner model but cannot ship as
the promised all-caller cutover.

### Next action and proof

When resource preflight permits, execute the new reader fixture and the existing
selected-attempt/native-handoff matrix first, under TESTING.md's isolated Home
and executable rules:

```sh
cargo test -p loopflow --lib task_started_tracks_published_review_history_not_reservation
cargo test -p loopflow --lib review_actions_preserve_selected_attempt_across_replacement
cargo test -p loopflow --lib intentional_session_move_exits_cleanly
cargo test -p loopflow --lib reopening_ask_cannot_overwrite_a_concurrent_completion
cargo test -p loopflow --lib ops::flow_run::tests
```

Keep the previously owed publication, historical selector, stale-provider/Ready,
corrupt-neighbor, repeated replacement, schema, populated migration, controller
and durable-store proofs; repeat populated preservation after canonical
materialization in a disposable source copy. Add an isolated actual CLI
status/roadmap read with a registered Task to demonstrate the corrected consumer.

Continue the shared taskless driver, general Run facts, all Session kinds,
launch/read/bind cutover, DTO/Swift pane identity and caches, repository Chapter
operation, offline import, exact-writer real-Home maintenance, configured proof,
measurements and deletion research followed by deletion. Reconcile the prepared
Run paragraph in `docs/lf.md:562` and specialist docs with final behavior. Retain
the supplied #1296 publication-record and existing-Task stacking reports as
unreproduced scope. No new product decision is required for these steps.

This review changed the Task-start reader, its regression fixture and this note,
and checkpointed the supplied compression note. No provider, Home, migration,
PR, Task disposition or Flow navigation changed. No intermediate publication.

## Previous review: publication recovery and retained Run lookup

2026-09-26 · LOO-298 · review-slice

**Not ready to publish.** Reviewed `cb4ee961ff88784db591758ad87605ca0008cfd8`
against `4cd64be3d0432aa04dd9256293eef7088db97ccc`, concentrating on the recovery
and lookup implementation in `d57ab33d7`. Entry tree was clean. The
[amended design](data-model-one-table-per.md), [review feedback](data-model-review-feedback.md)
and [Session decision](session-runs-and-current-run.md) govern over the older
Task title and copied handoff: taskless Invocations; Session owns Runs and a
current Run; Flow remains the template. All eight Done When obligations remain.

The slice advances the chosen owner model. Historical exact/prefix selectors
now consult retained SQL membership before interactive manifest dispatch;
invalid selected review captures appear on their own conversation rather than
aborting the list; artifact reconciliation precedes the SQL launch claim and
recorder construction. These are source findings, not executed behavior.
A remaining lookup-to-action race prevents accepting the exact-attempt claim.

### Finding: a replacement between lookup and action loses the Run selector

**P1 — carry the selected attempt through completion, Open and rename.**
Paths below are relative to `rust/loopflow/src/`. This is a source-traced
interleaving, not a reproduced runtime result:

1. An external caller addresses current review Run A. `find_session`
   (`ops/human_session.rs:585`) resolves A through its Session and returns its
   `FlowPosition`. The new `review_target` check correctly rejects A if it was
   already historical at that read.
2. Open replaces A with B under the same Session and human boundary. B is
   published; the retained feedback remains ready. Invocation, node, Skill and
   iteration stay identical, while the current Run and position version change.
3. Complete's `complete_flow` (`:717`) reduces A's position to a
   `FlowSessionToken`. That token has no Run ID or position version.
   `controller/task/mod.rs:764` reloads the current position, now B, and passes
   **B's** snapshot to the otherwise correctly fenced settlement transaction.
   `require_current_review_actor` protects a superseded provider's `LF_RUN_ID`,
   but an ordinary external CLI caller has no such actor. It does not protect
   the Run ID explicitly supplied to Complete. The operation can therefore
   complete B from a request that selected A. Teardown still receives A's old
   position (`human_session.rs:731`), so it can stop the wrong attempt as well.
4. Rename similarly resolves the supplied selector, waits for the Session
   launch lock, then resolves the **Session ID** (`:1469`) instead of retaining
   that selector. Open also re-resolves the Session ID (`:1085`) and enters
   `open_boundary` with only that ID. A concurrent replacement after the first
   lookup can turn an attempt-specific request into a current-Session action.

The store's exact-position comparison is necessary and remains intact. It does
not recover the expectation discarded by the caller. The existing new fixture
addresses A only **after** replacement, and checks the superseded provider actor
separately; neither exercises replacement between lookup and action.

Correct this as one caller/settlement change: retain the selected Run and
expected position from resolution to the mutation transaction; recheck the
original exact/prefix selector after acquiring the launch lock; make Open's
native-resume/replacement path honor that expectation too. Do not merely add
one more unfenced read before the effect. A stable Session-ID action may select
its current attempt, but must then settle and tear down that same attempt.
This spans lookup, controller settlement and native effects; no isolated
predicate change was made that would leave the other operation paths unsafe.

Smallest missing proof: deterministically pause an external Complete after it
resolves A, publish B under the same Session with retained feedback, then resume
the request. It must reject, leave B and the cursor unchanged, and stop no client.
Repeat at the rename lock and Open effect boundaries, including a prefix
selector. Then complete B once and verify the exact stopped Run and saved
feedback. Mock provider effects, not the store or lookup. Avoid sleeps as a
race synchronizer. Keep the existing sequential old-Run and old-provider proofs.

### Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Slice: interrupted publication | Retain ID/bytes; claim launch once; no false terminal receipt | `reconcile_reserved_manifest` checks inputs and retained creation time; SQL publication precedes `RunCapture` | `run_record.rs:1581,2210`; `reserved_publication_*`; `review_publication_retry_preserves_identity_and_claims_sql_once` | source advance; fixtures unexecuted |
| Slice: uncertain launch | Missing receipts cannot authorize replacement | Open refuses a published Run lacking native history and a valid terminal outcome | `human_session.rs:1196`; publication fixture calls Open with `resume=true` | source advance; automatic recovery remains a gap |
| Slice: historical identity | Older/completed Runs never become independent conversations | Exact and manifest-prefix lookup consult SQL membership; historical requests identify Session/current Run | `human_session.rs:585`; `flow_session_name_and_membership_survive_sql_run_replacement` | sequential source path repaired; concurrent action gap above |
| Slice: available neighboring Sessions | Malformed autonomous/review capture preserves valid list/Open | Only selected `InvalidData` becomes an identified unavailable review; other store errors propagate | `human_session.rs:1698`; `session_list_and_open_preserve_valid_reviews_beside_unreadable_captures` | source advance; fixture uses Open preparation, not provider resume |
| Done 1: validators/history | All nullable ancestry, structure and replacement constraints | Task review membership/current-Run constraints present; general constructors and runtime nesting absent | SQL drafts, Session transactions, repeated-replacement fixture | gap; no executed ancestry matrix |
| Done 2: common reader/bind | CLI, usage and sidebar share typed Run ancestry | Task-review rename uses SQL; no general bind/filter cutover; manifest readers remain | Session dispatch, `lf/commands/runs.rs:77,91` | gap |
| Done 3: execution preservation | Shared taskless driver; captured recovery; exact stale-result rejection | Task SQL capture retained; taskless file driver remains; action race above | `ops/flow_run.rs:108,136`, controller/session paths | gap |
| Done 4: Chapter operation | One repository boundary, active identity and frozen history | Activation still clears current by Wave | `store/sqlite/chapters.rs:44` | gap |
| Done 5: populated import | All old SQL/files preserved; idempotence and interrupted cutover | Task-review draft only; historical inputs retained; no full offline importer | Populated migration fixtures and draft dependency chain | gap; materialized rehearsal unexecuted |
| Done 6: DTO/desktop | Stable Session panes/history, typed grouping and cached reads | Existing Session wire shape; no integrated current-Run/history DTO or pane migration | Rust `SessionRecord`, Swift model/projection and branch diff | gap |
| Done 7: configured acceptance | Backed-up actual Homes; matching CLI/app; measured demo | No Home conversion, provider/app demo or comparable latency sample | No configured proof attempted | gap |
| Done 8: deletion/consistency | One owner per object; retired paths absent; checks green | Other Session owners, sidecars, file Flow and Started writers still reachable | Negative searches; static checks below | gap |

### Verification and negative architectural proof

Fresh preflight and safe `--recover` both fail: active `main-view-task` is
**15.3 GiB / 12 GiB**; this checkout is **332.9 MiB**; free disk is **97.3 GiB**.
Recovery preserved the active foreign build. TESTING.md's resource rule stops
product tests under unresolved pressure. No behavioral test, fresh-Home CLI
demo or materialized migration rehearsal ran. An installed older CLI would not
prove this branch. The new tests remain compiled-only evidence from the earlier
implementation's Clippy pass; this review did not rerun Clippy.

Fresh `cargo fmt --all --check` and migration validation pass: three ordered
drafts, all 52 shipped migration files unchanged. Architecture still fails only
SQLite owner coverage: **32/33**, missing `wave_chapters`; the other seven
inventories pass. Whole-branch whitespace still fails on the unchanged blank
draft dependency header and copied historical patch context. Applied draft bytes
were not rewritten for whitespace. This review's note-only whitespace check
does not clear those branch findings.

The removed Task table has no current SQL writer or compatibility view. SQL
Session membership is authoritative for retained Task review attempts, and
cursor checkpoints cannot overwrite their title, feedback or current Run.
Still reachable: four-source list (`human_session.rs:571`), Ask JSON and name
copy (`:1230,2190`), taskless `position.json` reads/writes, manifest/WorkCatalog
Run filtering, name/resolution/provider sidecars (`run_record.rs`), and
`record_task_start` (`lf/commands/run.rs:832`). No all-caller deletion proof
follows from the narrower repair. These are unfinished live dependencies;
deleting them before conversion would remove supported behavior.

Continue the coherent ownership cutover with the action race included. Once
preflight permits, run the four recovery/lookup commands in the design's latest
slice ledger plus the new interleaving proof, then the earlier schema,
replacement, populated-import, controller and durable-store proofs already owed.
Repeat populated migration after materialization in a disposable exact source
copy. Keep current-Run-only binding as an assumption, including retained earlier
usage and atomic nullable-Task mismatch rejection. Repository Chapter, offline
import, real maintenance, configured desktop proof, measurement and deletion
research remain required. The supplied #1296 publication-record and existing-Task
stacking reports remain unreproduced scope; no delivery state was repaired here.

This review changes only this topic note. No product source, migration, Home,
provider, PR, Task disposition or Flow navigation changed. It does not authorize
an intermediate publication; the following decision step owns navigation.

## Current disposition after integrating LOO-291

2026-09-26 · LOO-298 · review-slice

**Not ready to publish.** `lf rebase` completed onto
`4cd64be3d0432aa04dd9256293eef7088db97ccc` after checkpointing the review below.
Integration HEAD was `afed3cffd693169df30ac765f4869f8a3df41255`; this addendum
also covers the bounded corrections in its working diff. No push, Task
completion, installed Home change or Flow navigation was requested.

The amended design and all eight Done When obligations still govern. The
pre-rebase matrix below remains the detailed scope inventory; its exact source
line references and upstream-absence claims are historical. Reconciled source
now includes LOO-291's checkout binding, required Session Run ID, membership
DTOs and desktop work. These are useful prerequisites, but do not provide the
target Session/current-Run/history DTO or table-only general Run readers.

| Claim | Planned behavior | Integrated implementation | Proof | Result |
| --- | --- | --- | --- | --- |
| Task review ownership | Session retains name, feedback and history across replacement | SQL reservation and current-Run pointer retained; child manifest receives reserved ID and captured membership | `reserved_run`, `reserve_review_run`, launch dispatch, source review | source only; behavior gap |
| Rename through current Run | Human title survives generated suggestions and replacement; membership unchanged | Existing rename command routes Task reviews to SQL; other kinds still write sidecars | Added `flow_session_name_and_membership_survive_sql_run_replacement` | compiled, unexecuted |
| Preparation recovery | Interrupted publication resumes without losing evidence or duplicating provider | Task path still publishes immutable artifacts before SQL publication; upstream prepared-file path serves other kinds | `lf/commands/run.rs`, `begin_reserved_with_context`, `publish_run_binding` | gap; finding 1 below persists |
| Independent discovery | Unrelated malformed autonomous capture does not hide valid review | SQL selects review Sessions; `review_surface` validates each selected review's invocation for membership/actions | `list_flow_sessions`, `review_surface`, `open_review_sessions` | source only; selected malformed reviews can still fail list |
| One owner across callers | Common Session/Run/Invocation records, bind and shared taskless driver | Four-source inventory, file Flow/Ask, name-copy helper, manifest Run readers and sidecars remain | Negative source inspection after rebase | gap; intermediate publication remains disallowed |

Conflict resolution removed an upstream naming/membership test written for the
former Task file owner. Restored its useful behavior against SQL: rename by
current Run ID, replace the Run, reject a generated title overwrite, retain
feedback/membership/Task, agree with Session listing, and retain both Run IDs.
The test uses an ephemeral store and the existing isolated Home/executable
fixture, without provider launch. It does not substitute for the repeated
replacement/stale-actor or configured CLI/app proofs. Removed the now-unused
store parameter from `prepare_boundary`; post-rebase Clippy had rejected it.
The earlier schema assertion correction survived integration.

Post-rebase verification on these corrections:

- `cargo fmt --all --check` and isolated
  `cargo clippy --all-targets -- -D warnings`: **PASS** (Clippy 17.45 seconds).
- Migration validation: **PASS**, three ordered drafts and 52 unchanged shipped
  files. Architecture HTML consistency: **PASS** in the website environment.
- Architecture: **FAIL**, still **32/33** SQLite owners, missing `wave_chapters`;
  the other seven inventories pass. Working-diff whitespace passes; branch
  whitespace still reports the unchanged draft header and copied patch context.
- Behavioral execution: **NOT RUN**. Rebase recovery reran resource preflight
  and safe recovery: active `main-view-task` **15.3 GiB / 12 GiB**, **97.3 GiB**
  free. The active foreign build was preserved. This review reuses that receipt;
  compilation is not runtime SQL, provider, migration or desktop proof.

Next implementation should repair the exact artifact/SQL publication boundary,
then complete the approved owner/caller cutover. When resource preflight permits,
run the focused commands below plus
`flow_session_name_and_membership_survive_sql_run_replacement` and the
reconciliation command recorded in [questions](questions.md#rebase-onto-main-4cd64be3d-2026-09-26).
Repeat populated preservation after draft materialization. The new publication
tracking and existing-Task stacking reports remain explicit implementation scope,
not reproduced defects or repaired delivery state. No new approval is needed to
continue the approved implementation.

## Pre-rebase review: Task review Session ownership

2026-09-26 · LOO-298 · review-slice

Reviewed HEAD `11fdf61cdf62f629ba7961945fdb6b37ecfc855c` against Task PR base
`a48eeb6aed383123c51a6c6cc673591ae19b1417`; entry tree was clean. Scope is the
branch change, especially the Task review owner and subsequent compression.
The [amended design](data-model-one-table-per.md), [review feedback](data-model-review-feedback.md)
and [Session decision](session-runs-and-current-run.md) govern over the older
Task title: taskless Invocations and Session owning Runs plus a current Run.

**Disposition: not ready to publish.** Task reviews now use a real Session
owner, and the old unrelated-capture discovery defect is removed at source
level. This advances the design; it remains an internal prerequisite with no
executed behavioral proof, not the requested all-caller cutover. All eight
Done When obligations still apply. No navigation decision is made here.

### Findings and bounded correction

1. **Preparation recovery still strands a reserved Run.**
   `lf/commands/run.rs:754–774` publishes the reserved manifest before
   `publish_run_binding` marks the SQL Run published. An interruption between
   these writes retains the unpublished reservation. `reserve_review_run`
   (`store/sqlite/sessions.rs:84`) returns that same ID, but
   `run_record.rs:1860` creates an exclusive staging directory and renames it
   over the published directory. Existing staging or published contents make
   the retry fail. This confirms the recorded gap by source tracing; no fault
   injection ran. Recovery must reconcile the exact immutable input and launch
   evidence without deleting artifacts or assuming an absent receipt proves
   provider death. The newly reported upstream prepared-Run implementation
   must be reconciled before choosing that repair.
2. **The behavioral proof stops at the store boundary.**
   `review_session_retains_feedback_and_history_across_replacement_and_corrupt_neighbors`
   directly seeds a published Run, replaces it twice, and checks store
   discovery/settlement. It does not call CLI Open, reserve through the real
   child launcher, publish artifacts, or exercise
   `require_current_review_actor` from a superseded provider. It is useful
   ownership proof once executed, but cannot establish the required launch,
   reopen, stale-actor or configured desktop experience by itself.
3. **Fixed an obsolete schema assertion.**
   `store/migrations.rs::native_human_session_schema_uses_flow_invocations`
   applied the current draft tail, then required the now-renamed
   `session_run_id` and `ready_summary` columns. Updated it to require
   `pending_session_id` and the two historical inputs and reject the retired
   column names. No migration bytes changed. This is a source-detected test
   defect; execution remains blocked, so no red-to-green result is claimed.
4. **The remaining owners still prevent acceptance.**
   Public Session inventory remains a four-source union. Ask replacement still
   clears saved feedback (`human_session.rs:644,840`); ordinary/taskless Flow
   persistence still uses files; general Run readers still scan manifests.
   Bind, typed general launch ancestry, common taskless driver, Session history
   DTO/desktop consumers, repository Chapter rotation and offline import remain
   incomplete. Do not publish or activate this draft over an installed Home.

### Evidence matrix

`pass (source)` means inspected implementation only. No product behavior ran.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Slice: stable conversation | Two replacements retain identity, title, feedback and old Runs | Session owns feedback/current pointer; replacement appends Run under an immediate transaction | `sessions.rs:84,269,341`; repeated-replacement fixture | pass (source); execution gap |
| Slice: exact settlement | Stale replacement, Ready and Complete fail; feedback consumed once | Expected position/current Run/publication checks; completion and Task event share transaction | `sessions.rs:118,224`; `children.rs:231,271`; fixture at `durable.rs:1613` | pass (source); actor/CLI proof gap |
| Slice: available review | Unrelated malformed invocation cannot hide valid Session | Direct Session/current Run query; exact selected Task capture still validates | `human_session.rs:1167,1266`; `sessions.rs:149`; corrupt-neighbor fixture | pass (source); Open unproven |
| Done 1: complete validators | Nullable ancestry, node/tuple/parent and current-Run invariants | Session member FK and Run parent triggers present; general constructors and runtime parent tree absent | Session SQL draft; populated import fixture | gap |
| Done 2: common CLI reader | Launch, bind/rename and Run/usage/Session agree | Rename works through SQL for Task reviews only; other kinds/readers unconverted | `lf/commands/session.rs:33`, `human_session.rs:305`, `lf/commands/runs.rs:57` | gap |
| Done 3: execution preservation | Shared taskless driver; captured recovery and exact stale-result fencing | Task transactions retain history; file adapter and publication retry gap remain | `flow_run.rs:108,136`, finding 1 | gap |
| Done 4: repository Chapter | All Waves rotate with frozen evidence and active identities | Still Wave-scoped activation | `store/sqlite/chapters.rs:39` | gap |
| Done 5: populated Home import | Idempotent complete import; interruption/cutover preservation | Task review SQL conversion only; historical columns retained; no Home rehearsal | Draft plus `session_ownership_import_preserves_nested_reviews_and_nullable_parent_constraints` | gap; unexecuted |
| Done 6: DTO/desktop | Stable pane, Run history and typed grouping | Existing Rust/Swift Session wire shape unchanged | Branch diff and Session consumers | gap |
| Done 7: configured acceptance | Actual backed-up Home and installed CLI/app demo; measurements | No Home conversion, promotion, CLI demo or mounted pane proof | None attempted | gap |
| Done 8: deletion/consistency | One owner for every kind, no old readers/writers, passing checks | Task review inventory removed; other old owners and Started writes remain | Negative searches below; static checks | gap |

### Negative architectural proof and verification

`human_task_flow_positions` is gone. Current SQL no longer addresses
`task_flow_positions` or writes the two historical Session-input columns.
Cursor checkpointing an existing Session cannot replace its Run or feedback;
reservation, publication and Ready own those updates. Current invocation
claims and retained closed claims remain distinct. Those are useful reductions.

Still reachable: four-source `human_session::list` at `:305`, Ask JSON
read/write, `flow_run` `position.json` read/write at `:108,136`, manifest/subject
Run filters at `lf/commands/runs.rs:57`, provider identity/client/resolution
sidecars in `run_record.rs`, and Started writes in `chapters.rs:86` and the
invocation migration trigger. There is no complete architectural deletion proof.

| Check | Result |
| --- | --- |
| Resource preflight and safe `--recover` | FAIL: active `main-view-task` **15.3 GiB / 12 GiB**, **98.7 GiB** free; active foreign build preserved |
| Product tests, materialized migration rehearsal, configured demo | NOT RUN: TESTING.md resource rule blocks execution; installed code is not a substitute |
| `cargo fmt --all --check` | PASS |
| Isolated `cargo clippy --all-targets -- -D warnings` | PASS; compiled test targets, did not execute them |
| Migration validation | PASS: three ordered drafts; 52 shipped files unchanged |
| Architecture checker | FAIL: SQLite owner coverage **32/33**, missing `wave_chapters`; other seven inventories pass |
| Portable architecture HTML consistency | PASS in website environment |
| Whole-branch whitespace | FAIL: earlier blank draft dependency header and copied patch context; unchanged |
| Working-diff whitespace | PASS |

Clippy compilation cannot establish runtime SQL behavior. Earlier review findings about Task review
feedback reset and unrelated-invocation inventory failure are superseded by
the new source path, while their behavioral acceptance remains unproven.

### Next action and proof

The new directive reports LOO-291 merged as `4cd64be3d` / PR #1277 and requests
`lf rebase` at the next boundary. It adds two delivery gaps to LOO-298:
publication of #1296 missing from its Task PR row, and stacking already-created
Tasks. These are supplied observations, not independently reproduced defects
in this review. Keep them in scope without silently rewriting delivery state.
`lf rebase --plan` selects `direct_rebase` (`clean_authored`, 14 unique commits).
Preserve this dated review before integration; rebase may invalidate its exact
line references and executable evidence. Reconcile upstream prepared Run,
checkout binding, Session and migration-tail changes through `lf rebase`.

When resource preflight permits, run the focused proof under TESTING.md's
isolated environment, including the corrected schema assertion:

```sh
cargo test -p loopflow --lib native_human_session_schema_uses_flow_invocations
cargo test -p loopflow --lib review_session_retains_feedback_and_history_across_replacement_and_corrupt_neighbors
cargo test -p loopflow --lib session_ownership_import_preserves_nested_reviews_and_nullable_parent_constraints
cargo test -p loopflow --lib controller::task::planning_tests
cargo test -p loopflow --lib store::sqlite::durable::durable_store_tests
```

Keep the earlier projection and invocation preservation commands owed; repeat
populated migration proofs after materialization in a disposable source copy.
Add real launch-path fault injection around manifest/SQL publication and
superseded-Run completion, then finish the common Invocation/Run/Session owner
and all consumers. Preserve the current-Run-only bind assumption, earlier Run
attribution, and atomic taskless null-to-Task rejection. Chapter, Home import,
desktop/configured acceptance, measurements and deletion research remain the
full design's obligations. No intermediate publication is warranted.

## Earlier review: invocation retention

2026-09-26 · LOO-298 · review-slice

Reviewed HEAD `54abb81ff5deb165bc4e39473766efa53220d65b` against Task PR base
`a48eeb6aed383123c51a6c6cc673591ae19b1417`; the working tree was clean.
Scope: the complete branch change, with particular scrutiny of invocation
retention at `60639b3e6`. Governing intent remains the
[amended design](data-model-one-table-per.md),
[review feedback](data-model-review-feedback.md), and
[Session ownership decision](session-runs-and-current-run.md).
The later participant's approval (name unresolved) supersedes the older Task
title: taskless Invocations and Session owning Runs plus a current Run.

**Disposition: not ready to publish.** Retaining Task invocations advances the
approved architecture without adding a second Task writer. It does not complete
the shared Invocation/Run/Session cutover and is explicitly not an independently
publishable slice. Behavioral preservation remains unexecuted. This review
changes only this note; no PR, Task, Home or Flow navigation mutation occurred.

### Findings

1. **The retained invocation is not yet a stable conversation owner.**
   Completion now retains capture, cursor, pending Run, feedback and final claim
   in `flow_invocations`; restart and authorized reopen mark the old row
   replaced. However, Session discovery and exact review lookup still enumerate
   current Task invocations and decode each one before selecting human steps
   (`store/sqlite/durable.rs:392`, `ops/human_session.rs:1166`). An unrelated
   malformed current capture can still prevent reaching a valid Session.
   `open_boundary` still clears Ask feedback at `human_session.rs:780` and Task
   review feedback at `:799` before replacement. Invocation retention does not
   address either counterexample from the previous review. These are
   source-traced findings, not executed reproductions in this review.
2. **The implemented lifetime boundary is coherent at source level.**
   The partial index selects one current invocation per Task. Current writes
   and claims select `state='current'`; unclaimed position updates also match
   invocation ID. Human completion reads and compares the entire expected
   position inside an immediate transaction before closing it; worker completion
   matches the serialized claim, including invocation identity. Chapter reads
   retain historical execution while excluding closed claims from current
   worker activity. No bounded source defect was found requiring another code
   change in these paths. This is not a behavioral pass.
3. **Verification is still blocked by the measured resource envelope.**
   Preflight and safe recovery both fail at active
   `jack-heart/main-view-task`: **15.3 GiB / 12 GiB**, with **98.8 GiB** free.
   Recovery preserves the active foreign build. TESTING.md's
   [resource rule](../TESTING.md#bounded-and-honest) says unresolved pressure
   stops product tests. Therefore neither focused Rust execution nor disposable
   materialized migration proof ran. No installed binary or private Home was
   used as a substitute for the changed code.

### Evidence matrix

`pass (source)` describes inspected code only. Every full-design acceptance
claim remains open; authored or previously compiled tests are not executed proof.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Slice: retain invocation lifetime | Completion/restart preserve execution and review history | One `flow_invocations` table; close instead of DELETE; identity matches capture JSON | Draft `retain_flow_invocations`; `durable.rs:1177`, `children.rs:275,355`, `durable.rs:906` | pass (source); execution gap |
| Slice: stale writers and retirement | Reused versions cannot target a replacement; old execution cannot become untouched backlog | Invocation-ID update predicate, exact completion comparisons, historical chapter evidence and current-only claim query | `durable.rs:1030`; `children.rs:241,268`; `chapters.rs:98,101,125`; authored store tests | pass (source); execution gap |
| Slice: populated conversion | Preserve prior bytes and reject ambiguous identity/ancestry atomically | Copies all retained columns; uniqueness, JSON-ID equality and Task FK; no compatibility view | `migrations.rs:4778` fixture compares bytes and uses savepoint rollback for duplicate IDs/dangling Tasks | gap: unrun; fixture uses direct SQL, not installed promotion |
| Done 1: ancestry and Session validators | Nullable parent equality, structural checks, stable Session/current Run, fenced repeated replacement | Task FK and invocation identity constraint only; Run/Session owner APIs absent | Schema and current Store APIs | gap |
| Done 2: one CLI reader | Launch/bind/rename; Session/Run/usage agree, including terminal Tasks | Manifest/subject reads remain; Session bind, rename and Task filter absent | `lf/mod.rs:721`, `lf/commands/runs.rs:51`, `ops/human_session.rs:269` | gap |
| Done 3: execution preservation | Shared taskless/Task storage and driver, captured nesting and exact review recovery | Task history retained; ordinary Flow file persistence and existing cursor model remain | `ops/flow_run.rs:108,136`; new and existing recovery tests unrun | gap |
| Done 4: Chapter operation | One repository boundary, frozen history, all-Wave transfer/retry | Activation still clears current only for one Wave | `store/sqlite/chapters.rs:39` | gap |
| Done 5: populated Home import | Preserve old SQL/files, idempotence, interrupted publication and cutover | Forward Task migration only; no offline Home importer or rehearsal | Branch source/migration diff | gap |
| Done 6: DTO/desktop | Stable Session pane across replacement/bind; typed ancestry and cached grouping | No Rust/Swift DTO, shared fixture or desktop change | Branch path inventory and current Session model | gap |
| Done 7: configured acceptance | Backed-up actual Homes, matching CLI/app, retained identities and latency measurements | No Home inventory, conversion, promotion, mounted pane or configured demo performed | No runtime acceptance receipt | gap |
| Done 8: deletion/consistency | Retired owners unreachable; docs match implementation; checks pass | Old Session/Run/taskless owners and Started writes remain; canonical docs are still the target spec | Negative searches and static results below | gap |

### Negative architectural proof and static checks

Searched current Rust callers, SQL writers, migrations, CLI and Session paths.
The retired `task_flow_positions` table has no current SQL reader or writer;
its remaining SQL occurrences are historical migration inputs and fixtures.
There is no compatibility view or duplicate Task write. Historical root
`step_index` and `iteration` still feed `decode_flow_position` and cannot be
dropped before conversion. Final claims remain historical evidence, not active
authority.

The broader forbidden outcomes remain reachable: `human_session::list` still
unions four sources; ordinary Flow `read`/`write` still use `position.json`;
Run listing still scans manifests and matches subjects through `WorkCatalog`;
provider identity, client receipts and Session resolution remain mutable files;
`begin_chapter_task` and the retained trigger still write Started. These are
existing incomplete cutover paths, not new adapters selected by this slice.
They must disappear through the approved replacement before publication.

| Check in this review | Result |
| --- | --- |
| Resource preflight, then `--recover` | FAIL as described above; no product test ran |
| `cargo fmt --all --check` | PASS |
| `uv run python scripts/check_migrations.py` | PASS: two ordered drafts, 52 shipped migrations unchanged |
| `uv run python scripts/check_architecture.py` | FAIL: SQLite coverage 30/31, missing `wave_chapters`; other seven inventories pass |
| Website environment: `uv run python ../scripts/render_architecture_html.py --check` | PASS |
| `git diff --check a48eeb6aed383123c51a6c6cc673591ae19b1417` | FAIL: earlier draft dependency-header trailing space and copied patch context lines |
| `git diff --check` for this review note | PASS |

Prior Clippy and documentation-test receipts retain their recorded scope on
unchanged executable/documentation bytes; they were not rerun here. The new
table removes the earlier architecture inventory gap for `task_flow_positions`;
it does not resolve Chapter ownership. Applied draft checksums and original
copied patch evidence were not rewritten for whitespace. A clean working-note
check cannot clear those branch-range failures.

### Next action and proof

Continue the approved coherent owner cutover, using
[concept-review](concept-review.md) for the two binding consequences. Preserve
current-Run-only binding as the recorded assumption, earlier Run attribution and
usage, and atomic taskless null-to-Task rejection under nullable equality.
Do not add another Session projection or restore the removed human column.

The first Session proof must replace its Run twice, retain identity/title/feedback
and both previous Runs, reject stale replacement and old-Run Ready/Complete,
then consume the saved feedback once. The same valid Session must remain
listable/openable beside an unrelated malformed invocation, with the latter's
bytes and identified recovery problem preserved. Taskless review recovery must
work after template removal through the same owner and exact settlement fences.

Once preflight permits, run the isolated focused commands from TESTING.md:

```sh
cargo test -p loopflow --lib dropping_task_step_projection_preserves_execution_and_review_evidence
cargo test -p loopflow --lib retaining_invocations_preserves_populated_execution_and_review_bytes
cargo test -p loopflow --lib store::sqlite::durable::durable_store_tests
cargo test -p loopflow --lib stale_human_decisions_cannot_target_a_replacement_invocation
```

Repeat populated migration proofs after materialization in a disposable exact
source copy. Keep the real Home untouched during this proof. The complete
Chapter, Home import/cutover, DTO/desktop, configured acceptance, measurement,
and deletion obligations remain those in the design's eight Done When items.
This review chooses no navigation edge.

## Earlier review: projection-column removal

The assessment below records the earlier HEAD. Its two-table architecture gap
and 14.5 GiB resource reading are superseded by the current measurements above;
its Session availability and incomplete-cutover findings remain applicable.

2026-09-26 · LOO-298 · review-slice

Reviewed HEAD `6580b27cd3ccf5726dbb28c3efbdfee5b7629a76` against Task PR base
`a48eeb6aed383123c51a6c6cc673591ae19b1417`; the working tree was clean.
Scope: the complete branch diff, with executable changes limited to removing
four Task Flow projection columns and sharing their reader. Governing intent:
[amended design](data-model-one-table-per.md),
[completed review feedback](data-model-review-feedback.md), and
[Session ownership](session-runs-and-current-run.md). The later approval permits
taskless Invocations and makes Session the stable owner of Runs and a current
Run; the older Linear title and copied handoff do not override it.

**Disposition: not ready to publish.** This is a useful preparatory reduction,
with no new parallel owner, but its behavioral proof is still unexecuted and
the Task's owner cutover is absent. It is not an independently shippable slice
under the design. No PR mutation, Task completion, Home migration or Flow
navigation decision was made. This review changes only working notes.

## Findings

1. **Session availability now depends on every Task capture decoding.**
   `human_task_flow_positions` selects every position and calls
   `decode_flow_position` before testing `is_human`
   ([durable.rs](../rust/loopflow/src/store/sqlite/durable.rs#L392)). Previously
   SQL excluded autonomous rows. An invalid autonomous capture, cursor or claim
   now aborts discovery. `human_session::list` propagates that error before
   returning Ask, standalone Flow or interactive Sessions; exact Flow-session
   lookup also enumerates the same collection
   ([human_session.rs](../rust/loopflow/src/ops/human_session.rs#L269),
   [lookup](../rust/loopflow/src/ops/human_session.rs#L1162)). Thus an unrelated
   autonomous Task can make a valid review unavailable, not merely add a
   diagnostic to inventory. This is source-traced behavior, not an executed
   reproduction. Earlier notes identified the broader failure surface; this
   review follows its consequences through listing and opening a review.
   Resolve it in the Session owner cutover. Do not restore a stored human flag
   or silently discard corrupt execution evidence. Prove a valid unrelated
   Session remains listable/openable while the broken invocation remains a
   visible, identified recovery problem.
2. **The full target remains absent.** Current Session inventory is still the
   four-source union; ordinary Flows still read/write `position.json`; Run
   queries still scan manifests and match subjects. Bind, rename and
   `session list --task` are absent from this checkout's `SessionCommand`.
   Chapter activation remains Wave-scoped. These are existing remaining scope,
   not newly introduced adapters. They prevent treating documentation or this
   column deletion as the Task's acceptance result. In particular, the revised
   stable Session identity and current-Run transaction have no implementation
   or replacement-race proof yet.
3. **Proof remains unavailable, not green.** Resource preflight and safe recovery
   both fail on active `jack-heart/main-view-task`: 14.5 GiB / 12 GiB, with
   99.7 GiB free. [TESTING.md](../TESTING.md#bounded-and-honest) stops product
   tests under unresolved pressure. No foreign active build output was removed.
   The new migration and nested-review tests have prior compilation evidence,
   but neither that evidence nor this source review proves execution.

## Evidence matrix

`pass (source)` below establishes only the inspected shape. It does not mean
the corresponding behavior ran.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Slice: remove four duplicates | Capture/cursor supply Flow, step, node and human policy | One shared SELECT/decoder; all three changed INSERT/UPDATE paths omit the four columns | Full Rust diff; search all `task_flow_positions` references; forward draft | pass (source) |
| Slice: preserve prior execution | Capture, cursor, feedback, versions, claims, failures and chapter start survive | DROP COLUMN only; historical root inputs and claim predicates retained | Populated preservation test at `migrations.rs:4677`; execution not run | gap |
| Slice: discover nested review and leave it | Selected captured XOR child supplies human policy; autonomous successor disappears from review discovery | New test checks discovery, feedback equality, exact read and stale write rejection | `review_discovery_follows_the_captured_nested_step`; execution not run | gap |
| Done 1: ancestry and Session validators | Nullable Task inheritance, mismatch rejection, stable Session/current Run, fenced replacement | Target records and validators not added | Current schema/types; no corresponding source change | gap |
| Done 2: one CLI reader | Launch, bind/rename, Session/Run/usage agree, including landed Tasks | File scans and selector matching remain; CLI verbs/filter absent | `lf/mod.rs:721`, `lf/commands/runs.rs:51`, `run_record.rs:426` | gap |
| Done 3: execution preservation | Common taskless/Task invocation owner; captured recovery and exact settlement | Existing Task SQL and ordinary file adapters remain; fencing preserved in changed SQL | `ops/flow_run.rs:34,108,136`, `store/sqlite/durable.rs:1120`; existing tests not rerun | gap |
| Done 4: Chapter operation | One repository boundary, all Waves, frozen history | `save_chapter` still clears current by Wave | `store/sqlite/chapters.rs:33` | gap |
| Done 5: populated import | Import all old owners, idempotence, interruption and preservation | Only projection-column migration; no Home importer | Complete migration diff | gap |
| Done 6: DTO/desktop | Stable Session pane, current Run/history, cached typed grouping | No DTO, fixture or Swift change | Complete branch diff | gap |
| Done 7: configured acceptance | Backed-up Homes, matching CLI/app, real demo and measured reads | No import or installed candidate acceptance | No live mutation or demonstration attempted | gap |
| Done 8: deletion and consistency | Retired owners unreachable; docs match final runtime; checks pass | Four-source union, subjects, mutable sidecars, file Flow owner and Started writer remain | Negative searches below; architecture checker fails two owners | gap |

## Negative architectural proof

The searched current paths still include:

- `ops/human_session.rs:269`: Task reviews + Ask files + standalone Flows +
  interactive Runs; `list_ask_sessions` at 1131 reads JSON records.
- `ops/flow_run.rs:108,136`: reads and atomically replaces the ordinary Flow
  file; this still supports optional Task selection.
- `lf/commands/run.rs:740`: converts selector strings to `SubjectAttribution`
  for capture; `lf/commands/runs.rs:57` scans then filters through `WorkCatalog`.
- `run_record.rs:640,774,790,912,934`: provider identity, client attachments and
  Session resolution remain mutable filesystem state.
- `store/sqlite/chapters.rs:78`: writes Started history; the current chapter
  trigger also remains. Their retirement safety consumer is still live.

No current SQL reference to the four removed columns was found outside
historical migrations and migration fixtures. `step_index` and `iteration`
still feed the historical flat-progress decoder, so retaining them is justified.
No new Legacy/New dispatch, dual write or generic abstraction was introduced.
The cut advances the design locally without establishing its deletion boundary.

## Checks and demonstration boundary

| Command / evidence | Result in this review |
| --- | --- |
| `uv run python scripts/resource_envelope.py` | FAIL: active `main-view-task` build over budget |
| `uv run python scripts/resource_envelope.py --recover` | Same FAIL; active build preserved |
| `cargo fmt --all --check` | PASS |
| `uv run python scripts/check_migrations.py` | PASS: one draft; 52 shipped migrations unchanged |
| `uv run python scripts/check_architecture.py` | FAIL: SQLite map 29/31, missing `task_flow_positions`, `wave_chapters`; other seven inventories pass |
| `uv run python ../scripts/render_architecture_html.py --check` from `website/` | PASS |
| `git diff --check a48eeb6aed383123c51a6c6cc673591ae19b1417` | FAIL: blank draft dependency header has trailing space; inherited patch has space-only context lines |

The first HTML check was invoked from the root Python environment and failed
to import `fasthtml`; the prescribed website environment passed. That setup
failure is not an HTML regression. Earlier Clippy and documentation-test passes
apply to unchanged executable/documentation bytes; they were not rerun here.

The branch-range whitespace failure is narrower than a product defect but
means the earlier generic whitespace-pass wording is not a clean whole-branch
receipt. The copied patch retains its original context bytes. The already
committed migration was not rewritten for cosmetic whitespace: installed draft
checksums can make that a schema-history change.

The most important available local demonstration would be the populated SQLite
upgrade followed by nested review discovery. The resource boundary prevented
running it. An installed CLI would exercise different schema/code and cannot
substitute for that proof. No live, simulated-provider, or desktop behavioral
pass is claimed.

## Next action and proof

Once preflight passes, execute the two owed focused commands in the isolated
environment described in TESTING.md:

```sh
cargo test -p loopflow --lib dropping_task_step_projection_preserves_execution_and_review_evidence
cargo test -p loopflow --lib store::sqlite::durable::durable_store_tests
```

Also prove the populated migration on materialized drafts in a disposable
source copy, never by materializing this worker's checkout. The existing
regression checks retained bytes and recovered capture/cursor; full import
will additionally need typed, valid historical claims and end-to-end recovery.

Continue slices 2–4 as one owner cutover: typed Invocation/Run/Session storage,
shared taskless and Task execution persistence, exact current-Run replacement,
all writers/readers, then DTO/desktop consumers. Include the unrelated-invalid-
invocation counterexample above. Preserve the current-Run-only bind assumption
without rewriting earlier Runs. Follow with repository Chapter conversion,
Home rehearsal and configured acceptance, then deletion research. Keep the
present-tense documentation spec; make the code match it before publication.

No bounded executable fix was selected in this review: restoring a projection
or skipping corrupt records would undermine the chosen owner model, while the
required replacement spans the unimplemented architectural slice. The following
decision step owns navigation using this evidence.
