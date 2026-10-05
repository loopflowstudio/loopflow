# Start and finish Tasks without unrelated workflow prerequisites

Implementation plan · LOO-367 · drafted 2026-10-02, reconciled 2026-10-04

## Implementation counterexample · 2026-10-04

Inspection at `8094b45516ed6ce40305d7bdbefb0c7e87673c88` found that the
authorized revision's assumption about an existing safe Session handoff is not
supported by this branch. Production code was left unchanged. Jack Heart's
accepted experience below remains required; the previous implementation is not
ready for gate or demo of that experience.

- `ops/human_session.rs::bind` and `store/sqlite/sessions.rs::bind_session`
  change attribution only. They do not move a provider or change its cwd.
- `serve_locked` launches the conversation with `session.cwd`, then waits for
  the child to exit. `conversation_launch_args` selects native TUI mode. There
  is no directory-change acknowledgement in this path.
- `resume_native_session` stops the current native clients before resuming.
  The public Move here action explicitly warns that unsent text is lost.
  Reusing it automatically for binding or completion would weaken preservation.
  Calling it from the requesting provider also risks stopping the caller before
  the completion result returns. That risk is a source-derived counterexample,
  not a freshly reproduced provider failure.
- `cleanup_completed_task` correctly keeps live/unknown execution. Updating a
  Session row cannot prove that execution left the directory; extending the
  requesting-conversation exemption to cleanup would permit unsafe removal.

October 4 `lf task status LOO-353 --json` returned brief revision
`2026-10-04T17:58:39.566Z`: that Task remains kickoff-only pending design review,
and owns the replacement ongoing-conversation/interactive-handoff machinery.
Its accepted removal direction is not an available implementation to integrate.
Unpublished code in other checkouts was not inspected.

The missing interface is a Session-owned safe directory handoff: preserve native
identity/history and draft input, acknowledge the actual provider execution cwd,
and let a self-issued operation return before any driver replacement. On Task
completion it must leave the disposable checkout and trigger the existing cleanup
retry without ending the conversation. Failed handoff retains the checkout and
original outcome. LOO-367 must consume that interface rather than create a second
driver or extend worker/review machinery slated for deletion by LOO-353. Its
ownership and concrete mechanism need reconciliation with LOO-353 before the
dependent implementation continues; no new product preference is inferred.

Independent placement work remains in this PR's scope, but is not a substitute
for the handoff. Allocation must retain branch/base recovery facts without a PR:
today `create_prepared_task`, `insert_task_with_worktree`, `WorktreeInitializing`
and `restore_task_checkout` all use the first PR as that owner. Remove that
coupling together, create the first PR only at an explicit delivery operation,
and prove PR-free restoration and cleanup against retained allocation evidence.
Do not merely waive the empty-first-PR refusal. Preserve dirty original-directory
files in place; no automatic transfer of pre-existing edits is selected.

Required proofs: same-conversation bind/edit in the new checkout, self-completion
and response after safe departure, draft/history preservation, interrupted handoff
recovery, and independent live/unknown execution refusal. PR-free restoration and
no-landing cleanup also remain unproven. LOO-379 stays separate.

## Authorized revision · 2026-10-04

Jack Heart ended this review and requested an lf pursue process to redo the code
against the revised design. This section governs the next implementation pass;
older implementation descriptions below are evidence, not a competing specification.

- Default the normal open-ended-conversation → focused-Task transition to a
  worktree ready for edits. Preserve the conversation and Task identities/history.
  Planning-only filing/inspection need not allocate execution placement. Optional
  absence of placement is a supported case, not the primary working experience.
- A checkout alone creates no PR or Flow obligation. Remove the coupling that
  creates an unpublished first PR merely to obtain placement. Use existing owners;
  introduce no replacement Task workflow or configurable policy framework.
- Completing a research/no-change Task clears its disposable worktree even when
  nothing was published or merged. Keep the conversation available and the outcome
  visible. Resolve how execution leaves that worktree safely within the existing
  Session machinery; an indefinitely retained checkout is not success. Preserve
  pre-existing edits and actual independent work. Explain real blockers without
  manufacturing a delivery requirement from allocation.
- Retain issue-specific confirmation and shared Flow execution. Integrate against
  LOO-353's removal of Task workers/privileged managed Flows; use a common Flow
  presentation rather than deepening the managed/independent split. Do not implement
  LOO-353's entire deletion project here or restore its retired machinery during sync.
- LOO-353 explicitly deletes Session ready/complete APIs. It does not establish
  deletion of Task completion, so do not independently remove that operation on the
  strength of the earlier ambiguous remark.

Choose routine implementation mechanics autonomously and record consequential
tradeoffs. Prove the normal transition, edits landing in the intended checkout,
no-landing cleanup, preserved conversation/history/outcome, and real-blocker recovery
through public operations. Reuse valid evidence; add focused tests for changed
behavior. Run the authored pursue sequence and return at its next human demo.
This authorizes revision and that Flow's preparation/publication, not landing or
installed-Home promotion. Preserve all review notes and the separate research result
([LOO-379](https://linear.app/loopflow/issue/LOO-379)); do not absorb its repair scope.

Jack Heart's accepted direction is recorded in the Task brief and
[Task workspace review](../../docs/reviews/task-workspace.md): Task supplies
purpose, context and continuity; execution and delivery are optional. There is
no Task workflow to introduce. The mechanisms below describe branch implementation;
acceptance remains incomplete and implementation does not imply additional product approval.

<a id="working-product-direction--2026-10-04"></a>

## Review history and ownership

The accepted revision above supersedes the October 3 no-checkout default and
October 4 tentative wording. Jack Heart's original feedback and confirmation
remain in [the review note](../task-purpose-demo.md). The full earlier plan,
including the tentative direction and predecessor architecture, is preserved at
`d9fabaaea360be123f6165651d25b5546eb3f519:scratch/jack-heart/start-and-finish-tasks-without.md`.
Do not treat that historical plan as a second implementation specification.

LOO-353 owns worker/privileged-Flow removal and the ongoing conversation machinery;
its brief removes Session ready/complete APIs, not Task completion. LOO-364 owns
broader Session wake/switch UX. LOO-366 owns Project availability and optional
chapter resets. LOO-379 owns cancellation/deletion confirmation; its independent
[research and filing receipt](../execution-independence-research.md) remain separate.

## Review decision · 2026-10-02

Jack Heart confirmed that the conversation doing the work can complete its Task.
It need not finish its own Session or provider turn before requesting completion.
Task completion preserves that conversation and its history. At review time, the
current Exec was exempt but a non-managed Session's pending turn still blocked.
The branch now recognizes the exact requesting conversation for completion only;
cleanup and abandonment retain their protections. Jack's review accepted the
behavior, not implementation or acceptance evidence.

The earlier output-schema blocker is resolved by #1401, included in installed
0.12.31 according to Jack Heart's steer. Jack authorized recovery; supplied step
feedback confirms the corrected decision returned to implementation. The failed
request supplied no verdict. Original diagnostics remain at
`7c1d84b1e:scratch/flow-decision-schema-blocker.md`; no further recovery is required.

## Earlier demo and evidence

The October 3 demo proposed no-checkout binding as the primary path, and retained
an allocated checkout until the conversation settled. October 4 supersedes both
as success criteria. The full earlier scenario remains at
`8094b45516ed6ce40305d7bdbefb0c7e87673c88:scratch/jack-heart/start-and-finish-tasks-without.md`.
Planning-only create/inspect/complete without allocation remains required;
ordinary focused conversations now need the default checkout and safe handoff.
Explicit attributed/taskless Flow parity, Flow completion leaving the Task open,
and real delivery refusal/retry remain acceptance requirements. Existing operation
fixtures do not establish provider handoff or Desktop interaction.

## Current implementation · reconciled 2026-10-03

Task placement is now `Option<TaskWorkspace>` (path and slug together). The store
reads/writes NULL for absent placement, rejects half-present rows, excludes absent
placement from checkout inventory, and admits one Task by issue identity without
creating a PR or setting Started. Later delivery allocation attaches its first PR
and placement transactionally to that same Task. Existing events and identity remain.
The released columns were already nullable; the existing Flow cwd draft remains
the only migration for this Task.

Public Session binding admits an owned planning issue. Binding preview reads that
issue without admission, and generic Work resolution stays read-only; CLI execution
explicitly admits before requesting context. Task context and captured input render
without a PR or workspace. An unplaced Task uses the caller directory without
adopting its unrelated files or directory membership. Live steer context can use
its owning repository without delivery placement.

Registered completion checks cleanliness only for retained placement. Associated
work and PR promises still pass through the existing gate. The requesting
conversation allowance is confined to completion: its current Exec must identify
the provider generation recorded for that Session and its exact driver parent. Review Sessions,
stale callers and unrelated execution receive no exemption. Cleanup and abandonment
retain their previous blockers. A stateful test completes inside a recorded calling
conversation, then records that same conversation's terminal provider event.
The public binding → first `task_checkout` test now retains Task identity and the
bound conversation, completes after a simulated merged PR, keeps the checkout while
the conversation turn is pending, then cleans it up on a settled retry. An unrelated
Exec blocks first with unknown evidence and then with a real owned sleep process;
only its recorded settlement removes that blocker. Provider-turn and merge records
are fixtures, not a configured native-provider or GitHub demo.

Explicit `flow start TEMPLATE` uses the existing capture/driver with or without
Task attribution. The public CLI regression runs two operations, checks persisted
cwd and attribution, and proves Flow completion leaves the Task open without
inventing placement or a PR. Bare managed startup now admits without allocation;
only explicit placement options enter checkout preparation. Managed creation,
continuation, restart and automation read the captured cwd. New unplaced launches
capture the caller directory, including a non-Git folder for an admitted Task.
Managed planning still resolves through the owning repository. Restart captures
its replacement from the saved directory and checkpoints only retained placement.
Delivery recovery and no-active-PR refusal apply to placed Tasks.

Rust/Swift DTOs represent absent placement and an unregistered binding preview.
Desktop offers outcome entry through `task complete`; New Session uses the current
repository when no placement exists, while a lost retained checkout still uses
restoration. Review launch paths read the Flow's captured cwd. Optional-placement
snapshots, Session controls and the affected Swift modules compile headlessly.
The Flow pane reads independent members from Task work, shows their saved cursor
and completed-step count, and resumes/retries their exact Flow IDs. Managed controls
continue to use the managed snapshot. Shared FlowDetail fixture decoding preserves
that distinction.

Earlier supporting cuts remain: issue-specific provider confirmation instead of
whole-Wave refresh; Flow launch cwd independent of Task placement; no Task-Done-driven
Flow settlement. Recorded results settle through the graph, while new managed
launches retain planning checks. Cleanup retains unfinished managed Flows.

Review findings fixed on October 3: admission in a generic context reader would
have mutated inspection; moving it to execution/binding avoids that side effect.
An absent placement is not a missing checkout. A requesting-conversation exemption
must never enter cleanup. The fixture's PM database differs from its journal Home,
so the current Exec evidence is explicitly seeded in the PM fixture; it cannot be
presented as a real provider turn. Older detailed source findings remain at
`528625dc17ba4dd6440d7365c1a3073e1c71d205:scratch/jack-heart/start-and-finish-tasks-without.md`.

## Surviving contracts

Keep one Task, the shared Flow driver and the existing planning completion writer.
Admission and inspection remain independent of execution/delivery. A Task's optional
path/slug pair distinguishes no placement from a lost checkout; reads never invent
placement. Binding is write-once association, not Started, process control or Flow
authority. Preserve prospective usage attribution and historical identity.

Confirm mutations using the affected issue through normalized planning ingestion,
retaining revision ordering, ownership checks, creation/summary markers and pending
writeback. Never report an unknown provider outcome as confirmed completion.
Completion checks associated unfinished work and real delivery promises. Only the
exact requesting conversation may remain active during completion; that allowance
never authorizes checkout removal. Failed cleanup retains the original outcome.

CLI, agents and Desktop consume the same Rust operations and explanations. DTO
changes update the Rust/Swift fixtures together. No Swift lifecycle, alternate
completion subsystem, force switch or broad directory adoption is needed.

### Requirement audit

| Operation | Intrinsic facts/authority | Optional operation-specific requirements |
|---|---|---|
| File | Existing selected Project/Wave routing, provider mutation authority, idempotent issue identity | Agent, Flow, checkout and PR only for explicitly requested execution/delivery |
| Inspect | Known issue or retained Task identity; report freshness/missingness | No launch authority, current chapter or agent account; remote failure must retain available local history |
| Start ordinary work | Exact Session/Exec authority, valid attribution and actual cwd | Default focused conversations to placement without a PR/Flow obligation; unplaced execution remains supported |
| Associate existing work | Exact Session/Task identities, write-once bind and ancestry consistency | No active PR, managed Flow, Started transition or reopening of terminal Task |
| Record outcome | Nonempty summary and authoritative issue/Task writer | No Flow success or merge unless that evidence is part of the requested outcome |
| Complete | Outcome conflicts, associated unfinished work, unresolved effects and idempotent transition | File/PR/merge evidence only for retained placement and delivery promises |
| Managed delivery | Existing exact claims, provider revisions, PR head, disposition and retained-work proofs | Never waived by generic Task admission or association |

Project availability and optional chapter resets belong to LOO-366. Keep current
filing selection of an existing current Project; do not invent projectless
ownership, auto-create Projects, or change chapter rotation locking here. Narrow
entity confirmation after routing is in scope. Missing required ownership at new
admission remains an explicit error. Existing retained local association and
inspection do not reacquire unrelated coordination.

## Delete — do not maintain

October 4 revision inventory: remove the implicit first-PR creation from checkout
allocation, PR-dependent placement initialization/restoration, and PR-only checkout
cleanup. Move their branch/base preservation obligations to placement's existing
owner. Remove the first-allocation test's synthetic merge as proof of research
completion; retain it only as delivery regression coverage. Do not extend
`serve_locked`, native Move here or managed review APIs into a second handoff
driver. LOO-353 owns their replacement/removal boundary. These deletions remain
unimplemented, as does the revised default binding path.

Already removed: mandatory placement in admission/context/binding, unconditional
checkout inspection during completion, whole-Wave mutation confirmation, inherited
Flow cwd and Task-Done-driven Flow settlement. The current implementation section
records behavior and proof limits; earlier compression details remain in the Git
reference above. These completed cuts are not remaining deletion targets.

Retain Task completion, planning-only completion, associated-work protection,
real PR settlement and exact execution authority. Replace PR-dependent placement
facts and the managed/independent presentation only with their surviving owners;
do not polish worker or review machinery LOO-353 removes.

Use one migration draft for this Task, generated by `scripts/new_migration.py`.
Migrate released rows without changing IDs, paths, PR links, managed Flow selection,
Started history or event times. Update SQLite readers/writers, Task snapshots,
Rust/Swift DTO fixtures and cleanup in the same cut. Never emit invented empty
workspace values for a required DTO. Planning-only state and absent workspace
are distinct from unavailable or lost workspace evidence.

Forbidden: force/ignore switches, a configurable policy framework, an alternate
completion subsystem, dual writes, automatic completion on Flow finish, binding
as execution authority, broad directory adoption or fake checkout creation.

## Ordered work and acceptance

One coherent architectural change; the October 4 deletion inventory and handoff
interface remain before acceptance. Earlier focused passes do not prove them.

The core admission, context, explicit startup, completion and optional-placement
consumer changes exist. This remains one architectural PR, not a completed Task.

Commits `f656edf5f` and `25d339fb1` address the prior feedback about managed
placement and independent Flow display; that feedback no longer describes missing
implementation. The October 3 pass integrated `8c72e591e` (v0.12.32), including #1415
recovery. The October 4 research found newer local-main restart/review changes
(#1413 and #1429); preserve them during integration without restoring remote
restart prerequisites. No sync or fetch is claimed here. Gate
owns the provider/review/retry and Desktop interaction scenarios below. The failed
managed-launch regression proves capture, replacement, retained identity and zero
delivery allocation after a missing driver identity; it does not prove a successful
native worker launch. No configured installation, external provider mutation,
publication or Task completion is claimed.

Gate acceptance on the finished tree (reuse applicable focused results; add missing
behavior coverage rather than treating the command names as proof):

- `cargo test -p loopflow task_without_delivery`: existing stateful proofs cover
  filing/inspection, unrelated snapshot failure, failed issue confirmation/retry,
  public binding, requesting-conversation completion, first allocation, retained
  checkout and settled cleanup retry, plus independent live/unknown Exec refusal.
  These invoke public Rust operations with seeded driver and merge evidence.
  Remaining: exercise that lifecycle through real CLI dispatch and shared snapshots,
  including unavailable agent configuration and lost completion response. Confirm
  the original outcome and terminal event survive retry, and the requesting
  conversation can report success after completion without Flow settlement.
- `cargo test -p loopflow task_completion`: retain existing planning-only and
  delivery regression tests, plus open/publishing PR refusal, dirty/follow-up work,
  pending Ask, independent unfinished Flow and unknown/live Exec. Resolve each
  through its own operation and prove completion succeeds once. Missing checkout
  or unknown merge evidence must refuse rather than report empty work.
- `cargo test -p loopflow task_flow` and
  `cargo test -p loopflow --test documented_commands`: prove shared engine behavior,
  managed fencing and documented command resolution. Tests use deterministic
  provider side effects and real saved completion receipts, not live services.
- `swift test --package-path swift --filter Task` and
  `swift test --package-path swift --filter DTOFixtureTests`: shared snapshots drive
  no-delivery and managed-delivery controls and error recovery, including no
  workspace, failed launch with legal completion, and completion summary entry.
  Add headless view/interaction coverage through the existing Desktop test target;
  `uv run python scripts/test.py --list` identifies the affected app build suite,
  with unavailable native checks assigned to capable CI rather than a GUI demo.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`.

Gate scenarios must cross the source of truth and consumers: use the stateful
provider fixture to confirm issue state and summaries, read the real store and
CLI JSON, and decode the same wire shapes in Swift. A mock-call assertion alone
cannot prove any acceptance criterion. No real provider mutations are needed.

## Alternatives and failure review

A CLI-only bypass is smaller but leaves binding, Desktop and retained-work checks
coupled. A new Task workflow or policy object duplicates authority. Placeholder
workspaces avoid migration but make cleanup and membership unsafe. All rejected.

Success is an ordinary research conversation gaining durable Task continuity
without a delivery detour, then optionally becoming software work under the same
identity. Failure would be an apparently successful completion hiding live work
or an unknown provider result. Explicit absent placement, preserved membership,
per-operation checks and confirmation/retry proofs are the safeguards.

October 3 review (historical): the domain pair removes invented empty paths, admission has one
transactional owner, and completion keeps destructive retention separate. That
pass caught two residual couplings: managed planning still required a checkout,
and restart could validate one directory then capture from another. Owning-repo
planning and pre-captured replacement fix them. The allocation fixture also
confirmed that an unpublished PR blocks completion until delivery settles. A stale
controller test still expected Task completion to end its Flow; it is replaced by
separate preservation/new-launch-refusal and final-step-completion proofs.

Earlier focused receipts remain at
`d9fabaaea360be123f6165651d25b5546eb3f519:scratch/jack-heart/start-and-finish-tasks-without.md`;
no production code changed in this compression. Review removed superseded default,
architecture and completed-deletion instructions that competed with the accepted
revision. Safe Session handoff, PR-free placement and their proofs remain unresolved.

Earlier focused passes remain at `25d339fb1` and `f656edf5f`; gate retains provider/review/retry and Desktop acceptance.

Sync check (October 4, main `a1d2f8a591`): `cargo test -p loopflow --lib task_without_delivery` passed (9); `cargo test -p loopflow --test task_restart_tests restart_uses_old_valid_planning_and_preserves_invalid_work` passed (1); `cargo test -p loopflow --lib steer_failure_preserves_confirmed_or_uncertain_publication` passed (1); formatting and diff checks passed; broader checks remain with gate/CI.

Check: `git diff --check` passed; prose-only compression, no runtime rerun; gate owns revised behavior acceptance after implementation.
