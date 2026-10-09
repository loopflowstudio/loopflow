# One plan, optional Linear synchronization

LOO-406 · Infrastructure · revised 2026-10-08.

Jack Heart authorized implementation through publication for review, not landing
or installation. His relaunch comment `ee5f29c0-c176-4f15-815d-cdd07e7ddffd`
selects the coherent Wave schema/definitions and deletion cut, preserving the
ship-decomposed direction. Full synchronization remains unfinished. The older Task
brief's exclusion of export/sync and mandatory one-PR shape are superseded by the
October 8 decisions below. LOO-412 owns transport, not a second planning owner.

## Review approval — 2026-10-08

Jack Heart approved the reviewed design and behavior in the Task conversation:
“i think tis is approved from my perspective.” Product review is satisfied;
required verification and repair remain. This does not waive the retained
concurrent-write failure or authorize installation. Jack also described Asana's
fractional ordering keys; replacing local integer positions is a consideration,
not an accepted implementation change.

Jack Heart requires `queue` before landing: `compress → refresh → gate`.
The standalone gate pass does not substitute for that Flow.

## Queue preparation — 2026-10-08

Jack Heart selected `compress → refresh → gate`, not landing, installation or Task
completion. Compression is committed at `ffe986160`; refresh reconciliation at
`bc4c26fad` retains recorded base `04a4a296b`, already an ancestor. Gate reviewed
the five changed Rust files and updated architecture/CLI docs against `078a6642e`.
No further product edit was warranted. Earlier proofs retain their exact source
boundary; they are not a new pass for the changed receipt path.

Compression removed `settle_task_state(delivery, Option<error>)`: production only
used its error branch, while tests could manufacture success with `None`.
`task_state_error` cannot settle a receipt. Late-response fixtures now use provider
observation, retaining adopted conflicts, newer decisions and superseded uncertainty.
No provider-write policy or schema changed. Docs describe the surviving local
rotation/foreground delivery rather than the removed provider-first path.

The gate's disposable Linux runner could not acquire
`/source/target/.installation-proof.lock` within its 15-second bound (exit 1,
empty build log). Materialization, compilation, Clippy and focused receipt tests
never started. Its container was removed; no foreign build was interrupted.
This replaces the earlier 7m42s lock wait with bounded evidence, not a passing
build. macOS startup checks were not repeated. Swift/app checks and the remaining
matrix stay deferred to capable CI; checkpoint CI's classifier supplies no proof.

The enabled unseen-reopening fixture is byte-identical. Source review confirms
the intervening reopening occurs in state lookup after ownership acquisition;
unconditional mutation overwrites it, then matching readback settles. The test
also expects a still-pending loser, contrary to accepted observed-conflict
retirement. Another read cannot provide atomic protection. Precedence needs no
new decision; disposition of this stronger test expectation remains with review.
No assertion change, waiver, ignored test or provider atomicity is claimed.

The complete supplied Work seed and Release child GOAL/MEMORY were inspected.
Release's operation-entry lesson applies: helper/classifier success cannot prove
public or installed acceptance. No new whole-tree gate pass is claimed.

Pre-gate scratch and separate index/working patches, SHA-256 manifest, runner,
logs and context queries are retained at
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo406-queue-gate-ib9gc30k/`.
Separate review bytes and index remain unchanged. Earlier uncommitted notes survive
there and in `/tmp/loo406-queue-compress-4IEJHX/` and
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo406-realign-kce7_df9/`.
Selected scratch-only commits exclude scratch; these archives, not a claimed
scratch checkpoint, preserve that evidence.

## Standalone gate evidence — 2026-10-08

Gate began at `e58eab60f`; `078a6642e` repairs architecture ownership docs, explicit
Wave ingestion/Machine isolation, stale DTOs, abandonment and adoption fixtures,
and public reconnect's Initiative/Team/managed-account setup. It preserves ordinary
admission rules, source isolation and the enabled unseen-write failure.

Disposable Linux Rust 1.99.0 compiled the materialized draft. Migration preservation,
local lifecycle/DTO/adoption, native-runner and public Flow/work-watch reconnect
passed after focused repairs. The reconnect proofs cover independent inbound
changes, offline saves, lost replies, selection changes and close/reopen. No host
Machine was mounted; test execution had external networking disconnected.
macOS Clippy, Swift and isolated Python stopped at bounded startup, with helpers
sampled at `_dyld_start`; these are neither test passes nor a security diagnosis.
Do not repeat unchanged startup attempts. Swift app/model/view, Xcode and the
remaining full matrix need capable gate/CI. No installed acceptance is established.

Commands, logs, samples, runner and the original notes/review preservation archive:
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo406-gate-hh0jsek2/`.
PR #1503's `b6f34a6f8` checkpoint run `37871563805` passed only the scratch classifier;
product jobs were skipped. `ci.yml` provides materialized Linux and headless macOS
coverage only on an eligible candidate. No publication, scratch cleanup or landing
was performed by this gate. Release's operation-entry lesson still applies:
helper proofs do not substitute for public reconnect or installed behavior.

## Common delivery presentation — 2026-10-08

`6bb8b935a` adds `PlanningSyncStatus`; `086d3560c` deletes pending-only edit
projections. Task status, Wave/roadmap planning, Project results and Desktop read
creation, field/order, state and comment receipts. Unmapped creation is pending
when connected; attempted effects retain uncertainty/errors, and conflicts expose
saved and observed values, including null. Comments have no attempted-write bit.
Disconnection hides pending delivery, not losing edits. Project order is visible
on the Project and its Tasks. Execution is unchanged.

Creation receipts advance planning revision in the same migration draft. Fixtures
cover unmapped creation, fields/order, errors, conflicts and settlement; the gate
above supersedes the earlier Rust startup-only evidence, not missing Swift proof.
Earlier source/log detail and separate review bytes remain in the pre-compression
archive above and `/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo406-sync-2o44_e6l`.

## Project ordering delivery — 2026-10-08 (`b1f175bbf`)

The Project's existing `project_changes` receipt now owns `task_order`: the full
saved list, provider baseline and each attempted move's input/before/after lists.
Task edits save all changed ranks with that receipt in one transaction. The released
frontier retains its previously observed relative order as the delivery baseline. Scalar
Task rank receipts and their generic ingestion path are removed. The same migration
draft adds the complete-list observation to `pm_projects` and captured move evidence
to `project_changes`; no second outbox, schema draft or planning owner is introduced.

Foreground field delivery reads all Project issues and retains Linear's lexicographic
`prioritySortOrder`/`sortOrder` semantics. Moves use neighboring key intervals; equal
keys use recorded intermediate moves without flattening primary groups. Complete-list
ingestion recognizes these effects before reconciling later saves, so Loopflow's own
partial reorder is not a conflict. Detail reads never set rank. Matching readback
settles an attempt; an unchanged list remains uncertain without repeating the mutation.
Observed competing order adopts Linear and retains the losing list. New members
survive; omission alone defers delivery. Confirmed local deletion/refiling excludes a
Task from the delivery target while the original saved list remains in the receipt.
Scalar Project fields deliver before ordering so uncertainty cannot starve them.

Source fixtures cover foreground outage/reconnect, partial delivery with a lost reply
and store reopen, independent acquisition plus a later local save during a write,
observed conflict adoption, no replay of uncertain effects, tied keys, new members,
stale lists and removal. The existing public offline-edit fixture now checks one
Project ordering receipt, unchanged Task identity and local rank projection. These
prove operation behavior, not composed native/Flow/Desktop reconnect or installation.
The pinned Linear schema exposes both sort keys; the documented unconditional-write
race and enabled unseen-reopening regression remain unchanged. Nonrepresentable
floating-point intervals retain an explicit pending error.

Review repaired scalar delivery starvation and removal stranding. The prior export
compression is checkpointed at `dcd015403`. Pre-edit source, notes, index and separate
walkthrough artifacts remain with their SHA-256 manifest at
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo406-order-snxljykd`.

## Creation export — 2026-10-08 (`e68f2a423`)

The independent foreground loop delivers saved Task/Project UUIDs from existing
creation receipts. Project creation and Initiative attachment retain separate
attempts; exact readback recovers lost responses without replay. Common ingestion
attaches identity before inventory can duplicate it and preserves later local saves
against the captured baseline. Removal after an uncertain create acquires its
mapping before deletion. A Wave without an Initiative stays pending.
Operation fixtures cover reconnect, lost replies and later saves; composed and
installed acceptance remain unproved. Full protocol, review repairs and preservation
paths remain at the pre-edit snapshot above and `e68f2a423:scratch/explore-loopflow-s-own-store.md`.

## Deletion delivery — 2026-10-08

`fc056bb6b` consumes deleted receipts, including removed mapped Tasks. Exact trash
observation or captured mutation acknowledgement settles; absence and lost replies
retain uncertainty. A newer explicitly active Linear revision retires removal and
restores visibility with the losing receipt retained. Ordinary inventory proves neither
trash nor restoration. Workflow, Process, Session, PR and checkout history survive.
The foreground fixtures prove reconnect, lost replies, concurrent delivery and conflict
adoption, not composed or installed acceptance. Detailed protocol, fixture repairs and
preservation evidence remain at `fc056bb6b:scratch/explore-loopflow-s-own-store.md` and
the pre-export snapshot above. The unconditional provider race remains documented.

## Cancellation delivery — 2026-10-08

Commit `67ee4024a` delivers mapped cancellation from the existing state receipt in
the independent foreground loop. Completion, reopening and cancellation share one provider
state resolver and mutation. Resolve the issue's own Team and desired state before
marking the receipt attempted: failed reads or a missing state leave the same save
retryable. Once a write is attempted, response loss retains uncertainty; exact state
observation resolves it without blindly repeating the mutation. Observed conflicting
Linear state retires the local intention, retaining both values and the local
abandonment. No synchronization path acquires Workflow, Process or cleanup authority.

The isolated operation fixture covers offline cancellation followed by foreground
reconnect without another command/turn, lost mutation response, conflicting provider
completion and recovery when the Team initially lacks a canceled state. It establishes
this delivery lifetime, not public CLI/Desktop reconnect or installed acceptance.
The known unseen-write regression remains enabled; provider writes remain unconditional.
Complete pending-presentation acceptance and composed reconnect remain required. Cancellation is an internal implementation
slice; the full connected ownership cut still lacks shippable acceptance.

Pre-edit source, staged/index evidence, notes and review artifacts with SHA-256
manifest: `/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo406-cancellation-o0hnn0py`.
Prior compression is checkpointed as `22ee52c31`; the independent walkthrough edits
remain separate. The design's stash conflict retained both evidence sections.

The compression included in `fc056bb6b` shares the Workflow-state query across all
three outcomes. It also fixes completion's missing `position` field: default zeros
previously defeated lowest-position selection. The retained fixture covers the
issue's own Team and minimum position for completion, reopening and cancellation.
Obsolete provider-first fixture handlers are removed; adoption fixtures now share
one disposable store for Wave definitions and PM operations. Their prior failures
were isolation/setup faults, not evidence against the common planning writer.

## Field delivery and creation review — 2026-10-08

Jack Heart's comment `8821db44-0480-4493-8609-953207663f1d` rejects public
creation tokens. `lf task create` now generates the durable Task ID, saves the Task
and returns that ID. Separate invocations create separate Tasks, including identical
titles. No caller token, copy-and-retry instruction or title deduplication remains.
The store retains internal creation receipts and saved identities for automatic
synchronization/export. Public fixtures create normally; prefix-collision fixtures
seed deliberate IDs through the internal store.

`ops/planning_delivery.rs` adds an independent foreground loop consuming common
field receipts for mapped Tasks and Projects. Task title, description, nullable
assignee and membership, plus Project name, summary, status, Workflow, KRs and
metric targets use provider read/ingestion, captured receipt, attempted write and
readback. Project content patches preserve unrelated prose. A changed provider
baseline adopts Linear; attempts remain uncertain until observation resolves them.
There is no blind replay after an uncertain write. The existing migration draft
adds attempt/error evidence and acknowledged provider revisions to both receipt
tables; no new owner or migration.

A matching observation acknowledges only an attempted receipt. Before ingesting that
observation, reconciliation advances the baseline of subsequent same-field saves
that still share its prior value. The old response cannot acknowledge the newer
receipt or make its local value look like a provider conflict. Effect locks exclude
neither local saves nor inbound acquisition. Attempt capture rechecks the saved
provider revision and local deletion, preserving newer facts and tombstones.
Linear's unconditional mutation race remains; these checks provide no provider fence.

The Project ordering cut above replaces scalar rank delivery. Pending-presentation acceptance and composed
native/Flow/Desktop reconnect remain required. The foreground-lifetime fixture is
source evidence, not a public CLI/Desktop reconnect or installed acceptance claim.

## Core-model review — proposal, not accepted (2026-10-08)

Jack Heart requested a walkthrough centered on Task, Wave and the other core models
and invoked concept-review. Comment `34c1739a-8831-4578-96c9-797aeaa87743` records
the proposal: Task could own the complete shared planning record while placement,
captured Workflow, Session and Process remain machine-local records linked by Task ID.
TaskPlan currently contains only some planning fields; Task mixes checkout/control,
and PmItem supplies status/rank/assignee. Flattening TaskPlan alone would retain the
split. Project/ProjectPlan/PmProject have an analogous split; Wave identity versus
owned documents has a clearer purpose. No structural consolidation is authorized.

Open product question: should a Task completed in Linear display Done with a
still-running local Flow shown separately? No answer is inferred. This proposal
changes neither ongoing authorized implementation nor the accepted creation-ID
removal and Linear-wins policy. The ongoing review owns its walkthrough artifacts.

## Common ownership cut — 2026-10-08 (`84664e661`)

All Waves now use ordinary hierarchical addresses and the existing durable rows.
The one migration draft removes `personal_plans`, `PlanId`, personal Wave storage,
`PlanningAuthority` and `project_authority_on`. `sqlite/wave_documents.rs` stores
all imported top-level Markdown and custom Workflow definitions. Explicit ensure
creates ancestors and imports missing documents atomically; retries preserve stored
edits and source bytes. Existing authored IDs survive. The released-frontier
migration imports available definitions for registered Waves in its transaction;
missing directories leave identity intact for later explicit import. No file-backed
reader or personal namespace remains.

Wave config, goal/memory editing, inherited prompt context and CLI/Desktop planning
read stored definitions. Project ensure and default inbox creation use the same
Wave writer. Relocation changes stored addresses while definitions remain attached
to identity. It neither moves repository sources nor rewrites execution placement.
The old Git relocation/cleanup sequence and directory-driven identity reconciler
are deleted. Historical relocation receipts remain; recovery of previously uncertain
effects is not established by this cut.

Task deletion now commits local removal, planning revision and a stable pending
`deleted` field receipt together. Retry preserves the receipt; rollback preserves
both plan and history. Workflow, Session, Process, PR, checkout and completion
records remain intact. The provider-first delete/trash/readback writer, authority
selector and exclusive fixtures are deleted. Historical provider deletion tables
and retained identity evidence survive. Connected deletion reports pending sync;
mapped provider deletion now uses the foreground field-delivery loop described above.

Paired public CLI fixtures cover connected/offline and unconnected definitions,
source removal, ancestor context, stored Workflow source, relocation and deletion.
Deletion retains active execution and provider mapping, and verifies rollback,
retry and store reopen. Sixteen local-planning tests pass. Source checks also
cover released-frontier definition/link preservation, atomic ancestor rollback,
relocation collisions/cycles and direct/Task prompt context. These establish local
behavior, not installed conversion, composed provider reconnect or full Desktop
acceptance. Gate owns the broader and materialized migration checks.

Review aligned the Project Workflow catalog with stored selection, removed an
unnecessary Git lookup from Wave snapshots and fixed test setup
that stored a noncanonical repository path. The configured source remains canonical;
fixtures import definitions explicitly rather than restoring implicit file reads.

Commit `84664e661` is the coherent source boundary for LOO-412 to review and consume.
It is not an independently shippable connected product yet: pending presentation
and composed reconnect remain. No feature flag, adapter or rejected ownership
mode makes that missing behavior acceptable. Publication belongs to the authored
pursuit boundary; this implementation step neither lands nor completes the Task.

Reconciliation on October 8 inspected `bbc6eb8d3`: `185de5fbf` implements the
observed-conflict adoption requested by the earlier iteration feedback. Compression
is committed in `05d36d79f`, `e7e3f45b7` and `bbc6eb8d3` before this field-delivery cut.
`84664e661` remains the original consumable ownership boundary. The merged LOO-436
startup changes add no planning synchronization. The ongoing review is recentering `pr-review.html` on core models; pinned excerpts
and captures do not prove the current working content.

## Preserved common writers

The saved-plan reader (`459331192`), comments (`b3cd894f3`), abandonment
(`783305284`), Task creation/editing (`b6e291dd4`), shared field receipts
(`71c8e445c`), Project creation/selection (`764e90ae3`, `6570ee0f7`), rotation
(`4be12b5c8`) and refiling (`fa35cfd88`) retain one local transaction path.
Placement keeps its Wave locks and winning PR/checkout; captured Workflows survive
missing source definitions. Started work, unreviewed backlog and uncertain provider
transitions keep their identities and evidence. Local settlement proves no delivery.
Earlier implementation detail and preservation counterexamples remain at
`fa35cfd88:scratch/explore-loopflow-s-own-store.md` and
`f027890ab:wave/infrastructure/MEMORY.md`.

The existing migration draft starts at integrated v0.13.10 (`fc0bb97f7`). No second
draft or installed-store experiment was added. Release's complete immediate-child
goal and memory were read: its operation-entry recovery lesson remains applicable;
helper success cannot establish public reconnect or installed acceptance.

## Accepted conflict policy — Jack Heart, 2026-10-08

Jack Heart clarified: “If you are using Linear, then we want to defer to linear
in any conflict scenarios.” Linear wins all planning conflicts: status, text,
membership, rank and edits to an existing comment. Saves still commit locally
and show pending sync. Compare incoming facts with the saved delivery baseline;
an unchanged provider value is not itself a conflict. On a conflicting provider
change, adopt Linear's value, retain the superseded local intention in history,
and stop retrying that intention. Independent new comments remain additions.
Mandatory manual winner selection and local-wins conflict resolution are superseded.

Without Linear: “we want to defer to the host if possible, but also last write
wins and have try-not-to-clobber policy.” LOO-412 owns host preference, ordering
and transport. Prefer the host when available; otherwise last-write-wins with
best-effort avoidance of clobbering and recoverable losing edits. Exact host
selection and ties remain transport design details, not a new local storage mode.

Jack explicitly reaffirmed: “exec state though *doesnt go into linear*”. Only
planning synchronizes. Sessions, Processes, Flow progress, captured Workflow
position, reservations, checkouts and execution/cleanup authority stay local.
Execution may produce a planning outcome; importing that outcome never moves a
Workflow, stops a Process or cleans a checkout. Linear's conflict precedence
confers no execution authority. LOO-412 excludes the same execution state.

Jack's try-not-to-clobber policy supersedes the absolute zero-overwrite
requirement. The unseen-write regression remains enabled as contrary evidence;
its failure is neither an unanswered product choice nor proof of atomicity.
Acceptance still requires Linear precedence for observed conflicts, recoverable
losing edits and honest pending/uncertain outcomes.

Commit `185de5fbf` reconciles accepted Task/Project observations against
field baselines in the same transaction. A changed Linear value adopts locally and
retires the losing intention; an unchanged baseline preserves pending saves. Receipts
keep both values and stable IDs. State ingestion and delivery share this rule,
including uncertain completion followed by reopening. Old responses cannot settle a
newer local decision. Workflow position and execution records remain untouched.
Comment collisions adopt Linear's body, author and time, retaining the complete
local comment and first conflicting observation. CLI/Desktop show the saved local
body beside the current provider text; no replacement or repeated local direction.
Manual winner selection, its command flags, replacement-comment schema and exclusive
tests are deleted. Ordering and export are implemented above.

Jack invoked `ship-decomposed`: inspect existing seams and prepare independently
shippable changes. This supersedes the mandatory one-PR delivery shape. No seam
is accepted merely because a partial cut compiles; each slice must preserve
connected behavior and remove the obsolete path it replaces.

## User-keyed and shared planning — Jack Heart, 2026-10-08

Jack Heart selected: “default to user-keyed planning, but then make it possible
to all choose to share one, and make a good experience for those using that in
an approporiate way”. A common code remote never implicitly combines developers'
plans. Each user's machines select that user's planning destination by default;
collaborators can explicitly select one common destination. This is synchronization
selection using the same records, writers and APIs, not personal/shared model types.

LOO-412 owns stable user-key selection, planning remote/ref binding and shared-plan
exchange. User keys must survive machine replacement; do not infer identity from
Git display names or silently mint a different user identity on every machine.
Key provisioning/recovery remains a transport design detail. Joining an existing
plan must not silently publish or merge existing local planning into it.

The proposed shared experience shows the selected plan, authorship, assignees and
pending changes. Stable Task IDs preserve identity across people; branch naming
retains that identity and prefix ambiguity remains explicit. Separate creations
and comments accumulate; different-field edits survive. A worker's initiating host
must not gain blanket precedence over other collaborators. Shared conflict ordering
needs a deterministic rule under the accepted last-write-wins policy.

Planning selection is local connection configuration. Tracked `wave/` and `.lf/`
files remain shared repository content; personal planning must not silently rewrite
them. Ref separation avoids accidental mixing, not access control: private planning
requires a separately access-controlled remote. No real plan publication is selected.
Execution remains local even when developers deliberately share the plan.

Acceptance must include two independent developers in one code repo, one developer
on two machines, and two developers explicitly sharing a plan: no accidental
mixing, duplicate IDs, silent replacement on join or execution-state replication.

## Retained synchronization race evidence — 2026-10-08

`task_completion_preserves_linear_reopening_during_delivery` drives the real
`sync_task_state` writer against the existing loopback Linear fixture. Local
completion saves first; after the ownership read, while the client resolves the
completed state, another Linear client completes and explicitly reopens the issue.
Loopflow's unconditional mutation then overwrites that reopening with `completed`.
Readback agrees with its own write, so synchronization returns success and settles
the receipt. The new regression requires `unstarted` plus a retained local decision;
it fails. No sleep, clock ordering, live provider or installed store is involved.
This establishes a protocol failure, not composed CLI/Desktop acceptance.

Linear's [published schema at 7d2bc4279f](https://github.com/linear/linear/blob/7d2bc4279f1887cf763c59f9a173d9c590620023/packages/sdk/src/schema.graphql)
was inspected on October 8. `issueUpdate` accepts `id` and `IssueUpdateInput`;
neither it nor `issueBatchUpdate` exposes an expected revision/state condition.
`lastSyncId` is output, not a write precondition. No documented provider fence
was found. Another pre-write read only moves the race; readback, event history
and webhooks cannot prevent an already-issued write from replacing reopening.
This does not establish that every undocumented provider capability is absent.

The earlier design treated automatic propagation and zero concurrent overwrite as
an unresolved product choice. The accepted Linear-wins policy above replaces
that choice. The fixture remains contrary evidence about an unseen intervening
write, not proof that conflict precedence is unimplementable. Delivery must
honor observed Linear conflicts, preserve uncertain effects, and state the
remaining race honestly. Conditional-write research is no longer an unbounded
prerequisite to the local ownership cut.

The October 8 follow-up inspected Linear's [mutation documentation](https://linear.app/developers/sdk-fetching-and-modifying-data),
[GraphQL guide](https://linear.app/developers/graphql) and [webhook contract](https://linear.app/developers/webhooks).
These document updates and after-change notifications, not a conditional-write
guarantee. The GraphQL guide also says changes during an issue's first three
minutes are omitted from its activity log; activity history cannot be assumed
complete evidence of intervening reopening. This narrows the history-based
alternative without exhausting provider capabilities. No live mutation probe,
provider confirmation, or changed product requirement is established.

The subsequent Project-writer investigation inspected `projectUpdate` and
`ProjectUpdateInput` in the same pinned schema: only the ID and partial field input
are exposed, with no expected revision. This extends the unresolved concurrency
boundary to Project edits; no provider contact or live mutation probe occurred.

The same schema verifies UUID-v4 `id` inputs for `IssueCreateInput` and
`ProjectCreateInput`. The creation-export cut above implements saved UUID delivery and exact readback;
configured provider acceptance remains unproved.

## What to build

One locally owned plan with the same schema, APIs and behavior in every repository,
and optional repository-wide, two-way Linear synchronization.

Jack's accepted direction:

> either everything synced to linear or nothing
>
> on disk and code reads the same either way, with hooks for linear syncing,

Jack explicitly requested deleting `project_authority_on` and rejected the
`personal_plans` and `personal_workflows` split. Asked how a connected repository
behaves during an outage, Jack selected: “Save locally; show pending sync”.
Jack also requested Task ID prefixes matching Git more closely.

## Scope and placement

Infrastructure owns the local planning and synchronization boundary. A repository
has one plan. Its Linear connection applies to all its Waves, Projects and Tasks;
there is no personal/shared selector, per-Wave planning authority, or alternate
local writer. Optional Linear mappings retain identity and existing ticket aliases.
Linear-compatible fields and operations remain the public shape.

Cross-machine replication, Git plan refs, reverse host callbacks, account sharing
and a hosted service remain outside this PR. The earlier three-situation analysis
and Git-ref/callback proposals survive at
`dfe18ab6060901992b55e64842e23c4295673b08:scratch/explore-loopflow-s-own-store.md`.
A single local store is not a backup or cross-machine durability claim.

## The demo

In a disposable repository with no Linear connection, create an inbox Task, edit
its title, comment, select a workflow, place the Task and run work. Its records and
normal read APIs require no provider. Publication and verified GitHub merge use
the existing PR lifecycle; requesting a merge alone never means done.

Use the same commands in a Linear-connected fixture. Disconnect the provider,
edit the Task and append a comment: both commands succeed, and CLI/Desktop show
the saved values and pending sync. Restart the CLI and restore the provider while
the connection is active. Catch-up proceeds without a manual refresh or new agent
turn. The existing Task maps to one Linear issue; pending work settles without
losing later edits or duplicating comments. Complete and reopen from either side;
the same stored Task and UI reflect accepted changes while an observed conflicting
reopening retires delayed completion. The unseen intervening-write race remains
the documented best-effort limit. This is fixture acceptance;
configured Linear and installed acceptance remain separately identified evidence.

## Delete — do not maintain

Compression `086d3560c` removes the remaining pending-only presentation:
`TaskEdit::{sync_enabled,pending_changes}`, `ProjectPlanning::{sync_enabled,pending_changes}`
and `SqliteStore::project_with_changes`. Task edits and Project commands now use
`PlanningSyncStatus`, including creation, ordering, uncertainty and losing values.
Keep internal pending readers and captured baselines: delivery consumes them, and
fixtures still prove receipt identity, unchanged saves and conflict retirement.
Task status opens its store once for planning and sync; after the local-Task return,
only the provider-only case remains. Delete its unreachable local fallback.

Completed cuts that must stay deleted:

- `SqliteStore::settle_task_state`'s synthetic success branch. Failure recording
  uses `task_state_error`; late-response tests exercise actual observation. Keep
  superseded attempted receipts uncertain and preserve the enabled race counterexample.

- `tests/e2e/task_deletion.py`: its provider-first mutation handlers were obsolete;
  its remaining local-abandonment proof now lives in portable `task_abandonment.rs`,
  retaining Session/capture, unresolved Process, published PR and file assertions.
  `planning_reconnect_tests.rs` owns only HTTPS reconnect; its handler no longer
  inherits from another product fixture. Linux TLS isolation stays unchanged.

- Task-required `PlanningSync` and work-watch Task lifetime selection; repository
  ownership survives selection changes, and all repository consumers share it.

- Per-Task rank receipts and detail-driven rank updates: one Project `task_order`
  receipt owns delivery. Complete-list observation carries partial progress into
  later saves; tied keys, omission, removals and uncertain moves retain evidence.
- `TaskCommand::Create::creation_id`, public retry instructions and
  `SqliteStore::task_creation_intent`: ordinary creation generates its own ID.
  Keep internal transactional creation/export identity; no title deduplication.
- Personal namespaces/types/schema, `PlanningAuthority`, `project_authority_on`,
  file-backed Wave reads/edits, Git relocation rewriting and provider-first deletion.
  Keep source bytes, common writers, identity, historical uncertainty and execution.
- `SqliteStore::import_wave_documents` and `engine::workflow::workflow_source`
  wrappers; explicit ensure/migration still import atomically. Stored Wave documents
  and Workflow selection remain the reader source.
- Manual conflict selectors (`resolve_task_state`, `resolve_task_comment`, `--resolve`,
  `--comment` on Task sync), replacement-comment schema and exclusive fixtures.
  Linear adoption retains both values and never replays a retired intention.
- Uncalled provider-first Project writers (`set_project_status`, `create_project`,
  `attach_project`, `update_project`, `complete_and_archive_project`), payloads and
  exclusive tests. Common export owns captured creation and attachment effects.
- Separate `complete_item`/`reopen_item` senders and state queries, unimplemented
  cancellation refusal, obsolete creation/rotation/trash fixture handlers.
  `item_state_id` resolves the issue's Team and minimum-position state before
  attempting any of the three outcomes; failed discovery stays retryable.

Repository lifetime is checkpointed at `29777b8cb`; fixture separation at
`b6f34a6f8`. Detailed earlier reductions,
fixture failures and evidence limits remain at
`fc056bb6b:scratch/explore-loopflow-s-own-store.md`. The complete pre-compression
notes and separate review artifacts are preserved with index/working/branch patches
and SHA-256 manifest at
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo406-compress-sync-1yw_g05j`.
No schema, provider-write policy or execution authority changes in this reduction.

## Data and APIs

- Existing `Task`, `Project` and `Wave` rows own the plan. Stable UUIDs precede
  placement and optional Linear mappings. Creation receipts preserve original
  input and Project through edits/rotation. Workflows, Sessions, Processes and
  PR chains retain their existing owners and controls.
- Wave goal/memory and custom workflow definitions use the same stored shape for
  connected and unconnected repositories. Existing Git-authored content must be
  preserved through an explicit ingestion/cutover; connected mode must not keep
  another file-backed read path. No intermediate schema or second draft.
- One repository connection selects optional Linear synchronization. Use the
  existing repository configuration boundary; a new per-Project selector is
  forbidden. A configured connection implies all plan objects participate;
  an absent mapping means pending export, not an unsynchronized personal object.
- A local mutation and its pending provider effect commit together. Reuse existing
  writeback concepts where their meaning fits; completion-only `PmWritebackState`
  must not become a second contradictory sync status. Sync records contain the
  stable operation/object identity, captured local revision and required payload,
  provider mapping, acknowledged revision and unresolved error/conflict. They
  record delivery, not a second copy of the authoritative editable plan.
- `create_task(NewTask) -> Task`, `edit_task(id, expected_revision, patch) -> Task`,
  append-comment and Project/rotation mutations use one transaction path.
  The exact final Rust signatures can follow surviving APIs. Reads return these
  owned records and derived pending/conflict state, without a provider round trip.
- Active CLI/Desktop connections drive bounded synchronization independently of
  agent turns and unrelated outbound work. Local comments and completion commit
  immediately and propagate on that active path; inbound state, membership and
  comments enter the same writer and visible plan. Explicit repository sync is a
  recovery entry, not a prerequisite. Disconnection preserves pending/uncertain
  mutations; reconnect resumes by stable mutation identity, without echoes or
  duplicate Tasks/comments. No resident service, hidden turn/Flow replay or
  indefinite retry.

## Synchronization and preservation counterexamples

1. **Commit succeeds; reply is lost.** The saved Task retains its generated ID.
   Internal delivery retries reuse it across restart. Another explicit create is
   another Task; users do not provide a recovery token.
2. **Linear accepts; acknowledgement is lost.** Retain uncertain publication and
   the operation identity. Confirm by exact readback before another create.
   A description marker and two empty lookups alone do not prove duplicate safety.
   The inspected schema supports saved UUIDs; export uses them for exact readback.
   An absent readback after an attempt retains uncertainty without repeating creation.
3. **A later local edit arrives while an earlier push is in flight.** Acknowledge
   only its captured revision. Do not clear newer pending fields or overwrite them
   with the first response. Preserve comment identity and ordering separately.
4. **Linear changes the same field while a local edit is pending.** Preserve both
   values in history and adopt Linear as the current value. Supersede the losing
   pending intention rather than retrying it. Unrelated inbound fields advance.
   Keep observation age; an offline read is not a fresh remote fact.
5. **Linear completes or abandons work.** Reconcile the planning observation while
   preserving the established distinction between external state and local
   Workflow/Process authority. External state never grants process control or
   checkout deletion. Local done/abandoned edits also persist during outage and
   synchronize later; provider failure cannot roll back the local decision.
   For observed competing planning changes, Linear wins. Retain mutation identity
   and provider revision evidence; missing ordering is not proof of successful
   delivery. Preserve the unseen-write race as a documented limit. Remote completion
   updates planning/presentation, never advances a captured local Workflow.
6. **A repository connects with existing work.** Existing mappings and IDs survive;
   unexported records become pending, not a second plan. Disconnect retains records,
   mappings and pending/uncertain history. No live connection change is authorized
   as a fixture or migration shortcut.
7. **An old Project is absent.** Retained Task identity and checkout files remain
   readable. All current/history IDs, Session/native identities, comments, PRs,
   deletion receipts and selection evidence survive the released-frontier cutover.

## Semi-live synchronization — Jack Heart, 2026-10-08

Jack's comment `e36cf3ec-5376-4d29-8ab0-a69a64d70919` requires preserving
responsiveness to Linear completion, newly added Project Tasks and comments.
Inbound observation runs independently of local mutations, manual sync and outbound
effect delivery. Pending local edits cannot stall unrelated inbound fields,
membership, state or comments. Same-field conflict retains both values; external
state grants no Workflow/Process or checkout authority.

Jack's follow-up `93690dc2-5526-4ecf-a203-5e248339b0c9` requires local comments
and completion to save immediately and propagate while a CLI/Desktop connection
is active, without the next agent turn, manual refresh or outbound queue drain.
Incoming comments/completion share the active acquisition path with Project
membership and reach both the common store and visible planning. The independent acquisition loops below cover only part of the required contract. Prove both directions during pending
unrelated writes and after reconnect, including duplicate delivery, lost replies,
and delayed completion after explicit reopening. Preserve stable mutation IDs and
avoid echoes through the common writer; callbacks around individual commands are
not the synchronization architecture.

Jack Heart selected one custom Git planning ref for LOO-412’s two-way planning
prototype in comment `8b45e82d-6765-490a-a3d6-44b44611cedd`, then accepted the
cross-Task plan in `b5402659-b825-4c12-b278-5d56ae1aa518` on October 8.
LOO-406 owns common local planning storage/mutation APIs and Linear sync;
LOO-412 owns Git-ref transport between machines, excluding execution state.
LOO-412 can build isolated transport/protocol fixtures now. Full integration
consumes a coherent committed common-writer boundary from LOO-406, without
copying dirty code or implementing a second planning owner. The ongoing LOO-412
conversation owns that prototype. No machine replication here, real plan publication
to the public code repository, or competing pursue Flow is authorized.
Semi-live comments/completion, visible pending changes, stable mutation identities
and the accepted conflict policy remain required. Inbound planning never moves
Workflow, signals Processes or cleans checkouts; transport provides no Linear
echo or duplicate-export guarantee. Publication here remains for review only.

### Repository foreground lifetime — 2026-10-08

The implementation removes `PlanningSync::start`'s Task argument and the watcher’s
Task-selector key. The connection now owns one repository lifetime; Task and Wave
selection change only projections. Changing or clearing the repository drops the
old lifetime, and closing stdin ends it. Projection connections remain read-only;
the separate sync connection opens only after the reader accepts the schema.
No fabricated Task, resident, migration or execution authority is introduced.
Task runners resolve their existing Wave's repository before starting the same owner.
Export, fields and state/comments delivery consume that repository directly.

Comment acquisition now covers mapped visible Tasks in every repository Wave,
including unselected and newly acquired Tasks, with independent five-second bounds.
A slow or failed Task/Wave does not postpone other comment requests. Review removed
the old per-Task lifetime and repeated Task-to-Wave lookups, and caught the initial
whole-loop timeout that could starve later Waves. Provider precedence and uncertain
receipt semantics are unchanged; the unseen-write regression remains enabled.

Authored checks: the existing public offline work-watch test now selects no Task
and requires a delivery error, distinguishing actual foreground attempts from mere
local projection. A focused repository-comment fixture asserts unselected acquisition
without placement or Workflow change. Linux-only real-binary HTTPS fixtures exercise
work-watch and `lf -b --task INF-123 run reconnect` with a contained Claude stub.
They cover outage/restoration, unselected comments/completion/membership, local saves,
lost state/comment replies, unrelated rejected title delivery, stable identities,
selection changes, stdin close/reopen and repository deselection. Assertions retain
Workflow, PR and checkout boundaries. TLS trust stays disposable; no production
endpoint override was added. The standalone Linux gate above passed these public
fixtures after repair, superseding their authored-only status at `9f78346aa`.
That proof does not cover Swift or the subsequent `ffe986160` receipt changes.
Earlier source/setup limits remain at
`9f78346aa:scratch/explore-loopflow-s-own-store.md`; pre-edit notes and review bytes
remain under `/tmp/loo406-scope-CG1Klo/`. The bounded startup observations prove
neither a source failure nor a security cause.

Current source boundaries (reconciled 2026-10-08):

- `lf/commands/run.rs::prepare_task_input` refreshes comments before launch.
  `ops/task_input.rs::TaskInput::start_planning_sync` starts `PlanningSync` from the common
  runner. Its inbound loop waits 15 seconds after each bounded attempt; batch
  `poll` consumes live steers and interrupts every 5 seconds. Acquisition and
  delivery run independently of that consumption.
- `ISSUE_OBSERVATION_QUERY` reads title, description and paginated comments.
  A separate bounded acquisition loop refreshes repository Waves through the
  existing snapshot ingestion path, including state and Project membership;
  one failing Wave does not prevent requests for the others.
- `ops/pm.rs::load_show_snapshot` refreshes on Auto reads after one hour, with a
  five-second deadline; Force refreshes immediately, Never reads retained facts.
  Existing explicit refresh and exact-issue acquisition paths remain relevant.
- `lf/commands/work_watch.rs` projects through a read-only store, with a separate
  writable connection for the scoped repository's `PlanningSync`. Its five-minute
  planning clock covers Git/filesystem facts; `PlanningSync` separately acquires
  Linear inventory every 15 seconds.
  The source has comment-ingestion APIs and historical webhook schemas, but no
  current Linear webhook receiver/registration call. The CLI parser explicitly
  rejects `repo webhook serve`. This establishes no configured webhook delivery.

At `73059ecd1`, periodic refresh ran only in batch Codex/OpenCode. Current source
starts `PlanningSync` at `run_agent`, covering native Codex, native Claude, Claude's
batch subprocess and the existing batch harness without changing stdin or mode.
Desktop's work watcher starts the same lifetime for its repository and stops it
on repository change/connection exit. Inbound comment attempts remain independent of
outbound delivery, every 15 seconds with a five-second per-Task request bound; pending
comments, state decisions and fields have separate one-second delivery loops. Repository
configuration enables these effects; retained mappings alone do not reconnect it.
These are foreground-owned lifetimes, not a resident service or a turn/Flow retry.
Comment acquisition covers visible mapped Tasks; repository-wide comment/state delivery
selects mapped Tasks, and field delivery selects mapped Tasks and Projects.
Unmapped records use the independent export loop above; the common receipt projection above includes them, with final acceptance still unproved. Mappings alone never define whether connected planning has unsynced work.

The common comment transaction records the thread, its delivery UUID and local
Steer. Provider acquisition enters that same thread and the existing Steer ledger.
The comment UUID is also the Linear mutation ID: [Linear's published schema](https://studio.apollographql.com/public/Linear-API/variant/current/schema/reference/inputs/CommentCreateInput)
accepts caller-supplied UUIDs. Lost replies use exact ID/issue/body readback;
concurrent delivery attempts cannot allocate another comment. Corrections retain
provider revisions; echoes do not repeat local direction. A conflicting remote body
becomes current automatically; the original local comment and first conflicting
provider observation remain in the receipt. That intention leaves pending delivery.
Later provider corrections advance normally; delayed acknowledgements cannot erase
the retained values, republish the local body or manufacture another Steer.
Task/Project creation accepts supplied UUIDs; the export cut above implements
delivery and exact readback on the same foreground lifetime.

CLI and Desktop show pending comment IDs and retained losing local bodies. Partial/failed
reads retain the saved thread and expose a refresh error. Desktop Task frames now
carry comments; receiving a frame invalidates an older in-flight thread read.

Focused evidence: the comment operation fixture saves mapped Tasks' comments during
outage, reconciles a lost reply and concurrent delivery, rejects echoes, and retains
both collision values through a late acknowledgement. The real `run_agent` entry
with isolated provider stubs covers incoming and outgoing comments for native Codex,
native Claude and batch Claude while unrelated completion writeback stays pending.
The public CLI/work-watch fixture proves an offline save reaches the open Desktop
stream and survives closing/reopening that watcher without another refresh command.
That earlier offline-only proof did not restore Linear; the later Linux public
reconnect gate above covers restoration. Authored headless app tests cover pending
display, streamed thread updates and rejection of a late older read; current Swift
execution remains unverified.
One initial native fixture launched the real Codex executable because vendor setup
rebuilds PATH; its disposable provider Home and external-network denial contained
it, and it exited for non-terminal stdin. Corrected fixtures isolate parent PATH
and LF_BIN, as TESTING.md requires. The corrected entry checks pass.

Completion fixtures cover atomic rollback, immediate local completion, lost
replies, later local reopening and automatic adoption of observed Linear conflicts.
Inventory acquisition imports membership/completion independently; incoming planning
never advances Workflow. Comment fixtures cover adoption, authorship, rollback,
reopen, subsequent corrections and delayed acknowledgements without new comments
or repeated direction. Public CLI fixtures cover Task/Project fields, status,
Workflow selection, KRs and cross-Wave membership, preserving pending baselines
and retaining losing receipts. These source checks do not prove configured or
installed acceptance or composed native/Flow/Desktop reconnect. Release's
operation-entry lesson remains applicable; its complete child goal/memory were read.

## Prefix contract

Accept unique prefixes of at least four case-insensitive UUID hex digits, bare or
following `lf-`/`task_`. Display at least seven digits and lengthen when needed.
Ambiguity returns matching full IDs; it never chooses a Task. Full IDs and exact
Linear aliases keep working. Internal creation idempotency retains a full identity;
ordinary creation generates that identity itself. New branches retain the full UUID.
All CLI consumers share resolution, including file access, Session filtering and
process inspection. No fixed-length per-command alternatives.

## Remaining implementation and delivery

The queue gate's bounded Linux attempt also stopped at the shared build lease,
before materialization or compilation. Capable gate/CI still owes all-target
Clippy and focused `--lib task_completion` plus
`--lib saved_planning_retains_fields_and_decisions_independently_of_workflow`
checks for `ffe986160`'s receipt changes. The former includes the enabled known
failure; a partial green listing is not a passing suite. Earlier `078a6642e`
evidence does not cover these changed files. Exact runner/logs are above.

1. Headless Swift DTOs and `DesktopHeadlessTests.planningSync`, the full app/model/view
   suite, Xcode fallback app/test-runner build and remaining E2E/full materialized
   matrix still need capable verification. macOS startup did not reach these checks.
   Linux Rust DTO/public-stream proofs do not substitute for Swift consumers.
2. Preserve the enabled unseen-write failure and observed Linear-wins behavior.
   No atomic provider fence or test waiver is claimed. Identify honest shippable
   seams and publish only through the authored review boundary once its verification
   condition is satisfied; no landing or installation.

The `native_runner_reconnects_planning_without_another_turn` fixture passed in the
isolated Linux gate: native Codex, native Claude and batch Claude stubs continue
acquisition/delivery after outage without another turn, retaining identities and
execution boundaries. This is contained runner evidence, not native UI or installed
acceptance. Earlier dated authored-only notes retain their historical meaning.

## Done when

Focused implement checks build the changed code and test the touched operation.
Gate runs `uv run python scripts/test.py --rust --swift --loopflow --e2e` once on
final content, plus the existing disposable Linux migration/adoption proof from
TESTING.md. Headless Desktop checks cover the same views and saved data in both
connection modes. Public subprocess fixtures prove no-Linear lifecycle and
connected offline-save/restart/sync, inbound edits, comments, conflicts, failed
publication readback, rotation and retained execution history. Include Linear
completion, membership and comments during pending outbound delivery and after
reconnect, driven by actual active CLI/Desktop operation entry points. Local
comments/completion propagate without a subsequent turn or manual refresh; delayed
observed conflicts adopt Linear while preserving local execution and losing edits. Provider and GitHub
side effects stay contained; no source binary writes the installed store.

The earlier gate at `dfe18ab60` covers the superseded split. The standalone Linux gate
proves contained native-runner and public Flow/work-watch reconnect at its source
boundary, not current Swift headless consumers or installed behavior. Delivery retains observed Linear precedence,
uncertain effects and the failing unseen-write regression; no atomic cross-system
guarantee is claimed. Decomposed shipping owns delivery; installation remains outside
this work.


Earlier Project ID/baseline and fixture repairs, isolation incidents and pre-gate
preservation paths remain at `e89738258:scratch/explore-loopflow-s-own-store.md`
and in the pre-reconciliation scratch archive above. They establish no installed
outcome; the standalone gate section owns the later source evidence.

Check (queue gate): `cargo fmt --all -- --check`, `git diff --check`, architecture and Swift-boundary scripts PASS; materialized Linux build/Clippy/focused receipt tests NOT RUN (15s build-lock refusal); enabled unseen-reopening failure retained unchanged; Swift/app/full matrix DEFERRED to capable CI, checkpoint jobs skipped; prior standalone commands/results remain in the named archive.
