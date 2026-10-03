# Start and finish Tasks without unrelated workflow prerequisites

Draft implementation design · LOO-367 · 2026-10-02

Jack Heart's accepted direction is recorded in the Task brief and
[Task workspace review](../../docs/reviews/task-workspace.md): Task supplies
purpose, context and continuity; execution and delivery are optional. There is
no Task workflow to introduce. The mechanisms below are proposed implementation
choices, not additional product approval.

## Review decision · 2026-10-02

Jack Heart confirmed that the conversation doing the work can complete its Task.
It need not finish its own Session or provider turn before requesting completion.
Task completion preserves that conversation and its history. Implementation details
remain proposed; this review does not claim implementation or acceptance proof.
The current Exec is exempted by the associated-work checker, but a non-managed
Session's pending turn still blocks. Cleanup shares that checker: keep any new
completion allowance separate from cleanup and abandonment protections. The
unrelated color-scheme message was withdrawn and contributes no requirement.

The earlier output-schema blocker is resolved by #1401, included in installed
0.12.31 according to Jack Heart's steer. Jack authorized recovery; supplied step
feedback confirms the corrected decision returned to implementation. The failed
request supplied no verdict. Original diagnostics remain at
`7c1d84b1e:scratch/flow-decision-schema-blocker.md`; no further recovery is required.

## Outcome and demo

Jack can file a research Task, associate an existing conversation, record its
finding and complete it without creating a checkout, PR or Flow. Later choosing
a Flow or software delivery preserves that same Task and all its work.

The demo starts with `lf task create --wave infrastructure --title "Investigate a slow command"`,
then `lf task status <issue> --json` and
`lf task complete <issue> --summary "Measured startup; retained findings in the issue"`.
The Task is complete, the original summary is readable, and there are zero new
worktrees, PRs and FlowSessions. Repeat with an existing Session bound before
completion: its history remains associated without allocating delivery placement.
Issue completion from that same running conversation and let it report the result
afterward. With an allocated checkout, keep the checkout available while the
conversation still uses it; completion and safe cleanup have separate conditions.

Then run the same explicit template through `lf flow start <flow>` and
`lf --task <issue> flow start <flow>` on an unfinished Task. Both use the same
capture, driver, review and retry semantics. The attributed version adds Task
context/history. Flow completion leaves the Task open. A managed delivery Task
with an open PR refuses completion with a concrete reason; after authorized
settlement it completes once, even after a lost provider response.
These are target behaviors; taskless `flow start` currently refuses.

This serves Infrastructure's dependable self-hosting and architecture reduction
objectives and the chapter KR about advancing a Task from an ordinary Session
without plumbing blockers. No quantitative chapter targets were supplied.

## Current source findings · reconciled 2026-10-02

- `ops/task.rs::task_create` already supports filing without execution. Its
  `options: None` branch creates no runtime Task, checkout, PR or Flow.
  `task_complete` already routes an unregistered issue to
  `ops/pm.rs::complete_planning_task`. Keep both paths.
- Planning completion validates nonempty summary, checks provider outcome,
  confirms completion and publishes one marker-keyed summary. Existing
  `ops/pm/task_planning_tests.rs` covers original-summary preservation,
  lost responses, pending writeback and conflicting terminal outcomes. The
  recorded focused suite result below covers the current confirmation slice.
- `task_pm::create_and_load_task` and `pm_update_async` now confirm mutations
  through affected-issue lookup and normalized ingestion. Current-Project routing
  still precedes filing and creation-marker recovery. The no-delivery fixture
  seeds fresh planning for that routing; it proves post-mutation independence
  from whole-Wave acquisition, not filing during arbitrary planning outages.
- `work/task/mod.rs::Task` requires `worktree` and `workspace_slug`;
  `create_prepared_task` creates the Task and first PR together. This is the
  admission coupling to remove. A fake empty path would make absent placement
  ambiguous and risks associating unrelated cwd history.
- `ops/run.rs::resolve_work_selection` requires a registered Task, its Project
  and a PR before constructing context. `human_session::binding_target` also
  requires registration. Thus a planning-only issue cannot yet accumulate normal
  bound execution through those paths without delivery preparation.
- `bin/lf.rs` routes `FlowCommand::Start` through `task_run` and rejects a
  missing Task. However `lf/commands/flow.rs::execute` already persists optional
  Task attribution and uses the shared `drive`. Resume already distinguishes the
  marked managed Flow from independent attributed Flows. There is no engine to add.
- `complete_task` unconditionally inspects checkout cleanliness. Its existing
  `task_completion_gate` protects associated unfinished work, open/publishing
  PRs and committed follow-up. `store/sqlite/task_work.rs` already unions
  explicit binds with checkout membership, including absent-checkout history.
  Retain that reader and managed Flow distinction from LOO-358.
- `cleanup_completed_task` already runs after durable completion and retains the
  checkout when associated work blocks cleanup. It now retains the checkout for an unfinished managed Flow even after its worker
  exits. Cleanup no longer ends that Flow. The new requesting-conversation
  allowance must remain separate from this retention check.
- `task_actions::derive_task_actions` applies launch refusal before considering
  completion; its no-PR case recommends resuming a saved Flow. `task_flow` owns
  shared control explanations. Desktop's `RegistryQuery`, `PodiumModel` and
  `TaskFlowView` consume these operations, while the local Session launcher
  obtains Task checkout placement. UI-only relaxation would leave CLI and agents
  blocked and would misrepresent permissions.
- Recovery distinguishes unknown historical Execs from current execution without
  recording their exit. Session-driver reads serve only that resumption exception;
  cleanup and abandonment block unknown Execs without needing those extra reads.
  Upstream #1415 is included through the current base; it already owns this
  recovery distinction and missing-PR discovery. The retained local edit only
  avoids Session-driver reads in retention checks; it grants no completion
  exemption. Existing `task_work` tests cover recovery versus retention. Preserve
  these behaviors while removing unrelated delivery prerequisites.

Relevant retained memory: planning and completion have separate writers; bind is
write-once and can target done/landed Tasks; membership grants no execution
control; missing observations are unknown; Flow completion is not Task completion.
The dated continuation and Task-deletion sections of the full supplied memory
were read along with LOO-358 and current Wave guidance.

## Persistence and completion counterexamples · 2026-10-02

Source inspection found two dependencies missing from the proposed deletion cut.

- `store/sqlite/flows.rs::insert_flow_in` discarded cwd whenever Task attribution
  was present, and released SQLite triggers enforced that restriction. The reader
  then derived cwd from Task placement. Optional placement alone would make a new
  Flow unreadable, and later allocation would change its historical directory.
  The branch now records every Flow's launch cwd and removes that fallback and
  its two triggers. The single `optional_task_workspace` draft freezes existing
  inherited paths; unknown historical placement stays NULL. A released-frontier
  migration test and persistent Flow test cover this cut. The latter seeds the
  unplaced Task row directly: it does not establish public Task admission or binding.
- Task completion previously ended the managed Flow in both the driver and
  cleanup. Both shortcuts are removed. The driver consumes recorded step results
  through the ordinary checkpoint/graph path, and checks managed planning only
  before a new agent/operation launch or review preparation. A recorded final
  result can finish its graph after the Task becomes Done; an unexecuted later
  step remains unfinished and receives the existing terminal-Task refusal.
  Cleanup retains the checkout while its managed Flow is unfinished, including
  an idle Flow with no claim. Regression tests cover these boundaries without
  granting completion authority to a seeded Task or starting a real provider.

Review finding: removing only the Done shortcut stranded even a recorded final
operation because `resolve_managed_task_planning` rejects terminal Tasks before
result consumption. Moving that check to launch boundaries preserves the existing
terminal launch rule and exact receipt fencing. Task completion neither fabricates
Flow success nor discards a real successful step. Public requesting-conversation
completion and native-turn acceptance remain unimplemented/unproven.

Compression reuses `resolve_launch_task`'s validated managed Task for review
preparation, deleting the repeated selection and Task lookup. Recorded-result
consumption remains outside that check. The issue-confirmation Task-plan write
remains necessary: normalized planning ingestion does not update that retained row.

Task placement columns are already nullable in the released schema. The required
migration concerns the Flow cwd dependency, not converting non-null Task columns.
Rust decoding still requires placement, and admission remains coupled to the first
PR. Continue editing this Task's one draft rather than adding another migration.

## Chosen architecture

Keep one Task, one shared Flow engine and the existing planning completion writer.
Remove placement assumptions at their current owners.

1. Make Task placement explicitly optional. Represent the path and workspace slug
   together as `Option<TaskWorkspace>` in Rust so a half-present placement cannot
   escape the store. Existing nullable columns retain present placement unchanged; absent
   placement is SQL NULL, never an empty string or the caller's directory.
   The Task keeps its existing identity, Project, Wave, outcome and event history.
   No second Task table or workflow policy record is needed.
2. Separate registration of existing issue identity from checkout/PR preparation
   inside `ops/task`. A shared admission function is used by explicit Task
   attribution and Session binding when the issue has no runtime row. It resolves
   the existing owned issue, ensures its existing durable ancestors and writes
   the Task once, without reserving work or setting Started. Pure inspection and
   planning-only filing/completion still need no runtime row. Registration races
   converge by issue identity; terminal Tasks are never implicitly reopened.
3. `task_checkout` attaches placement through the existing LOO-355 allocation,
   restoration and PR machinery. Preserve existing checkout command semantics;
   ordinary registration must not call it. Retain initializing/recovery receipts
   and leases. A placement allocation failure leaves the admitted Task recoverable
   under the same identity and cannot silently create a replacement Task.
4. Context and work readers accept no workspace and no PR. Task purpose and known
   ancestry still render; PR-specific context appears only when present. A direct
   attributed command without placement runs in the explicitly supplied caller
   cwd. Recording that command does not adopt the whole directory as Task-owned.
   With retained placement, preserve current placement routing. Explicit binds
   remain additive and prospective usage attribution remains unchanged.
5. Route explicit `flow start <template>` through the shared capture/driver with
   optional Task attribution. Preserve the managed marker, claim and saved resume
   path where a managed Flow is selected; absence of delivery placement is not a
   reason to create it. Bare Task Flow start may use its Project default or resume
   its captured managed Flow. Taskless start requires a template. Placement flags
   remain explicit requests for the existing checkout operation before launch.
   Replacing a managed Flow uses existing stop/restart fencing and preserves
   independent Flows and Task identity. LOO-364 owns broader Session wake/switch UX;
   this change only removes delivery admission assumptions from those operations.
6. Registered completion uses `complete_task` and its existing gate. Always check
   associated work, outcome conflicts and unresolved effects. Check dirty files,
   committed ranges and cleanup only for actual placement; check PR settlement
   only for recorded PRs. No placement means no files to clean, not permission to
   ignore explicitly associated work. A missing previously allocated checkout
   remains missing evidence and retains existing recovery protections.
   The requesting conversation's current turn and its exact completion invocation
   must not block the Task transition. Resolve that relationship from existing
   authoritative Session/Exec evidence; cwd, membership and causal ancestry alone
   grant no exemption or control. Other unfinished work, pending review boundaries,
   unknown execution and unresolved delivery still block through their existing
   owners. Do not broaden a shared idle check: cleanup and abandonment retain their
   own protections. Completion must not terminate the requesting conversation,
   settle its Flow or delete a checkout it still uses. Retained cleanup is reported
   separately and remains retryable without changing the original outcome.
7. Confirm planning mutations by the affected issue's fresh authoritative facts
   through existing normalized ingestion. Keep revision ordering, ownership checks,
   cancellation/duplicate conflict handling and idempotent creation/summary markers.
   Whole-Wave refresh may update coordination separately but cannot invalidate an
   already confirmed lower-level operation. A failed issue confirmation remains
   explicitly unconfirmed. Preserve registered completion's pending writeback
   contract and expose it; never label it confirmed provider completion.
8. Project operation-specific availability in existing Rust snapshots/actions.
   An unavailable Flow or agent cannot disable a legal completion or inspection.
   Desktop exposes outcome entry and completion for planning-only and registered
   Tasks using the same command, and renders optional placement in Files/Session
   launch controls. No Swift lifecycle or policy matrix. Agents use the same CLI
   and explanations. Update DTO fixtures in every mirrored language together.

### Requirement audit

| Operation | Intrinsic facts/authority | Optional operation-specific requirements |
|---|---|---|
| File | Existing selected Project/Wave routing, provider mutation authority, idempotent issue identity | Agent, Flow, checkout and PR only for explicitly requested execution/delivery |
| Inspect | Known issue or retained Task identity; report freshness/missingness | No launch authority, current chapter or agent account; remote failure must retain available local history |
| Start ordinary work | Exact Session/Exec authority, valid attribution and actual cwd | No managed claim, PR or newly allocated checkout; managed progression retains its planning checks |
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

Delete unconditional workspace/PR requirements from registration, context,
non-delivery completion and Task action derivation. Replace the Task-only
`FlowCommand::Start` dispatch; do not retain a parallel ordinary start engine.
Remove tests that assert these obsolete prerequisites, replacing them with
behavior proofs. Correct the Task module's claim that every Task owns Flow
progression and a PR chain.

Remaining targets at their existing owners:

- `Task::{worktree, workspace_slug}` as mandatory placement; replace together
  with optional `TaskWorkspace`, including store and DTO consumers.
- `create_prepared_task` as the admission prerequisite in attribution and binding;
  retain it only for explicit delivery preparation.
- `resolve_work_selection`'s unconditional PR requirement and
  `FlowCommand::Start`'s Task-only dispatch.
- `complete_task`'s unconditional checkout inspection and
  `derive_task_actions`' launch refusal as a prerequisite to completion.
- Task-Done-driven `end_flow` in `lf/commands/flow.rs::drive` and cleanup's
  implicit managed-Flow settlement: removed, with recorded-result and idle-Flow
  retention regressions. Actual graph completion still owns Flow settlement.
- The Task-bound Flow cwd restriction and read-time placement fallback: removed
  in this branch, with retained-path migration and focused tests.

The implemented confirmation cut already removes `load_wave_async` and the
post-mutation `refresh_pm_snapshot` calls. Keep current-Project filing selection
and the shared completion writer; neither is a deletion target.

Keep `task_complete`, `complete_planning_task`, the existing gate, PR settlement,
managed claim fencing, LOO-355 checkout/rotation operations and LOO-358 membership.
Do not repair or duplicate those mechanisms as a prerequisite to deleting coupling.

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

One coherent architectural change; these are internal steps, not compatibility
stages to ship independently.

Implementation status · reconciled 2026-10-02: three supporting cuts exist:

- Issue-specific confirmation replaces whole-Wave acquisition after mutation.
  Current-Project selection before filing remains. Stateful fixtures cover failed
  confirmation, creation-marker recovery, original-summary preservation and zero
  invented delivery objects for planning-only operations.
- Flow launch cwd persists independently of Task placement. Released-frontier
  migration and store tests cover retained paths, including absent placement;
  the unplaced Task is seeded directly, not admitted through a public operation.
- Task completion no longer ends the managed Flow. Recorded results settle
  through the graph; new managed launches and review preparation retain planning
  checks. Cleanup retains unfinished managed Flows. Compression shares the
  validated Task lookup for launch and review preparation.

None removes the remaining public admission dependency. Source inspection still
finds mandatory `Task` placement, registration-dependent binding/context,
Task-only `FlowCommand::Start`, and unconditional checkout inspection in
`complete_task`. The next slice is step 1 below; the supporting cuts do not prove
the no-delivery conversation outcome or Desktop parity.

Review finding: a retry of `task create` still acquires current-Project routing
before reusing the creation marker. The new confirmation path must not be described
as eliminating all filing reads. Failed issue acquisition remains an explicit
failure; registered completion retains its existing pending-writeback state.

1. **Next implementation slice:** remove mandatory placement from admission and membership,
   update readers/writers and DTO consumers while preserving existing rows.
   The Flow cwd migration is already present; Task placement columns are already
   nullable and need no redundant schema conversion. Add a focused
   `task_without_delivery` test: bind an existing conversation to an admitted Task,
   read membership, and prove zero PR/checkout/Flow creation; then allocate the
   existing delivery path and prove identity/history survive.
2. Cut over context and explicit Flow startup to shared execution. Compare attributed
   and unattributed captures/cursors, review waits, failures and retry counters.
   Switching the managed selection retains independent work and rejects late writes
   from the replaced worker. Completed Flow leaves Task open.
3. Remove remaining completion/action dependencies. Narrow planning confirmation
   and its stateful failure/retry coverage are implemented. A registered no-placement Task completes with
   evidence through the existing writer, once its associated work is settled.
   Apply Jack Heart's requesting-conversation decision: its own active turn may
   remain open, while other associated work and delivery obligations must settle.
   Separate completion eligibility from destructive cleanup eligibility at their
   existing owners; preserve the requesting Session and checkout across completion.
4. Cut over Desktop controls, Session launch placement, CLI/help and agent guidance.
   Update `docs/lf.md`, architecture reference and relevant builtin skills. Keep
   stored historical skill wording historical. Complete migration and consumer
   tests before removing obsolete assertions.

Remaining acceptance checks on the finished tree:

- `cargo test -p loopflow task_without_delivery`: add behavior coverage across
  persistent store, real CLI dispatch and shared snapshots. Cover file → inspect →
  bind → finish → complete, no provider agent available, unrelated coordination
  failure, lost response and retry. Assert zero invented delivery objects,
  readable original outcome, stable identities and no duplicate terminal event.
  Invoke completion inside the bound running conversation, then prove that the
  same conversation can report success. Repeat with an allocated checkout and
  settled delivery: retain its files while in use, then safely retry cleanup after
  execution settles. A lost completion response preserves the original summary
  and terminal event. Unrelated live or unknown execution remains a blocker;
  Task completion supplies no Flow settlement or process-control authority.
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

Simulated review finding: merely relaxing `task_completion_gate` would neither
admit a research Session nor fix the context reader's PR requirement, and could
weaken delivery. Start the remaining cut with registration and placement while
preserving the gate's evidence obligations. The seeded-store Flow-directory proof
does not establish public binding; no Task completion or acceptance follows.

Check: `git diff --check` passed; `lf context --wave infrastructure --json` fits memory/scratch limits; prior unchanged-code results retained: `cargo test -p loopflow --lib task_completion` 11 passed, `cargo test -p loopflow --lib lf::commands::flow::tests` 4 passed, `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed; remaining public-operation and Desktop checks belong to implementation/gate.
