# Focus on your own work

Implementation plan drafted through kickoff, 2026-09-30. Jack Heart supplied the
product decisions; mechanisms and sequencing below are kickoff recommendations.
Status: the design is accepted for implementation. Build Unit 1 first, then
Unit 2. Change the plan only when implementation finds a counterexample.
Placement: Product, matching its shared workspace/user-contract responsibility.
[Evidence](kickoff-evidence.md) records the source findings behind the plan.

## Problem and accepted direction

Jack wants to work on the software without monitoring agents or shepherding work
between stages. The primary workflow describes participation: interactive skills
are nodes; background Flows are the edges. Prepared work can wait for Jack.

The Task workspace brings together all Sessions in its worktree, additional
terminals, and files, with expand/collapse and strong focus on one Session.
Interactive Flow Sessions remain direct. Asks are also direct Task conversations
and must fit that same experience. Independently started Sessions group under
the Task by worktree even without Flow membership.

Desktop discovery initiates one ongoing Session for each repository and each
Wave. Ctrl-C in those primary Sessions starts a fresh one for that scope. The
orphan/all-Sessions browser becomes diagnostic. Primary conversations do not
replace interactive Flow Sessions or become a mandatory relay for Task Asks.
Flow editing stays in existing files; improve default selection and source access.

Jack's next refinement gives each launched Task one ongoing TaskSession: its agent
runs the Task's Flow and works with Jack to change it. Interactive substeps retain
their own windows/Sessions. Jack clarified two switch timings: “finish, then switch”
and “switch now”. “Finish” means reach the next loop point of the innermost
active loop and exit instead of running its decider. Switching replaces the Flow
invocation while the TaskSession,
Task identity, checkout, and accumulated work continue. It does not splice new
steps into an already captured invocation.

Wave Sessions combine design and operation: autonomously solve problems to get
accepted work shipped or to its next interactive point, while remaining ready to
capture an emerging design and turn it into Tasks. The repo Session is broader:
Loopflow onboarding, general help, and an agent of last resort when specific work
or workflows are stuck. It must be useful before any Wave or Task exists.

[Jack's verbatim direction](intent.md), [kickoff findings and probe](kickoff-evidence.md),
and [current Ask evidence](ask-evidence.md) preserve the reasoning and corrections.

## The demo

While Jack works in one Task, another reaches an interactive stage. Its existing
Task row and collapsed Wave row indicate that a conversation is available; nothing
steals focus. Jack selects that indication and reaches the exact Task Session
without inspecting agents or opening the diagnostic browser.

That Task has a design Session, a newly raised Ask, an independent conversation
launched from its worktree, and a shell running a local server.
All are available beside its files. Jack expands the Ask alongside the design,
focuses the Ask, answers it, and returns to the prior arrangement. Only the Ask's
caller resumes. A new Session never steals keyboard focus. Switching Tasks and
returning preserves the terminal processes, draft input, file edit, and layout.

The compact workflow shows interactive stages. Selecting Demo focuses its direct
Session. Expanding the connecting edge reveals the actual automated steps and
evidence. Normal work needs no visit to the diagnostic Sessions browser.

After the primary-conversation unit lands, discovering a repository with two
Waves starts three primary Sessions. Repeated discovery and app reopening find
the same current conversations. Ctrl-C replaces one primary Session; the Task's
Flow and Ask Sessions remain untouched.

## Delivery shape

Three additive units, each delivering a usable experience without compatibility
adapters. Unit 1 is the fully specified keystone and ships as one PR; its internal
cuts are not separate partial products. Unit 2 introduces primary Task, Wave,
and repo Sessions, including TaskSession-led Flow switching. Unit 3 makes default
selection and direct Flow-file editing effective. Keep the
later units here as bounded implementation direction; elaborate their final
launch plans when selected. Keep all three units visible during design review.
This kickoff files no follow-up Tasks and launches no implementation workers.

Unit 1 remains the selected delivery boundary. Its local implementation now includes
Home/worktree identity in checkout receipts, planning readings and outer slots;
one retained Task workspace with collapse, focus/restore, shells and files;
interaction projection and exact direct-Session navigation; Task/Wave participation
indications; and caller-scoped raw Ask keys. The old Task terminal owner is removed.
Focused Rust/Swift proofs and the fallback Xcode build are recorded in
`kickoff-evidence.md`. These remain internal changes in one PR.

Remaining acceptance belongs to the configured demo: cross-Task discovery without
focus theft, real provider Ask/Flow continuation, retained input/processes/documents,
file split restoration, and at least 20 measured layout actions plus idle CPU/process
counts. Remote Flow readings still carry recorded Home/Task identity with an explicit
unavailable checkout-resolution reason. Owning-Home remote association must be
verified before claiming association complete across Homes. No shipping claim or
performance result follows from local model/transport proof.
Unit 2 is not launch-ready:
native primary-turn delivery remains unproved, and Flow replacement needs the
durable transition specified below. Upstream `3dc89bc9a` has already removed the
Wave resident, listener, external chat bridge, and their scheduling/turn claims.
Unit 2 must establish primary-owned observation delivery from the retained outbox;
there is no live resident consumer to migrate. Restoring that service is outside
this design. [Evidence](kickoff-evidence.md) separates
source findings and executed probes from required implementation proof.

## Unit 1 — the Task workspace

### Delete — do not maintain

Removed: `TaskWorkspaceView.swift`, `TaskTerminalStore`, `TaskWorkspaceSection`,
and the exclusive `TerminalIdentity.taskTerminal` case. Their consumers now use
the retained Sessions workspace; keep these predecessors deleted. Preserve file documents, shell
processes, and direct Session completion in the surviving window-owned registry.
Replace path-only registry/outer-slot keys with Home and checkout identity; keep
terminal identity independent of grouping. Focused retention tests own this proof.
The compression pass also removes full-inventory reconciliation from the pane
store and the view-owned file-focus backup. Explicit Session removals preserve
other repositories' panes and invalidate resolved Sessions in Undo; file
visibility derives from the retained preference and current zoom.

Jack Heart's supervising direction places configured desktop/provider proof at
the final demo boundary. Local implementation and focused proof continue before
that demonstration; shipping still requires it.

### Interaction

Use the existing Work navigation to select a Task. Replace its details-versus-
terminals mode switch with one workspace:

- A compact Task header and participatory Flow strip.
- A left strip listing the Task's Sessions and shells. Each row has its title,
  meaningful state, and expand/collapse action. Files have their own toggle.
- A central area of retained terminal panes. One is prominent initially; Jack
  can expand others alongside it. Files open beside the terminal area.
- Focus temporarily gives one Session the content area while retaining a compact
  Task breadcrumb and Restore action. Restore reinstates panes, ratios, files,
  and selection exactly. It does not reconstruct processes.

These are reversible layout choices, not new product approval requirements. Use
existing visual components. On first visit, select the current interactive Flow
Session if present, otherwise an open direct Session; do not instantiate every
unopened provider client. When Unit 2 adds a primary TaskSession, ordinary Task
entry selects that conversation initially; choosing an available interactive stage
still goes directly to its own Session. On later visits preserve Jack's selection. Incoming
Asks appear in the list without auto-expanding or focusing. All Session kinds
remain named and discoverable.

Review recommendation: make participation visible from the existing navigation,
not only from inside a Task. Task rows show the names/count of unresolved direct
Ask and current interactive Flow conversations. Collapsed Wave rows roll up that
same information; selecting it reveals the Task and exact conversation. Keep
stable ordering and current keyboard focus as new work appears. This is a derived
view of existing boundaries, not a new inbox, notification database, or planning
state. Independent idle chats and running agents do not inflate that count.

Separate three facts in presentation: a conversation is available to join; its
agent is still preparing or discussing; its summary is ready for Complete. The
current `SessionState::Ready` means the last of these, not the first. A pending
Ask or authored interactive boundary can be available before its agent marks
Ready. Use boundary identity, Run evidence, and existing legal actions for these
readings. A provider start failure offers recovery and does not pretend the
conversation is usable. A closed provider is not a completed boundary. Operational
failures are shown as needing recovery; ordinary interactive work carries no
"blocking agents" alarm. An unavailable refresh preserves the last reading with
its stale state rather than clearing the count or inventing new readiness.

Collapse hides a surface while its process and input remain live. Closing a view,
terminating a shell, completing an Ask, and resolving a Flow stage remain distinct
operations. An empty Task shows its directive and offers a conversation, shell,
and files. If no checkout exists, use `lf task checkout` to prepare one without
starting automation or opening a hosted PR. Its ordinary local `TaskPr` record
retains branch/base history; it is not a published PR. File access to an existing
checkout must also work when there is no active local `TaskPr`. Restoring a missing
checkout still needs recorded branch/history evidence: show the existing recovery
error when that evidence is absent, rather than inventing a base or resetting work.

### Source of truth and association

Run manifests/provider history own conversations. Ask records own their waiting
callers. Flow positions own authored interactive boundaries. Task records own
worktree placement. `SessionRecord` is a derived reading; it does not become a
second Session database.

Add an explicit, derived `workspace: Option<SessionWorkspace>` to `SessionRecord`:

```text
SessionWorkspace {
  home_id: HomeId, worktree: PathBuf, task_id: Option<TaskId>,
  unavailable: Option<String>
}
```

Resolve a local Session's recorded working directory to its actual checkout root,
including subdirectories and symlink spellings, then match that root to recorded
Task worktrees on the same Home. Reuse `engine::git::worktree_root` and canonicalize
its result; do not replace it with a lexical ancestor match, which would absorb a
nested repository into its parent Task. Cache resolution by distinct cwd and
recorded placement path within a snapshot, then index canonical roots. The budget
is at most one root lookup per distinct input path, not one subprocess per Session
or UI repaint. Different subdirectories may require separate root discovery.
Remote association is produced by its owning
Home; never canonicalize a remote path against the desktop filesystem.

Use this association for Task placement, breadcrumbs, workspace lookup, and
Session navigation. Keep `work` as recorded Run attribution and `flow_membership`
as captured execution membership. Location association can group a conversation
without granting it Flow completion authority. For missing checkouts, preserve
an association established by recorded placement/explicit Task binding and show
its unavailable state. Unresolvable evidence stays diagnostic; do not invent a
Task from a branch name or basename. Reassignment is not a manifest rewrite.

Implement a shared snapshot resolver in `ops/human_session.rs`, used by list,
open, and other operations returning a Session reading. Batch Task and durable
Work-placement reads outside the Session loop: `Task` holds its worktree but not
its Home, so `&[Task]` alone cannot establish the required identity. The resolver
receives recorded Run cwd/worktree and explicit Work attribution as evidence;
its per-snapshot index joins Task ID, owning Home, and recorded checkout.

Read that index directly from existing `tasks` and `work_placements` under one
SQLite read snapshot. Do not implement it with `list_tasks(None)` plus placement
calls: the current Task query inner-joins `projects`, so missing chapter metadata
can silently omit a recorded checkout. Return minimal internal placement rows,
including Task ID, issue identifier, recorded checkout and optional Home evidence;
missing placement remains explicit. Reuse this checkout reading for file access
so a Project lookup cannot reintroduce a planning dependency there. Project/Wave
labels remain optional presentation evidence. This adds a query, not a persisted
registry or synthetic Project. A Task missing from the planning outline remains
reachable through its Session and recorded checkout without fabricating ancestry.

A failed placement snapshot is an unavailable Session refresh, never a successful
empty index. Preserve `PodiumReading.unavailable(lastGood:reason:)` and do not feed
an empty inventory into surface reconciliation. Initial failure shows unavailable
with no invented previous inventory. Successful reads can report individual missing
placement records. Add the resolver to the shared surface-return paths, including
standalone Flow Sessions and completion results, so opening a Session cannot replace
its associated reading with one that lacks workspace evidence.

An actual resolved checkout is authoritative for location grouping even when
Run attribution names another Work. If the checkout is missing, retain a known
Task association only from matching recorded placement or explicit Task binding,
with `unavailable` explaining the missing checkout. Do not assign an unbound
missing subdirectory by string prefix. An unavailable placement read retains
known UI inventory as stale; it cannot certify a fresh unbound association.
Multiple Tasks claiming the same Home/root are ambiguous: keep the workspace
with no selected Task and an explanation rather than duplicating the Session.

Use `(home_id, worktree)` for workspace identity and Task ID for navigation;
equal path strings on different Homes never share panes or documents. Terminal
identity remains independent of workspace membership: moving a Session between
groups must reuse the same window-owned surface, not change its pool key.
This resolver does not replace publication or execution authorization.
Current `task_for_checkout` matches branch names;
`CanonicalRepo` collapses siblings to main. Neither is the required identity.
Update Rust/Swift DTOs and shared fixtures together, with required-or-Optional
fields and no DTO defaults.

### Retained layout and focus

Keep `SessionsWorkspaceRegistry` as the window-local owner, replacing its current
String path key with the resolved Home/worktree identity. Implementation inspection
also found path-only `TaskWorkspaceSnapshot` and `WorktreeLayoutStore` inputs:
carry owning Home and resolved checkout through prepared Task readings and outer
worktree selection before changing the registry key. An unstarted Task need not
have a Session from which to borrow this identity. Do not infer its Home from the
Wave, use a path-only alias, or create a second workspace while Home evidence loads.
Generic local shells need the existing read-only `lf home id --json` identity;
retain known identity on a failed refresh. Keep its shared
`GhosttySurfacePool` and each workspace's `MultiplexerStore` and `TaskFilesStore`.
Reconcile Session existence against the successful repository inventory before
updating workspace membership. A Session leaving one group is not an absent or
completed Session; remove its old layout placement without releasing its surface.
Do not create another terminal or document registry.

Extend `MultiplexerStore` with collapsed pane IDs and operations
`setCollapsed(paneId:collapsed:)` and `reveal(sessionId:)`. Preserve leaves and
ratios in its existing layout tree; derive visible layout by omitting collapsed
leaves. Do not implement collapse through `close` or `reconcileSessions`.
Expanding an unopened Session explicitly invokes the existing open path; listing
or focusing an already mounted surface does not launch another provider.

Use existing zoom state for terminal focus. Keep the file-visibility preference
in the workspace and hide files while zoomed; exiting focus exposes that same
preference without a second saved value or view lifecycle callback. Keep file documents
and shell identities alive across Task changes. Presentation-only reconciliation
must not interpret a failed Session refresh as an empty inventory and remove panes.
A confirmed resolved Session can leave the live layout and remain in history.

Consolidate the Task entry point in `SessionsView`/`WorkSurfaceView`. Reuse the
terminal renderer and move shared components out of the large view as needed.
Remove the separate `TaskTerminalStore`, `TaskWorkspaceSection`, and old standalone
Task workspace once their callers use the retained workspace. Retain the global
terminal diagnostic tools, not a second implementation of Task terminals.

### Files without a PR dependency

Separate the current `TaskWorkspace` checkout data from PR diff-base data.
`task_file`, `task_save`, and directory listing need Task placement only. Changes
and diff operations continue to require the relevant base; absent PR means no
PR diff, not an invented base commit or an inaccessible file browser.

Extend the existing file API:

```text
lf task files ISSUE [DIRECTORY] [--cursor CURSOR] [--show-ignored] --json
TaskDirectory { path, entries: [TaskFileEntry], next_cursor? }
TaskFileEntry { path, kind: file|directory|symlink }
```

List one directory on expansion, sorted directories-first, with pages of at most
500 entries. Include unchanged and untracked files. Hide Git metadata; initially
hide ignored paths with a visible Show ignored toggle. Never recursively read
file content to populate the tree. Reuse existing path handling, read limits,
encoding states, revision checks, and retained-save recovery. Symlink navigation
uses existing checkout boundaries. Do not add file deletion or terminal editors
as a substitute for the browser.

Enumerate immediate children with filesystem metadata, then batch Git ignore
classification for those paths; no recursive `ls-files` inventory is needed.
Use an opaque cursor naming the directory, ignored visibility, and last emitted
sort key. Order directories first, then path bytes within kind; resume strictly
after the key and return at most 500 entries. Directory pages are live readings,
not a filesystem snapshot: refresh resets pagination after a filesystem change.
Reject a cursor for another directory or visibility selection explicitly.

Source inspection found an important asymmetry: `file_snapshot` follows an
in-checkout symlink, while the save path uses `O_NOFOLLOW` for every component.
Keep that writer. Show symlink entries, allow bounded in-checkout inspection,
and make a file reached through a symlink read-only with an explanation. Reject
outside-checkout traversal; do not recursively expand link cycles. Do not offer
Save and discover this restriction only after Jack has edited the document.
Creating files is outside Unit 1; existing tracked and untracked text files can
be edited without an active PR. Unit 3's builtin customization needs a distinct
explicit creation operation, since the current revisioned save only updates an
existing text file.

Add `read_only_reason: Option<String>` to `TaskFileSnapshot` and its Swift mirror
and fixtures. Text is editable only when this field is absent. Rust derives the
reason from every component below the resolved checkout root, including symlink
parents; the directory entry's leaf kind cannot determine editability. Keep content
state and revision unchanged for readable symlink targets. Access is a fresh reading,
not a promise against a later filesystem race; the existing writer remains final
authority and retains its no-follow checks.

`TaskFileDocument` must update access before its same-revision early return and
on every merge/load path. Disable editing, manual Save and autosave together;
cancel a queued autosave when access changes. Preserve an existing draft, selection
and undo state when a regular file becomes read-only, even when disk bytes have not
changed. Restoring a regular file allows saving only after a fresh reading and the
existing revision/conflict checks. The explanation belongs beside the document,
not solely in a disabled toolbar button. Prove a regular file replaced by an internal
symlink to identical bytes, plus a regular leaf under a symlink directory. A fixture
that only opens an initial symlink misses the retained-document failure.

Extend `TaskFilesStore` with directory readings while retaining its document
cache, observation, and save method. Directory readings own the file navigator;
Changes is an optional view over those same documents. Replace `refresh`'s
unconditional `taskChanges` dependency: initial browsing, directory expansion,
filesystem invalidation, and post-save refresh must work without a diff base.
Keep directory and Changes errors separate, so an unavailable comparison cannot
erase a successful directory reading or present file access as failed. Disable
Diff with its reason when no base exists; do not leave its pane spinning.

Reuse file observation to invalidate affected loaded directory pages and refresh
retained documents. Reset pagination for those directories, preserving expansion,
selection and drafts; do not recursively populate the checkout or reload every
page. Refresh Git comparison metadata only for Changes. File browsing works with
a checked-out, unstarted Task and no active PR, including after saving and after
an external edit. Removing the Rust read/save prerequisite alone is insufficient:
the current Swift navigator, timer, invalidation and post-save paths all refresh
through `taskChanges`.

### Interactive stages and background edges

Derive the compact view in Rust from the existing `FlowGraph` and captured cursor.
Add `project_interactions(graph) -> InteractionGraph` in `engine/flow_graph.rs`;
include its output alongside the detailed graph in Flow snapshots/catalog entries.

Stages refer to exact human Skill occurrence keys. Transitions refer to the
underlying automated nodes and route structure between stages. Start/end anchors
represent work before the first or after the last interactive stage. Keep branch
alternatives and repeat targets; determine reachable next interactive occurrences
by traversing automated nodes with a visited set. Preserve route conditions and
references into the detailed graph rather than enumerating every possible path.
A Flow with no human stages presents one background span with inspectable work.
Prune routes ending at other interactive stages before reverse reachability.
Otherwise a repeat back to the source stage can incorrectly contribute work to
its exit edge even though that work requires another interactive visit.

Do not manufacture one named YAML Flow for every edge. Current composed Flows can
span several interactive stages. Label transitions with their authored Flow
context and expose their actual captured steps. The inspector remains truthful
about branches, loops, and the current pass. A running edge is preparing work;
a Session becomes ready for participation only from actual boundary evidence.

Selecting a stage focuses the exact Session by invocation, occurrence, and pass;
never by matching the skill name. Selecting a future stage inspects it and does
not launch it early. An Ask raised on an edge appears in the Task list with a
link to its caller; it does not alter the authored graph or masquerade as a
planned node. All shared Flow consumers receive updated fixtures.

### Ask fits this workspace

Keep `lf ask`, its optional skill, and its focused Session completion lifecycle.
Keep direct Flow Session completion/navigation semantics. Both use the same Task
workspace and terminal presentation; their recorded actions determine the controls.

Raw Ask without a key allocates a new UUID on every call. Blocked-decision Asks
have reusable boundary keys. Raw Ask now also accepts an optional stable `--key` scoped
to its caller Run, using existing keyed storage. Repeating that question joins its
Session or returns its retained answer; distinct questions remain distinct. Do not
identify questions by hashing prose. Automatic recovery uses its exact existing
Flow key. Update headless/Ask guidance to reuse a key for a repeated question,
ask in the current conversation when Jack is already present, and otherwise
raise consequential unresolved choices with evidence. No mandatory Wave relay.

### Internal cuts and proof

1. **Implemented internal cut — association and prepared files.** The shared workspace
   association and whole-worktree file reading remove the PR dependency from
   file read/save. Recorded focused proofs provide partial coverage; the cases
   below remain the acceptance contract, including connected desktop proof.
   Prove an unbound Session in a Task subdirectory groups correctly,
   a sibling checkout does not, and an unstarted checkout can read/save a file.
   Exercise that no-PR case through the Swift directory navigator, post-save
   refresh and filesystem invalidation; a direct Rust read/save pass alone does
   not prove that the browser works. Failed Changes reads leave files usable.
   Include symlink cwd aliases, a nested repository, equal paths on two Homes,
   conflicting Run attribution, missing placement, and an unavailable Task read.
   Include a retained Task/placement whose Project metadata is absent; association
   and checkout file access survive without inventing planning ancestry. A failed
   placement snapshot retains existing panes and marks the reading unavailable.
   Exercise pagination past 500 entries and the ignored toggle; prove the tree
   shows unchanged/untracked files, symlink inspection stays read-only, and an
   external writer's revision conflict retains the draft without a PR. Replace an
   open regular file with a same-content symlink and prove editing/manual Save/
   autosave stop while the draft survives, including a symlink in a parent path. Move
   Swift grouping and DTO fixtures with the resolver so the new reading has a
   real consumer in this cut.
2. **Implemented; configured demo pending.** Retained Task workspace: integrate the inventory, collapse, focus/restore,
   files, and shells. Prove layout transitions preserve the same surface identities
   and documents, including a live Session reassociated between workspace groups.
   Delete superseded Task terminal ownership in this cut.
3. **Implemented; configured demo pending.** Participatory Flow projection and direct Session navigation; expose Asks in
   that workspace and complete retry behavior. Derive participation indications
   on existing Task/Wave rows. Prove loops, branching, repeated skills, independent
   conversations, exact caller/occurrence resolution, and discovering a review
   under a collapsed Wave without stealing focus or counting idle agents.
4. Run the configured desktop demo across Rust readings, Swift grouping, real
   Ghostty input, save/readback, and Ask/Flow continuation. Ship the whole unit.

Unit 1 is done only when the opening demo works with actual retained processes,
not just ViewInspector output or a passing layout model.

## Unit 2 — primary Task, Wave, and repo Sessions

### TaskSession and changing direction

Launching a Task ensures one primary TaskSession in its recorded checkout and
starts its selected Flow. Repeating launch/resume finds the same TaskSession and
current invocation. Merely filing or viewing an unstarted Task does not start
one. The workspace gives the TaskSession a stable main position; interactive
substeps, direct Asks, independent Sessions, terminals, and files remain available
beside it. A prepared review opens its own window/pane when selected, and a
request for review does not replace the main Task conversation.

Implementation recommendation: the TaskSession agent operates the existing Flow
runtime. It starts/resumes execution, reads outcomes, and works with Jack on the
Flow to run next. `FlowPosition`, captured definitions, worker claims, and explicit
decisions remain the execution authority. Do not turn its transcript into another
cursor, ask it to infer completed steps from prose, or run a second implementation
worker inside the main conversation. Normal automatic progression does not need an
extra agent turn at every step. Interactive Session completion returns feedback to
the recorded Flow boundary; TaskSession awareness does not become another approval
gate. Existing `controller/task` already owns those boundaries and step launches.

Jack can say “finish this loop, then run the new Flow” or “switch now”. Both keep
the TaskSession and start a fresh invocation of the selected Flow:

- **Switch now:** validate the target from the Task checkout, stop the exact current
  worker, confirm it has stopped, preserve the final work, and start the replacement.
  Unknown termination prevents overlapping execution. Invalid replacement leaves
  the current Flow running. Reuse/refine `task_restart` instead of adding another
  restart mechanism; its current sequence checkpoints before stopping, so final
  writes need reconciliation before replacement starts.
- **Finish, then switch:** continue to the next loop point of the innermost
  active loop, then exit the old Flow and start the replacement **instead of
  invoking the decider**. Save that exact loop occurrence and target in the
  existing Task execution state. Do not run `loop-decide`, ask it for Advance or
  Iterate, finish an outer loop, or continue into publishing. In `pursue`, a
  request during implementation runs compress and refresh, then switches before
  `decide`. The TaskSession names that point in its response. Jack can cancel or
  replace the pending switch. If no such loop point exists, keep that absence
  explicit rather than silently redefining “finish” as the whole Flow.

The target definition is validated and captured when the switch is accepted, so
later file saves do not silently alter the queued choice. Conceptual switch data:
source invocation, target capture, and timing (`now` or exact boundary occurrence).
Persist it with current execution, consume it atomically against that invocation,
and reuse the same replacement on retry. This is not a new scheduler or a second
Flow graph. If the old Flow blocks before the selected boundary, the switch stays
pending and the TaskSession can work with Jack on recovery or switch now; it must
not call failure a finished loop. Restarting desktop or resetting the conversation
must not forget or repeat an accepted switch.

#### Resolve the loop point from the captured execution

`ExecutionCursor.child` represents selected XOR paths, not nested loops. Loops
are backward `repeat.from` edges in each captured body. Resolve the active leaf
body first, then its selected ancestor bodies: find repeat intervals containing
the current position and choose the unique smallest containing interval in the
deepest body that has one. Equal-start intervals such as `pursue`'s `decide` and
`decide_delivery` select the earlier endpoint while implementation is active.
Use structural occurrence keys and the captured invocation, never skill names
or composed Flow names. Do not select a loop in an unchosen XOR alternative.

The runtime permits crossing intervals, for example `start … middle … A→start …
B→middle`. At `middle` neither interval contains the other. Keep both concrete
endpoints visible and require an explicit endpoint before accepting a deferred
switch in that case; continue the current Flow unchanged. Likewise report no
active loop when no interval contains the cursor. This is a proposed handling of
an undefined case, not a new restriction on valid Flow definitions. If the
selected decider is already claimed or has a retained verdict, its pre-launch
boundary has passed: report that fact instead of promising to skip it or silently
selecting another pass. Jack can choose switch now or a later explicit boundary.

Pin the occurrence within the current activation of its body. The root
`iteration` counter also increases for nested repeats, so equality with the
root counter at request time would miss an outer endpoint after an inner repeat.
Use the selected path/ancestor activation and that decider's traversal count to
identify the next visit; test an inner repeat while an outer switch is pending.

#### One durable replacement transition

Extend the existing restart operation and Task store transactions. The proposed
pending switch holds a request key, source invocation, captured `QueuedInvocation`
including its allocated replacement ID, exact timing, and replacement phase.
Cancellation and changing a queued target compare against that request. Save it
separately from the cursor fields that a running worker settles, so a worker's
older in-memory `FlowPosition` cannot erase an accepted request.

Acceptance and worker claim must serialize in the same store transaction domain.
While work finishes before the selected point, its normal claims remain legal.
Once that point is reached, claim/reclaim and interactive preparation must observe
the pending replacement before preparing or launching the decider. Cover
`drive_task`, external resume/recovery, and direct Session completion; a check in
the TaskSession prompt or desktop poll is too late. A concurrent claim either
wins before acceptance (report the passed boundary) or loses to the accepted
switch. Do not invalidate the currently running worker merely to save the request.

For switch now, record replacement intent before stopping. For either timing,
settle the last boundary, confirm the old execution has stopped, then capture the
final checkpoint. Atomically retain the superseded invocation/cursor and install
the already captured successor under the expected source identity. Never expose
an intermediate absent position that ordinary resume could fill with a default
Flow. Recovery reuses the saved successor ID and existing worker claims; it does
not reload edited YAML or allocate a second invocation. Retain the switch receipt
in Task history so a repeated request can return its result after promotion.

Current restart forces a PM refresh and its checkpoint helper may push. Keep
metadata refresh separate from replacing a known Task's execution: an unavailable
refresh retains recorded ownership with an explicit stale reading. Use a local
checkpoint after stop; normal publication still owns push and PR operations.
If checkpointing fails, retain the prepared replacement and stopped state for
retry. Checkpoint preservation includes final worker writes without stopping or
relaunching unrelated Sessions in the shared checkout. No reset, clean, or replay
may discard those files.

Retain the old invocation and outcomes as history. Outstanding interactive Sessions
from a superseded invocation remain inspectable and clearly marked superseded;
completing one cannot advance the new Flow. Unrelated Sessions in the checkout stay
independent. Switching does not complete a review, discard files, or replace the
TaskSession itself. Flow editing in Unit 3 can supply the replacement definition;
saving alone still changes no running invocation.

The accepted automatic Wave wake still applies. The Wave can inspect TaskSession
and worker output, but TaskSession operations, Wave recovery, and runtime retries
must share the existing Task execution ownership. A pending switch or active recovery
must be visible before another actor acts. TaskSession introduction must not create
another agent independently repairing the same work.

Proof: launch one Task and retain its main conversation while a direct design
review opens separately. Queue a switch during implementation: compress and
refresh finish, the pending switch is consumed before the innermost `loop-decide`
launches, and exactly one replacement starts. No decision Run, outer-loop completion,
publish, or repeat occurs. Prove nested loops select the inner point and repeated
skill names select the exact occurrence.
Exercise crossing repeat intervals, an unchosen XOR path, a claimed decider,
and nested iteration changes; no unsupported case may silently pick another
endpoint. Race acceptance against claim and worker settlement, and recover after
stop/checkpoint/promotion independently. Prove the captured successor survives
each interruption with the same ID, including when PM or Git hosting is unavailable.
Then switch now during a later attempt, preserving the last writes and the same
TaskSession. Prove duplicate requests, process loss during replacement, pending
switch recovery, failure before the boundary, target edits after capture, and stale
review completion. Main-session text input remains usable while the Flow runs.

### Scope identity and lifetime

Desktop discovery ensures repo/Wave Sessions; Task launch ensures TaskSessions.
List/status operations remain read-only. Use existing ordinary interactive Session
execution and provider continuation. Add a small pointer record to the existing
Home-local store:

```text
PrimarySession {
  scope: Repository(CanonicalRepo)|Wave(WaveId)|Task(TaskId),
  run_id: RunId,
  replacement_run_id: Option<RunId>
}
```

Home is the store's ownership boundary; route remote Wave operations to their
placed Home. Repository identity is the existing local canonical repository,
not a required hosted remote. Wave and Task identities survive name changes;
TaskSession placement follows the Task's checkout on its owning Home. The record
owns only the current ordinary Session and any prepared replacement. Run records
remain the history and provider/account authority; no new transcript or
Task/Project Session database.

Expose `lf session ensure --repo|--wave NAME|--task ISSUE --json` and
`lf session replace ID --json` through Rust operations. Ensure acquires the
existing style of per-scope launch lock, prepares a Run before publishing the
pointer, and launches once. Retries use the same prepared Run; failed starts show
an actionable state. Replace takes the expected current Run ID, records replacement
intent by saving the prepared successor ID, stops the previous execution, and
atomically promotes that successor and clears the pending ID. Retry finishes
that same replacement. Launch no successor while the old provider's termination
remains unknown. Add an ordinal-free migration draft
and recovery tests; do not modify published migrations.

Reuse the prepared-Run launch path, generalizing its owner token to include a
primary scope without turning that Session into an Ask/Flow boundary. The desktop
calls ensure when discovery changes, independent of pane visibility. Subsequent
windows attach using existing Open/Move here semantics rather than racing another
client. No provider start or registry mutation happens in SwiftUI body evaluation.

Use an explicit primary launch path. Repository discovery must not invoke bare
`lf`'s default-main relocation and move unrelated edits. Primary conversations
start with repository guidance and, for Waves, current goal/memory and planning
references. Supply explicit scope instructions rather than the current generic
Wave executive-loop seed. A replacement receives current durable guidance and
work references; previous history stays inspectable without replaying it all.

Author self-contained builtin `task/session`, `wave/session`, and `repo/session`
prompts. These
are proposed skill names, not new runtime agent types. Compose the relevant
judgment from design, wave/operate, launch-plan, and init; do not concatenate all
four prompts or require a user-facing design/operate mode switch.

- **TaskSession:** carry the Task's intent and current design, operate its Flow,
  explain current work, and collaborate on changing direction with the two switch
  timings above. Keep interactive substeps direct and read their retained feedback.
- **Wave Session:** understand the objective, current work, and accepted direction.
  Take useful authorized actions to progress Tasks to shipping or the next direct
  interactive stage. Investigate blockers, recover through existing Task controls,
  and reconcile evidence before starting anything. Existing Tasks and Flows own
  execution; the Session does not become their second driver. Leave prepared
  interactive work ready for Jack and continue useful independent work. When Jack
  introduces an idea, explore and capture it in scratch, distinguish proposals
  from accepted choices, then create or reuse Tasks when the direction is ready.
  Existing Tasks can keep running while design develops in this conversation.
- **Repo Session:** welcome and orient someone from their immediate goal. Explain
  Loopflow through useful action, help start direct work or select/create a Wave
  when needed, and diagnose problems that have no working narrower entry point.
  It works with zero Waves, zero Tasks, and unavailable PM. Discover setup only as
  needed for the requested action; do not run the full init checklist on discovery.
  An unavailable managed service leaves local help/design/diagnosis usable. Do not
  infer empty planning from a failed read or repair unrelated accounts to proceed.

At startup, the Wave Session reconciles current evidence and advances authorized
work; it does not wait passively for a greeting. The repo Session orients from
known context and invites the first goal when none is known. Neither manufactures
Tasks solely because the app opened. Prompt autonomy uses existing authorization;
inspection and navigation themselves remain read-only. Eager Session creation is
an explicit discovery operation, revising the older passive-chat product direction.

Design writing needs a stable checkout. Reuse Loopflow's existing create/reuse
agent-worktree operation with explicit scope placement, not its move-main-edits
path. The former Wave resident placement wrapper was deleted upstream;
use `engine::worktrees::ensure_agent_worktree` as the surviving shared owner.
Preserve existing files and recorded checkout placement on replacement. Primary placement uses
the local default ref and does not require the helper's current origin fetch;
add an explicit local-base path to the shared placement operation. Unavailable
Git hosting or PM must not prevent conversation with an available provider.
Shipping implementation stays in Task worktrees. Where no Task/PM exists, the repo
Session can still capture designs and perform authorized local work using ordinary
Loopflow operations. Transfer actual design/evidence into Task scratch at launch.

### Observation and attention

#### Delivery mechanism that must be proved first

The current `human_session::open` resumes a native provider CLI through
`commands/util.rs`; it does not retain a `Harness` handle. Separately,
`Harness::send_input` drives structured provider turns (Codex RPC, Claude stream
input, OpenCode HTTP). Existing provider history supports continuity, but does
not connect these two owners or serialize their input. Starting a second Harness
against the same provider history is not an acceptable substitute for delivering
to the current primary Session.

Before implementing Unit 2's pointer/navigation cutover, run a focused integration
spike through the actual configured native provider: open a primary candidate,
leave draft input, deliver one structured operational wake while idle, then queue
another during Jack's turn. Both turns must enter the same conversation through
one execution owner, preserve draft input, and publish durable turn receipts.
Repeat across Replace and a lost delivery acknowledgement. Terminal keystroke
injection, killing the terminal for every wake, or an independent responder fails
this proof. A Harness-only simulation proves scheduling, not this integration.

Compare two mechanisms at this seam: a supported control channel into the
existing native client, or a Session-owned Harness with an interactive client
attached to that same owner. Prefer the existing native client if it can meet
the contract. The second route requires a concrete interactive surface and
attachment path; the Harness trait alone does not supply either. Neither route
is established by current source inspection. If the spike requires replacing the
accepted native terminal experience or adding a resident service, return this
unit to design review; do not silently relax automatic wakes or terminal retention.
Unit 1 is independent and can proceed while this later unit remains bounded.

The receipt boundary also needs work. The outbox and its pending/delivered
operations survive, but the Wave listener and its `TurnOpened.answers`
claim/requeue implementation were deleted in `3dc89bc9a`. Current source has no
production caller of `pending_observations` or `mark_observation_delivered`.
Retained `delivered_at` values describe historical transfer, never recovery
completion. Establish durable claims, incomplete-turn recovery, and receipts
under the primary Session's one execution owner using the existing outbox/store.
Do not treat those guarantees as inherited from the deleted journal or restore
the listener to obtain them. Before acting, reconcile retained observations and
any existing Task worker/operating Run; missing listener state proves no provider
has stopped. Explicit `wave/operate` Runs and TaskSession recovery must share
the Task's execution authority. Filtering one wake source alone cannot prevent
two agents attempting recovery. This mechanism remains Unit 2 work, conditional
on the native turn-delivery spike.

Independent Run failures and repo attention are not current child observations:
`ObservationRecipient` names Wave/Project and `ChildRef` names Task/Project.
Extend the existing durable observation mechanism to reference originating Runs
and repository scope, with a caller Run plus explicit stable key for repo
attention and a Run/outcome occurrence key for automatic failure observations.
Record evidence references and reason, not copied transcripts. Keep pending
observations addressed to scope across primary replacement; record the receiving
Run/turn only when claimed. A completed provider turn acknowledges handling,
not proof the Task recovered. Re-read the originating work and retain action
evidence before retrying uncertain delivery; exactly-once effects cannot follow
from a transport acknowledgement alone. No second inbox database is proposed.

Jack accepted automatic Wave turns for operational blockers while Jack works
elsewhere. Planned reviews and decisions requiring Jack stay in direct Task
Sessions. Unit 2 must demonstrate recovery after the Wave Session has gone idle;
a prompt that only acts at startup is insufficient.

Jack accepted simplifying communication around existing output: every Task
Session produces evidence where it already works, the Wave can read all of its
Tasks' output, and the repo Session can read across its repository. Worktree-based
Task association applies to independent Sessions as well as Flow Sessions. Reading
scope does not depend on explicit Flow membership or grant completion authority.

Keep access to evidence separate from requests for attention.
Use current Task/Run state and outcome summaries to identify useful next actions,
then read the relevant Session transcript, command output, artifacts, and failure
evidence on demand. Preserve source identity and missing-evidence states. Agents
should not need to duplicate a Task result into a special Wave message channel.
An arbitrary sentence buried in a log is not a reliable wake signal: derive wakes
from durable operational outcomes and existing Work observations. Normal output
remains readable without triggering a primary turn for every update. Direct
interactive boundaries stay available to Jack without becoming recovery alarms.

The repo Session can read across its repository without consuming every Task's
output on every turn. Jack accepted one narrow push capability: any Session
associated with the repository can request repo attention by referencing its
existing output and stating why broader help is needed. A Wave might raise a
failure shared by several Tasks; a Task or unassigned repo conversation can also
request help directly. Going through the Wave is not mandatory. The repo Session
reads the original evidence and keeps the resolution with the originating work.

Include this capability in Unit 2. Record the source Run/Session, relevant outcome
or artifact reference, and reason at the source; deliver the reference to the repo
scope's current primary Session. Requests draw attention asynchronously. Adding a
caller-wait protocol is outside this minimal scope. Do not copy a second conversation,
create an inbox screen, or add general-purpose threaded messaging. Use existing
operations to retain findings and act on the originating work; a reply alone must
not resume a waiting Flow. Direct Task Asks continue to bring decisions to Jack.

Task events continue through the existing durable runtime. Deliver operational
wakes to the same persistent Wave Session, serialize with Jack's conversation
without submitting through terminal input, retain pending observations across
replacement, and coalesce repeat observations of the same unresolved occurrence.
Current Work observations persist without the removed resident dispatcher.
Connect selected operational outcomes to the primary owner and fence recovery
against other operating Runs through existing Task controls. The implementation must
establish provider turn delivery and receipt ownership without a second supervisor,
planning cursor, or new resident service. These are Unit 2 implementation gaps to
resolve, not evidence that native primary delivery already works.

Intercept Ctrl-C only in a primary Session terminal and invoke Replace. The
provider's ordinary interrupt key behavior is insufficient to guarantee a fresh
conversation. Flow Sessions, Asks, and shells retain their current Ctrl-C behavior. Whether
Ctrl-C also resets the proposed primary TaskSession remains a product choice;
resetting that conversation must never silently switch or restart its Flow.
A crash or normal process exit shows that state and a retry action; it does not
cause an unbounded fresh-agent loop. Collapsing a pane does not replace anything.

Repository/Wave navigation opens the primary conversation with planning/context
available beside it. Task selection still opens the Task workspace. Move the
orphan/global Session browser to Diagnostics. Native desktop Wave conversations
use this Session path. Upstream already removed the resident chat surface and
external bridge; there is no responder to detach. Keep explicit Wave operation
available, and establish one consumer for selected blocker observations as
specified above. Do not restore external chat transport or automatic governance
scheduling as a prerequisite for primary Sessions.

The desktop is the eager initiator. Closing it does not invent a new wake service;
ordinary process/session lifecycle applies, and pending work remains durable until
reconnection. Operation while desktop is absent remains a separate scope choice.
General messaging is not required for Task Asks or automatic Wave recovery. Repo
attention requests are the sole new explicit escalation capability; they share the
same primary-turn delivery and replacement guarantees, with no separate Ask inbox.

Done when one repo plus two Waves yields three current primary Sessions after
concurrent/repeated discovery; app reopening retains them; Ctrl-C produces one
fresh successor with old history accessible; and Task Flow/Ask continuations
survive unchanged. Prove prepared-start failure, replacement interruption,
multiple windows, unavailable provider, and remote placement without duplicate
local launches. Also demonstrate first-use repo onboarding with no Waves or PM,
a Wave Session moving accepted work to its next interactive stage, and Jack
introducing a new idea that becomes a design and Task while existing Task work
continues. Trigger an operational failure after the Wave Session becomes idle:
it wakes, reads the Task's original evidence, and performs one useful recovery
without Jack opening it. Include a failed independent Session associated by Task
worktree, not just a Flow worker. Duplicate observations, Ctrl-C replacement, and
an existing explicit Wave operation must not duplicate the recovery. An interactive review
remains available to Jack throughout. Demonstrate a repo attention request from
an independent Task-worktree Session and one from a Session outside any Wave.
Both reach the current repo Session across replacement, preserve the source
evidence, and retain their results with the originating work. Repeated delivery
must not duplicate recovery or resume a direct Ask. Verify the repo conversation
can diagnose unavailable planning without requiring a resident or pretending the
backlog is empty. Prompt inspection alone
is not proof of these behaviors. This lifecycle, navigation, and role change ships
as one unit.

## Unit 3 — defaults and effective Flow editing

Expose the chapter's existing Project recommendation as Default Flow in Wave
settings. Update only that field through the existing PM authority, preserving
KRs/targets and Task overrides. Defaults spanning chapters are not introduced.

Edit Flow opens the actual source in the selected checkout via the same file
editor. For a builtin, offer Customize in repository: write its authored YAML to
`.lf/flows/<canonical-name>.yaml` and use the existing override resolution. For a
collapsed edge, choose among its referenced Flow sources; do not generate a new
Flow from the display projection.

Catalog, preview, validation, and invocation must use that checkout. Refresh the
catalog after save. New Task preparation may preflight the origin, but revalidate
in the actual placed worktree before capture. An invalid definition remains saved
and visibly invalid; never quietly execute a builtin fallback. Existing invocations
remain pinned. An explicit TaskSession switch applies the selected captured
replacement now or at the next inner loop point before its decider; other new
invocations read the saved
source when they start. Saving alone neither switches work nor propagates to
sibling checkouts.

Done when changing a chapter default affects a new Task, an explicit Task choice
wins, and a saved uncommitted Flow edit appears in preview and executes from that
worktree. Another checkout retains its own source. Resuming a captured invocation
retains its original steps. Prove builtin customization and invalid YAML as well.

## Failure states, budgets, and verification

Unavailable planning must not remove known Sessions or files. Missing checkout
shows the recorded Task and recovery action; no silent creation during read-only
rendering. A stale Session read preserves the last successful inventory with an
error. Missing provider history cannot be labeled a user reset. File conflicts
retain the draft through existing recovery; binary/oversize files stay explicit.
No layout operation can complete work. No Ask may be resolved merely because its
pane closes or its provider exits.

Expand/collapse/focus of retained surfaces performs zero CLI, Git, or network calls.
Opening an unmounted Session uses its explicit launch path and is measured separately.
Target p95 under 100 ms for retained-surface actions over at least 20 samples on
the configured Mac, using existing desktop performance instrumentation. Preserve
Product's separate `hierarchy_interaction_ms` and `task_workspace_ready_ms` signals
and their accepted-input-to-usable-content endpoints. This is
a proposed acceptance target, not an achieved measurement. Collapsed surfaces
must not draw continuously; processes may continue doing useful work. File tree
expansion reads one directory page. Discovery launches each scope once and does
not block navigation on provider/network startup. Record process count and idle
CPU before/after with several Tasks open; no unsupported speedup claim.

Focused proof commands and extension points:

```sh
cargo test -p loopflow --lib ops::human_session::tests
cargo test -p loopflow --lib engine::flow_graph::tests
cargo test -p loopflow --lib task_files
swift test --package-path swift --no-parallel --filter MultiplexerStoreTests
swift test --package-path swift --no-parallel --filter TaskFilesTests
swift test --package-path swift --no-parallel --filter WorktreeWorkspaceTests
swift test --package-path swift --no-parallel --filter WorkspaceNavigationTests
swift test --package-path swift --no-parallel --filter TaskFlowProofTests
```

Add behavior cases to those owners and `WorktreeWorkspaceTests`; include shared
Rust/Swift Session and Flow fixtures, then affected suites once through
`uv run python scripts/test.py --reuse-passing`. Native UI changes also require
`uv run python scripts/test.py --loopflow` per TESTING.md, covering the Xcode
fallback build as well as SwiftPM/Ghostty. Extend `SessionsInteractionTests` and
the existing desktop performance scenario for the opening demo. A simulated
provider can prove mechanics; label it and also run one real configured provider
Task Ask/Flow handoff before claiming the intended experience works.

Kickoff's production-model zoom/close probe passed. It confirmed reuse of zoom
and disproved close/undo as collapse. The follow-up checkout-root probe also
passed for subdirectories, symlink aliases, and distinct main/linked roots.
First-cut implementation evidence and remaining proof are recorded in
`kickoff-evidence.md`. No full gate, primary-session launch, or live Ask
demonstration has occurred. The Unit 2 native turn-delivery spike remains required.

## Alternatives, exclusions, and review

Chosen: worktree association in shared Rust readings, retained native terminals,
existing Ask/Flow ownership, and a derived interaction graph. Swift-only path
matching would diverge from CLI readers; branch-name matching loses checkout
identity. A new terminal grid would duplicate the existing multiplexer. A separate
Ask inbox or central agent relay would add navigation to Task work. A new graph
editor or runtime is unnecessary for the accepted file-based authoring.

Success looks like returning to the same Task with its working materials intact
and immediately creating or deciding. Failure looks like a wall of live terminals,
focus stolen by new activity, duplicate primary agents, or edited Flows that never
run. The layout defaults, sole surface owner, explicit Session lifetimes, and
same-checkout proof directly address those failures.

Exclusions: changing authorization, removing Flow captures, merging all chats into
one transcript, restoring the removed resident scheduler or external chat bridge, new
cross-Home process supervision, automatic cross-worktree Flow propagation, and
changing direct Flow/Ask Ctrl-C semantics. Exact backend storage and new DTOs are
implemented with migrations/fixtures where required, not compatibility defaults.

Review finding: the initial design conflated primary-agent reply routing with
Task Asks. The chosen plan preserves direct Task Asks and Flow Sessions and keeps
general-purpose messaging outside the required path. A second finding separated
file access from PR history and collapse from pane destruction. These changes remove dependencies
that would otherwise break the promised workspace before any UI polish.

Placement evidence: Product GOAL assigns the shared user contract and one desktop
workspace here; Infrastructure retains runtime recovery authority and Intelligence
retains raw context/trace ownership. This Task proves workspace behavior, not the
Wave's external-product weekly-progress KR. Current chapter KRs are unavailable.
Product memory's native resume, sole surface ownership, exact boundary identity,
and measurement constraints remain binding. Its older passive discovery behavior
is superseded only by Jack's explicit eager-primary-Session direction.

Review-design findings, 2026-09-30: the original demo began after Jack found the
right Task and therefore did not prove freedom from monitoring. The proposed
navigation indication and cross-Task demo close that gap without another inbox.
Source inspection also distinguishes boundary availability from `SessionState::Ready`,
which enables Complete after a summary. Jack accepted automatic Wave wake on
operational blockers, reading existing Task/Session output, and a narrow explicit
repo attention request. Reliable delivery and single recovery ownership are
required implementation work. The later implementation acceptance at the top of
this plan selects Unit 1 first; this review record does not expand that boundary
to Unit 2 launch or a general messaging system.
