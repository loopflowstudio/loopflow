# Keep every Wave ready for work — LOO-366

Draft implementation design, 2026-10-03. Jack Heart's accepted product direction
comes from [LOO-366](https://linear.app/loopflow/issue/LOO-366): ordinary Projects
need neither chapters nor a default Flow. The mechanisms below are proposed;
kickoff is not implementation approval. Base: `12016c6d6`.

## Outcome and demo

Opening a Wave in Desktop prepares its ordinary In Progress Project. Existing
names, content, Tasks, checkout, PR, Session and Flow identities survive. CLI and
agents can do the same with proposed `lf wave ensure <wave> --json`. Status,
roadmap, background refresh and Project inspection never initiate provisioning.
An unavailable sibling Wave or global reset does not prevent ordinary work here.

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

- `ops/chapter.rs::select_current` already selects by Started status, not name.
  Move current-Project selection and `sync_projects`/`record_project` to
  `ops/project.rs`, which currently only resolves Task ownership and builds plans.
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

Add `lf wave ensure <wave> [--project <provider-id>] --json` as the explicit
activation operation. Default behavior performs one scoped fresh acquisition:

1. Acquire the existing per-Wave planning lock, shared with plan editing and
   reset. Resolve configured repository/Team/Initiative and pending transition.
   Do not contact other Waves or launch a Session.
2. Read complete membership and retained exact IDs. Failure, missing pages,
   malformed ownership or unknown retained identity is unavailable evidence,
   never an empty inventory. Existing cached planning remains inspectable.
3. Exactly one In Progress Project: reuse it and sync confirmed provider facts.
   Several: report names and IDs without choosing. During an identified reset,
   return its recorded intended successor only after validating the exact pair
   and current provider facts; otherwise preserve the ordinary conflict.
4. None current: resume this Wave's pending creation first. Otherwise adopt the
   sole eligible Backlog/Planned Project by status-only update. Multiple eligible
   Projects require `--project`; Desktop shows that same explicit selection.
   A selected Project must belong to this Wave, must be nonterminal, and must not
   replace an existing different current Project implicitly.
5. With no eligible Project and a confirmed complete inventory, provision an
   ordinary Project named for the Wave (no chapter date/suffix or metadata).
   Persist its chosen UUID before the first mutation. Create, attach and activate,
   reading back each uncertain effect by exact identity. Names may later change.
6. Re-read current membership before returning success. A concurrently created
   external competitor is reported; never delete it, rename it, or silently pick
   the locally created Project. Repeated calls resume the same pending identity.

Sole-candidate promotion is a reversible design assumption, recorded separately.
Terminal Projects are history and are never reopened. A later genuinely empty
current generation may create another ordinary Project; UUID reservation must
not be permanently derived from the Wave name or reused from a completed Project.
A pending target that is archived, deleted, moved or terminal stays an explicit
conflict rather than authorizing a replacement UUID.

Linear owns connected Project facts, status, content and membership. Existing
normalized SQLite planning remains the read model. Inspection of
`ops/pm.rs::resolve_context` and `planning.rs` found Linear-only Project IDs and
required repository/Initiative binding; there is no existing local-only Project
writer to reuse. Older memory describes local planning as accepted direction,
not an implemented capability. This Task targets configured Waves, as its
acceptance specifies. Missing binding reports the existing connection action;
provider failure never switches to invented local planning. A local-only planning
implementation is separate scope, not a prerequisite or second store here.

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
current selection, and any predecessor completion are confirmed. Preserve it
across crash, response loss and process replacement. Query it by Wave and, for
reset re-entry, the explicit reset name; do not select “latest” by timestamp.
Retain settled receipts as operation history without giving them current-plan
authority. An unresolved older transition cannot be overwritten by another reset.

The supported concurrency boundary is the configured owning Home, consistent
with Jack's one-client decision. Competing Desktop/CLI processes share the lock
and reservation. External Linear edits are not lockable; recheck ownership and
surface conflict. Do not claim a distributed transaction or invent a fleet lock.
Retain existing stable reset successor IDs where a target already exists.

## Optional coordinated reset

Keep explicit `lf repo new-chapter <name> --dry-run --json` and apply as the
coordinated operation. Desktop's “Realign Projects…” previews the same result
and applies only after an explicit action. No reset occurs on Wave opening.
Expose affected Wave, predecessor/successor and Task dispositions in that preview;
show unavailable Waves and unresolved work before applying.

Replace shared-predecessor-name inference with independent per-Wave selection
plus the exact transition receipt. Different predecessor names are normal.
Existing explicit target Projects retain identity and content. Copy an optional
Flow only when present; retain current KR/metric reset semantics and historical
provider content. A reset can start with no prior chapter or no prior Project.

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
Unknown evidence remains unresolved. Only proven untouched backlog expires.
Complete predecessor after transfer, never merely because successor activation
succeeded. No global rollback, Session restart, Task completion or inferred KR win.

## Consumers and deletions

This is one coherent delivery; internal slices are implementation order:

1. **This slice:** move ordinary Project ownership out of `chapter`, remove
   nonempty-Flow create/adopt/update/reset gates and name-prefix rejection, and
   preserve provider bytes on ordinary adoption. Cut CLI/status/DTO consumers
   over with it. Focused test: `cargo test -p loopflow --lib project_ensure` with
   new regression cases proving an existing no-Flow Project survives adoption.
2. Add transition persistence and explicit ensure, with concurrent/crash/uncertain
   response tests through real operation entry points and a stateful fake provider.
   Missing binding must not create synthetic local plans. Keep schema migration preservation
   tests at the released frontier, not successive draft versions.
3. Replace reset name inference with receipt recovery. Keep existing preservation
   tests, replacing tests whose sole assertion is a required shared name/Flow.
4. Add Desktop selection activation through `RegistryQuery`, separate Project and
   primary-conversation outcomes, and explicit reset preview/apply/retry. A Project
   error does not hide its conversation or preserved last-good Task history.
   Rename ordinary “Chapter” labels to “Project”; metrics belong to the Project.
   Add required-or-optional Rust/Swift JSON fields and fixture updates together.
5. Update `docs/lf.md`, command reference, planning architecture, repo guide and
   builtin `start-chapter`, `wave/start-chapter`, `repo/session`, `wave/session`
   instructions at their actual owners. Ordinary workflows call ensure only at
   activation, not observation. Reconcile the LOO-367 call site without importing
   its lifecycle changes into this Task.

Delete `plan_rotation`'s cross-Wave `predecessor_names` heuristic, fallback
`feature` during Project creation, empty-Flow refusals, and
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
  Backlog and current no-Flow adoption preserve all bytes/IDs; multiple current
  and multiple eligible Projects return explicit ambiguity; outage and partial
  inventory produce no provider writes; interrupted create/attach/activation
  resumes exact UUID; archived pending identity is not recreated. Assert outcomes,
  not mock call wiring.
- `cargo test -p loopflow --lib chapter`: different ordinary names, missing Flow,
  no previous chapter, response loss at every mutation, unrelated Wave outage,
  and external competing successor. Snapshot Task, PR, checkout, Session and Flow
  identity before/after; retain start-versus-retire and unknown-evidence cases.
- Add `rust/loopflow/tests/project_readiness.rs` and run
  `cargo test -p loopflow --test project_readiness`: CLI ensure → fake Linear
  state → persisted planning → Wave status/roadmap and JSON consumed by Desktop.
  Repeated status/roadmap must leave provider Project count/status and transition
  admission unchanged. Failed reset on Wave B must not block ensure/use of Wave A.
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

Check: `git diff --check` passed; `lf context --wave infrastructure --skill kickoff --json` confirms both authored sources fit; behavior/build checks deferred to implementation and gate (prose-only kickoff).
