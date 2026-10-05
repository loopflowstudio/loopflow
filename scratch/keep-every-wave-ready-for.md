# Keep every Wave ready for work — LOO-366

Implementation design, drafted 2026-10-03; review concluded by Jack Heart before
the October 5 implementation attempt. Jack Heart's accepted product direction
comes from [LOO-366](https://linear.app/loopflow/issue/LOO-366): ordinary Projects
need neither chapters nor a default Flow. The selected configuration/KR boundaries are approved for implementation;
the unresolved configuration publication mechanism below is not settled. Base: `12016c6d6`.

October 4 direction from Jack Heart: every Wave has exactly one active Project,
independent of chapters; tools and skills coordinate optional repository-wide
rotation through the Wave hierarchy. Jack subsequently specified that the Wave's
Project must be an explicit configuration field, suggesting
`wave/<wave>/config.yaml`. This supersedes status-based current selection and the
inferred automatic sole-candidate activation rule. Jack also requested a simple core API: validate the configured Project and access,
or create one when no Project is configured. Name matching and candidate selection
are excluded from the core. The exact field layout and recovery mechanics below
remain proposals.

Jack Heart further selected KRs as the chapter creation boundary: chapter review
and planning generate KRs before creating chapter Projects. That planning may
also generate Task candidates, but adding them is a separate step, optionally
run globally immediately afterward. A complete future Task plan is not required.

## Outcome and demo

Every Wave has one explicitly configured current Project, normally In Progress.
Opening a Wave in Desktop ensures that Project independently of any chapter. Existing
names, content, Tasks, checkout, PR, Session and Flow identities survive. CLI and
agents can do the same with proposed `lf wave ensure <wave> --json`. Status,
roadmap, background refresh and Project inspection never initiate provisioning.
An unavailable sibling Wave or global reset does not prevent ordinary work here.

On opening, show retained Project and Tasks while preparation runs. With no
retained plan, show “Preparing Project…” until acquisition resolves. An outage
shows its cause and Retry, never an empty-project prompt. The configured ID
selects the current Project even if another provider Project is In Progress.
Other Projects do not block opening the configured one. Project
preparation and the primary conversation have independent outcomes. Reopening
retries ensure; periodic refresh only reads. Key results to the opened Wave so
a late response from Wave A cannot replace Wave B's displayed plan.

Demo: open a configured Wave with no Projects, see its ordinary Project and
usable Task section, close/reopen it, and get the same provider UUID. Open a
second Wave whose ordinary Project has no `flow:` line and a name such as
“Summer work — customer requests”; that exact Project remains current. CLI ensure
returns the same records. A failed global reset stays visible under an explicit
“Realign Projects…” action while both Waves remain usable. Task creation and
completion without a managed Flow are LOO-367's integration responsibility;
this Task removes their Project-level prerequisites.

This directly supports the supplied KR about creating and advancing Tasks without
plumbing blockers. No numeric metric targets were supplied; do not claim a KR
from synthetic tests or readiness alone.

## Findings that determine the design

- `work/wave/config.rs` currently reads Wave policy and Initiative binding from
  `GOAL.md` frontmatter; no per-Wave `config.yaml` reader exists. Add the selected
  Project field once, with no duplicate copy in frontmatter. Moving other Wave
  settings is outside this change unless separately selected.
- `ops/chapter.rs::select_current` currently infers selection from Started status.
  Replace that inference with configured-ID resolution in `ops/project.rs`, and
  move `sync_projects`/`record_project` there. Audit every current-Project consumer,
  including Task routing; changing ensure alone leaves competing selection rules.
- `pm/linear.rs::{create_project,adopt_project}` and
  `ops/chapter.rs::{update_plan,rotate,apply_rotation}` independently reject empty
  Flow strings. `ProjectContent` parsing already permits an absent Flow as an
  empty string. Keep that representation in this cut; remove the prerequisites
  and omit an empty `flow:` line when rendering. Do not invent a default `feature`
  merely to create a Project. Explicit Flow launch validation remains with LOO-367.
- `ops/pm.rs::checked_projects_with_store` retrieves retained IDs missing from
  membership, validates ownership, projects migration adoption, then canonicalizes
  names. `canonical_project_name` rejects unknown em-dash prefixes. Ordinary
  adoption must preserve raw provider name/content, not reconstruct them from a
  chapter parser. Remove name-as-ownership validation; ownership comes from IDs.
- `plan_rotation` infers a shared predecessor name across Waves and can recover
  a Completed predecessor only using that common name. Its module explicitly has
  no partial-operation record. Different ordinary names invalidate this recovery
  strategy. It also cannot distinguish a genuine competing current Project from
  the intended two-current-Projects interval without target context.
- `create_project` already accepts a chosen UUID, but creation and Initiative
  attachment are separate mutations. Failure after creation can leave an unattached
  Project. `find_project` supports exact-ID recovery and reports archived records
  rather than pretending they are absent. Retain this distinction.
- LOO-364's `ops/human_session/primary.rs` serializes a scope, admits a durable
  identity, then retries launch using that identity. Reuse those principles, not
  Session storage or provider startup as a Project prerequisite. Existing
  `chapter::rotation_lock` is a per-Wave OS lock with a 30-second bound.
- Linear's published [GraphQL schema](https://raw.githubusercontent.com/linear/linear/master/packages/sdk/src/schema.graphql)
  was inspected October 3: `ProjectCreateInput` accepts a caller UUID and optional
  content/status; `ProjectUpdateInput` supports status-only updates and exposes
  no revision precondition. This supports exact-ID reconciliation, not a claim
  of cross-mutation atomicity. The repository already uses these mechanisms.
  Provider duplicate-ID/error and attachment behavior still need configured proof.
- Desktop's `WaveDetailPane` refreshes status every 30 seconds and calls its
  ordinary plan `chapterAndTasks`/`WaveChapterView`. Ensure belongs at explicit
  Wave selection/opening, outside this polling loop. `RegistryQuery` remains
  the shared CLI transport and decoded snapshot owner.

## Chosen operation and authoritative state

Proposed Wave configuration:

```yaml
# wave/infrastructure/config.yaml
pm:
  linear_project: <stable Linear Project UUID>
```

The configuration owns which Project is current. Linear owns that Project's
status, content and membership. SQLite retains its existing synced read model,
not an independently writable current-Project pointer. A transition receipt may
record intended changes but never overrides the configured selection. Preserve
other configuration keys and authored bytes when updating this field.

`lf wave ensure <wave> --json` is the explicit activation operation shared by
Desktop and agents. It takes no candidate-selection argument. To use an existing
Project, configure its exact ID. Reads never write configuration or provision
Projects. No matching by name, ranking by status, or candidate discovery belongs
in this API.

1. Acquire the per-Wave planning lock shared with plan editing and reset. Resolve
   the configured Project, repository/Team/Initiative and pending transition.
   Do not contact unrelated Waves or launch a Session.
2. With a configured ID, fetch that exact Project and confirm ownership. Reuse
   an In Progress Project; activate the configured Backlog/Planned Project by a
   status-only write. Names and missing Flow do not affect selection. A failed
   lookup, archive, deletion, terminal state or changed ownership is actionable
   evidence about that binding, never permission to select or create another.
3. With no binding, resume an unfinished creation by its recorded ID, or reserve
   one new UUID and create an ordinary Project named for the Wave, with no default
   Flow. Attach and activate it, then save its ID to the Wave config. A pending
   explicit reset retains its own recovery; opening never starts a competing
   creation. No search for adoptable Projects precedes ordinary creation.
4. Confirm provider facts and atomically save the reference while preserving
   unrelated config edits. If a response or config write fails, retry the same
   reserved ID. Never overwrite a different ID written in the meantime. Read
   back configuration and provider outcome before settling the operation.

The configured ID is the only current-Project selector. Other provider Projects
may exist; their names and statuses cannot change that selection or block opening
this Project. The API does not promise exactly one In Progress Project throughout
Linear; it maintains one configured current Project for the Wave. Terminal,
archived or inaccessible configured Projects report their condition; opening
never silently reopens or replaces them.

Existing Waves need their known Project UUIDs explicitly seeded in configuration
as part of rollout, preserving existing work. That setup is separate from the
runtime API; do not ship a heuristic matching layer as a migration convenience.
Creation requires successful connection/access checks. A provider outage with
no configured ID must not be interpreted as permission to create elsewhere.

Ensure and observational readers resolve the same configured ID. During a reset,
the predecessor remains selected until the explicit configuration switch; afterward
the successor is selected and unfinished predecessor work remains visible. Opening
does not move Tasks, complete Projects or switch the reset's config reference.
Status reports pending recovery separately from current selection and marks
stale/unavailable provider evidence truthfully.

Inspection of `ops/pm.rs::resolve_context` and `planning.rs` found configured
Linear planning, not a local-only Project writer. Missing connection reports the
existing connection action; outages never invent local Projects. Configuration
selects identity; it is not a second planning store.

## Persist only the missing recovery evidence

Add one narrow Project transition receipt in the existing SQLite owner, using
one migration draft for this Task. Receipt fields: Wave ID, reserved successor
provider ID, optional predecessor provider ID, optional explicit reset name,
creation timestamp and settlement timestamp. A unique unfinished transition per
Wave serializes admission alongside the Wave lock. For reset, persist the exact
selected predecessor/successor before effects; for initial ensure, predecessor
is null. Existing Project facts remain in their existing tables. Do not copy
Tasks, KRs, names, provider status or execution cursors into this receipt.

This is operation recovery evidence, not a Chapter object or second plan. Fresh
provider observations determine the next missing effect; a phase counter must
not assert an effect happened. Readback settles the receipt only when attachment,
the configuration reference, and any predecessor completion are confirmed. Preserve it
across crash, response loss and process replacement. Query it by Wave and, for
reset re-entry, its recorded target IDs; reset names are descriptive metadata,
not Project lookup keys. Do not select “latest” by timestamp.
Retain settled receipts as operation history without giving them current-plan
authority. An unresolved older transition cannot be overwritten by another reset.

The supported concurrency boundary is the configured owning Home, consistent
with Jack's one-client decision. Competing Desktop/CLI processes share the lock
and reservation. External Linear edits are not lockable; recheck ownership and
surface conflict. Do not claim a distributed transaction or invent a fleet lock.
Retain explicitly selected reset successor IDs; allocate new IDs independently
of display names. Remove name-derived successor UUID generation.

## Optional coordinated reset

Keep explicit `lf repo new-chapter <name> --dry-run --json` and apply as the
coordinated operation. Tools and their operating skills coordinate participating
Waves across the repository and through its hierarchy, with a unified chapter
name and metadata. Approximate synchronization permits recoverable partial
progress; it does not impose chapter membership on ordinary Project access.
Every completed rotation selects one current Project per Wave in configuration. Desktop's “Realign Projects…” previews the same result
and applies only after an explicit action. No reset occurs on Wave opening.
Expose affected Wave, predecessor/successor and Task dispositions in that preview;
show unavailable Waves and unresolved work before applying. Require supplied KRs
for every participating successor before the first chapter creation mutation.
Ordinary ensure has no KR requirement.

Replace shared-predecessor-name inference with independent per-Wave selection
plus the exact transition receipt. The configured Project identifies each
predecessor; different predecessor names are normal.
Reset accepts exact successor IDs or reserves new ones. Do not locate targets
by chapter name. Existing explicit target Projects retain identity and content;
apply only the authored next-plan changes supplied to the operation. Copy an
optional Flow only when present. Install the supplied new KRs and any supplied
metric targets; preserve predecessor KRs, targets and outcome evidence. Never
create an empty-KR chapter Project and promise to plan it afterward. A reset can
start with no prior chapter or no prior Project.

Retain complete preflight before the first coordinated provider write. Persist
all selected transition identities before apply. Acquire participating Wave locks
in stable ID order to avoid deadlock; ordinary ensure only acquires its own lock.
Re-entry recovers each pair, including successor activated, Task partly moved,
predecessor completion response lost, or earlier Waves already settled. A later
Wave failure leaves earlier progress intact and reports where retry resumes.
Pending global progress does not add a prerequisite to an unrelated Wave.

Keep existing fresh Task classification, missing-membership exact lookup,
`move_chapter_task`, Task-start reservation fencing and positive cancellation
confirmation. Started/claimed/authored/published work moves with its Task ID,
checkout, PR, managed Flow and independent Session/Exec association unchanged.
Unknown evidence remains unresolved. Remove automatic backlog expiration on
chapter rotation. Started work carries forward; unreviewed backlog remains
visible under its original Project until explicitly carried, revised or retired.
Even proven untouched Tasks do not expire merely because the chapter changes.
This preservation rule supersedes the earlier draft's automatic expiration.
Expose unresolved backlog in the planning follow-up without making an exhaustive
review or every future Task a prerequisite for creating the successor.
After successor activation and required Task dispositions are confirmed, update
the config reference from the recorded predecessor to the successor, then
complete the predecessor. Persist both IDs before effects; retry accepts config
matching either endpoint and reconciles the missing effects. A different config
ID is an intervening decision, not permission to overwrite it. Config and provider
changes are not one transaction: prove crashes on both sides of the switch.
Complete predecessor after transfer, never merely because successor activation
succeeded. No global rollback, Session restart, Task completion or inferred KR win.

## Chapter review, planning and Task admission

Accepted October 4 boundary from Jack Heart:

1. **Review and plan:** a skill reviews the previous chapter's outcomes and
   unfinished work, then formulates the next chapter's per-Wave KRs. It may
   produce proposed metric targets and Task candidates alongside them. Reuse and
   reconcile the existing repository/Wave `review-chapter` and `start-chapter`
   skills rather than building a second planning framework. Final skill/Flow
   composition and names remain implementation choices.
2. **Create chapter Projects:** the synchronous operation takes each selected
   Wave's chapter key, name and nonempty authored KRs, plus optional targets and
   Flow. It creates/configures successors and preserves ongoing work. It does
   not generate KRs, await an agent, or require newly planned Tasks. Validate KRs
   before provider writes; the planning skill owns their substantive quality.
   Confirm those KRs in provider content before switching the Wave's reference.
3. **Add Tasks:** a separate step reviews/adopts Task candidates into the exact
   created Projects. It can run across all participating Waves immediately after
   creation or incrementally later. Re-enter through the existing Task creation
   owner without duplicating already admitted candidates. Its failure leaves the
   chapter Projects and KRs usable; it does not roll back Project creation.

Candidate notes are authored planning output, not durable Tasks or another
planning database. They must survive between these steps and remain available
when Task admission fails. Use the existing planning artifact path; retain links
to destination Project IDs after creation. The exact output format remains to
be specified with the skills. No complete future Task inventory is required.

Preserving existing work belongs to rotation, not the optional creation of new
Tasks. Review and planning account for old KRs and backlog; neither an empty new
Project nor wholesale old-Task cancellation establishes a successful transition.
Initial KRs are required for chapter creation but can evolve through ordinary
Project edits afterward. Ordinary non-chapter Project ensure stays independent
of this review/planning sequence and may create a Project without KRs.

## Chapter facilities and history

October 4 direction from Jack Heart: retain higher-level facilities to create a
Project for a new chapter for a particular Wave. Jack also raised cross-Wave
historical chapter inspection as a capability to design. These facilities compose
the explicit Project operations; they do not add discovery heuristics to ensure.

Proposed interface shape (command spelling is not settled):

- Wave-scoped chapter rotation accepts a Wave, explicit chapter key and authored
  KRs, creates
  a successor or uses an explicitly supplied Project ID, preserves ongoing work,
  and switches that Wave's configured reference through the reset path above.
- Repository-wide rotation invokes that same operation for participating Waves
  with one chapter key. The current repository reset API remains the integration
  point; it must not grow a separate implementation of Wave rotation.
- Read-only chapter inspection takes the chapter key and returns linked Projects
  across Waves, including completed/archived history, their Wave identity,
  provider ID, name and observed status. It reports unavailable history as such.

Proposed data: optional chapter metadata on each participating Project containing
an immutable shared chapter key and a display name. The key is chosen once for a
coordinated chapter and passed explicitly to each Wave operation; Project names
remain independent and may change. Ordinary Projects omit the metadata. Each
historical Project retains its own membership when the Wave config advances.
No Chapter table or independent chapter lifecycle is needed for this lookup.

The metadata's exact provider representation and projection into the existing
SQLite planning model remain to be designed. Prefer existing Project content
metadata if sufficient; do not assume Linear provides a custom field. History
must retain exact known Project IDs when current Initiative membership omits old
Projects. Chapter inspection queries explicit metadata, never title parsing or
current Wave config alone. Do not infer historical membership from similar names;
any backfill uses explicitly established associations.

Cross-Wave historical inspection is a proposed extension, not yet a required new
CLI surface. Before including it in delivery, settle its representation and prove
that renamed and completed Projects remain discoverable after later rotations.
Wave-scoped chapter creation is part of the requested higher-level design.

## Consumers and deletions

This is one coherent delivery; internal slices are implementation order:

1. Add the per-Wave Project configuration field and shared configured-ID reader;
   seed existing bindings from explicit IDs without provider mutations on reads. Move ordinary
   Project ownership out of `chapter`, remove
   nonempty-Flow create/adopt/update/reset gates and name-prefix rejection, and
   preserve provider bytes on ordinary adoption. Cut CLI/status/DTO consumers
   over with it. Focused test: `cargo test -p loopflow --lib project_ensure` with
   new regression cases proving an existing no-Flow Project survives adoption.
2. Add transition persistence and explicit ensure, with concurrent/crash/uncertain
   response tests through real operation entry points and a stateful fake provider.
   Missing binding must not create synthetic local plans. Keep schema migration preservation
   tests at the released frontier, not successive draft versions.
3. Expose Wave-scoped chapter rotation and compose it for repository reset.
   Replace reset name inference with receipt recovery. Keep existing preservation
   tests, replacing tests whose sole assertion is a required shared name/Flow
   or automatic untouched-backlog expiration. Require KRs for chapter creation
   without introducing that prerequisite into ordinary ensure.
4. Add Desktop selection activation through `RegistryQuery`, separate Project and
   primary-conversation outcomes, and explicit reset preview/apply/retry. A Project
   error does not hide its conversation or preserved last-good Task history.
   Rename ordinary “Chapter” labels to “Project”; metrics belong to the Project.
   Add required-or-optional Rust/Swift JSON fields and fixture updates together.
5. Update `docs/lf.md`, command reference, planning architecture, repo guide and
   builtin `review-chapter`, `wave/review-chapter`, `start-chapter`,
   `wave/start-chapter`, `repo/session`, `wave/session` instructions at their
   actual owners. Compose review/planning → chapter creation with KRs → separate
   Task admission, retaining candidate output across that boundary. Ordinary workflows call ensure only at
   activation, not observation. Reconcile the LOO-367 call site without importing
   its lifecycle changes into this Task.

Delete `plan_rotation`'s cross-Wave `predecessor_names` heuristic, fallback
`feature` during Project creation, name-derived successor IDs, candidate-selection
logic, empty-Flow refusals, and
`canonical_project_name`'s unknown-prefix refusal. Move reusable functions rather
than copy them. Migration-marked old-content conversion remains only where
released-data preservation requires it; remove its Flow refusal, not its evidence.
Do not add Project “legacy/new” variants, chapter-required adapters, duplicate
DTO defaults, broad recovery frameworks, or provider mutation in status reads.

## Acceptance at gate

New test names below are implementation targets, not existing passing checks.
Use the repository's isolated fixture environment and compiled test CLI; prevent
native providers and configured accounts from being launched by synthetic tests.

- `cargo test -p loopflow --lib project_ensure`: no Project → one Started record;
  repeat/two processes → identical UUID and one provider Project; ordinary
  configured Backlog and current no-Flow Projects preserve all bytes/IDs;
  configured identity is independent of other Project names/statuses; absent
  configuration creates and records one ID without candidate discovery; failed
  access produces no provider writes; interrupted create/attach/activation
  resumes exact UUID; archived pending identity is not recreated. Failed config
  writes retry the same identity; concurrent config edits survive. Assert outcomes,
  not mock call wiring.
- `cargo test -p loopflow --lib chapter`: different ordinary names, missing Flow,
  no previous chapter, response loss at every mutation, unrelated Wave outage,
  and externally changed configured/recorded successor identity. Snapshot Task, PR, checkout, Session and Flow
  identity before/after; retain start-versus-retire and unknown-evidence cases.
  Missing/empty KRs reject chapter creation before provider writes; supplied KRs
  survive retry without duplication or lost content. Chapter creation succeeds
  with no new Task candidates. Ordinary ensure still succeeds without KRs.
  Unreviewed backlog remains visible and unretired after rotation; predecessor
  KRs and outcome evidence remain intact. Failure in later Task admission does
  not undo Project creation or its configured reference.
- Skill/Flow acceptance demonstrates previous-chapter review producing next KRs
  and retained candidate notes, chapter creation completing before Task admission,
  and global follow-up resuming without duplicate Tasks. Verify authored order
  and real operation outcomes, not just matching phrases in prompt files.
- Add `rust/loopflow/tests/project_readiness.rs` and run
  `cargo test -p loopflow --test project_readiness`: CLI ensure → fake Linear
  state → persisted planning → Wave status/roadmap and JSON consumed by Desktop.
  Repeated status/roadmap must leave provider Project count/status and transition
  admission unchanged. Failed reset on Wave B must not block ensure/use of Wave A.
  At every reset interruption, ensure, status and roadmap select the configured
  ID, retain predecessor Task visibility and leave reset effects untouched on
  opening. Cover crash before/after config switch and provider completion; stale
  checkout configuration must not reverse a later binding or resume an old reset.
- `scripts/test_desktop.sh -Xswiftc -gnone`: headless activation/reopening,
  selection race, retry/error, Project labels, independent primary conversation,
  reset preview/apply and JSON fixtures. `uv run python
  scripts/check_swift_multiplatform_boundaries.py` retains shared transport boundary.
  Use app/view builds/tests; no screenshot, display or permission dialog required.
- Gate runs affected migration/architecture checks, formatting and all-target
  Clippy once under TESTING.md. Implement only builds and runs its focused cases.
- Configured acceptance remains required after implementation: use explicitly
  designated fixture Waves for absent, ordinary Backlog/current and reopening
  scenarios, record provider UUID/status plus Task identities, and demonstrate
  useful Wave A operation while another Wave/reset is unavailable. Configured
  proof has not run. If fixture destinations or write authorization are
  unavailable then, report that exact acceptance gap; do not reset production
  Waves or count simulated success as configured proof.

Done means the opening/CLI experience works and repeated calls preserve identity,
ordinary names and empty Flow are usable, reads remain observational, and explicit
reset recovers without sacrificing started work. Full Task admission/completion,
new primary Session behavior, credential repair, release plumbing and a generic
multi-product platform are excluded.

## Review and remaining evidence

October 5 source inspection confirms an authority conflict, before production
edits. The reviewed design is preserved at
`e04c83513573cc09883fb2b92ebdb63e06a22c95:scratch/keep-every-wave-ready-for.md`.
`work/wave/context.rs` canonicalizes registry identity but explicitly gathers
authored files from the executing checkout. `work/wave/config.rs` joins the
supplied repository path; `ops/pm.rs::resolve_context` reads Initiative policy
from that path. Desktop's `WaveDetailPane::refreshDetail` passes `repoPath` to
`RegistryQuery.status`. `repository.rs::CanonicalRepo` collapses worktree
identity to the main checkout, but does not publish configuration. The earlier
claim that this branch already supplies a remote-main definition resolver was
incorrect; it described remembered intent, not this branch's implementation.

Counterexample: checkouts A and B both configure Project P. Reset in A creates Q,
switches A to Q, completes P, and settles its receipt. B still configures P;
exact-ID ensure must reject completed P, so ordinary work there is unavailable.
If both began without a binding, after A creates P and settles its receipt, B
still has no binding and can reserve another Project. A Wave lock serializes
these calls but does not change B's file. Using the last settled receipt to pick
P or Q would make receipts a second selection authority, expressly excluded.
These are source-derived counterexamples, not executed provider tests.

Remaining ownership decision: designate one shared file location for all
Project-binding reads/writes (for example the canonical main checkout), or make
publication and checkout synchronization part of switching the binding. The
first changes checkout-local configuration semantics and writes outside the
invoking Task checkout; the second introduces a Git delivery dependency into
opening/rotation. Neither is selected by the reviewed design. A shared SQLite
pointer, silent push, or stale-checkout refusal does not satisfy its constraints.
Dependent implementation stops here under the implement skill's authority-model
rule. All five implementation slices and configured acceptance remain outstanding.


The main failure risk is a timeout followed by creation under a new UUID. Reserving
identity before effects and reconciling exact provider state addresses that risk.
Review also removed an unsupported assumption that a local-only Project writer
already existed; configured Linear planning is the inspected implementation.
A second risk is silently overwriting authored content during adoption; status-only
writes and byte-preservation tests are mandatory. Review rejected a bootstrap
chapter (keeps the dependency), creation during status (hidden writes), and
name-derived permanent Project IDs (cannot support later completed generations).
Wild success is routine opening with no planning ceremony. Wild failure is
ordinary Projects becoming a second chapter framework; keep the receipt confined
to unfinished mutations and leave status/content in the provider.

Checks: October 5 `git diff --check` passed; `lf context` passed (memory 15,976/16,000 tokens; scratch under 12,000). No production code changed; behavioral verification remains outstanding.
