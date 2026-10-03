# Start and finish Tasks without unrelated workflow prerequisites

Implementation plan · LOO-367 · drafted 2026-10-02, reconciled 2026-10-03

Jack Heart's accepted direction is recorded in the Task brief and
[Task workspace review](../../docs/reviews/task-workspace.md): Task supplies
purpose, context and continuity; execution and delivery are optional. There is
no Task workflow to introduce. The mechanisms below describe branch implementation;
acceptance remains incomplete and implementation does not imply additional product approval.

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
Explicit startup now has an operation-only CLI proof; provider, review and retry
acceptance remains below. Bare taskless startup still requires a template.

This serves Infrastructure's dependable self-hosting and architecture reduction
objectives and the chapter KR about advancing a Task from an ordinary Session
without plumbing blockers. No quantitative chapter targets were supplied.

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

Removed: mandatory Task placement, PR-dependent context and Session binding,
unconditional checkout inspection during completion, whole-Wave post-mutation
confirmation, inherited Flow cwd, and Task-completion-driven Flow settlement.

Compression review on October 3 removed the obsolete Task clone that rewrote
placement solely for steer publication; publication already resolves the owning
Wave repository. Binding now calls admission directly, and checkout restoration,
cleanup and their tests retain one workspace reference. PR observation again takes
its mutation lock before reading the PR, preventing a stale pre-lock row from
replacing a concurrent publication. PR observation acquires no checkout lock for unplaced Tasks.

The follow-up implementation removed: managed launch/restart's unconditional delivery
allocation, checkpoint and no-active-PR prerequisites; saved-Flow resumption's
workspace lookup; and Desktop's managed-only Flow display. Exact managed claims,
review boundaries, captured graphs and placed-Task delivery recovery remain.

The follow-up compression removes the temporary replacement Task built during
first-checkout allocation: existing admission is retained directly before attaching
placement. Rotation and missing-checkout checks reuse their validated workspace;
PR recovery errors name the checkout holding the stash. Unplaced context avoids an
irrelevant Git branch lookup. The independent Flow DTO fixture now contains only
two research steps, with completed and current progress checked in Rust and Swift,
instead of duplicating the feature graph's unrelated review and routing topology.
No deletion target remains from this pass; broader acceptance stays with gate.

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

The core admission, context, explicit startup, completion and optional-placement
consumer changes exist. This remains one architectural PR, not a completed Task.

Commits `f656edf5f` and `25d339fb1` address the prior feedback about managed
placement and independent Flow display; that feedback no longer describes missing
implementation. Local main is the already-integrated `8c72e591e` (v0.12.32),
including #1415 recovery; no newer upstream facts were fetched. Gate
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

Simulated review: the domain pair removes invented empty paths, admission has one
transactional owner, and completion keeps destructive retention separate. This
pass caught two residual couplings: managed planning still required a checkout,
and restart could validate one directory then capture from another. Owning-repo
planning and pre-captured replacement fix them. The allocation fixture also
confirmed that an unpublished PR blocks completion until delivery settles. A stale
controller test still expected Task completion to end its Flow; it is replaced by
separate preservation/new-launch-refusal and final-step-completion proofs.

Check: `git diff --check` passed for this prose-only realignment; reused `25d339fb1` results: `cargo test -p loopflow --lib task_without_delivery` (9), `cargo test -p loopflow --lib ops::run::tests` (8), `cargo test -p loopflow --test dto_fixtures independent_flow_detail` (1), `swift test --package-path swift --filter DTOFixtureTests` (23), `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed; prior focused evidence remains at `f656edf5f:scratch/jack-heart/start-and-finish-tasks-without.md`; gate owns provider/review/retry and Desktop interaction acceptance.
