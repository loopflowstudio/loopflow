# Resolve Tasks through a synced local store and use the official worker runtime

Status: design review approved by Jack Heart on 2026-09-29. Jack requested
“mark review approved and complete / proceed with the flow”. Approval covers the
current design, including repository-owned, worktree-sensitive Wave definitions,
remote main as the shared baseline, per-Wave Initiatives and portable `A/B` names.
It authorizes completion of this review Session and continuation through the saved
Flow, not an Advance/Iterate verdict, Task completion or installation promotion.
Remaining policy choices stay explicit in questions.md. The first planning slice
is implemented locally; the complete design is not finished. Review source
inspection was against `a3820bf7e`; implementation evidence follows below.
Review feedback: [repository connection and Task validity](repository-planning-connection-review.md).
Open choices: [questions](questions.md).
Research: [Apollo, Relay, Realm and PowerSync](planning-store-sync-research.md).
Command walkthrough: [Dave takes an idea to running work](idea-to-task-command-story.md).
Wave mapping and remaining migration details: [Wave existence and Linear migration](wave-existence-and-linear-migration.md).

## Implementation checkpoint — 2026-09-29

Reconciled against `01f7814ef`, including the planning-evidence implementation
at `3508257b0` and its subsequent compression. The connected
planning slice is implemented; the full approved single-PR outcome remains open.
The last-fetched `origin/main` is still `a3820bf7e`. No new integrated LOO-298
execution contract or owner reply was found; no fetch or coordination retry ran
in this reconciliation. Shared execution migrations remain blocked.

### Implemented behavior

- Exact connected lookup acquires an issue before resolving ownership, stores
  Project-less issues and resolves identifier/UUID aliases without execution
  allocation. PM operations and Task lookup share the same reader and ownership
  resolver. Confirmed writes still force provider acquisition.
- Normalized repository/provider/UUID entities replace `pm_snapshots` payloads.
  Wave lists join Project membership to the shared entities in one read transaction.
  Forward migration preserves prior observations and confirmed-deletion fences.
  Detail reads preserve existing rank within a Project; list omission never erases
  Task facts. Required nullable fields prevent partial responses clearing facts.
- Linear list and detail use one issue decoder. Parsed `updatedAt` revisions order
  issue facts at nanosecond precision. Older/unknown revisions cannot overwrite
  newer known facts; contradictory equal-revision facts fail without replacement.
- Verified issue webhooks persist UUID-scoped revision/removal receipts even
  without a cached issue or execution. Complete detail at or beyond the revision
  repairs invalidation; partial events never replace complete entities. Removal
  receipts fence later ingestion, including events received before caching.
  Existing steering/inbox owners remain unchanged. No execution schema changed.
- Task status returns planning, optional execution, `planning_state`,
  `planning_stale` and `planning_error`. Inspection preserves the prior observation
  age through soft/hard/forced refresh failure and exposes invalid/removed facts.
  Status observes PRs without completing Tasks.
  Rust/Swift planning fixtures and `RegistryQuery.taskStatus` cover the envelope;
  Swift has a typed execution projection, not complete action/runtime parity.

### Planning-evidence follow-through — 2026-09-29

Task status now uses the shared observation reader and returns `planning_state`
(`available`, `invalid`, `removed`, `absent`, `unavailable`) with optional retained
facts. Hard-stale and forced inspection failures retain last-good facts and their
original acquisition date. Uncached resolution failures produce an unavailable
status envelope without execution. A null detail response reports scoped absence;
only confirmed removal receipts establish removed state. Managed readers filter
invalid/removed observations and retain the existing hard/forced refresh refusal.
This does not select cached-Task outage admission.

Realign reproduced a concurrent-observation counterexample: a confirmed removal
arriving during an in-flight detail lookup was overwritten in the returned state
by the lookup's null response (`Absent` instead of `Removed`). Inspection now
preserves the removal receipt's state and dated facts. The regression uses the
existing test-server barrier, not timing sleeps, and also checks that managed
reads refuse the Task and no execution is allocated. This repair adds no storage,
restoration or outage policy.

Project list/detail queries now acquire `updatedAt`. Shared writes order Project
facts separately from issue revisions, so a newer Project observation can win
when its enclosing issue request began earlier. Missing Project content or
relationship fields fail acquisition instead of clearing known data. Unknown or
older revisions cannot overwrite known facts. Equal-revision contradictions fail.

Membership no longer uses list acquisition time as replacement authority. A list
omission without removal evidence is unresolved. Initiative/Team relationship
changes cannot be ordered by Project `updatedAt` alone: the writer retains known
facts and persists unresolved ownership, excluding it from managed readers. List
and detail replays cannot clear that state. Acquiring relationship-specific
revision/removal evidence and its repair remains outstanding; this implementation
intentionally does not guess a winner or silently reconcile ownership.

Review caught two concrete integration hazards and fixed them: Project writes
were conditional on accepting the enclosing issue, and treating every Project
omission as unresolved would also break confirmed chapter archival. Project
writes now have independent revisions. The existing chapter archive operation
records its successful provider acknowledgement in the planning store; current
views exclude the predecessor even after delayed lists. A forward planning-only
migration carries completed chapter archival receipts into that evidence.
Historical facts remain. Provider completion (distinct from archive), external
archive acquisition and restoration semantics remain separate unfinished work.

The self-review also preserved execution inspection when the owning checkout is
missing; planning-context failure cannot hide already-recorded execution.
No execution schema, runtime selection, execution ownership, account or live
provider state was changed by this follow-through. No LOO-298 coordination retry
was needed for this planning-only schema change. Its execution-contract blocker
remains unchanged. Infrastructure memory now reflects these implemented inspection
and Project-revision contracts and the archival/canonicalization lessons; its
earlier hidden-invalid and acquisition-ordered Project guidance is superseded.

Focused local proof receipts:

- `cargo test -p loopflow --lib ops::pm::planning_lookup_tests`: all eleven cases
  passed, including forced failure, retained invalid/removed/absent facts,
  partial Project fields, independent Project revisions and removal during a null
  lookup. Realign reran this group after the new race case first failed with
  `Absent` instead of `Removed`. The ordering case had previously been rerun
  after distinguishing unknown revisions from known older revisions.
- `cargo test -p loopflow --test planning_lookup_tests --test dto_fixtures`:
  public CLI matrix passed; all eleven DTO tests passed. CLI cases include
  identifier/UUID, more-than-seven-day-old planning, invalidation, cached and
  uncached removal, and unavailable planning without execution. No execution,
  execution Project or extra worktree was allocated. Forced inspection is a
  shared-reader proof; Task status has no new refresh flag.
- Swift `ContractTests.taskStatusPlanningFixture`: passed for the six evidence
  states and Project revision. Existing Ghostty missing-symbol and macOS
  CVDisplayLink deprecation warnings remain outside this change.
- `chapter_rotation_previews_retries_and_preserves_dated_history`: passed with
  acknowledged archival excluded from the active view. Its old delayed-list
  assertion now expects unresolved omission while preserving current membership.
- `migration_preserves_planning_identity_and_removes_snapshot_storage`: passed,
  including populated chapter archival receipts and delayed predecessor ingestion.
- `missing_worktree_status_is_actionable_and_read_only` and
  `repeated_status_of_merged_task_never_completes_work`: passed.
- `uv run python scripts/check_migrations.py`, `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, and `git diff --check` passed.
  Scratch links resolve and the full acceptance matrix is unchanged.

The other behavioral receipts above are reused in this reconciliation; DTOs and
public command shapes are unchanged by the race repair. Realign also passed
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, whitespace and
local-link checks, and verified the full acceptance matrix is unchanged.
These are simulated-provider and disposable local CLI proofs. The complete
acceptance matrix remains required and unproven. No affected-suite/repository
gate, live provider demonstration, installation promotion or Flow navigation is
claimed here.

Compression follow-through (2026-09-29): managed planning reads now return the
record directly; inspection owns stale/error reporting. The unused synchronous
managed reader and its duplicate result wrapper are removed. Status resolves
execution once, preserving explicit deleted-Task history lookup. Wave list reads
take Project display names from their existing join rather than searching the
Project vector for each issue. Membership conflict evidence deliberately stays
outside the ingestion transaction so rejection cannot erase that uncertainty.

Review reproduced obsolete fixture writes that erased Projects or replaced known
ownership. The fixtures now use archive/revision evidence or acquire a distinct
ambiguous Project. Removing duplicate initialization setup then exposed a real
scope bug: canonical Wave-path repair left normalized facts under the old alias.
The existing repair now moves all Waves and planning entities under that alias
atomically. It does not merge conflicting observations or move planning between
different repositories. The initialization proof passes using the shared fixture.

Focused verification for this reduction: ten planning-reader/writer cases,
two Wave chapter cases, populated migration and public deleted-history lookup
(14 library tests); public planning-only CLI status; all eleven Rust DTO tests;
repository ownership CLI refusals; repeated merged-PR status without completion;
missing-worktree inspection; and initializing-worktree status/wait/roadmap.
Project rename assertions cover both joined list and detail display names.
The repository-alias proof additionally retains populated planning through
canonicalization. Swift shapes are unchanged, so the recorded Swift proof applies.
No execution migration, provider mutation, outage-policy selection or full gate
follows from this reduction. Cross-repository Wave relocation with populated
normalized planning still needs its own integration proof; the existing relocation
case uses empty planning for the relocated Wave.
Formatting, `cargo clippy --all-targets -- -D warnings` and whitespace checks pass.

### Relationship acquisition contract gap — 2026-09-29

The next implementation pass inspected Linear's public SDK schema at
[`b37823be308a42f837277671f3ded66d33d92e6c`](https://github.com/linear/linear/blob/b37823be308a42f837277671f3ded66d33d92e6c/packages/sdk/src/schema.graphql)
and its [webhook contract](https://linear.app/developers/webhooks). The schema was
downloaded at that exact revision and compared with the inspected master copy;
both have SHA-256 `cc4263f66d6e79f188b1e6b08af5f0fe8dd32dd3c0cdae3c070606e8d1e5e0eb`.
This is public contract research, not authenticated provider behavior or a live
mutation experiment.

Observations:

- `InitiativeToProject` supplies join identity, endpoints, `createdAt`, `updatedAt`
  and nullable `archivedAt`. Project-scoped and workspace-wide join connections
  support pagination and `includeArchived`. The delete mutation returns
  `DeletePayload`; the schema does not establish that a deleted association remains
  queryable as an archived join or supplies a recoverable deletion revision.
- `Project.teams` returns Team entities, without a Project–Team join revision.
  `Project.history` exposes timestamped records with `entries: JSONObject!`, but
  the inspected contract does not define those entries' Team/Initiative deltas,
  retention, completeness or an ordering checkpoint for a current relationship set.
- Project `archivedAt` is affirmative archive evidence. The existing adapter
  queries it only on direct Project ownership, then discards it when converting
  to `PmProject`; issue detail and Project lists do not acquire it. This is a
  concrete acquisition gap independent of ownership repair. A null value alone
  does not establish an ordered restoration under the accepted contract.
- Documented webhooks cover Projects and Initiatives, but do not promise join
  removal events, Project–Team membership revisions or a replayable complete event
  log. Delivery IDs/timestamps and mutation `lastSyncId` do not establish a read
  checkpoint protocol. No documented guarantee was found that Project `updatedAt`
  orders either relationship set.

Consequently the public contract inspected here does **not establish the required
repair authority**. This is not a claim that Linear can never supply it. Adding join
timestamps would handle positive join facts but would not safely replace the whole
Initiative set or repair Team ownership. Simulated responses cannot fill that gap.
Current nested connections also stop at 50 without acquiring `pageInfo`; any
replacement-set proposal must first prove complete acquisition and reject partial
or failed reads.

Decision requested through `lf ask`: retain strict relationship revision/removal
proof and keep disputed ownership unresolved pending a provider contract, or
explicitly revise the requirement to permit a newly acquired complete Project
relationship set to repair ownership. The latter would need local fencing against
already-in-flight observations and honest freshness, but still cannot promise
server-side total ordering or snapshot consistency from the documented API.
This is a product consistency decision, not an implementation assumption.

Dependent relationship implementation is stopped. No production code, schema or
tests changed in this pass, and unchanged behavioral tests were not rerun. Existing
proof receipts remain applicable; legitimate ownership repair, delayed-response
fencing and external archive acquisition remain unproven. Chapter archive receipts
and independent Project/issue ordering are unchanged. No shared execution migration,
provider mutation, publication, promotion or Flow navigation occurred. The complete
single-PR acceptance matrix and all other remaining scope below are retained.

### Remaining implementation

1. Acquire ordered Initiative/Team relationship evidence and an explicit repair
   for unresolved Project ownership. The public-contract gap above requires a
   bound decision before choosing repair authority. The current writer retains uncertainty
   instead of replacing membership by acquisition time. External Project archive
   observations also need acquisition; known chapter archive acknowledgements are
   integrated. Wave `synced_at` still does not date newer joined issue facts.
   Public invalid/unavailable/absent inspection and Project fact revision ordering
   are implemented in the follow-through above.
2. Implement genuine local Task/Project identities and lifecycle through the same
   store. The current reader acquires Linear observations; it does not supply the
   approved local-only lifecycle. Resolve connection migration controls before
   publishing any private planning. Coordinate any execution identity changes
   with LOO-298's owner first.
3. Resolve cached-Task outage admission before enforcing validity at every managed
   launch/resume/worker boundary. Prove ordinary Flow and Session independence
   from invalid/terminal managed Tasks. Do not infer execution permission from
   retained inspection or cache invalidation.
4. Obtain LOO-298's execution contract, then implement bounded cross-store
   discovery, independent official-runtime selection, deliberate pins and delayed
   startup. Preserve existing identities, claims and captured invocations.
5. Implement contextual Wave imports and the remote-main baseline, explicit
   create/link operations, portable Initiative hierarchy and predecessor
   completion after transfer. Resolve remaining baseline/transition choices in
   [questions.md](questions.md) before dependent behavior.
6. Finish design auto-placement, launch-plan artifact handoff, full execution/action
   DTO fixtures and the two-store public-CLI command story. The complete acceptance
   matrix below is unchanged; these internal slices are not separate deliveries.

### Proof and counterexamples

Recorded proofs at `2976e1d34` and `b8e0e1d60` remain baseline evidence. The
follow-through above names the reruns; unrelated proofs were not rerun:

- Eight planning-lookup cases cover detail/list/shared-writer convergence,
  equal-revision conflicts, webhook invalidation, uncached removal, incomplete
  detail, and retained refresh age. List/detail acquisition, creation retry,
  explicit post-merge completion and webhook steering passed after compression.
- All 12 active planning-mutation tests passed (one child entry is exercised by
  its parent). Public CLI identifier/UUID planning-only and stale-status cases,
  repeated merged-PR inspection without completion, missing-worktree/PR-cache
  failure, and the corrected conflicting-Project-ownership matrix passed.
- Rust DTO fixtures (11 tests), Swift `ContractTests.taskStatusPlanningFixture`,
  all 17 receiver tests, four parser/signature tests and the chapter-history
  proof passed. Every new status fixture has `execution: null`; these fixtures
  do not prove execution/actions or CLI success when both planning and execution
  are unavailable. Source still returned an error at that earlier boundary; the
  follow-through's public CLI proof now covers it.
- All 15 active OAuth tests passed together after the logging proof moved into
  an isolated process/subscriber. The ignored child entry is invoked by the
  normal test. This resolves the earlier grouped capture failure; no production
  credential logic changed and no retry masks it.
- Populated migration and five disposable installation-container checks passed.
  These are local/simulated-provider evidence. The full PM/repository gate, live
  Linear and the approved end-to-end command story remain unproven.
- Formatting, Clippy, migration checks and whitespace checks passed at the
  recorded boundaries. Swift emitted Ghostty missing-symbol warnings; the Linux
  installation build reported an unused variable in `controller/wave/metrics.rs`.
  Those warnings are retained limits, not new failures in this reconciliation.

Review corrected invalidation repair based solely on equal acquisition time and
positive fenced responses masquerading as absence. The chapter proof rejected
an older expectation that stale snapshots restore predecessor membership: current
membership stays unchanged and historical Tasks are read through chapter receipts.
A subsequent direct-cache assertion also failed because those entries had never
been acquired as normalized Task entities. It was not evidence of lost history.
The earlier ownership matrix reached Git authentication with an obsolete fixture;
its corrected conflicting Initiative ownership now fails before checkout.

At that earlier boundary, hard-stale/forced inspection refused failed refresh and
both planning readers hid invalidated bytes. The planning-evidence follow-through
above replaces those inspection limitations while preserving managed refusal. No restore/recreation semantics are inferred from the
permanent removal fence. Existing installation/Session continuity mechanisms are
useful discovery inputs, not LOO-298 agreement or independent runtime/store routing.

The current reconciliation also found a concrete counterexample to the documented
unknown-revision contract: a content-edit webhook without `updatedAt` manufactured
an empty revision and reached the timestamp parser before invalidation. The added
case failed with `invalid planning revision: the 'year' component could not be
parsed`. The parser now routes missing/empty revisions through the existing
planning-invalidation event, deferring ordered steering until a complete
observation. No new event type, storage or execution policy was needed.

Verification for that repair passed:
`cargo test -p loopflow --lib provider_revisions_and_webhooks_converge_without_execution`
(one convergence case, including invalidation, detail repair and no execution),
`cargo test -p loopflow --lib webhook::tests` (four tests), and
`cargo test -p loopflow --lib lfd::tests` (17 tests).
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
`git diff --check` passed. Documentation checks confirmed the full acceptance
matrix is unchanged and local scratch links resolve. Broader recorded proofs
were not rerun. The plan, questions, research checkpoint and Infrastructure memory
now agree on completed freshness work, resolved OAuth capture and remaining gaps.
No live provider, execution migration, publication or Flow navigation occurred.

## Command experience: start with an idea

The primary walkthrough follows one fictional technical founder/CTO, Dave, who
can use the CLI directly but usually delegates mechanics to his agent. The
[full script](idea-to-task-command-story.md) selects commands through a single
invoice-export feature:

```sh
lf skill design                 # creates a design worktree automatically
lf skill launch-plan            # carries the design into one Task and starts it
lf task status APP-42           # the returned Task ID gives work a handle
lf task comment APP-42 "Use the current date filter for the export."
lf task run APP-42              # continue when the saved Flow needs resuming
# Only when an account blocks work:
lf auth status
# Once work is running and Dave wants visibility:
lf runs --active --task APP-42
lf usage --task APP-42
# After delivery reveals two continuing responsibilities:
lf wave status billing
lf wave status self-service
```

This is the target command experience, not a claim that automatic design placement
already works. `lf wt create invoice-export` is the explicit alternative first
step; design reuses a suitable existing worktree. The design session must make
its working context and next command usable without assuming a parent-shell
`cd`. Launch-plan verifies that the real design reaches the Task checkout before
launch; it does not restart planning or create competing execution.

Teach accounts and monitoring when work supplies a reason to inspect them.
The existing spellings are `auth`, `runs`, `usage` and `top`; this story does not
select namespace renames. Provider connection and store/runtime internals stay
behind ordinary Task commands. The same commands serve Dave and his agent.
The story ends after delivery: Dave asks for future work on billing accuracy and
customer self-service, then inspects their Wave plans. The script proposes
`lf wave create <name> --objective "..."` for missing owners, followed by ordinary
planning-only Task creation and Wave status. That creation surface is not present
in current source; its spelling/bootstrap semantics remain a product proposal.
Waves inherit the repository connection. Each owns a durable beneficiary/outcome,
a current plan and future Tasks; the original Task stays completed in its history.
Do not equate this scene with approval of a Wave-existence migration.

A companion cancellation case keeps `lf flow code "Prototype a printable invoice
summary"` usable in the checkout without reviving a canceled Task. It no longer
interrupts the successful story's ending.

## Intended experience

Task operations always use a local planning store. When a repository uses Linear,
Linear is the source of truth for what planning exists and sync maintains the
local representation. Without Linear, the local store owns planning. Connection
belongs to the repository; Waves do not select independent planning modes.

An explicit Task selector must resolve to a real planning record before work
starts. A branch, PR or old execution record cannot manufacture a current Task.
A fresh connected store refreshes from Linear before declaring a Task missing.
A network or permission failure means unresolved evidence, not confirmed absence.

Managed Task execution requires a valid Task and a matching plan. If the Task is
missing or execution no longer matches the plan, report it as invalid and stop
managed progression. Jack explicitly cut automatic reconciliation and continuing
mismatched Task execution from this change. Keep ordinary Flow execution available
in any worktree regardless of Task status; that execution does not advance the
invalid Task Flow.

Workers use the machine's official selected lf at each new step, unless explicitly
pinned. Selecting a new runtime must preserve the execution directory and exact
saved invocation. A release installed between steps is picked up at the next step.

## Sharing boundaries

Jack explicitly selected three product layers:

| Layer | What is shared | Owner |
|---|---|---|
| Open-source Loopflow without Linear | Wave goals, memory, Flows and Skills through the repository | Authored repository files; Git integrates changes. |
| Loopflow with Linear | Planning: the Wave's shared Tasks, priorities and progress | Linear; each local store is its synchronized representation. |
| Paid layer | Shared execution and coordination between participants | Outside this open-source Task's scope. |

Shared Flows and Skills are authored definitions. Sharing them does not share
captured invocations, cursors, claims, Runs or Sessions. Ordinary Git updates to
definitions do not rewrite already-captured execution.

Without Linear, each operator can maintain a private local plan and run Tasks.
Cloning or pulling the repo brings goals, memory, Flows and Skills; it does not
import someone else's backlog, Task status, claims, Runs or Sessions. Do not build
a Git-exported planning database or a Task-plan merge policy. Git handles these
authored files through its ordinary review/merge workflow; the local database
cannot silently overwrite those shared files during import.

Connecting Linear introduces shared planning. Any publication of an existing
private plan remains an explicit, previewed migration; authentication alone does
not publish it. A colleague can then discover the same provider planning without
inheriting another person's worker, execution ownership or account state.

Cross-Home discovery in LOO-334 finds one operator's existing work across their
known execution locations. It does not add teammate worker visibility, distributed
Task claiming, fleet scheduling or execution handoff. Linear Task status is shared
planning evidence, not cross-user execution authority. The paid execution layer
is neither implemented nor designed by this change.

## Original failure and surviving constraints

The reported LOO-298 incident had a Linear issue, PR #1296 and a checkout, but the
official CLI said no Task existed. Its record was in another installation's store.
The incident also reported a wrapper retaining older executable and data-directory
settings, divergent account/usage data, and a worker arriving after its caller had
withdrawn the claim at a ten-second deadline.

Baseline inspection at `a3820bf7e` established the following. The first three
defects are repaired by the planning slice; the runtime and startup defects remain:

- `ops/task.rs::task_status` required execution and could complete work from a
  merged PR. It now reads planning independently and observes PRs without completion.
- `ops/task_pm.rs::resolve_task_async` rejected uncached Wave membership before
  refresh. It now acquires the exact issue into the shared reader first.
- `pm/linear.rs::OwnedIssueNode::into_ownership` rejected Project-less issues.
  It now preserves existence and represents the missing relationship explicitly.
- Repository `engine/config.rs::PmConfig` already has provider and Linear Team
  fields. `ops/pm.rs::read_repository_team` reads them. Wave Initiative mapping
  can remain distinct from repository connection ownership.
- `engine/process.rs::resolve_current_home_lf_binary` can retain the current
  executable's installation rather than choose the machine's current selection.
  `ops/task_destination.rs` couples runtime and database selection and refuses
  differing local/installed Task IDs.
- `ops/task.rs::launch_task_process` releases its claim after startup-wait failure;
  `ops/child.rs::CHILD_STARTUP_GRACE` is ten seconds. A late child can become stale
  without another worker competing with it.

These repairs separate planning lookup from execution allocation. They do not
establish all managed-admission boundaries or justify arbitrary explicit Tasks.

## Accepted decisions

Jack subsequently requested a narrative in command selection: start with
`lf wt create` or `lf skill design` automatically creating a worktree, continue
through `lf skill launch-plan`, then introduce Task commands and later accounts
and monitoring. Jack suggested ending with Waves that organize future work around
two aspects of the initial feature. One composite persona/story replaces parallel
persona tours.
The linked walkthrough records the placement gaps and makes no shipment claim.


Jack placed the connection at repository level, then clarified: “if we are using
Linear as a source of truth it should be the source of truth for what planning
there is”. Jack subsequently selected one local store interface, synchronized to
Linear when available, and refusal of explicit Tasks absent from the applicable
planning store. The Apollo analogy describes the desired local-store simplicity;
it does not select a library, an offline mutation queue or a new sync service.

Jack also selected invalidation of execution that does not match the plan, and
continued ability to run an ordinary Flow in a worktree regardless of Task status.
These decisions remove the kickoff's proposal to keep progressing historical
Task execution through planning disagreement. Official worker runtime by default
and deliberate visible pinning remain the original Task direction.

## One planning store interface

Reshape existing planning storage and operations in place. Task commands, generic
Session Task selection, Wave views and the app use one local planning reader.
Provider synchronization supplies that reader; do not retain a competing direct
provider command path that bypasses the local model.

Use stable provider identity for synced records (repository/provider scope and
Linear issue UUID), with issue identifiers as aliases. Local Tasks have genuine
local identities, not fabricated Linear UUIDs. Preserve existing Task IDs and Run
foreign keys when attaching planning to execution. Planning can exist without a
checkout, invocation, claim or Session. Syncing an issue allocates planning only.

Use the existing PM cache and local planning machinery as the implementation
starting point, consolidating duplicate readers/writers. TaskSpace and TaskOps
remain conceptual planning/execution boundaries, not additional services or CLI
namespaces. LOO-298 owns the replacement execution schema; integrate with its
surviving Task owner rather than introducing a temporary second schema.

Normalize planning into entity records within the
existing SQLite store. Issue lookup, Wave listing, confirmed mutations and webhook
ingestion update the same Task by stable identity. Lists hold membership and order;
they do not own additional title/status copies. The draft migration now removes
serialized per-Wave snapshots and their payload consumers. `PmSnapshotRow`
remains a typed view assembled from normalized records, not another persisted
planning copy. This follows Apollo's shared-entity pattern;
it does not select Apollo as a dependency or require a general GraphQL cache.

Separate acquisition policy, stored freshness and managed Task validity. Missing
cached data requests a fetch; stale data requests refresh; a confirmed missing or
ineligible Task refuses managed work. Reuse bounded PM refresh mechanisms rather
than duplicating policies in CLI, workers and Swift. Offline managed execution
remains an open product choice.

### Research translated into implementation

The [source comparison](planning-store-sync-research.md) supplies the rationale
and evidence limits. These are the adopted target contracts; the checkpoint
distinguishes completed integration from remaining work:

| Research lesson | Loopflow contract | Verification |
|---|---|---|
| Apollo shares entities across queries | One planning record per stable Task identity; lists reference it. Detail, bulk sync, mutation and webhook use the same writer. | Detail/list order and mutation results agree without duplicate records. |
| Apollo separates storage from acquisition | Existing SQLite remains the local reader; repository connection selects the sync source. Reuse bounded refresh policy centrally. | The same reader works with and without Linear; empty connected stores fetch before absence. |
| Relay separates presence from freshness | Missing cached data, stale observations and invalid Task planning remain distinct. Cache eviction is never provider deletion. | Failed refresh retains dated data; partial lists never establish removal. |
| Apollo watches committed cache changes | App and CLI share the Rust store. Existing view refresh/subscription paths consume committed changes. | Task detail and Wave views agree after sync; Swift has no independent planning writer. |
| Apollo distinguishes optimistic from confirmed state | Connected mutations govern execution only after provider confirmation and local ingestion. | Rejected/pending Task creation cannot launch managed work. |
| Realm and PowerSync require additional offline write semantics | No durable offline mutation queue, automatic conflict merging or new sync service in this scope. | Failed writes remain failed/pending attempts, never successful planning or launch authority. |

Keep entity facts and query coverage separate: a fetched page establishes its
returned items, not the absence of all others. Apply returned fields without
clearing values omitted by partial responses. Prefer explicit domain updates or
complete entity refreshes to a generic GraphQL field-merging framework. Reuse
provider revision/order evidence to avoid old observations overwriting newer
facts; where ordering is unknown, refetch instead of declaring a winner.

Use existing webhook ingestion as an update/invalidation input and bounded fetch
as repair. Neither webhook arrival nor paginated API traversal is a transactional
server checkpoint. Do not reproduce PowerSync's checkpoint protocol without a
provider contract supporting it. Ordinary reads may reuse fresh local data;
explicit refresh and misses acquire provider facts before the shared reader.
Managed admission remains distinct from reading cached data.

### Repository connection and sync

Reuse repository `.lf/config.yaml` PM configuration for provider and Team scope.
Move connection controls/documentation to the repository owner. Wave Initiative
bindings map Waves into the provider hierarchy; a missing mapping cannot make
one Wave fall back to local authority inside a connected repository.

Connected planning represents Linear's Tasks, membership, title, status and
ordering. Local execution facts do not compete with those fields. Sync refreshes
planning through narrow writes that cannot accidentally overwrite claims, Runs
or invocation state. Do not describe this as replacing a separately authored
connected plan: Linear defines that plan.

On an explicit lookup, reuse a sufficiently fresh local record or refresh the
requested issue according to the shared acquisition policy. A connected cache
miss must attempt the existing direct issue query inside sync before reporting
absence. Do not require a preexisting
PM snapshot, local execution row or chapter receipt. Commit the observation to
the local planning store, then use the shared reader. Use existing bulk sync,
webhook and freshness paths for lists; avoid one network request per rendered row.
Keep the last successful observation's timestamp on failed refresh.

An issue with missing Project or unresolved Wave membership still has a planning
record and can be inspected. Managed Task launch needs enough valid planning to
supply its work and ownership; missing relationships report that limitation.
Only repository-bound Initiative associations participate in Wave mapping.

A provider outage does not disconnect the repository. Malformed or missing
connection configuration in a checkout previously known as connected is a
configuration discrepancy, not permission to author independent local planning.
Repository definitions establish the Wave set; connection bootstrap details remain open.

### Wave mapping selected; existence and migration details under review

Jack's mapping concern includes hundreds of provider objects. The
[scale findings](wave-existence-and-linear-migration.md#initiative-size-limits--verified-boundary)
find no published Project/Initiative count cap; actual capacity remains unverified.
Jack clarified that outgoing chapter Projects must be completed so they leave
the active plan. Chapter rollover transfers started unfinished Tasks with their
identity intact, settles backlog under the existing cancellation policy, then
completes the predecessor Project and retains its history. Completion closes the
chapter; it does not claim that every KR succeeded or complete transferred Tasks.
Keep the existing archival behavior after closure. Current source archives
predecessors in `ops/chapter.rs` but does not explicitly set their provider status
to completed there; integrate this requirement in that existing operation.

For 100 Waves over 12 chapters, the settled result is 100 current Projects and
1,100 historical Projects, not 1,200 active Projects. Current-plan reads must
exclude completed/archived predecessors and load history only when requested.
Retain complete pagination and incremental refresh. No documented evidence says
completion removes an object from any provider storage quota; the accepted
benefit is a bounded active plan under either Initiative mapping.

Jack selected one Initiative per Wave, including subwaves, after considering
one for the repository. Each Initiative groups that Wave's current and completed
chapter Projects. The [comparison](wave-existence-and-linear-migration.md#alternative-one-initiative-for-the-repository)
retains the alternative as decision context, not an implementation option.

Use path names consistently: `A`, `A/B`, `A/B/C`. Each path names a distinct Wave
and Initiative. When the Linear account supports sub-initiatives, also set the
corresponding native parent relationship. Without that feature, the same names
and Loopflow hierarchy work with flat Linear Initiatives. Native hierarchy is
optional enrichment, not a prerequisite or a different Wave tree. Interpret
Jack's “parent status” as the parent relationship, not lifecycle status. Keep
stable provider UUIDs across renames; naming does not replace identity.

Prove slash-name creation, lookup, hierarchy and retry on accounts with and
without the feature. Provider depth limits must not restrict Loopflow's path
hierarchy. Missing capability is distinct from permission, network or mutation
failure; report those failures instead of claiming a native link succeeded.
Keep direct Project membership separate from descendant aggregation. Existing
company names or multiple native parents need adoption rules; this decision
does not authorize rewriting them. Discovery and migration details remain open.

Jack then steered the design toward creating Waves in Loopflow and publishing
or linking them to Linear, rather than deriving all Waves from workspace
Initiatives. This supersedes the earlier automatic Initiative-is-Wave proposal.
The [connection design](wave-existence-and-linear-migration.md) follows that direction:

- Repository-authored definitions establish Waves in both local and connected
  mode. Git shares goals, memory, Flows, Skills and stable Initiative bindings.
  A fresh clone sees the same Wave set and reuses those bindings.
- Linear owns shared chapter/Task planning for connected Waves. The local store
  syncs their mapped provider records. Reading unrelated Initiatives may suggest
  link/import candidates; it must not add Waves automatically.
- Creating a Wave while connected creates its corresponding Initiative through
  the explicit create operation, or links an explicitly selected existing one.
  Reads, login and Git pulls do not implicitly publish or duplicate Initiatives.
  Preserve an unsuccessful publication as incomplete setup, not a successful
  shared plan or a fallback independent local plan.
- Connecting existing local Waves previews link/create and Project/Task mappings.
  Reuse provider UUIDs on retry and across clones; names are not identity.
  Importing an existing company Initiative is an explicit operation, not automatic
  workspace adoption. Do not rewrite its existing planning to force a chapter.
- Linear remains authoritative for connected Tasks, their status and membership;
  repository authority over Wave definitions does not make a Git Task-plan replica.
  Unavailable/missing mapped Initiatives are connection discrepancies, not grounds
  to delete Wave definitions or recreate provider objects during reads.
- Migration transfers planning references, never execution authority. Invalid Task
  execution stays invalid; ordinary worktree Flows remain available.

Exact connection/import controls, existing Project selection, disconnect and
Wave deletion semantics remain open. The mapping direction does not authorize
provider writes in this review or implicit publication of existing private plans.

### Wave definitions follow worktree context

Jack clarified that the local store owns Loopflow's Wave view, importing definitions
from the repository. With an owning worktree, use its complete current files,
including uncommitted additions, edits and deletions. Without an owning worktree,
use the configured remote-main definition (normally the last fetched `origin/main`).
Jack proposed this baseline after ruling out dirty main as definition input. The
[context contract and proof](worktree-wave-definitions.md)
specify the conceptual resolver `waves(repo, worktree=None)`.

Do not merge main's Wave set into a branch view: a deleted Wave must stay absent
there. Keep main and other worktree answers independent even when one store serves
them. Refresh imports before returning API results; no manual sync or commit is
needed. Preserve stable Wave identities, provider mappings and shared planning;
contextual definitions do not create a separate Task-plan writer per branch.
All Wave lists, detail and selection use the same resolved context. Invalid or
unreadable files are not an empty successful import or a fallback to main.

A branch deletion changes that view immediately but does not delete provider
planning, historical execution or another worktree's Wave. Linear sync cannot
restore a Wave absent from the selected repository view. Ordinary worktree Flows
remain available. The proposed shared baseline uses the remote-tracking commit,
excluding unpublished local main commits and dirty main files. Ordinary reads
use the last fetched ref rather than claiming live remote freshness. Main should
stay clean; reads do not perform cleanup. Local-only repositories with no remote
baseline, explicit main-checkout context and refresh cadence need final policy.
Outward synchronization of edited mapped definitions remains open; API reads do
not imply provider writes.

### Planning writes

Local repositories create/edit/complete/delete Tasks in the same planning store
without calling Linear. Extend Linear-only Task and Project reference types so
local records use real local identities. Keep the one-current-Project invariant
and reuse existing Wave setup/chapter operations; no fake provider snapshots.

Connected writes go to Linear and update the local representation from confirmed
results. An attempted write is not current planning truth. Retain existing
idempotent creation markers; after an ambiguous response, inspect remote state
before retrying. Do not add a general offline mutation queue in this change.

Task creation must not require rotating a chapter just to populate a local
receipt. Resolve an existing, uniquely identified provider Project; if genuinely
ambiguous, show candidates and allow explicit selection for that operation.
Do not infer a chapter from an In Progress label or rewrite chapter history.

## Explicit Tasks and ordinary Flows

| Situation | Behavior |
|---|---|
| Connected issue exists, local planning is empty | Sync it into planning, then resolve normally. Do not allocate execution during status. |
| Explicit Task absent after conclusive resolution | Refuse managed Task work with a clear missing-Task error. Do not invent a Task from a branch or PR. |
| Linear unavailable, uncached selector | Report unable to resolve; do not launch or claim confirmed absence. |
| Task or execution attachment no longer matches the plan | Report invalid and stop managed progression. No automatic reparenting, reconciliation or historical-plan continuation. |
| Canceled/completed Task with a checkout | Report planning status; do not automatically reopen it or launch managed progression. |
| Ordinary Flow requested in that worktree | Run it without requiring a valid Task or settling the Task's managed invocation. |
| GitHub unavailable | Keep planning visible, with unavailable/dated PR evidence. |

Validate at managed launch/resume and subsequent worker boundaries so continued
Task work cannot bypass current planning validity. Use existing authority and
claim settlement; do not create another watchdog or cancellation ledger. This
Task does not implement proactive termination of an already-running provider
when remote planning changes. Invalidation must not advance the managed cursor.

Ordinary Flow selection must bypass implicit Task launch routing when a checkout
has an invalid or terminal Task. Retained attribution may remain historical where
supported; it grants no Task advancement authority. Cover both CLI and generic
Session entry points, rather than merely adding a new escape flag.

No cleanup or deletion of authored files/history is requested. Leaving bytes
untouched does not require recovery UI or make an invalid execution eligible.
The exact treatment of cached planning during an outage remains a review question.

## Cross-Home discovery

Planning and execution have different placement needs. Connected stores can each
sync the same Linear planning without creating duplicate workers. Local-mode
planning stays with its owning store. Locate existing execution before allocating
new execution, and report its directory and how to reach it when necessary.

Build transient candidates from the selected directory, existing installation
selections/retained receipts, known Home routes and Loopflow's known development
layout, including the legacy standard store. Reuse `known_installations` and the
minimal read-only Work identity reader. No machine-wide Task registry, recursive
search of arbitrary directories, copied credentials or foreign-store migration.
Canonicalize aliases and deduplicate by file identity: copied stores can share a
Home ID. Report uninspected locations and the bounded search scope.

Inspect minimal identity/location facts without requiring complete historical
parents. Unsupported schemas are uninspected, not empty. Detailed reads use a
compatible executable at the owning location. Reach commands address executable,
`LF_HOME` and `LF_DB_PATH` explicitly; remote paths use existing transport.
Recommend a retained historical pair only when its artifact/store is verified.

If multiple divergent records exist, show locations and require explicit selection
for ambiguous writes. Do not merge them or pick the newest. Location selection
cannot make planning-invalid execution valid. A local Task found elsewhere can
be reached in its owning store without importing it.

Git/GitHub supply branch and PR evidence, never substitute planning records.
Associate exact recorded repository/branch or explicit PR Task links; issue-like
branch names are candidates only. Preserve unpushed refs and distinguish merged
history from an active successor. `task status` may sync planning and observe PRs,
but must not complete a Task as a side effect.

## Official worker runtime

At every worker boundary, resolve the machine's selected installation, independently
of execution-store placement. Official does not mean an HTTP check for the newest
release. Inherited `LF_BIN`, PATH and the parent's retained installation do not
constitute deliberate pins.

Capture one verified artifact/digest for each child and record it in existing Run
evidence. Keep that process's executable stable; choose again at the next boundary.
Same-Run tool wrappers use that Run's executable/store pair. Preserve exact
invocation, claim and pending review while changing runtime bytes.

Proposed controls in existing Task run options:

```sh
lf task run DEM-334
lf task run DEM-334 --lf-bin /absolute/lf
lf task run DEM-334 --official-lf
lf task status DEM-334 --json
```

Pin/clear flags are mutually exclusive and persist on the existing execution
owner without replacing its Flow. A pin contains canonical artifact path/digest;
changed or missing bytes fail explicitly. Status distinguishes policy, pin,
last attempted runtime and next resolved runtime. Default migrated policy is
official; inherited environment never becomes a recorded pin.

Installed startup must honor an explicitly addressed compatible execution store
independently of its default store. Share path canonicalization with launch.
Retain branch-source isolation: arbitrary source binaries cannot migrate the
installed host store. Unknown private drafts remain a compatibility failure,
with pending work preserved; no downgrade or silent database switch.

Without a selected installation, source execution may use its private store and
invoking runtime, visibly reported. When an installation exists, stale wrappers
cannot force that fallback. Account and quota consolidation remain out of scope.

## Slow startup

After the bounded startup observation deadline, report starting and retain the
exact claim. Release on proven spawn failure or process death using existing
process fencing, not elapsed time. Concurrent retry observes the same child; a
late worker can publish under the retained claim. Proven-death replacement must
still reject stale late children. Coordinate this boundary with LOO-298's Run/Exec
ownership rather than building a parallel liveness mechanism.

## Integration and deletion path

The command walkthrough additionally requires verifying design auto-placement,
reuse of an explicit worktree, session working-context continuity and launch-plan's
artifact handoff. Current `wt create` requires a name, and current launch-plan
cannot adopt arbitrary unbound implementation checkouts. Implement or explicitly
resolve these gaps before demonstrating the opening sequence; do not manufacture
a second design or Task to make the story appear continuous.

1. Finish convergence on the normalized reader already implemented for detail,
   Wave lists and PM operations. Payload storage and its consumers are removed;
   incomplete ownership is representable and lookup refreshes before resolving
   membership. Issue webhooks now feed shared invalidation and provider revisions
   order Task facts. Project fact ordering and public invalid-state evidence now
   follow the same owner. Finish relationship-specific revision acquisition and
   repair of unresolved membership; acquisition timestamps cannot settle it. Preserve
   migration history and the existing deletion receipts. No second planning copy.
2. Adapt Task/Project identities and planning writes for local operation. Preserve
   IDs and execution through forward migration; coordinate with LOO-298 first.
3. Update Task operations, direct `--as task:` resolution, Session selection,
   Wave views and Swift DTO/actions together. Represent optional execution and
   planning freshness explicitly, without fabricated idle/worktree defaults.
4. Enforce managed Task validity while keeping ordinary worktree Flows independent.
   Delete the kickoff's mismatch-continuation and automatic reparenting proposals.
5. Reuse location evidence for discovery and runtime/store routing. Replace the
   local/installed Task-ID equality refusal with actual resolution and selection.
6. Persist runtime policy on the surviving execution owner and repair startup
   admission. Update CLI, planning/Homes docs and TESTING.md alongside consumers.

Before shared execution migrations, obtain LOO-298's current contract through the
authorized ordinary Work Run. The kickoff attempt through official
`lf --as task:LOO-298 -b : ...` failed as unregistered before launch. No owner reply
exists. Supplied Wave memory is prior design evidence, not integration agreement.
Do not edit its branch, repair auth or copy stores. Planning work can proceed
without shared execution migrations; use `lf rebase` when integrated work exists.

## Proof and finish line

Extend `scripts/test_task_installation.py` and existing real-CLI fixture support.
Use a disposable OS account/container with two stores, a Git repository/local bare
remote, simulated Linear/GitHub and distinguishable installed artifacts. Mount no
host installation or credentials. Provider simulation is not configured live proof.

1. A owns execution; B starts without planning snapshots or a chapter. Public
   status by identifier/UUID in both syncs the same provider Task, finds its exact
   branch/PR and locates A. The reach command works. B gains planning, not a second
   worker/invocation. Source-private lookup does not modify foreign DB/WAL bytes.
2. Planning-only and Project-less issues are stored and inspectable. Unresolved
   relationships prevent only operations requiring them. Permission denial,
   timeout, partial GraphQL errors, archive omission and confirmed removal remain
   distinguishable; pagination omission is never conclusive deletion.
3. Two Waves share repository authority. Missing one Initiative mapping never
   enables local fallback. In an unconnected repository, exercise real local plan
   setup and Task create/edit/run/complete without any Linear requests. Discover
   its owning store from B; same-name local Tasks do not collapse.
4. Fresh Linear changes control stored title/status/order/membership. Confirmed
   absent Tasks and mismatched execution refuse Task run/resume/next-step launch
   without creating a claim or advancing the cursor. A nonexistent explicit
   selector creates nothing. Ambiguous provider write retries neither duplicate
   Tasks nor overwrite newer planning.
5. In the very same checkout, run an ordinary Flow with missing, canceled or
   mismatched Task planning. Prove execution reaches its intended work without
   resuming or settling the invalid Task invocation. Merely accepting CLI syntax
   is insufficient. Cover the generic Session selection path too.
6. Run two actual worker boundaries. Select R2 through normal disposable install
   between them; the next worker consumes its claim with R2's digest and unchanged
   invocation/execution directory/review. Poison PATH and inherited LF_BIN and
   LF_CONTROL_BIN with R1 so the original defect would fail the proof.
7. Pin R1, change official selection and prove execution/status retain the pin.
   Clear it and prove R2 runs next. Modified/missing pin and incompatible store
   preserve the pending boundary and report failure.
8. Delay an actual child beyond ten seconds. The caller reports starting and the
   late child consumes the same claim. Concurrent retry launches no duplicate.
   Separate proven-death/replacement evidence rejects a stale child.
9. Divergent execution copies, unreadable stores and filesystem aliases produce
   honest locations without migration or timestamp takeover. Equal Home IDs do
   not collapse distinct stores. Invalid execution remains invalid after routing.
10. Rust/Swift DTO fixtures and Task actions agree on planning-only, unavailable,
    invalid, terminal-with-checkout and pinned states. Migration tests preserve
    populated historical IDs and execution records. Status never completes work.
11. List-then-detail and detail-then-list share one planning identity. Confirmed
    mutations update both views. Partial responses/lists cannot clear unrelated
    fields or imply deletion, and older observations cannot resurrect confirmed
    removed planning. Failed refresh preserves last-good data and observation age.

12. Replay the linked command story from design to launch-plan to one Task, then
    status/steering/continuation and ordinary Flow after Task invalidation. Prove
    auto-created and explicitly created worktrees converge on the same design
    handoff. Accounts/monitoring enter only at their story moments. The final Wave
    scene plans two future outcomes without launching or duplicating the completed
    Task. Proposed Wave creation/bootstrap must be resolved before claiming that
    scene works; its inclusion does not establish current implementation scope.
    Source checks alone are not an end-to-end demonstration.

13. Two independent local-only users of one repository receive the same committed
    Wave goals, memory, Flows and Skills and retain separate private plans and execution. Pulling
    goal, memory, Flow and Skill definitions updates imports no Tasks, worker claims or history. With Linear,
    both can read the shared plan, but no execution control transfers with it.

14. Prove [worktree-specific Wave imports](worktree-wave-definitions.md): main and
    two worktrees share one local store yet return their own definitions. Dirty
    add/delete/edit and revert affect the owning view immediately; read order,
    branch switches and Linear refresh cannot overwrite another view or resurrect
    locally removed Waves. Context-free reads use the remote-main baseline.
    No provider mutation or execution allocation follows from definition import.

Primary end-to-end command after extension:
`uv run python scripts/test_task_installation.py`. Use one focused behavioral proof
per changed boundary; affected suites once at gate. Rust changes require
`cargo fmt` and `cargo clippy --all-targets -- -D warnings`. The checkpoint above
records the partial implementation's proofs; this full matrix remains unproven.

## Scope and implementation handoff

In scope: repository connection, one synced planning store, explicit Task validity,
local lifecycle, worktree-sensitive Wave imports with a remote-main baseline,
per-Wave Initiative mapping and chapter closure, cross-Home location, independent
ordinary Flows, official runtime and pins, slow startup, consumers and public-CLI proof. Internal slices do not
individually meet the full Task's finish line.

Out of scope: shared execution between participants (the paid layer); Git export,
import or merging of private Task plans; reconciling execution that disagrees with
planning; continuing an
invalid Task Flow; merging stores/accounts/quotas; a new sync service or offline
mutation queue; remote fleet discovery; replacing LOO-298's execution schema;
Wave deletion policy; automatic chapter rotation; host promotion.

Current first slice now refreshes an explicit connected issue into local planning
and reads it through the shared store without allocating execution. Focused proofs
cover a missing Project, missing selector and unavailable provider. Managed
validity and ordinary Flow independence still require the remaining integration. Jack approved proceeding with this
architecture. The cached-Task outage policy and connection-transition semantics
remain explicit in questions.md; they must not be silently inferred from library
behavior. They do not prevent the first slice. No Flow navigation is selected here.

## Retained observations and evidence limits

- At kickoff, official `/Users/jack/.local/bin/lf task status LOO-298 --json`
  returned `Error: no Task exists for "LOO-298"`. Ambient help exposed an older
  command surface; the official entrypoint exposed current Task/Session commands.
- The first coordination request used an invalid untyped selector. The corrected
  `--as task:LOO-298` request failed before launch; no reply established the current
  execution-schema contract. No LOO-298 branch was edited.
- Earlier source inspection and Linear documentation supported direct issue
  lookup, partial GraphQL errors and archive exclusion from default lists:
  [GraphQL API](https://linear.app/developers/graphql) and
  [rate limiting](https://linear.app/developers/rate-limiting). These are retained
  kickoff findings, not fresh provider verification or evidence of live payloads.
- Kickoff review caught Home-ID aliasing, Project-required existence, runtime/store
  coupling and Linear-only Project identity under local creation. Those constraints
  remain; the direct-read architecture and mismatch-continuation proposal do not.
- The interactive design review performed no implementation, live provider
  request, migration, publication, promotion or acceptance demonstration.
  Subsequent implementation and local proof are recorded in the checkpoint above.
