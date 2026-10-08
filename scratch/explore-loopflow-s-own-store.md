# One plan, optional Linear synchronization

LOO-406 · Infrastructure · revised 2026-10-08.

Jack Heart authorized implementation through publication for review, not landing
or installation. His relaunch comment `ee5f29c0-c176-4f15-815d-cdd07e7ddffd`
selects the coherent Wave schema/definitions and deletion cut, preserving the
ship-decomposed direction. Full synchronization remains unfinished. The older Task
brief's exclusion of export/sync and mandatory one-PR shape are superseded by the
October 8 decisions below. LOO-412 owns transport, not a second planning owner.

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
provider deletion delivery remains unimplemented.

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
It is not an independently shippable connected product yet: export, field/deletion
and cancellation delivery, pending presentation
and composed reconnect remain. No feature flag, adapter or rejected ownership
mode makes that missing behavior acceptable. Publication belongs to the authored
pursuit boundary; this implementation step neither lands nor completes the Task.

Reconciliation on October 8 inspected `dd91ddb83` plus the local compression diff.
The merged LOO-436 startup changes do not implement planning synchronization.
Compression is checkpointed at `05d36d79f`; `84664e661` remains the original
consumable ownership boundary. The retained `pr-review.html` labels its older split implementation as
superseded and is historical review evidence, not a walkthrough of this cut.

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
tests are deleted. Export and outbound field/cancellation/deletion delivery remain.

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
`ProjectCreateInput`. Creation can retain a caller-generated UUID through retries;
exact readback and concurrent-create behavior still need implementation and proof.

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
the same stored Task and UI reflect accepted changes while delayed completion
preserves newer reopening. This is fixture acceptance;
configured Linear and installed acceptance remain separately identified evidence.

## Delete — do not maintain

The ownership cut deletes personal namespaces/types/schema, authority dispatch,
file-backed Wave reads/edits, Git relocation rewriting, provider-first Task deletion
and their exclusive fixtures. Common content, comments, placement, selection,
rotation and refiling writers remain. Historical deletion identities/confirmations,
uncertain transitions, registered Wave IDs, source files and execution survive.
Repository definitions are explicit ingestion sources; builtins remain available.
No intermediate schema or compatibility writer is retained.

The compression pass removes the unused `SqliteStore::import_wave_documents`
entry and `engine::workflow::workflow_source` wrapper. Explicit ensure and migration
retain the import transaction. Config parsing shares `parse_wave_config`; config
and status read one document instead of materializing all Wave memory. Hierarchy
imports reuse one repository Workflow read, and Project catalogs use the definitions
already acquired. Prompt assembly resolves the repository once for ancestor reads.
The existing minimal-schema observation fixture had five values for four columns;
its corrected insert retains the same identity assertions. The Wave projection test
now expects the owned empty plan without provider inventory and retains saved
fields beside malformed provider evidence; metric evaluation still reports that
evidence unavailable. These reductions change
neither synchronization policy nor the known provider race. Planning documentation
removes the obsolete split-writer claim; the conflict cut below implements observed
Linear precedence while retaining the unseen-write limit.

The conflict cut removes `PlanningChanges::retain`, `resolve_task_state`,
`resolve_task_comment`, `--resolve`, `--comment` on Task sync, and the comment
resolution/replacement columns from the existing draft. The common reconciliation
paths replace their exclusive tests. The enabled unseen-write regression remains.
Review exposed exact provider Project IDs losing to duplicate slugs; lookup now
orders exact IDs first, covered by public refiling with the duplicate slug retained.

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

1. **Commit succeeds; reply is lost.** Retry the same creation identity, not a new
   Task. Pending effects remain attached to the existing row across restart.
2. **Linear accepts; acknowledgement is lost.** Retain uncertain publication and
   the operation identity. Confirm by exact readback before another create.
   A description marker and two empty lookups alone do not prove duplicate safety.
   Verify whether the Linear API supports caller-supplied IDs; otherwise retain
   uncertainty instead of claiming success or blindly issuing another create.
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

Current source boundaries (reconciled 2026-10-08):

- `lf/commands/run.rs::prepare_task_input` refreshes comments before launch.
  `ops/task_input.rs::TaskInput::refresh` now starts `PlanningSync` from the common
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
  writable connection for the selected Task's `PlanningSync`. Its five-minute
  planning clock covers Git/filesystem facts; `PlanningSync` separately acquires
  Linear inventory every 15 seconds.
  The source has comment-ingestion APIs and historical webhook schemas, but no
  current Linear webhook receiver/registration call. The CLI parser explicitly
  rejects `repo webhook serve`. This establishes no configured webhook delivery.

At `73059ecd1`, periodic refresh ran only in batch Codex/OpenCode. Current source
starts `PlanningSync` at `run_agent`, covering native Codex, native Claude, Claude's
batch subprocess and the existing batch harness without changing stdin or mode.
Desktop's work watcher starts the same lifetime for its selected Task and stops it
on scope change/connection exit. Inbound comment attempts remain independent of
outbound delivery, every 15 seconds with a five-second request bound; pending
comments and state decisions have separate one-second delivery loops. Repository
configuration enables these effects; retained mappings alone do not reconnect it.
These are foreground-owned lifetimes, not a resident service or a turn/Flow retry.
Comment acquisition follows the selected Task; repository-wide delivery currently
selects mapped Tasks only. Unmapped records still need export and pending-state
projection through the common writer. The existing mapped receipt projection must
not become the definition of whether a connected repository has unsynced work.

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
Task/Project creation accepts caller-supplied UUIDs in the inspected schema;
their delivery and exact-readback contract remains unimplemented.

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
It does not restore Linear or prove pending effects drain after provider recovery.
Headless app tests prove pending display, streamed thread updates and rejection
of a late older read.
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
Linear aliases keep working. Creation idempotency still requires a full identity;
a selector is not an identity allocator. New branches retain the full UUID.
All CLI consumers share resolution, including file access, Session filtering and
process inspection. No fixed-length per-command alternatives.

## Remaining implementation and delivery

1. Implement Task/Project export and field, cancellation and deletion delivery
   with stable mutation identities, exact lost-reply readback and pending/uncertain
   outcomes. Include unmapped connected records. Retain the independent foreground
   acquisition and delivery lifetimes; no resident or turn/Flow retry.
2. Finish common CLI/Desktop pending/conflict presentation and composed active
   native/Flow/Desktop reconnect checks. Prove incoming state, membership and
   comments during unrelated pending delivery, and immediate local saves followed
   by propagation without another turn or refresh. Execution stays local.
3. Gate the complete content, identify honest independently shippable seams and
   update PR #1503 through the authored publication boundary. The current ownership
   source cut is available for review; missing connected delivery is not waived.

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

The earlier gate at `dfe18ab60` covers the superseded split. General synchronization remains substantial implementation work. Delivery must retain the implemented observed-conflict policy and uncertain effects. No atomic cross-system guarantee is claimed. Decomposed shipping now
owns the delivery plan; installation remains outside this work.

Check (2026-10-08 conflict adoption): network-isolated Rust test binaries (completion/comments, abandonment, released-frontier pending decisions, five public local-planning cases) 25 PASS; `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` PASS; separately rerun unseen-write regression FAIL as recorded; gate owns full/materialized Rust, headless Desktop and Linux adoption.

Review repaired Project ID precedence over duplicate slugs and retained delivery
observations in the existing provider snapshot so later saves use the adopted
baseline. State reconciliation parses provider revisions through the existing
ordering function. The comment operation fixture now selects a disposable Home
and one database throughout; its earlier production-store guard rejection wrote
no production data. All source checks run with inherited LF/Linear authority
cleared and external network denied. The full connected product remains unfinished.

Preservation: the pre-realignment code, docs, scratch, separate GOAL edit and
review artifacts, with SHA-256 manifest, are retained at
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo406-realign-ck_545tb/`.
That snapshot retains the earlier ownership/compression preservation references.
The GOAL edit and historical review artifacts remain unchanged.

Before conflict reconciliation, exact scratch and Wave memory bytes were preserved
with a SHA-256 manifest at `/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo406-conflicts-w_8arsgw`.
