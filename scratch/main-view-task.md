# One workspace for work and Sessions

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

1. **This slice:** join shared Work/Session readings into the compact unified list and A/D workspace presentations; keep autonomous and upcoming Tasks visible, integrate existing detail/Session access, preserve live terminals, and prove the behavior. Reconcile current main with `lf-new` and LOO-284 before selecting shared API changes; do not build parallel ownership or lifecycle logic.
2. Complete optional per-subject conversation lifecycle and required workspace integration through shared APIs, preserving existing Session/human-boundary evidence. Keep the complete target in view while implementing internal cuts.
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
