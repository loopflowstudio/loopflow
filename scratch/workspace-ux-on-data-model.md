# Workspace UX after the data model

LOO-303 · Product · 2026-09-26

Status: approved in interactive review on 2026-09-26 with the Project-template /
Task-invocation presentation correction below. The current participant's name
is unresolved. Navigation is implemented with local proof and a
[slice review](workspace-navigation-review.md); installed acceptance remains owed.
The [template slice review](workspace-template-review.md) records the later
navigation/template checks, status correction and remaining acceptance.
Source inspected at `c832aaede633a8996bf694dfb7ae2579dbabce3a`.
The supplied placement stacks this Task on LOO-298 / PR #1296. That is placement
evidence, not evidence that its owner conversion is complete or merged.

## Problem

Jack wants to reach the experience enabled by the new data model: put an orphan
conversation under its Task without losing the terminal, understand a Flow before
it runs and its exact attempts afterward, and reach work from the keyboard or a
link. The existing sidebar, warm palette, pages and terminal chrome are the base.

Jack's evening comment `41ea97a6-1532-4453-87b2-86fc7634aad2` reports that the
installed LOO-291 build `0.12.22+5838f53a6` is working well and explicitly
prioritizes this UX before a teardown. It adds Task deep links, multiple Run
attempts at one step position, early implementation of independent pieces, and
**light mode only**. This supersedes the title's dark-mode scope and the older
plan's visual acceptance ordering. It does not establish acceptance of this Task.

The canonical model is [Architecture](../docs/architecture.md) and
[Architecture Reference](../docs/architecture-reference.md#core-models-and-apis),
amended by that comment. Session owns Runs and a current Run. Invocation Task is
nullable. Run attribution is write-once; Task Started is the set-once column.
The copied [decisions](from-loo291/demo-native-workspace.md) remain evidence;
the sidecar, unbind, six-pane cap and Session-as-Run-child proposals in the older
[S6 note](from-loo291/session-launch-surfaces.md) are superseded.

This serves Product's shared user contract and inspection/recovery objective.
Neither shipping this workspace nor opening a Session proves external progress
in Cube, Etude, Kata or Hootro. The supplied chapter has no metric targets.

## The demo

Run `open 'loopflow://task/LOO-303'`: the current workspace opens that Task's
page, without starting work or entering its sole Session. Press ⌘K to open a
Wave, Task or named Session. On an unstarted Task, expand a folded sub-Flow;
after starting its Flow, inspect the captured graph and a node's **attempt 2**.

Open **Orphan sessions · N**, type a draft in a retained conversation, choose
**Bind to task…**, and confirm the named permanent target once. Its Task becomes
started and reachable in the sidebar, with the same Session, terminal and draft.
When that was the last orphan, the workspace enters that Session under its Task.

## Approach

### Delivery boundaries and ownership

Build the navigation and template pieces before waiting for the tables. They use
existing reads and do not introduce a replacement Session inventory. Then consume
the completed LOO-298 contracts for attempts, grouping, control room and bind.
The parent must land before this stacked PR lands. Reconcile parent changes
through `lf rebase`; do not rebuild its model or repair its PR bookkeeping here.

| Fact or operation | Owner | LOO-303 change |
| --- | --- | --- |
| Session identity, current Run, typed ancestry, history, readiness and bind | LOO-298 Store and shared CLI | Consume the final DTOs/actions through `RegistryQuery`; no alternate writer |
| First Task assignment and `started_at` | Shared Run transaction | Refresh shared Task evidence after bind; never set Started in Swift |
| Exact position and current Run attempt | Shared invocation/Run projection | Require attempt history and current-attempt selection, render the same values in three places |
| Template composition | `engine/flow.rs` shared resolution | Preserve composition boundaries during resolution; project a folded template |
| Captured execution and nesting | LOO-298 invocation owner | Read the captured graph and entered child invocations; never reload templates for history |
| Destination, disclosure, recents, picker draft | Per-window/repository `WorkspaceNavigation` | Add palette and typed navigation requests; disposable presentation state |
| Terminal lifetime | Window's `SessionsWorkspaceRegistry.surfaces` | Retain existing surfaces while changing their sole mounted location |
| Planning content and Task identities | Existing shared roadmap/local planning reads | Add exact Task destination resolution using those owners |

Before table-dependent implementation, require a source/fixture receipt for:
all-kind Session rows with stable ID/current Run; typed current Task/Wave and
repository identity; Task lookup independent of current roadmap membership;
write-once bind with selected-Run fencing; shared Started; exact invocation/node/
iteration Run history and current attempt; retained completed invocations; and
taskless invocation legality. A schema containing `runs` is insufficient.
The inspected parent supplies only a subset, documented below.

### One navigation path: palette and Task URL

Add typed navigation requests in the existing Podium/window state. The palette
and URL route to that same path. Destinations distinguish **Task page** from
**Session**: opening a Task through either entry always displays its details,
even with one Session. Existing sidebar single-Session drill-down stays intact.
No navigation request prepares a checkout, starts a Flow, binds a Run or takes
over an external client.

`loopflow://task/ISSUE` resolves exact issue identifiers, never titles or directory
names. An optional percent-encoded `repo` query narrows ambiguous references;
without it, resolve across the selected Home's registered repositories. One match
selects its canonical repo and Task; several show a repository-qualified chooser;
none show a useful unresolved destination with Retry, preserving the current
workspace. An unavailable read is not a no-match. Parse one decoded path segment;
reject malformed/extra segments and never execute URL values as shell text.

Extend the existing local roadmap reader with proposed
`lf roadmap --task ISSUE --all --json` (or repo-scoped `--task` without `--all`).
Reuse its Task/Wave projection and selector resolution. The filter searches
retained planning and registered Tasks, including completed/prior-chapter Tasks;
it returns all exact candidates with canonical repo and stable identities. Do not
call Task launch/preparation or require an active PR. A Task with incomplete
cached planning remains a named, bound destination with unavailable details.
Normal roadmap behavior is unchanged when the filter is absent. This is a narrow
shared read extension over existing owners, not a new navigation database.

Keep the supplied Task evidence in the selected reading so the current-only
roadmap refresh cannot immediately eject a historical destination. Do not insert
it into the current chapter plan. Late results carry the destination request's
generation and cannot replace a later click or link. Retain a pending request
through cold-start loading and consume it once after the target Podium exists.
Route to a current `PodiumView` window, not the older `WavesView` repo scene. Reuse
an appropriate workspace window or create a Podium window with the pending typed
request. Do not broadcast a Task selection to every window.

The palette uses the selected repository's Wave, full Task plan (including
unstarted Tasks), Session and Flow readings. ⌘K opens it in every workspace
content mode; arrows highlight, Return selects, Escape dismisses. Rows show name,
kind, disambiguating Wave/issue/provider and applicable shortcut. Empty search
shows recent successfully visited destinations first, then the current inventory;
nonempty search ranks exact identifier/name, prefix, then substring with stable
ties. Keep at most 20 recent destination identities per window/repository; no
durable history store. Remove unavailable recent entries on a successful reading,
retain last-good rows visibly stale on failure. Sidebar search remains a filter.

Flow rows open template inspection; with an explicit Task context, **Choose Flow
for ISSUE** opens the existing selection/start or confirmed restart path. Merely
selecting a Flow never starts it. Action rows reuse shared legal actions and
existing handlers, revalidate on activation, and name their target. The initial
palette has existing actions; Bind appears only when the shared contract arrives.
Opening the palette takes keyboard focus from the terminal; Escape restores the
prior valid responder. Existing terminal shortcut monitors must defer while the
palette or bind confirmation owns input. No palette keystroke reaches a PTY.

### Template view without inventing execution

The shared loader already has `Flow.items` and `FlowRef`, and expands ambiguous
Skill references into sub-Flows using its own lookup rules. Preserve a resolved
composition tree during this **same traversal**; derive the execution list and
template projection from it. Do not add a Swift YAML loader or a second Rust
resolver with different skill-vs-Flow precedence. Keep captured invocation
execution independent of this ephemeral template tree.

Extend `FlowCatalogEntry` with a required-or-explicitly-optional template shape
and a content revision identifying the resolved sources. A template consists of
ordered Skill, Op, composition-group and Xor nodes, explicit children and return
edges. Give each composition use its own local template ID, even when two uses
have the same name. These are template disclosure keys, never runtime node IDs
or invocation parents. Preserve empty groups and all Xor alternatives. Missing
sources/cycles use the existing named unavailable result, never a partial graph
represented as executable. Update Rust/Swift/fixture consumers together.

`TaskFlowView` shows composed groups folded by default; disclose recursively with
keyboard and pointer. Reuse existing typography, graph drawing, connectors and
Flow picker. Folded groups summarize their contents and preserve incoming/outgoing
return edges at their boundaries; two return edges stay distinguishable. Expansion
state keys by repository, template revision and group ID, so a changed definition
does not silently reuse an old selection. Flattening the template must equal the
shared loader's execution semantics, including policy and return targets.

The Task presentation states are precise:

| Evidence | Display |
| --- | --- |
| No attributed Runs and no invocation | Folded Project Flow or explicit selection; no execution state |
| Invocation prepared, no Run yet | Captured plan, identified as prepared; no fabricated running node |
| Independent Runs but no invocation | Folded template with **Flow not started**; Task is already Started |
| Invocation with Runs | Fully expanded captured invocation, cursor, per-edge return counts and entered runtime children |
| Finished/stopped earlier invocation | Retained captured history, with a distinct action to choose the next Flow |
| Missing historical capture | Named unavailable history; never substitute today's template |

The independent-Run case resolves a literal gap in “until a Run exists”: a new
conversation or bound orphan starts its Task but creates no Flow invocation.
It cannot switch to a nonexistent graph. This is an explicit implementation
assumption, not a new claim of Jack's approval.

### Step attempts and Session membership

Jack's evening amendment defines a position value `(invocation, node, iteration)`
with several Run attempts and one current attempt. LOO-298 must expose this from
its invocation/Run writer and read projection. No new persistent “step position”
object is proposed. For each position, the shared read supplies ordered Run IDs,
an authoritative current Run ID (nullable when no attempt exists), ordinal when
known, timestamps/outcomes, and associated Session IDs. New reservation order
must be stable and serialized at that position; wall-clock sort or largest ID
does not establish the current attempt. Historical gaps stay unknown.

Three surfaces consume that projection:

- Node detail shows **attempt 2** and an expandable list of prior attempts, with
  outcome/provider/time and exact Run/Session links. Selecting an earlier attempt
  does not move the cursor or change which attempt is current.
- The running line joins the exact current Run at that position: skill,
  **attempt 2**, that Run's elapsed time and provider. It must not pick the first
  active Run for the Task or use the Task's updated timestamp.
- A Session chip reads its current Run's recorded position and ordinal. Clicking
  opens that exact invocation/node/iteration/Run, including earlier or completed
  invocations. A later same-named skill is never substituted.

A retry stays on the same position and increments the Run attempt. A loop return
changes the iteration tuple; a restart changes invocation identity. Runtime child
entry links to the parent entry and owns its local positions. A Session's Run
replacement retains Session ID/name/history but changes its current attempt;
old callbacks cannot overwrite it. Provider retry/usage streams **inside one Run**
remain separate from these position-level Run attempts (the current architecture
reference's retry/failover sentence concerns that inner level).

### Project Flow on the Wave page

The current participant (name unresolved) clarified in interactive review on
2026-09-26 that “wave flows” means the Project's default Flow for its Tasks.
Present that template on the Wave page through its current Project, using the
same folded/disclosable template UI. Project remains its owner; Tasks inherit
the default and may explicitly choose another Flow. This is not a Wave execution
surface and introduces no Wave default, Wave invocation inspector or Wave-level
start controls.

Task pages show their captured invocations and attempts. Invocations outside
Tasks remain in overall monitoring, including those attributed to a Wave; they
receive no Task-like page or standalone main-pane inspector. Flow catalog rows
still inspect templates and never start execution merely by selection.

Session presentation is distinct from invocation presentation. The existing
Session inventory and null-Task orphan rule remain unchanged by this correction;
outstanding review Sessions are not silently hidden or resolved. Their treatment
is an implementation assumption, not separate participant confirmation.

### Orphan room: membership and terminal ownership

Orphan means an unresolved Session whose **current Run has null Task**, including
Wave-only Runs. A bound Session absent from the current roadmap is still bound:
keep its Task identity and read its destination on demand. During failed reads,
retain the last-good classification with a stale warning. No cwd inference fills
missing ancestry and no load failure empties the room.

Keep the existing bottom section, collapsed by default, with independent chevron
disclosure and header navigation. Header opens `.controlRoom`; it no longer opens
the first Session. Give each repository a typed room workspace key, distinct from
checkout paths, inside its window's existing workspace registry. Cache grouping
and direct ID lookup on changed readings using the parent implementation; do not
reintroduce per-body grouping if the parent already removes it.

The room retains a pane for every orphan, with no six-pane or hidden overflow cap.
Initial layout recursively splits the set into balanced halves along the longer
axis. Preserve pane identity, focus, ratios and zoom on unchanged reads; insert a
new Session into the largest suitable leaf and reconcile departures without Close
undo. Narrow panes keep the name and Bind affordance usable; zoom provides working
space. Do not launch all unresolved/external providers on room entry. Retained
local terminals are displayed; others retain existing Open/Move here placeholders.

Only the active room or checkout host mounts terminal NSViews. Today's hidden
checkout view remains in a ZStack at opacity zero; adding another host beside it
would mount the same pooled NSView twice. Conditionally mount the active host and
retain layouts and pooled views outside it. Detaching releases input focus, never
the Ghostty surface. Before remount, retire the old representable's focus callback;
verify late layout/focus updates cannot steal the newly mounted view.

Resolve a Session to its existing `TerminalIdentity`, including `.shell(paneId)`
when a provider was launched in a companion shell. Never replace that shell with
a `.session` surface. Multiple Session records can attach to one shell: retain
all logical Session tiles, but mount that terminal in exactly one active tile and
show **Shared terminal · Show here** in the others. Moving the sole mount is
presentation only. Closing a room view never kills the shell or completes a
Session. This explicit rare-case treatment preserves the one-native-view rule
without dropping orphan records or launching duplicate clients.

Room strips reuse current dark terminal chrome: Session name, quiet cwd/provider,
shared state and **Bind to task…** at rest, even with one tile. No new palette,
sidebar restyle, pane border or duplicate split toolbar. Keep existing split/zoom
keys where applicable and ordinary Session actions in their existing owner.

### One bind interaction, three entry points

One `SessionBind` presentation/operation is called from the room, palette and
Task-page Session rows. Target context may preselect a Task, never authorize it.
Bound Session rows show their assignment and shared unavailable reason; they offer
no rebind. The picker includes unstarted and done/landed Tasks through the shared
Task reader. Wave-only Runs list only compatible Tasks (or display shared reasons).
The existing checkout-to-Task resolver may supply a suggestion for an old orphan;
Swift does not infer it from a path. A suggestion never writes ancestry.

Selecting a row previews the permanent target. One explicit action, e.g.
**Bind “Parser review” to LOO-303**, confirms repository, Wave and issue/title
with “This assignment cannot be changed.” The same screen includes a Wave target
when supported by the shared bind API. Wave-only binding does not remove an orphan.
Do not show a second CLI confirmation after the UI confirms. A row selection alone
is not confirmation and Return while searching must not accidentally submit.

The parent API must carry Session ID, expected current Run, exact typed target and
confirmation of that target; its transaction rechecks current Run, ancestry and
legality. The CLI's own confirmation uses the same operation. Exact flag names
follow the delivered API, not a separately invented Swift transport. No generic
`--yes` is sufficient if it lets a replacement Run or re-resolved label become the
recipient. A replacement/conflicting bind during the picker invalidates the
selection; show the current fact and require a new confirmation for a new intent.
A duplicate delivery of the same confirmed assignment returns its existing result.

The nullable invocation/Task invariant means some taskless Flow orphans cannot
independently bind to a Task. Keep them in the room and show Rust's reason; do not
hide them or invent kind-based legality in Swift. Current-Run-only binding leaves
prior Session Runs' attribution intact, as the parent design assumes.

On success, apply the authoritative Session result and refresh Task/Session reads.
Use request generations so a poll begun before bind cannot restore the old row.
Keep the terminal pool unchanged; update the room layout by membership only. An
open Session on a done Task still makes that Task reachable without reopening it.
While orphans remain, keep the room and focus the nearest remaining pane. If the
last orphan was bound here, navigate to that same Session under its Task only if
the originating repo/room is still selected. If the last orphan disappears through
completion elsewhere, show **No orphan sessions** and navigation out. A delayed
bind response after repository navigation updates its origin without moving the
current window. Unknown write outcome triggers readback, never optimistic success
or an automatic write to another target. Errors preserve the pane and draft.

## De-risking

Observations below are from this checkout, not the installed build or parent HEAD.

| Question | Finding | Impact on design |
| --- | --- | --- |
| Is the dependency ready? | `session.rs` contains Session/current Run and typed ancestry, but `SessionCommand` has no Bind or Task list filter; public `SessionRecord` still exposes Work/path, and the inherited design documents only Task-review conversion. | Early navigation/template work proceeds. Room/bind and attempt UI wait for full contracts, not table names. |
| Does the engine retain composition? | `engine/flow.rs::expand_with_chain` flattens nested items with `items.extend`; `flow_graph.rs` exposes only name arrays in `parents`. | Preserve boundaries at resolution. Two adjacent references to the same Flow have identical parent arrays; grouping those arrays cannot recover two uses. |
| Can the current wire show attempt 2? | `ops/task_flow.rs::PinnedTaskFlow` has graph/current/completed/returns, no position Run history; `session.rs::Run` lacks node/tuple/ordinal. | Request the shared attempt projection as part of the parent integration contract; no Swift counting heuristic. |
| Is the running line exact? | `TaskFlowView.runningProvider` picks the first active Run with Task Work; elapsed reads Task `updatedAt`. | Change both with exact attempt projection; independent helper Runs are a required counterexample. |
| Does any Run imply an invocation? | Architecture allows null invocation on Task Runs and bind changes ancestry only. | Started and Flow mode must be separate, including a bound orphan before first Flow. |
| Can two room hosts share a pool safely? | `SessionsView` keeps `WorktreeNodeView` mounted at opacity zero; `GhosttySurfacePool.view` returns the same NSView. | Conditional host mounting is required; retaining a pool alone is not sufficient. |
| Are all local Sessions `.session` terminals? | `SessionsStore.localTerminal` resolves recorded terminal IDs to `.shell`; `openSession` focuses that existing shell. | Room must support shell attachment and multiple records sharing one terminal. |
| Does current orphan grouping match the model? | `WorkspaceProjection.unmatchedSessions` means unmatched roadmap; Wave-only records are placed under Waves. | Replace classification with typed Task nullness after DTO cutover; bound historical Tasks stay reachable. |
| Is URL plumbing already present? | `Info.plist` registers `loopflow`; `LoopflowApp.handleDeepLink` handles open/portfolio/sessions; `open` selects a repo scene backed by `WavesView`. | Reuse registration, add Task parsing and a Podium-targeted request path. URL receipt alone is not navigation proof. |
| Is existing Work status enough to resolve an issue? | `commands/work.rs` parses typed IDs and projects only Work/placement/status; current roadmap omits historical Tasks. | Extend the existing roadmap projector with exact Task lookup; avoid the launch resolver. |
| Is broad perf work required here? | Assign-on-change and performance instruments exist; Session polling and per-access projection remain in this source. | Reuse instruments and parent's projection reduction; LOO-300 owns streaming and broader optimization. |
| Can this kickoff validate appearance? | This run has no rendering environment; no captures or native interaction were performed. | Implementation must gather mounted proof and installed captures later; old mocks do not supply acceptance. |

## Alternatives considered

| Approach | Mechanism and tradeoff | Decision |
| --- | --- | --- |
| Build the room now over Work strings and sidecars | Fast initial screens, but bind and correct orphan grouping still require the entire missing owner conversion; identity would change again | Blocked by the original problem; rejected |
| Wait for every parent table before any UX work | Avoids integration overlap but unnecessarily blocks keyboard navigation, links and template disclosure | Rejected; these reads already have owners |
| Independent navigation/template cuts, then consume final shared execution contracts | Delivers useful paths early; requires explicit identity/DTO integration proof before room/attempt UI | Chosen; one shared reader/writer per fact |

Wild success: Jack jumps directly into a Task, understands the folded plan, and
classifies a live conversation without thinking about Runs or terminal ownership.
When recovery is needed, attempt history explains it without pretending the step
restarted. Wild failure: retries look like new progress, a mistaken bind hits a
replacement Run, or a hidden host steals the draft. Exact identity and native
retention proofs below target those failures.

## Key decisions

- Preserve LOO-291's installed composition. No teardown, polish program or dark
  theme work. Jack's evening steer governs over the old title.
- Build palette and deep link first; template disclosure next with one shared
  resolution pass. Do not wait on unrelated parent migrations for these cuts.
- Bind legality/confirmation/current-Run fencing and attempt selection are shared
  contracts. Swift owns presentation and input state only.
- Distinguish a Task that has started from a Flow invocation that exists. Preserve
  independent conversation launch and completed invocation history.
- The Wave page presents its current Project's default Flow for Tasks. Only Task
  pages get invocation detail; non-Task invocations stay in overall monitoring.
  Session inventory and orphan membership remain unchanged by this decision.
- Keep one mounted native terminal per identity and window, with retained drafts
  across room/Task/repository navigation. Closing a view is not completion.

## Scope

- In scope: ⌘K; exact Task URLs and read-only resolution; folded template view;
  the current Project's default Flow template on the Wave page;
  captured invocation/child and attempt presentation; correct orphan section,
  room and universal bind; affected DTOs, focused tests and user docs.
- Out of scope: LOO-298's migration/driver/storage rewrite or PR-record repair;
  dark mode; current-surface teardown/polish; Wave-page launch expansion; first-run
  onboarding; automatic historical binding; rebind/unbind; Session streaming and
  broad performance work; provider authentication; a new Chapter UI; separate
  Task-like detail pages for non-Task Flow invocations.

## Done when

1. **Navigation:** a real dispatched ⌘K event opens the palette from a terminal;
   typed search never reaches either retained PTY; Escape restores the prior
   responder. Wave/Task/Session entries and repeated navigation resolve exact
   identities. A Task row goes to its page even with one Session. No launch or
   Started write occurs from inspection. Include stale data and two windows.
2. **Deep link:** `open 'loopflow://task/LOO-303'` reaches its Task page on cold
   launch and in an existing workspace. Test another repo, unstarted/completed/
   historical Task, duplicate identifiers, unknown identifier, malformed input,
   unavailable Home, and two links racing a click. Only the intended window moves.
   Shared local reader fixtures prove no active PR, process or provider requirement.
3. **Template:** repeated same-named compositions remain separate; recursive
   disclosure, empty composition, Xor paths, repeated skills and both return edges
   survive. Flattened projection equals shared expansion. Missing/cyclic source
   is named unavailable. Rust and Swift fixtures have no silent defaults. The
   Wave page reads its current Project's default, with no duplicate Wave setting
   or new execution controls; Task overrides remain independent.
4. **Invocation/attempt:** failed Run A then current Run B at the same position
   renders attempt 2 in node detail, running line and Session chip. Concurrent
   independent Run C cannot change the line. Old attempt selection stays exact;
   loop returns, child entry and restart preserve their distinct identities.
   Delete/change template sources after capture; retained history still renders.
   Unknown historical ordinal stays unknown. Non-Task invocations remain visible
   in overall monitoring and have no new main-pane invocation destination.
5. **Room/bind:** mounted production views with owned PTYs exercise parentless,
   Wave-only, bound-but-missing-roadmap and shell-attached Sessions,
   including more than six orphans and two records on one shell. Bind from all
   three entry points names/confirms the target once, preserves Session/surface/
   draft/membership, and reads Started from the server. Done Task stays done.
   Wave-only bind remains orphan. One mount per terminal, no automatic Move here.
6. **Races:** replace current Run while the picker is open; competing bind;
   delayed pre-bind poll; rejected/uncertain write; complete elsewhere; rename
   during bind; bind response after repo switch; close/Undo after membership loss.
   None redirects assignment, resurrects an orphan or steals focus. Previous
   Session Runs keep their ancestry and no invocation membership changes.
7. **Configured acceptance:** identify candidate binary/commit, selected Home/store
   and actual Session/Run/Task IDs. Capture 1440×900 and 1100×800 on live data;
   walk the demo with Jack on the installed app. Include retained draft and child
   reply after binding, external-client placeholder, and attempt 2. Fixture PTYs,
   HTML mocks, app compilation and screenshots without interaction are bounded
   evidence, not Jack's acceptance. Promotion follows the existing release path
   after the parent conversion; this kickoff does not perform it.
8. **Deletion/review:** remove superseded first-orphan navigation, roadmap-based
   orphan classification, duplicate bind UI, first-Task-Run status selection, and
   flat-only template preview when their replacements are live. Reuse the parent's
   deleted grouping/sidecar work rather than recreating it. Update `swift/README.md`
   and `docs/lf.md` with the actual routes, bind confirmation and Flow states.

For implementation, follow `TESTING.md` resource preflight/isolation/time limits.
Run focused behavioral proofs per cut; affected suites once at gate. Existing
anchors: `WorkspaceNavigationTests`, `WorkspaceNavigationProofTests`,
`TaskFlowTests`, `TaskFlowProofTests`, `SessionsStoreTests`, `WorktreeWorkspaceTests`,
`MultiplexerStoreTests`, `SessionRenameTests`, and Rust `dto_fixtures` integration
tests. Add focused Task-link/palette/room cases where those suites cannot express
the behavior. Rust changes require formatting and all-target Clippy; terminal
changes require SwiftPM proof plus the Xcode compile path. No full suite is
justified for this scratch-only design.

## Forbidden outcomes

- Calling a schema draft a usable bind contract, or a Task start an invocation.
- Counting active Runs, provider streams or Session replacements in Swift to
  invent a position's attempt number/current attempt.
- Reconstructing template composition by contiguous equal parent names or runtime
  parents; reloading current YAML to draw historical execution.
- Rebinding/clearing attribution, adding sidecars, guessing ancestry from cwd, or
  treating missing roadmap/read failures as orphan evidence.
- Two mounted hosts sharing one NSView; automatic provider launch/takeover of every
  room tile; releasing a surface because it leaves the room.
- A Task URL starting work, opening only a repo, entering the sole Session, or
  selecting every window through a global notification.
- Claiming visual or installed acceptance from this design or prior LOO-291 proof.

## Internal slices

1. **Keyboard navigation and links:** palette over current readings, shared typed
   destination dispatch, exact Task read and Podium URL delivery, focus retention.
   One coherent user path and independent of the new Session tables.
2. **Folded templates:** shared resolution retains composition; catalog DTO,
   disclosure UI on Task pages and for the current Project default on the Wave
   page; source/fixture equivalence proof. Independent of Run owners.
3. **Parent integration:** integrate final LOO-298 DTOs/operations, verify the
   contract checklist, remove overlapping obsolete code once. Extend its shared
   attempt projection if the delivered parent lacks the evening amendment; do not
   manufacture an interim Swift model. Parent incomplete means this slice waits;
   slices 1–2 remain useful work.
4. **Invocation and attempts:** current and retained execution, child invocations,
   exact Task attempt detail/status/chip. Source-independent
   recovery proof and independent-Run counterexample.
5. **Control room and universal bind:** correct grouping, one mounted host,
   unlimited tiles, one picker/confirmation path, post-bind reconciliation and
   races. Deliver room and working bind together, not a decorative dead-end room.
6. **Configured demo and deletion:** affected checks, installed live captures and
   Jack's walkthrough, then inspect/delete obsolete presentation and document the
   resulting experience. No new visual teardown.

## This slice

Repair return detail across folded composition boundaries, then finish dispatched
recursive keyboard disclosure. Node descriptions read the complete original graph;
layout and return endpoints keep using the folded projection. Prove both distinct
returns across folded/expanded/folded states at the root and inside an XOR path,
and preserve captured invocation detail and traversal counts.

Use one focusable disclosure button on each recursive group, including empty
groups and XOR alternatives. Dispatched Tab/Shift-Tab must reach the appropriate
label; Right/Left expand/collapse, Space/Return toggle. Inspection invokes no Flow
control. Hidden retained terminals must be ineligible for keyboard focus; both
surfaces and the exact draft/child reply survive returning to the Session.

The [focused receipt](workspace-return-proof.md) owns observations, failures and
executed evidence. Prior navigation/template receipts keep their original scope.
The complete eight Done When obligations remain authoritative, including installed
links, configured provider/draft proof, captures and Jack's verdict. Room/bind and
attempt work still require the explicit LOO-298 contract checklist. No publication
or Flow navigation decision is selected here.

## Slice ledger

- 2026-09-27 folded-return and keyboard repair: preserved the inherited draft at
  `502ba4601`, then reproduced root and XOR hidden-target labels in a mounted
  workspace. Detail reads the original graph recursively; projected layout and
  return endpoints stay unchanged. Shared recursive disclosure handles Tab,
  arrows, Space and Return with the existing expansion state. Dispatched Tab
  exposed hidden terminals accepting focus; AppKit eligibility now follows the
  existing enabled input without releasing surfaces or changing host ownership.
- Focused final Swift proof passes nine tests, including both distinct returns,
  root/XOR fold transitions, captured traversal counts, nested/empty disclosure,
  exact focus, unchanged PTY buffers, retained draft/child reply, palette, Monitor,
  click-focus and paste routing. Source review kept original/projection roles
  separate and added no Run/Session writer. The [receipt](workspace-return-proof.md)
  records failures, fixture key-loop correction and final compile evidence.
  All parent-contract and configured-acceptance obligations remain; no publication
  or Flow navigation decision.

- 2026-09-27 template review: actual CLI catalog checks and 16 focused Swift
  checks pass. Reproduced and repaired Started being presented as proof of
  independent Run membership; the status now says "No Flow recorded" while
  keeping the template. Five affected Flow checks and the extended dispatched
  failed-inventory Return proof pass, as does Xcode app/test-runner compilation.
  The [review matrix](workspace-template-review.md) retains parent-contract,
  installed/keyboard/visual acceptance and architecture gaps. No publication
  or Flow navigation decision.

- 2026-09-27 navigation repairs and template implementation: preserved the supplied
  concept review through `lf commit`, reproduced both navigation defects, and
  repaired current-result selection and bounded historical recents. Added repeated
  Session activation and mounted two-window/two-link race proofs. Retained the
  shared exact reader and single selected-detail owner.
- 2026-09-27 folded templates: retained composition in the existing resolver,
  added template-local IDs/revision to matching Rust/Swift catalog fixtures, and
  replaced flat previews with shared disclosure on Task, Wave and catalog surfaces.
  Both return edges survive folding; source changes invalidate disclosure. Fixed
  the chapter summary to expose the same effective Project Flow as Task launch.
  Captured execution and parent storage contracts are unchanged.
- Focused evidence: 49 Rust resolver/graph tests, 10 DTO tests, one actual CLI
  default-projection test and 16 Swift model/mounted tests pass; the final five
  template checks pass again after interaction changes. All-target Clippy,
  formatting, platform boundaries, whitespace and Xcode app/test-runner compilation
  pass. The inherited architecture
  owner-map gap remains 32/33. The [proof receipt](workspace-template-proof.md)
  owns commands, failures, build evidence and configured acceptance limits.


- 2026-09-26 navigation review: repaired exact Task lookup escaping a registered
  repository without Git metadata; the two-repository collision/completed-Task
  CLI regression and existing no-start inspection proof pass. Removed palette
  actions whose existing destination is unavailable. Eight focused Swift checks
  pass, including retained native input and Flow-inspection dismissal. The
  [review matrix](workspace-navigation-review.md) owns final checks, inherited
  architecture gap and remaining installed/multi-window acceptance. No publication
  or Flow navigation decision.
- 2026-09-26 navigation implementation: checkpointed the approved review notes
  through `lf commit` before edits. Added one exact Task filter to the roadmap
  reader, typed palette destinations, per-window Task-link delivery with cold
  pending retention, and generation-fenced destination lookup. Retained current
  and historical Task pages share the existing selected evidence; no new planning
  or Session store, provider launch, ancestry write or parent conversion.
- Source review removed an initially redundant historical-Task cache and combined
  palette/Flow inspection into one modal presentation. Fixed empty-current-plan
  routing and historical breadcrumbs. Missing owning Project is an explicit
  error; missing cached planning remains a named retained Task warning instead of
  a fabricated template. Session grouping and room membership remain unchanged.
- Verification: real isolated CLI lookup passes; all-target Clippy, formatting,
  whitespace and Swift platform boundaries pass. Thirty focused Swift tests pass,
  including the production keyboard/PTY proof and existing navigation behaviors.
  The Xcode app/test-runner compile passes. Commands, initial counterexamples and
  exact proof limits are recorded in [navigation evidence](workspace-navigation-proof.md).
  No installed activation, external writes, timing claims or human acceptance.
- Final dispatched-key review found the search editor swallowing arrow navigation.
  Handling the editor's AppKit commands repairs it; seven destination/native
  checks pass on the final input implementation, including exact Wave selection
  after Down/Return and retained Task-page/terminal behavior.

- 2026-09-26 interactive review: the participant approved the design after
  clarifying that the Wave Flow UI presents the current Project's default
  template for Tasks. Removed the standalone non-Task invocation inspector and
  the agent's mistaken Wave execution UI. Non-Task invocations stay in overall
  monitoring. Full proof and parent-integration requirements remain unchanged.
- 2026-09-26: read inherited design/decisions, repository conventions, Product
  objective/memory and current architecture spec. Inspected the source named in
  De-risking at the supplied base. Preserved inherited LOO-298 notes unchanged.
- Incorporated Jack's evening steer: positive installed-build report, no teardown,
  light only, independent work first, Task URL and Run-attempt presentation.
- Source review identified composition-boundary loss, hidden mounted terminals,
  shell attachment, old repo-window routing, missing attempt projection and the
  independent-Run/no-invocation case. The design addresses each explicitly.
- This run performed read-only source inspection and authored scratch Markdown.
  No product tests, live provider read, parent-status read, native rendering,
  installed modification or external Task/PR mutation was performed.
- Simulated code review checked ownership and the failure cases against the
  complete design. It kept Task page navigation distinct from single-Session
  drill-down, rejected name-based composition grouping, and required one mount
  for shell-backed conversations. No generic action registry or second Session
  inventory is needed. The proposed exact Task filter must reuse the roadmap
  projector rather than create a second Task detail shape.
- Documentation verification: working-diff whitespace passes. The first combined
  link probe found an inherited reference to `data-model-slice-review.md`, which
  is not among this Task's copied artifacts. That missing evidence remains
  explicit; it is not reconstructed or counted as validation. New design links
  and the new agenda link are checked separately. Product tests are unnecessary
  for these scratch-only changes.

## Navigation compression (2026-09-26)

Reviewed `abfa364b0` against the active base `c832aaede`, following the complete
approved design through the roadmap CLI, RegistryQuery, window routing, navigation,
palette, historical Task display, Session grouping, terminal ownership and tests.
Jack's light-only/no-teardown scope remains unchanged.

| Fact | Owner | Reduction or reason to retain |
| --- | --- | --- |
| Exact Task identity and planning | Existing shared roadmap projector and durable Task/Project records | Keep `roadmap --task` as a filter on that reader, with unavailable evidence distinct from no match. No new DTO or persistence owner. |
| Selected historical Task | `WorkspaceNavigation.selectedTaskEvidence` | Retain outside the current plan so refresh cannot eject an inspected historical page. Breadcrumb and details consume the same evidence. |
| Palette sheet content | `WorkspaceNavigation.palette` | Replaced `palettePresented` plus `inspectedFlow` with one optional enum: search or Flow inspection. Sheet rendering, dismissal and shortcut exclusion read it directly. |
| Task-link request and reading | `PodiumModel`, fenced by destination generation | Keep asynchronous resolution and its error/ambiguity presentation separate from palette search; late results cannot replace a later click. |
| Window delivery | `WorkspaceLinkRouter`, weak native windows and one pending URL | Keep pending cold delivery and per-window targeting; no global selection broadcast. |
| Focus and terminal lifetime | Palette coordinator's weak responder, window workspace registry's surfaces | Keep focus restoration independent of navigation selection. The sheet never owns or releases a PTY. |

Removed both old palette fields and migrated all readers/writers without aliases.
Terminal shortcuts now defer throughout search and Flow inspection. Source review
checked the search-to-inspection transition and dismissal as one modal lifetime.
Extended the existing native keyboard proof to select a catalog Flow, retain the
same sheet, dismiss once and preserve both surfaces, alongside its existing
search isolation, restored responder, draft/child reply and Task-detail checks.

The remaining `WorkspaceProjection` grouping, reverse Session lookup, current
Work/path DTOs, flat Flow preview and Task-level running-line selection still have
live consumers. Their approved replacements depend on template resolution or the
parent's all-kind Session/bind/attempt contracts. Current `SessionCommand` still
has no bind, and `Run` lacks position/attempt projection; deleting those consumers
now would remove capability. No parent storage/migration or public wire changes
were made. User documentation still describes the same routes.

Focused native proof passes: `SessionChromeProofTests/paletteRetainsTerminalInput`
(one test, 4.6 seconds; 44.5 seconds including build). It uses fixture transport
and owned cat PTYs, with Ghostty's timer-renderer fallback; it is not configured
provider, installed-app or compositor proof. Resource preflight passed at 89.5
GiB free. Swift platform boundaries and working-diff whitespace pass. Logs are
under `.lf/tmp/workspace-navigation/compress-*`. XcodeGen and signed ad-hoc Xcode
`build-for-testing` pass (42.0 seconds); hosted UI tests were not run. No broader
behavioral suite was repeated; unchanged
Rust/navigation receipts retain only their prior scope. All full-design acceptance
and parent-integration obligations remain open; no publication or navigation edge
is selected.

## Template compression (2026-09-27)

Reviewed `3f84174d0` from a clean tree against the active base `c832aaede`,
following navigation, exact roadmap lookup, shared template resolution, capture,
catalog DTOs, Swift presentation, fixtures and user docs. Jack's approved
light-only/no-teardown scope remains unchanged.

| Fact | Owner | Reduction or retention |
| --- | --- | --- |
| Resolved composition and content revision | Rust's existing Flow resolution traversal | Keep distinct composition uses and the one flattening implementation. Captured `ConcreteStep` remains the execution input; a template tree is not an invocation. |
| Template node semantics and topology | Catalog `FlowGraph`; template items reference its keys | Keep graph plus disclosure structure: the tree adds boundaries and empty groups without copying node policy. No wire or persisted shape changed. |
| Visible template diagram and group activation | Shared Swift `TemplateDiagram` | Root and XOR paths now use one projection/render/click-to-expand path instead of two copies. |
| Projection inputs | `FlowTemplateProjection(graph:items:expanded:)` | Removed `FlowTemplate.project` and the public construction helper used to fabricate an empty revision for a branch. Projection requires items, not a catalog revision. All callers migrated; no adapter remains. |
| Disclosure and selected node | Repository navigation owns expanded group IDs by revision; each diagram owns its inspected node | Retain these different lifetimes. The revision-keyed subtree resets local inspection; the redundant root inspection state and explicit reset hook are removed. |
| Historical destination and selected details | Bounded recent descriptors and `selectedTaskEvidence`, respectively | Keep both: one permits revisiting; the other owns the selected detail. Exact readback/generation fencing remains separate from terminal lifetime. |

Simulated source review checked the shared diagram at root and XOR callers,
revision-driven state replacement, return-edge remapping and every removed API's
caller. The existing five-test Flow filter passes: shared fixtures retain repeated
and empty groups, XOR, both returns and full-expansion equivalence; the mounted
production proof retains Task/Wave disclosure, revision reset, controls, captured
execution and the two owned terminals' draft/companion behavior. Tests now construct
the projection directly; no capability assertion was deleted. User documentation
still describes the same behavior and needs no wording change.

Resource preflight passes (94.5 GiB free). The isolated, nice +10, four-worker,
serial Swift command has a 900-second process-group limit and completes in
35.8 seconds (7.1 seconds executing). Swift platform boundaries and working-diff
whitespace pass. Logs: `.lf/tmp/workspace-navigation/compress-template-*`.
XcodeGen and signed ad-hoc Xcode `build-for-testing` pass (47.8 seconds for the
build), compiling the app and runners through the terminal fallback configuration.
Hosted UI tests did not run. The mounted Swift proof uses fixture transport and
owned cat PTYs, not configured providers or installed acceptance.

Intentionally retained the Session union/Work DTOs, unmatched-roadmap grouping,
reverse lookup, hidden checkout host and Task-based running line. Source still
has no Bind command or authoritative position/attempt projection. Removing these
consumers before the parent conversion would remove capability. No persistence,
Rust resolver, wire fixture, navigation or provider ownership changed; previous
receipts retain only their recorded scope. The inherited `wave_chapters` owner-map
gap, parent checklist, installed cold/warm links, configured provider/bind proof,
captures and Jack's verdict remain open. No publication or Flow navigation decision.

## Measure

Reuse existing `hierarchy_interaction_ms` and `task_workspace_ready_ms`; keep them
separate. Add scenario labels for palette selection, Task URL destination, room
entry and post-bind retained Session. Measure accepted input to the correct usable
destination, including focus/identity; bind storage round-trip is a separate span.
Use the existing recorder/harness, no telemetry or new product instrumentation UI.

Proposed UX targets from the inherited study: p95 ≤100 ms for warm navigation and
retained Session switches, ≤50 ms palette filtering. These are design goals, not
chapter commitments or measured results. Collect at least 20 comparable samples
per scenario before reporting p95, at the existing small/large Task populations
plus 1, 4 and 12 orphan tiles. Record hardware/build/Home/population, failures,
unstarted attempts, cold/warm distinction and observer overhead. All orphans get
tiles; resource measurements do not authorize a cap. A native capture/input
endpoint does not prove compositor hitches or provider startup latency.
