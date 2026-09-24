# One workspace for work and Sessions

## Latest human direction — 2026-09-24

**Latest amendment: one Wave, one current chapter Project.** The human directed
this work to reconcile with `loopflow.projects` to remove organizational clutter.
The public outline is now **repo → Wave → Task → Session**. Project is an internal
chapter record, never another row, picker, breadcrumb or replacement Chapter tier.
Wave owns the enduring objective; its current Project owns Tasks, KRs and metric
targets, presented together through Wave details. This supersedes the separate
Project level in all older proposals below, including Full presentation.

The sibling already has shared Rust/Swift roadmap fields `chapter`, direct `tasks`
and `unavailable_tasks`. Integrate that contract with this branch's retained
outline and multiplexer; do not copy its older navigation or hide a Project array
only in Swift. Preserve Task identity and Session/Run links across chapter moves,
including historical and unmatched Sessions. Read-only comparison and the remaining
integration proof are in [the reconciliation review](review-wave-reconciliation.md).
The two performance experiences and one optimization Task per area remain in scope.

The human finds the current app confusing and wants to strip the interface back,
then design elements together one by one. This is not a customizable widget
canvas. They requested performance testing and one first-pass optimization Task
per relevant area, preceded by reviewing the previous Product chapter's KRs and
planned work. [Canvas chapter review](canvas-chapter-review.md) records that review,
the selected hierarchy and its implementation boundary. This direction
supersedes continuing the current full-interface demo as the next design step;
it does not erase the implementation, evidence, or historical KR verdicts below.

The human then selected the first element: **one repo → Wave → Project → Task →
Session hierarchy**, appearing in one place and able to compress away unnecessary
levels, including a flat list of Sessions and singleton Project levels. This
supersedes the earlier ban on Session children and the competing Work/Session
navigation presentations. The linked canvas design records the compression
rules and remaining interaction choices. Shared identity and terminal ownership
remain unchanged by presentation compression.

The later **Launch decision — LOO-291** in that document now governs the next
slice: implement the native outline, Task Monitor in the existing multiplexer,
and both repeatable performance measurements together here. Compact first, one
presentation menu, and initial Task Monitor content are implementation defaults;
they do not require another design approval. Earlier A/D, cardinality and study-only
next-step proposals below are historical where they conflict with that decision.

Within a selected Task, the human wants two content views: **Sessions** restores
the existing embedded Ghostty workspace; **Monitor** initially shows only that
Task's current active Runs. Run history, throughput and richer monitoring follow
later. Session leaves open Sessions at their exact identity. Both views share
the same Task selection and preserve terminals when switching. Reconcile later
Watch features with LOO-293 rather than adding another competing Task destination.

Further clarification: Sessions and Monitor belong in the **same existing
multiplexer**, and may be visible together. A switch is acceptable initially
but must select/reveal retained pane content, not establish independent workspace
layouts. Extend the existing pane-content representation for a Task-bound Monitor
and retain the existing split/focus/zoom and native surface owners.

Define performance measurement up front for two high-level experiences:
**hierarchy navigation** and **Task workspace opening/switching**. Implement
repeatable scenarios and rendered/usable endpoints alongside the first native
increment, retain baselines and regression reports, and plan one simple
first-pass optimization Task for each. The canvas design contains the measurement
contract; instrumentation and baseline results are not yet established.

Task handoff — 2026-09-23. The human requested attaching this design to existing LOO-291 and advancing the Task. The agreed direction is compact Wave/Project grouping, both A/D presentations, and optional bounded conversations. Implementation choices called provisional below may be resolved within that direction; the external proving workflow remains human-selected.

## What to build

Make one workspace show open and upcoming Tasks across Waves and Projects, with optional short-lived conversations at repo/Wave/Project/Task scope and full-width or list-plus-work presentations.

User anchors:

> “a working-enough sessions view that i can use to manage my Loopflow”
>
> “see all my open / upcomoing work (across waves and projects)”
>
> “tasks that are running autonomously and those with open sessions are on the same footing”

> “Expandable Wave → Project → Task tree”

> “But the wave/project layers should be pretty minimal/collapsible so that its relaly just one annotated well organized list”

> “make lots of differetn mock ups and ask me what i like”

> “to be clear the option to have a conversation. i think we want to start them over mostly and encourage not just keeping one forever”

## Visual exploration — current work

[Interactive mockups](main-view-variations.html) · [Comparison sheet](main-view-comparison.png).

Eight alternatives share sample Tasks and compact Wave/Project grouping. They vary navigation rather than colors:

| Version | Opening a Task | Returning to the list |
| --- | --- | --- |
| A · List → focus | Uses the full content area | Back to retained list position |
| B · Unfold in place | Expands within its row | Collapse the workspace |
| C · Pull-up workspace | Opens over the lower list | Dismiss the drawer |
| D · Split on demand | Temporarily narrows the list alongside work | Close the task to restore full list width |
| E · Task sheet | Floats above the overview | Dismiss the sheet |
| F · Overview + tabs | Retains a workspace tab per visited Task | Select All work |
| G · Summon the list | Makes selected work primary | Open a temporary list overlay |
| H · Zoom into a group | Opens inline, compressing other Wave groups | Expand another group or return to overview |

All are disposable interaction studies with simulated terminal content, not native-app or live-Session proof. A session-bearing Task opens its sample Session; autonomous/upcoming Tasks open work details. This direct-entry behavior is another proposal to evaluate. The mockups predate the one-current-Session-per-subject direction. Per-level Session entry, real split panes, unavailable evidence, and the final distinction between details and Session selection need the next round.

The human's first-round feedback: “I like A and D the best,” followed by “I think yeah it should probably be possibel to get both views.” The direction is to make both presentations available for the same Task. [Round two](main-view-round-two.html) compares A and D with I: explicitly show/hide the work list without leaving that Task. The mockup starts full-width; that default is provisional. The toggle choice remains while switching Tasks during the visit. Ask which presentation should open first; neither this preference nor its persistence across app launches has been selected.

## Placement

Product → Desktop → [LOO-291](https://linear.app/loopflow/issue/LOO-291/join-current-projectkrtask-planning-to-native-sessions).

The local roadmap identifies this existing Task. Its planning-to-Session navigation requirement fits; this draft makes unified work visibility explicit. The design has been attached to LOO-291 in Linear; its original Task requirements remain included. LOO-251 owns remaining native Sessions proof; LOO-284 owns shared Session actions and Work labels. Consume that shared contract rather than duplicate it.

## Integration with `loopflow.lf-new`

Read on 2026-09-23: [worktree-workspace design](lf-new-context/lf-new.md), [registration counterexample](lf-new-context/session-registration.md), and [two-level layout mockup](../../loopflow.lf-new/scratch/worktree-layout.html). These are uncommitted design artifacts, not shipped behavior; the written design is newer than the mockup's worktree-only New work wording.

- The work navigator selects a subject; opening its workspace restores the checkout's conversation **and companion shells**, including layout, processes, output, and focus. A/D controls navigator visibility around that workspace, not around an isolated terminal. Both presentations can contain the two layout levels from `lf-new`: outer worktree slots and inner terminal splits. Hiding the navigator changes neither tree.
- `lf-new` owns the proposed per-worktree grouping, nested splits, free-shell behavior, configured launch destination, and live terminal attachment. Integrate those surfaces here rather than preserve the current repository-wide multiplexer as the target or build competing workspace state. Focus an already-visible checkout instead of mounting a native surface twice.
- Its latest creation proposal is New task → create a tracked Task under an existing Project → prepare the owned checkout → launch bound design using configured app/terminal. That avoids making arbitrary repo-session adoption the default creation path. No autonomous controller starts merely because design opens. Project choice at repository scope is still unresolved.
- Repo/Wave/Project conversations remain useful independently of creating implementation Tasks. One current conversation per subject does not prohibit companion shells, servers, or commands. Several subjects may share the main checkout; checkout identity alone must not collapse their distinct conversations.
- Existing arbitrary shell-launched agents and unbound checkouts must remain reachable. How additional such agents relate to one *current* conversation per subject needs explicit reconciliation; do not enforce the new model by hiding clients or stopping them.
- A supplied screenshot in `lf-new` shows embedded agents labeled ELSEWHERE because the app knows a shell surface but cannot associate its subsequently launched Session. Group by checkout, but identify the live client/terminal independently. Selecting that Session must focus its existing shell, never use Move here to stop the client already in front of the human.

The next mockup should combine compact Work navigation, per-level Session actions, and checkout workspaces with companion shells. Current A/D mockups establish navigation only; they do not yet demonstrate this integrated workspace.

## The demo

Open Loopflow on the human's repository. In one navigator, see an autonomous Task, a Task with an open human Session, and an upcoming Task with no runtime. Each has its title, Wave/Project context, and recorded condition. None requires switching between Work and Sessions.

Select the human Session and continue in its retained terminal. Inspect the autonomous Task's directive, Activity and PR without creating a Session. Inspect the upcoming Task's directive and Project definition/KRs. Return to the original Session with its pane, draft, and split layout intact. Completing that Session updates its Task row; it does not complete or remove the Task.

Loopflow is the requested initial example. The existing Task's external-work proof still needs a human-selected workflow; self-hosting evidence does not satisfy it.

## Interactions

The work list is the overview. An opened Task supports both full-width workspace (A) and list-plus-workspace (D), with an explicit show/hide work list control. Showing the list is optional. Switching presentations preserves selected Work, Session, checkout workspace, both split levels, companion shells, terminal state, draft, and each view's scroll position. The control works identically for autonomous and upcoming Task details. It never opens or closes a provider client. Preserve native Session continuation and explicit Move here. The first-open default and whether the preference survives app relaunch remain open.

The main navigator is one organized task list across the selected repository, grouped by Wave and Project. Those parent layers are compact collapsible headings, not prominent navigation rows or cards. Use restrained typography, tight spacing, and minimal indentation so Task titles share a readable alignment. Show Wave/Project context once per group rather than repeating breadcrumbs on every Task. Default membership is every incomplete Task, including those with no Work record or Session. Preserve Project rank within each Project and stable parent ordering. Do not move rows merely because execution starts or a Session opens.

Disclosure on a group heading expands or collapses it; its label can open Wave or Project details. Task selection opens Task details; their placement remains open. Neither action launches a provider or destroys retained terminal state. Keep expansion and selection per window/repository across refreshes and repository switches. Start with Wave/Project groups expanded, so Tasks dominate the list. Each heading or Task row has a compact action for its own Session; Sessions do not add a fourth nested list. Collapsed headings retain a compact Task count and Session indicator; expanded headings avoid extra metrics and controls. Unavailable evidence is never counted as zero. Search reveals matching Tasks under their compact group headings and restores prior expansion when cleared.

Each subject has one stable row or heading and at most one current user Session. Show execution evidence and Session presence separately: an open Session does not prove autonomous progress. A completed Task with a current Session remains reachable with completion explicit. Missing planning stays visible and does not discard the Session.

Wave and Project context opens their definitions and KRs. A Task opens its directive, condition/reason, Activity, PR/worktree references, and Sessions; zero Sessions is explicit. Preserve existing lifecycle actions through shared APIs. New bulk controls, automatic prioritization and shared-PTY work are outside this change.

## Data structures and key functions

### Optional conversations, one current at a time

User direction:

> “open a session with either the repo, wave, or proejct, but much like wiht a task you really only have 1 such session at a time, you only have the "one" session per level”

Each individual repo, Wave, Project, and Task offers an optional conversation, with at most one current user conversation at a time. Encourage bounded conversations and fresh starts. The durable subject persists independently of its conversations. The repo is a Session scope, not a new durable Work kind.

- Put Start conversation on the repo header, compact Wave/Project headings, and Task rows when no conversation is current. It starts fresh rather than automatically resuming the last historical conversation. Merely selecting a subject does not start a conversation.
- While a conversation is current, offer Continue and an explicit Start fresh. Continue returns to that exact conversation; Start fresh ends the prior ordinary conversation and begins a new one with fresh conversational context. Preserve prior history for deliberate access. Do not automatically replay old transcripts into fresh conversations.
- Finish conversation leaves the subject with no current conversation. It does not complete the Task, stop its autonomous work, remove its checkout, or close companion shells. A later Start conversation begins anew. Labels must distinguish conversation completion from Task completion.
- Repeated Continue actions across views/windows converge on the same Session identity. Starting fresh is an explicit lifecycle change, never a side effect of navigating. Client ownership still uses explicit Move here.
- The chosen Session can use A or D. Changing layout, hiding its view, or collapsing its parent does not finish it. Historical conversations are history rather than sibling current-session rows.
- Autonomous Runs remain independent execution evidence. Their presence does not consume or replace the subject's human Session.
- Repository-level exploration has a home before a Task exists. A repo Session can concern work in a checkout without that path becoming a Task or another hierarchy level. Do not infer Task ownership from cwd.

This supersedes the proposed standalone-Session/worktree inventory as the primary navigator. Do not add an “unfiled” queue or multiple current Session children under each subject. Worktrees own terminal grouping as designed in `lf-new`; they are not a fourth planning kind. Prefer that design's Task-first creation path for new implementation work. Existing unbound work still needs access and a non-destructive path into tracked work; do not silently create a second current repo conversation.

Existing Rust/Swift Session records already carry optional typed Work attribution, cwd, and stable Session identity; they do not yet establish this cardinality or a repo-session association. Shared start/continue/finish operations must own that relationship, not a Swift-only filter that hides extra live Sessions. Before implementation, reconcile existing multiple Sessions and Ask/FlowStep human boundaries: preserve pending decisions and history without silently closing, replacing, or hiding them. Optional conversation controls must not silently approve, iterate, or release a mandatory human boundary. Exact replacement/recovery semantics remain to be designed. No migration or API support is claimed by this sketch.

Next mockup: a repo Session, a Wave Session, a Project Session, and Task-owned autonomous/human work, all opened through their corresponding header/row. The separate Sessions-only window may become unnecessary once these paths have equivalent reachability.

### Existing shared values

Retain `RoadmapSnapshot`, `RoadmapTask`, `SessionRecord`, `WorkReference`, and `PodiumReading` as source values. `WorkspaceProjection` preserves Wave/Project nesting and each subject's optional current Session; a view-only `WorkspaceTask` carries one planning Task and its optional current Session. Its key uses repository plus planning Task id, so starting runtime does not change identity. Session identity remains stable independently of view placement. Expansion is a view-local set of typed node keys, separate from terminal layout and Work selection.

Proposed Swift seams:

- `projectWorkspace(roadmap: RoadmapSnapshot, sessions: [SessionRecord]) -> WorkspaceProjection`: associate by typed identity; retain unmatched Sessions explicitly.
- `selectWork(_ selection: WorkReference?)`: inspect without opening a provider.
- Proposed `startConversation(for: SessionTarget)`, `continueConversation(id:)`, and `finishConversation(id:)`: explicit shared lifecycle by repository or typed Work identity. Fresh replacement is a deliberate transition; exact backend API and recovery remain to be designed.
- Existing `SessionsStore.select(_:)`: open/focus the returned Session through its shared preparation path.

Planning Task ids and durable `runtime.workId` differ. Use the explicit shared relationship, never titles, branch names, or cwd guesses. Session identity remains its own id. Rust owns legal actions; Swift owns selection, filters, pane focus, and opening errors.

## Current system and integration

`PodiumView` switches between `.sessions` and `.work`. `WorkSurfaceView` uses `nowSections`, which omits clear conditions. `SessionsView` lists Sessions and separately reads the roadmap to enrich labels. `PodiumModel` already reads planning and Sessions. `SessionsWorkspaceRegistry` retains per-window/per-repository multiplexer and surfaces.

Replace the root switch and parallel navigation with the unified tree, including the competing cascading hierarchy in `PodiumConsole`. Reuse work inspectors and Activity. Integrate `lf-new`'s per-window/per-worktree surface ownership and outer worktree layout; do not retain repository-wide grouping as the target or implement a second workspace registry here. Consolidate shared reading ownership; delete the Sessions-only hierarchy lookup and obsolete Work/Sessions return buttons. Scope changes cannot reconcile panes against another repository's Session list. Failed reads retain last-good evidence with visible freshness/errors; unavailable Sessions must never appear as confirmed zero Sessions. Session interaction must not wait for a slow planning query.

## Scope and internal slices

Additive series. The keystone is an indivisible navigation replacement shipping as one PR; its internal slices are:

1. Join shared Work/Session readings into the compact unified list and A/D workspace presentations; keep autonomous and upcoming Tasks visible, integrate existing detail/Session access, preserve live terminals, and prove the behavior. Reconcile current main with `lf-new` and LOO-284 before selecting shared API changes; do not build parallel ownership or lifecycle logic.
2. **This slice:** establish one repeatable native measurement command for the reconciled repo → Wave → Task → Session outline and retained Task workspace. Use fixed small/large shared DTO populations, three owned PTYs, actual captured pixels and retained focus/input checks; journal every started attempt and preserve failures, timeouts and source drift. Provide per-scenario first/warm samples and like-for-like comparison reports before choosing an optimization. The SwiftPM host did not expose SwiftUI accessibility descendants, so this increment measures forced native bitmap capture plus text verification and PTY response, with observer overhead explicit. It is not compositor presentation or frame-hitch evidence. Production correlated phases, scrolling during refresh, configured registry/provider costs, rendered/usable regression budgets and human acceptance remain the full measurement contract. Preserve the integrated chapter/direct-Task ownership and keep bounded active discovery/shared refresh in this core.
3. Remove superseded navigation, integrate remaining inspectors, and prove configured behavior. No runtime/cardinality or terminal-ownership shortcuts count as completion.

Follow-up: Task-directive editing through the existing PM API, preserving unsaved text on rejected writes and displaying authoritative refreshed text after success. This remains part of LOO-291's requested outcome; shipping the keystone alone does not complete that Task. External-work trials and long-lived-registry performance measurement remain its full proof obligations. Do not create follow-up Tasks during design.

## Done when / forbidden outcomes

Focused fixtures prove stable identity and visibility for autonomous, interactive, upcoming, repo/Wave/Project Sessions, repeated-open identity, completed-with-open-Session, and unavailable-source cases. Configured UI proof demonstrates the demo, expansion surviving refresh/scope switches, and retained split/focus/draft behavior while navigating the tree. Capture paint and Session-readiness timings before and after on the same population; do not invent thresholds.

No separate authorities for Work/Sessions inventories, oversized parent cards, cascading navigation columns, deep Task indentation, duplicate Task rows, session-required visibility, Task-required Session visibility, forced taskification, implicit provider launch on Task selection, inferred lifecycle legality, terminal reset during inspection, or healthy-empty rendering after failed reads. A separate Sessions-only window may be removed only once standalone, top-level, and pre-Task work remain reachable through the unified workspace. Showing a selected Session in its own content view is compatible with one unified work inventory; simultaneous list/terminal visibility is not required by the accepted direction.

## Evidence ledger

- 2026-09-23: local `lf roadmap --json` found LOO-291 and Desktop's definition/KRs; this was a cached planning read, not a fresh Linear sync.
- Source inspection confirms separate root surfaces, clear-condition omission in Now, and retained per-window/repository Sessions workspaces. No UI behavior or performance has been tested in this design pass.
- 2026-09-23: the human selected Wave → Project → Task hierarchy, then clarified that Wave/Project layers must be minimal and collapsible so the result reads as one annotated, organized list. Initial expansion, collapsed counts, and search behavior above are proposed details.
- 2026-09-23: the human questioned “beside your terminals.” Persistent side-by-side list/terminal placement is an unaccepted assistant assumption, now removed from the requirements.
- 2026-09-23: built eight interactive HTML studies and visually checked the comparison capture through `lf screenshot`. These are layout/interaction evidence only, not proof of native terminal retention or current Work state.
- 2026-09-23: the human favored A and D and then indicated both views should be available. Added a focused second round with both originals and a show/hide-list hybrid. Default presentation remains undecided.
- 2026-09-23: the human raised worktrees/Sessions before taskification and top-level agent Sessions. The Task-only sample population is insufficient. Shared Session types already allow absent or non-Task attribution; new-worktree membership, taskification/adoption, and external vendor-session discovery remain unproven.
- 2026-09-23: the human proposed one current Session per repo/Wave/Project/Task. This replaces the separate standalone/worktree-list proposal. Shared identity/cardinality and reconciliation with existing human boundaries remain design work.
- 2026-09-23: inspected `loopflow.lf-new` at the human's request. Incorporated its Task-first creation proposal, nested workspace/terminal layouts, and shell-launched Session registration counterexample. No files in that checkout were changed; no implementation or live proof is inferred from its draft/mockup.
- 2026-09-23: the human clarified conversations are optional and fresh starts should be encouraged. One-current-at-a-time is a cardinality constraint, not a permanent conversation per subject. Start/Continue/Finish/Start fresh above are proposed controls for that intent.
- 2026-09-23: the human requested advancing the Task, then clarified that the pre-identified Task should receive this design. Reuse LOO-291 rather than create a duplicate. Earlier queue timing does not block this explicit advance; external-work proof remains unselected and must not be fabricated.
- Open: per-subject Session lifecycle, checkout choice/taskification, first-open presentation, repository breadth, external proving workflow, and the directive-editing follow-up.

## Slice ledger — native capture/input measurements, 2026-09-24

[Desktop measurement runner](desktop-performance.md) adds one opt-in native
command for the current outline and existing Task multiplexer. Fixed 8/256-Task
populations retain three real PTYs while exercising eleven scenarios. The final
source-stable baseline records 462/462 passing observations; three report tests
cover incomplete attempts, percentile eligibility and comparison boundaries.
Per-attempt capture/verification costs, failures and source drift remain explicit.
The capture/input endpoint is not compositor presentation; frame hitches,
scroll/refresh, production phases, configured costs and measured budgets remain
open. Five-column empty-state clipping remains a recorded composition limitation.
No optimization, provider interaction, publication or Task completion is claimed.

## Slice ledger — Wave-only integration, 2026-09-24

[Wave integration](wave-integration.md) brings committed chapter binding/direct-Task
reads into this branch through a local Loopflow rebase. Removed Project outline
nodes, ordinary Project conversation scope and inspector; retained chapter history,
full Wave Task inspection, unknown-work evidence and exact Session/Run attribution.
Historical Project-bound Sessions resolve through shared Wave ancestry. Each
repository/window retains its selected Task evidence during transfer. A mounted
native proof changes chapter membership while the original Monitor, Session draft
and companion PTY remain usable. The 62-test focused Swift pass, Rust DTO/ancestry
receipts and static analysis are recorded in the integration report. The sibling's
uncommitted metric-ownership correction is excluded; no live migration or installation.
Both performance experiences, bounded discovery and configured/human acceptance
remain full-Task work. Concurrent performance code is preserved separately.

## Slice ledger — Task Monitor panes, 2026-09-24

[Task Monitor implementation](task-monitor-implementation.md) adds a retained
Task-bound Monitor to the existing multiplexer and restores each Task's pane
choice. One Podium demand reading supplies exact Run attribution and visible
stale/incomplete evidence. Six focused tests pass, including both real direct
Session and shell-attached PTY draft/focus/companion proofs. Saved-pane content
validation and a native Monitor focus target correct reproduced failures.
Fallback compilation is blocked at resource preflight; no compiler verdict.

Continuous refresh remains blocked on bounded native discovery. Both experience
runners/baselines, configured trials and the simplified human demo remain core
requirements. The observation time and explicit Refresh action describe this
usable first increment without claiming live polling or performance completion.

## Slice ledger — required Session → Run, 2026-09-24

Rust and Swift now require the explicit Run reference on every projected Session.
Ask publication and human Flow transitions prepare the Run before the boundary
is visible; launch consumes that identity and native resume keeps it. Existing
boundary IDs continue to target actions. The child-to-parent temporary binding
file and late identity/Ready writes are removed. Unbound persisted boundaries
use explicit `session open --json` preparation; listing remains read-only and
reports an actionable error until prepared.

Focused capture, CLI, Flow transition and cross-language contract evidence is
in [session-run-implementation.md](session-run-implementation.md). This slice
establishes identity, not live ownership or configured provider readiness. The
active-Run projection, Monitor panes, both native performance measurements and
the simplified human demo remain open. Concurrent outline work is preserved
without being claimed as this slice's implementation. LOO-293 is not adopted,
closed or removed. No publication or Task completion.

The subsequent [recovery correction](session-run-review.md) releases the
preparation lock before waiting on a resumed conversation. A real CLI regression
reproduced the blocked second open; first-launch and resume cases now pass with
a local stand-in provider, preserving the prepared Run identity. Configured
provider/UI and remaining canvas obligations are unchanged.

The subsequent [active discovery review](active-runs-discovery-review.md) fixes
existing native clients being omitted when the new generic capture binding is
absent. Native discovery uses existing per-Run receipts independently; generic
capture intervals still establish exact Exec attribution. All 16 configured
Session Run references resolve, and five active Session Runs are observed.
One unbound live Exec remains explicit incomplete evidence. Discovery still
visits retained Run directories; Monitor integration and both experience
measurement runners remain required next work.

## Slice ledger — navigation implementation, 2026-09-23

The first slice replaces the root Work/Sessions switch and cascading console
with the compact shared Work list. It retains the existing per-window terminal
workspace, joins exact typed durable identities to stable planning rows, and
keeps unmatched/multiple/pending human Sessions accessible. Task inspectors now
include the directive and Project definition/KRs alongside existing actions and
Activity. A/D presentation is window/repository state, initially full-width.

Current main's Session DTO and native view match the prepared base. Shared legal
actions/display path (LOO-284), conversation cardinality, and lf-new's nested
worktree layout are not present and have not been invented in Swift. No wire API
changed. The implementation removes the labels-only roadmap read and Session
poller; Podium owns the shared readings, SessionsStore retains presentation and
opening state, and SessionsWorkspaceRegistry still owns layouts/surfaces.

Before declaring the slice fully proven, complete configured-app navigation
with live Sessions on a host where Ghostty can create surfaces. The local
fixture proofs and exact outstanding failures are recorded in
`navigation-proof.md`; they are not the ten external-work trials.

Remaining LOO-291 scope: shared LOO-284 action/display contract integration;
optional bounded per-subject conversations and required lf-new workspace
integration; Task-directive editing; human-selected external-product trials;
and published measured paint/interaction budgets with long-lived-registry
trials. No Task completion or publication is claimed by this implementation.

Review-slice correction, 2026-09-23: saved Work selection now survives returning
through unavailable planning; truncated snapshots cannot establish that selected
Work was removed. The focused regression reproduces the former loss and passes
both missing-evidence cases after correction. `review-slice.md` contains the
claim matrix and precise configured-native proof still needed. Slice publication
is not approved by this review; remaining LOO-291 scope is unchanged.

Native-proof investigation, 2026-09-23: Ghostty reports `error.OutOfMemory` before
navigation assertions. A minimal AppKit C-API host using the same artifact also
fails with user config omitted, both before and after window attachment. The
process sees a screen and Metal device; the earlier no-rendering explanation
is not established. `native-surface-diagnostic.md` and its probe preserve the
reproduction. Restore that boundary before the configured navigation trial;
no production change or publication is justified by this diagnostic alone.

Second review correction, 2026-09-23: portfolio discovery also reconciled selected
Work against partial planning. Removed that redundant call and extended the
regression through portfolio refresh and repository return. All 25 focused
navigation/model tests pass; complete planning refresh and explicit selection
validation retain their behavior. Configured native proof remains unmet, so
publication and Task completion remain unclaimed.

Native restoration, 2026-09-23: the isolated probe traced OutOfMemory to
CoreVideo display-link creation returning -6661. With that capability unavailable,
GhosttyManager now selects the existing timer renderer; supported hosts retain
their configured vsync behavior. The unchanged real-PTY fixture then exposed a
resize/search focus bug. Focus requests now belong to the retained native view
and are consumed on attachment/transition, removing the SwiftUI coordinator.
The draft/focus test and the independent window-isolation test both pass, one
each, with exact receipts in `native-surface-diagnostic.md`. No assertion was
weakened. Configured full navigation, split/scroll behavior, resolution, visual
quality and timings still need proof. This restores a necessary boundary without
changing planning, Session lifecycle, the pinned artifact, or remaining Task scope.

Compression follow-up, 2026-09-23: preserved the existing Task terminal's manual
focus when no selected-pane binding is supplied. The native view still owns one
focus request; its update distinguishes disabling from unchanged selection.
The extended real-PTY regression failed before the correction and passes after,
including the previous draft/hiding/search checks. No further model/API reduction
was justified; `compress.md` records the inspected paths and intentional boundaries.

Iteration 2 proof, 2026-09-23: added a mounted SessionsView regression using real
Ghostty PTYs, fixture planning/Session records, and the existing toolbar actions.
The final focused test passes: A/D and detail/overview navigation plus repository
return preserve both surfaces, split layout, selected Work, native focus,
unfinished input and the companion terminal's scrolled viewport. No production
change was needed. `navigation-proof.md` records its exact command and limits.
Configured provider continuation/resolution, navigator scroll, visual quality
and timing proof remain open; the integration fixture is not that human-path
trial and does not change full LOO-291 scope or publication disposition.

Iteration 3 navigator retention, 2026-09-23: the mounted work-list proof exposed
repository return resetting its native scroll offset from 1,200 points to zero.
A/D and planning refresh already preserved it. WorkspaceNavigation now retains
one per-repository/window list offset; WorkspaceNavigator records scroll geometry
and initializes SwiftUI's scroll-position binding from that value when remounted.
No planning, Session or terminal authority changed. The focused proof and its
limits are recorded in `navigation-proof.md`; configured provider interaction,
visual quality and timing obligations remain open.

Iteration 4 completion proof, 2026-09-23: extended the mounted native workspace
regression through Complete, Task details and return to the companion terminal.
It passes: Session row/pane/surface disappear, selected Task remains incomplete,
details show no Sessions, and the original companion child still responds.
The shared completion response is mocked; provider continuation and caller
release are not established. Normal LaunchServices launch of the current app
also failed to expose workspace controls through AX. No production correction
was needed; configured interaction and timing proof remain open. Exact command,
receipt and boundary are in `navigation-proof.md`.

Iteration 4 review correction, 2026-09-23: extending the native completion proof
with Undo exposed the completed pane returning and taking companion focus.
Complete now uses the existing non-undoable Session reconciliation path instead
of Close view. The focused test passes after reproducing the failure; Task,
companion, draft and navigation assertions remain intact. `review-slice.md`
records the before/after receipts. Configured proof and full Task scope remain
unchanged; no publication or completion is claimed.

Iteration 5 asynchronous completion proof, 2026-09-23: the mounted regression
now covers completion after switching repositories as well as completion while
viewing. The held shared response is released only after the original native
views detach. Cleanup reconciles the originating workspace before return and
leaves the active repository's Session reading, selection and pane layout intact.
Undo remains cleared, and the original Task context and companion child survive
return. No production change was needed. This is native fixture evidence with a
mocked CLI response; configured provider interaction and timing obligations
remain open. Exact receipt and limits are in `navigation-proof.md`.

Iteration 5 review correction, 2026-09-23: Close view followed by external Session
disappearance could leave a stale Undo layout. A focused regression reproduced
the pane returning and taking companion focus. Reconciliation now invalidates
that saved layout when it contains an absent Session, while preserving Undo for
a still-present Session. Both cases and the two mounted completion cases pass;
`review-slice.md` records the receipts. Configured interaction and timing proof
remain open; the broader Task scope is unchanged.

Iteration 6 configured planning, 2026-09-23: an activated LaunchServices launch
now exposes the real workspace through AX. That path revealed inherited
`LF_WAVE_ID` narrowing the machine roadmap read and failing from the app's `/`
working directory. The existing CLI launcher now clears only that ambient Wave
variable; Home configuration, explicit targets and the shared roadmap API remain
unchanged. Real AX navigation proves the Task directive/no-Session state, A/D
switching and search retention through a planning poll. A proof-owned provider
then continued the same conversation in the native terminal; UI completion
removed its pane while preserving the companion process and incomplete Task.
Exact draft fidelity is unproven: the submitted prefix lost two characters.
One launch exposed no Task controls by the observation deadline. Scoped AX
latencies, the failures and the concrete next procedure are in
`configured-ui-proof.md`; no generic permission blocker is inferred. Configured
scroll/repository/visual proof, controlled before/after timings and the full
Task obligations remain open.

Iteration 7 configured proof, 2026-09-23: recovered the prior proof-owned Session
in the installed development Home; inherited control-Home reads explained its
apparent absence. Exact provider-history text now proves retained draft and native
continuation, followed by UI completion with companion/Task survival. A separate
configured repository-return trial preserves selection, list scroll and shell
input. Three alternating base/branch count-availability observations use the same
verified planning/Session population; these are scoped measurements, not the
required budgets or twenty-trial series. Configured inspection exposed lost
command titles on remount and unreadable primary text in the light palette. The
native view now retains its title, and the workspace explicitly uses its palette
for primary text/search prompt. The title regression fails before and passes both
cases after; configured title and visual receipts are in `configured-ui-proof.md`.
Configured terminal scrolling and runner launch/input reliability remain open;
external trials and the full LOO-291 scope remain unchanged. No publication or
Task completion is claimed.

Iteration 7 handoff boundary: another writer subsequently integrated lf-new and
checkpointed the preceding source/evidence as `b12beba8b`. The final historical
configured probe encountered the new `New terminal` surface and stopped before
input; all preceding receipts retain their exact binary attribution. Builds and
fixture runs from this pass stopped while the other writer changes appearance
and terminal-id fixtures. Their integrated result needs its own configured proof;
this pass neither overwrites that work nor claims to validate it.


## Integration amendment — 2026-09-23

The latest human direction supersedes the earlier mandatory Task-first and
one-current-conversation proposals above. **New conversation** starts a fresh
conversation at the visible repo/Wave/Project/Task scope and honors the configured
destination. **New terminal** starts an ordinary shell. Neither creates a Task,
enforces cardinality, replaces another provider, or runs an autonomous operating
pass. Bounded conversation lifecycle and explicit creation/adoption remain open
design, not requirements to silently enforce in Swift.

Integrated the committed lf-new contribution through `lf rebase` in this Task's
owned checkout after checkpointing its newer code and configured receipts. The
registry now retains per-checkout terminal layouts under repository worktree
slots, sharing one native surface pool per window. The newer opening-state
reduction, title retention and native focus fixes remain. Required terminal IDs
are consumed with their full PTY attachment contract. No new Session policy or
labels-only planning query was added. Main still lacks LOO-284's shared actions
and display path; that integration remains outstanding.

Focused reconciliation exposed and corrected completion reverting to undoable
pane close after repository navigation. Restored non-undoable reconciliation;
both native cases pass. Window-local appearance resolution now keeps the custom
palette and native control scheme aligned, and live-data light/dark captures
confirm readable Work/Task/search/toolbar text. These captures do not establish
the pending installed New conversation demo. Exact commands, source hashes,
review findings and limits are in `lf-new-implementation/integration.md`.

Remaining LOO-291 scope: shared LOO-284 actions/display path, bounded conversation
lifecycle, directive editing, human-selected external-product trials, and
published paint/readiness budgets with the required long-lived-registry trials.
The prior configured provider receipts remain attributed to their preceding
binary. No Task completion or publication follows from this integration.

Review correction after integration, 2026-09-23: shell-attached Sessions now expose
the existing Complete action without changing terminal identity or closing the
shell. The regression reproduced missing controls, then passed success/rejection,
multiple attached records and continuing shell input; direct Session completion
also still passes. A fresh configured shell trial proves real Task details,
visible launch scope and exact input through navigation on the integrated build.
It does not prove New conversation startup or provider completion. The aggregate
verification attempt stopped at main's active build-cache resource limit. See the
current review in `review-slice.md`; publication and full Task completion remain
unapproved. No installed app or user-owned Session was changed.

Iteration 8 — rejected completion: inventory reconciliation treated a rejected
Complete as a failed open and replaced it with live state, clearing the error.
SessionItem now keeps the completion error independently of its opening state;
retry clears it and successful completion removes the item. No wire type or
lifecycle policy changes. The existing native shell proof now reconciles the
inventory after rejection and requires both the visible error and live terminal;
it failed before and passes after. Direct completion and repository return
also pass (two tests, four cases). Configured New conversation proof is recorded
separately in `iteration8-proof.md`; the full target and external trials remain.

Iteration 9 — configured terminal viewport: the signed integration proof app
rendered rows 168–200 and the bottom marker, then a positioned wheel event
scrolled to rows 1–33. The same viewport survived Show work list → Work details
→ All work → Return to terminals → Hide work list. Inspected screenshots and
executed probe are in `configured-ui-evidence/iteration9/`; this closes that
bounded configured scroll gap without a source change. Session-row/nested-layout
attempts stopped before provider launch. The last runner reported AX trusted,
but AX's focused application was Warp PID 33576 rather than owned app PID 72044;
cached activation signals disagreed. All nine owned app processes exited, no
Session was created or changed, and LOO-291 remains incomplete. See
`iteration9-proof.md` for failed attempts and remaining scope. No timing budget,
external-work trial, gate, publication or Task completion is established.

Iteration 10 — Session-row and nested workspace proof: the preceding review's
mounted test passes with two fixture Sessions, four real PTYs, both split levels,
hidden-slot return, exact draft and responding children. Current source matches
that receipt. A fresh configured trial launched an exact proof-owned Task Session
with verified local attachment, then stopped at the probe's already-visible-list
assumption before row selection. The corrected fresh attempt stopped before UI
actions at exact AX focus despite granted Accessibility. The first disposable
Session was completed through the CLI; both apps and its provider exited. No
production/test edits or reruns were warranted. The configured row/nested gap,
manual procedure and scoped observations are in `iteration10-implement.md`;
full Task proof obligations remain unchanged.

Iteration 11 — Active evidence correction: preserved the human's explicit
checkout-activity membership and replaced the Task row's ambiguous Running label
with Provider in checkout. One planning-completeness check now governs Wave
visibility, count certainty and confirmed Active emptiness. The regression fails
before correction and passes both with and without recorded out-of-plan Work;
it clicks the retained Wave heading and reads its stranded Task in details.
Three focused tests/four cases pass. No configured app was launched or changed;
the human demo and full Task obligations remain open. See `iteration11-implement.md`.

## Slice ledger — directive editor, 2026-09-24

Added Edit directive to the existing Task inspector, using the shared PM update
operation and Podium's authoritative roadmap refresh. A captured target keeps
the edit on its original Task; a read generation prevents pre-save polling from
restoring old planning. Failed saves retain the editor draft; accepted writes
with failed reads stay explicit. Three focused tests (four cases) pass for the
operation, refreshed rendering and polling race. Mounted editor interaction is
not established by those tests. See [iteration12-proof.md](iteration12-proof.md).

The fresh configured row/nested trial stopped before input at a locked desktop
with system AX focus on loginwindow. No provider was launched or human Session
changed. The editor's signed review build is separate from the ongoing demo.
The authorized external edit round trip, other configured proofs, LOO-284 and
full Task budget/trial obligations remain open. Nothing was published or completed.

## Slice ledger — configured editor, 2026-09-24

Iteration 13 closes the configured Cancel path: targeted Accessibility actions
select LOO-291, open its editor, read the exact shared directive, enter a local
draft, cancel, and reopen the original text. Shared planning stays unchanged.
No production change, PM write or provider action was needed. The probe now
traverses AXSheets and waits for asynchronous presentation. This path does not
require global keyboard focus and does not prove provider input. Exact receipts,
failed probe assumptions, owned-app cleanup and scoped observations are in
[iteration13-proof.md](iteration13-proof.md). Save/rejection interaction, the
authorized external edit, shared LOO-284 integration, configured nested workspace
proof and full Task trials/budgets remain open. Nothing was published or completed.

## Slice ledger — configured Session picker and nested workspace, 2026-09-24

Iteration 14 adds stable Session accessibility IDs to the existing multiple-Session
picker actions. A fresh signed configured trial selects only its owned provider,
restores its existing pane from the other checkout, retains both two-pane groups
and their four PTY children through details navigation, then completes the Session
through UI while preserving the panes. The provider PID/birth receipt stays equal
through navigation. Existing human Sessions remain untouched.

[Iteration 14 evidence](iteration14-proof.md) records the exact identities, failed
probe assumptions, build/source hashes and cleanup. Pane selection and retained
PTY children do not prove keyboard focus, draft fidelity or shell responses.
The editor's injected rejection proof from review 13 is also now recorded; a real
authorized external edit is still open. Shared LOO-284 integration, external-work
trials and published/measured budgets remain required. No publication or Task
completion occurred.


### Iteration 15 — shared Session actions and Work path (2026-09-24)

Implemented the missing shared contract on this existing branch after comparing
local main and LOO-284's unstarted snapshot. Rust projects action labels/help,
unavailable reasons and stable Work ancestry; CLI and Swift consume them.
Deleted Swift legality/replacement inference and its duplicate resolution-label
enum. Ready is enforced before FlowStep client stop and again at settlement;
Iterate's predecessor requirement uses the existing flow lookup. Local surface
presence and prepared/error state remain separate.

Four configured records agree through Rust output and production Swift DTOs;
four-PTY nested selection and direct Session completion proofs pass. The fresh
read-only app launch had a trusted runner and one window but did not expose the
expected count control during its bounded observation. Keep that UI result
unproven. Exact tests, review and limits are in
[iteration15-shared-sessions.md](iteration15-shared-sessions.md). No provider or
human Session was touched. Configured action/nested-input proof, external edit
and trials, and measured budgets remain open; no publication or completion.

### Iteration 16 — configured host diagnosis and live contract comparison (2026-09-24)

A current disposable signed build reproduces the missing controls. The element
returned in AXWindows is AXApplication equal to the root, so the earlier count
of one does not establish an accessible window. Delaying inspection until after
the launch callback does not correct it. A bounded direct host observation then
establishes a locked console and system AX focus owned by loginwindow; the
runner remains Accessibility trusted. No provider or Session was touched. Stop
configured interaction until unlock; do not add production workarounds from this
host result or infer the cause of older observations retrospectively.

Fresh shared reads compare five Projects, 145 Tasks and four Sessions against
production Swift values, including seven KRs and the exact Task-attributed
Session join. All 154 records pass the documented field comparisons. This is
one read-only population, not configured UI trials, an external edit, an all-kind
Session action matrix, or a timing budget. A rerunnable read-only probe now exits
before launch on the observed locked-host boundary. The current signed review
build and exact resume command are in [iteration16-proof.md](iteration16-proof.md).
No production edits, publication, landing or Task completion occurred. Configured
controls/nested input, human-selected external trials/edit and budgets remain open.

### Iteration 17 — one hierarchy interaction study (2026-09-24)

The newer canvas amendment supplies the missing usability direction: one
compressible repository → Wave → Project → Task → Session outline. It also
specifies Task Sessions/Monitor content in the existing multiplexer and two
performance measurement experiences. These supersede the preceding configured
retention trial as the next design step; previous behavioral evidence remains.

[Interactive study](hierarchy-study.html) uses one sample hierarchy and one
presentation menu. Full structure, compact singleton Projects and flat Sessions
share exact selection and mounted simulated drafts. Compact Project names remain
accessible in their Wave row. Six Sessions cover two repositories, repeated names,
repo/Wave conversations and unavailable ancestry; upcoming Tasks remain in the
hierarchy. The content area is a simulation for checking selection, not a proposal
for replacing Ghostty or the Task multiplexer.

Browser interaction checks pass for all three presentations, separate same-name
Session drafts, compressed Project access, folding, upcoming Tasks and unavailable
ancestry. Compact and flat screenshots were visually inspected. Receipt and
screenshots: `/tmp/loo291-hierarchy-study/`. No native or timing result follows.

Review: the actual app still routes Task selection through Podium details and
presents separate All work/list/terminal controls. The study removes those from
its composition only. Native replacement must reshape these existing owners;
shipping the HTML does not remove any app path. The presentation menu, default
compression, and selected-content placement remain proposals for human review.
The next native increment must include the canvas measurement contract and retain
the existing terminal/multiplexer ownership. No production source, user client,
PM Task, installed app, publication or Task completion changed in this pass.

## Slice ledger — native outline contribution, 2026-09-24

[Native implementation and proof](outline-implementation.md) replaces the competing
navigation controls with compact/full/Session presentations and explicit leaves.
The existing multiplexer retains real PTYs across these presentations. Repository
selection remains through the existing scoped reader. Monitor, cross-repository
flat coverage, performance measurements and configured acceptance remain open.
The concurrent Session → Run contract slice above remains its existing writer’s
work; this ledger does not relabel that backend work or complete LOO-291.

## Slice ledger — shared active Runs, 2026-09-24

`lf runs --active [--task …]` now joins capture intervals and existing native
client receipts to shared OS ownership evidence, then resolves typed Work through
the same Run attribution reader as Sessions. Swift has the matching required DTO
and RegistryQuery read. Seven Rust tests, one Swift test and clippy pass; exact
commands/source hashes and limits are in [active-runs-implementation.md](active-runs-implementation.md).

The concurrent native-discovery regression showed that a new index alone lost
clients from already-running launchers. Existing receipts remain authoritative,
which currently requires walking retained Run directories. Bounded discovery is
still core work before frequent Monitor polling. Monitor panes, retained Task
pane choice, both native performance journeys, external trials and the final
human demo remain open. This slice establishes no publication or Task completion.
