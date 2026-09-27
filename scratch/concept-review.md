# Workspace concepts after the parent assessment

2026-09-27 · LOO-303 · reviewed HEAD `5dea707d6`; supplied slice review
preserved at `aa4bdcf95`

Keep the approved model. The [parent assessment](workspace-parent-contracts.md)
and its [slice review](workspace-parent-review.md) complete the requested
dependency investigation. No new product defect or concept change was found in
this pass. The next implementation trigger is a parent revision delivering the
missing owner/consumer contracts. Repeating the completed assessment on the same
revision or reopening the repaired keyboard/return path would add no evidence.

The [complete design](workspace-ux-on-data-model.md) remains unfinished. This
bounded judgment selects no Flow edge, publication or Task disposition.

## Usage first

Jack wants to open exact work, understand its plan and attempts, and permanently
assign an orphan conversation without losing its terminal or draft. Preserve
light mode, the existing composition and sidebar single-Session shortcut.
The [interactive correction](workspace-ux-review-feedback.md) keeps the Project's
default template on its Wave page, captured invocation detail on Task pages, and
non-Task execution in overall monitoring.

Keep `swift/README.md:102–133` and `docs/lf.md:706` unchanged. They already explain
palette navigation, Task links, historical recents, recursive disclosure and
exact lookup. No affected skill needs a new workflow. Their local behavioral
evidence remains bounded by the outstanding installed acceptance.

Concrete existing journey: Jack opens a Task through its link or palette entry,
reads details without entering its sole Session, expands a composed Flow and
inspects a return. Folding the target changes the arrow endpoint while keeping
its name. Returning to the Session retains its terminal. If the Task read fails,
the workspace stays in place with Retry; successful historical absence removes
the recent descriptor, while a failed read does not. Inspection starts no work.

The approved remaining journey is still **unimplemented**: Jack opens the orphan
room, chooses a Task, reviews the named permanent assignment and confirms once.
The Session moves under that Task with its draft intact. If its current Run
changed during selection, the shared operation rejects the stale intent and the
picker requires fresh confirmation. An uncertain write reads back the selected
identity before claiming success. This preserves the accepted contract; it is
not a newly proposed interaction or a claim about the current CLI.

## Model in one screen

| Action or concept | Identity and owner | API or transition |
| --- | --- | --- |
| Open a Task | Stable Task and canonical repository; shared planning reader | `roadmap --task` → `openTaskDestination` |
| Revisit / retain a historical page | Bounded descriptors / selected Task evidence | Exact readback / `selectedTaskEvidence`, outside current-plan membership |
| Understand the plan | Resolved template composition and graph | `resolve_flow` → catalog → shared `FlowTemplateView` |
| Disclose a composition | Template revision and local group ID | Existing expansion binding; original graph supplies hidden target names |
| Inspect progress | Captured invocation, node and iteration | Shared current/retained projection; public attempt history still owed |
| Inspect a retry | Run ID and writer-assigned ordinal at that position | Parent `position_runs` / selected attempt; no Swift counting |
| Keep a conversation | Session ID, title, feedback, member Runs, current Run | Replacement changes the selected Run, not the conversation |
| Assign an orphan | Current Run ancestry; exact confirmed target | Shared write-once bind, absent in the assessed API |
| Display its terminal | Window-owned terminal identity and surface | One active mount; presentation grants no process authority |

A Flow is a template; an invocation captures execution. A composition is an
authored group, a return is an edge, a pass has an iteration, and a position may
have several Run attempts. Provider retries inside one Run are separate. Session
continuity spans Runs. These distinctions explain behavior Jack needs; collapsing
them would hide retries, rewrite history or lose the retained conversation.

## Findings and simplification judgment

### Storage has progressed; the public contract remains the boundary

The earlier statement that the parent lacks Run position fields is superseded.
At inspected parent `d814eb617`, `session.rs:37` includes node, iterations and
attempt. `store/sqlite/runs.rs:76` resolves ancestors in the write transaction;
`:144` assigns the ordinal; `:196` selects the attempt under invocation/version
and pending-Session fences; `:234` reads ordered position history. This is a
useful shared owner, not a reason to invent another one in Swift.

The smallest source check that could disprove the dependency was to trace those
values into consumers. `position_runs` has Store forwarding and test callers,
but no public presentation consumer at that revision. `PinnedTaskFlow`
(`ops/task_flow.rs:35`) lacks attempt history/current Run; `SessionFlowMembership`
(`ops/human_session.rs:242`) lacks attempt ordinal. The existing
`TaskExecutionSnapshot.run_id` is acknowledged by the assessment but does not
supply the full historical position contract. `TaskFlowRecord::Finished` carries
only the Flow name despite retained SQL capture. Storage and public inspection
remain different completion boundaries.

That source still combines Task review, Ask, taskless Flow and interactive
sources at `human_session.rs:575`. `SessionCommand` (`lf/mod.rs:733`) and
`SessionActionKind` (`human_session.rs:121`) contain no Bind. The assessment also
retains the typed repository/ancestry, Started-consumer, runtime-child and common
taskless-owner gaps. Its five unchanged public fixtures support that boundary.

These are pinned source observations, not a fresh claim about the parent's live
status. The assessment records published `ca1be1116`, newer local `d814eb617`,
and unfinished Ask edits; only a new integration receipt can establish later
readiness. Its supported route is `lf rebase` onto the parent tracking branch.
No integration or parent operation is performed by this review.

### The selected simplification removes a user-visible ambiguity

Today `TaskFlowView.swift:144` uses Task update time and `:156` chooses the first
Task-matching active Run for the running line. An independent helper can therefore
supply the provider for another Run's displayed step. This is the already recorded
attempt-slice mismatch, not a new defect discovered here.

Approved target: node detail, running line and Session chip identify the same
exact attempt. Following that interaction through the shared projection removes
the Task-wide provider/time heuristic. It also makes earlier-attempt selection
an inspection action instead of a cursor change. No additional position record,
client-side ordinal, or current-template reconstruction is justified.

Likewise, orphan status will follow the current Run's null Task, so a bound
historical Task cannot disappear into the room merely because it left the plan.
Current `WorkspaceProjection.swift:75` still classifies by roadmap matching;
`:107` still reverse-searches Session ownership. Those consumers remain live
until the parent contract and working replacements arrive. The room must also
replace the opacity-hidden host at `SessionsView.swift:371–400` with one active
mount while retaining the pool. Deleting these paths now would lose behavior.

### Keep the independent navigation and template owners

`PodiumModel.swift:81–146` keeps late-result fencing, exact historical readback
and Retry together; palette and links converge at `openTaskDestination`. Recent
descriptors and selected evidence have separate jobs. Combining them would create
an unnecessary historical snapshot cache.

`engine/flow.rs:407` resolves composition and `:233` supplies one execution
flattening path. Template disclosure cannot replace captured execution.
`TaskFlowView.swift:575,649` retains the original graph for detail and the folded
projection for arrow placement. The resolved numeric-target counterexample
justifies both values. Recursive disclosure uses existing expansion state;
terminal input eligibility stays separate from requested focus and lifetime.
The [return review](workspace-return-review.md) already exercises these boundaries.

No optional redesign or further deletion is selected. The disclosure list and
diagram, existing history evidence, and terminal pool still serve required
behavior. The assessment introduces no new user-facing concept to remove.

## Remaining work and smallest next proof

No new product decision is required. Current-Run-only binding and the independent
Run/template treatment remain explicit inherited assumptions; this review does
not turn them into additional participant approval.

On a new parent owner/consumer conversion revision, reassess the existing eight
contracts, integrate through `lf rebase`, and preserve the child's historical
Task lookup. Extend shared attempt projection where still needed. First prove
failed A → current B at one position with independent Task Run C active: detail,
running line and Session chip all identify B as attempt 2; choosing A retains A.
The completed parent assessment is not the next implementation task again.

Carry every unresolved slice-review obligation forward:

- Retained invocation/child history, loop returns, restart, source-independent
  capture and explicit unknown historical membership/ordinal.
- Every null-Task orphan, including Wave-only and non-bindable taskless reviews,
  more than six tiles and shared-shell Sessions, with one native mount and no
  automatic takeover.
- One permanent-target confirmation across room, palette and Task rows; replacement,
  competing bind, old polls, uncertain writes, completion, rename, repository-switch
  and Close/Undo races. Previous Runs retain ancestry and membership.
- Installed cold/warm Task links and keyboard behavior, configured provider/draft
  retention, live 1440×900 and 1100×800 captures, and Jack's installed-app verdict.
- Final deletion and documentation reconciliation, plus the inherited failing
  `wave_chapters` architecture inventory (last receipt: 32/33 owners).

## Evidence limits

Preserved the supplied parent slice review through `lf commit` at `aa4bdcf95`.
Inspected the active diff inventory, affected documentation, local navigation,
template/terminal consumers and immutable parent source at `d814eb617`, including
the attempt call sites and public types. No new remote/source-readiness claim
is made. The parent assessment and review retain their dated state receipts.

Only scratch notes differ from executable revision `792a3ce40`. The retained
`return-review-swift.log` records nine tests in five suites passing;
`return-xcode.log` records successful build-for-testing. Reuse them within their
local fixture/owned-PTY and compile limits. Earlier CLI/resolver evidence retains
its original scope. No earlier proof is invalidated by this notes-only judgment.

Only this concept note changes after the preservation checkpoint. No product
tests, builds, rendering, installation, Home mutation, external message or
publication occurred. Whitespace and local-link checks validate the note only.
This supplies review evidence to the following deciding step.
