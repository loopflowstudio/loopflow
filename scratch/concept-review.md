# Workspace navigation and template concept review

2026-09-27 · LOO-303 · reviewed HEAD `84d42b5e7`

Keep the current navigation and template model. The folded return-target defect
and recursive keyboard gap from the previous concept review are resolved by the
[latest slice review](workspace-return-review.md). No new concept change or
product defect was found in this pass. The next work is the explicit parent
contract assessment and the remaining configured acceptance, not another repair
of the already exercised disclosure path.

This is a bounded concept judgment. The complete
[approved design](workspace-ux-on-data-model.md) remains unfinished; this note
neither chooses a Flow edge nor authorizes publication or Task completion.

## Intent and usage first

Jack requested exact Task links, keyboard navigation, understandable templates,
then exact attempts and permanent Session binding. Preserve the accepted light
workspace, existing sidebar single-Session shortcut, retained terminals and
drafts. The [interactive correction](workspace-ux-review-feedback.md) places the
Project's default template on its Wave page and captured invocations on Task
pages; non-Task execution stays in overall monitoring.

Keep the affected usage guidance unchanged: `swift/README.md:102` describes the
palette and historical recents, `:123` describes recursive disclosure and named
return targets, and `docs/lf.md:706` gives the exact Task lookup. No affected
skill needs a new workflow. The README now includes the former proposed
expectation: inspecting a return names its target even while its containing
Flow is folded. Local proof supports that statement; installed acceptance is
still separate.

Concrete journey: Jack opens a Task through its link or palette entry, reads
its details without entering its sole Session, tabs to a composed Flow, expands
it, and inspects an outer return. Collapsing the target's group changes the
arrow's visible endpoint but leaves the named target unchanged. Returning to the
Session preserves its terminal and unfinished draft. The existing sidebar
shortcut still opens that Session directly.

Recovery remains simple: an unavailable template explains which definition
cannot be previewed; an unavailable exact Task read preserves the workspace and
offers Retry. A recent historical Task is checked against its recorded identity
and repository. Successful absence removes that recent entry; transport failure
does not. Inspection never starts work. This is the intended supported journey,
with local behavioral evidence and the installed-delivery limits below; no new
interaction is proposed.

## Model in one screen

| Action or concept | Identity and owner | API or transition |
| --- | --- | --- |
| Inspect a Task | Stable Task identity and repository; shared roadmap reader | Task URL or palette → exact `roadmap --task` → `openTaskDestination` |
| Revisit a destination | At most 20 descriptors per window/repository | `remember` / `openPaletteTask`; absent historical Tasks use exact readback |
| Keep a historical page selected | `selectedTaskEvidence`, outside current-plan membership | Selection and successful planning refresh update the existing evidence |
| Inspect a template | Resolved composition plus graph; content revision and local group IDs | `resolve_flow` → catalog → shared `FlowTemplateView` |
| Fold or expand a composition | Revision-keyed group set in repository navigation | Diagram and disclosure button change the same expansion binding |
| Describe a return | Original graph's exact target key | Recursive `FlowGraph.node`; projection only places the arrow |
| Inspect execution | Captured invocation, node and iteration; exact attempts still owed | Captured graph takes precedence over the current catalog |
| Open a conversation | Session identity; window terminal pool | Existing Session action and focus path; inspection adds no process authority |

A composition is an authored group, a return is an edge, a pass has an iteration,
and a Run is an attempt. Disclosure keys and template revision identify none of
those execution attempts. Session owns conversation continuity; its selected Run
supplies execution and ancestry. The parent conversion must finish that contract
before the room and attempt surfaces consume it.

## Findings and preserved behavior

### The return repair removes the unnecessary mental translation

Before: folding a target forced Jack to interpret `0` or `3/fix/0` as a step
name. Now: the same named target survives folded, expanded and folded states,
while the arrow attaches to its visible containing group.

Source matches that experience:

- `TaskFlow.swift:215` projects hidden nodes into groups without rewriting the
  original return target. `TaskFlowView.swift:468` maps visible loop endpoints.
- `TemplateDiagram` passes its original graph as `detailGraph` at
  `TaskFlowView.swift:575`. Detail uses that graph at `:649` and resolves the
  target recursively at `:1004`, through `TaskFlow.swift:16`.
- Captured rendering at `TaskFlowView.swift:362` supplies its own complete graph.
  It needs no template reconstruction or copied target labels.

The smallest counterexample has already executed: the mounted proof inspects two
returns outside a folded target group, then repeats inside an XOR alternative
with a different target name (`TaskFlowProofTests.swift:278`). It checks both
edge labels through all three disclosure states. Captured detail separately
checks the saved traversal count (`:348`). Reuse the latest slice review's
passing evidence; there is no reason to repeat this implementation.

Original graph and folded projection are useful distinct values. Combining them
would lose hidden descriptive facts again. No second resolver, wire field,
label cache or execution-edge mutation is needed.

### Keyboard disclosure uses one interaction and one expansion owner

`TemplateDisclosureStyle` (`TaskFlowView.swift:529`) gives each recursive group
one button for pointer, Right/Left, Space and Return. It consumes the existing
`configuration.isExpanded`; composition disclosure still updates the shared
revision-keyed set. XOR disclosure remains local SwiftUI presentation. Empty
groups remain reachable even though they contribute no executable nodes.

The executed keyboard counterexample also exposed a separate boundary: a hidden
retained terminal accepted first responder. `GhosttyTerminalView.swift:304` now
carries enabled input into AppKit eligibility (`:511`). Requested focus remains
separate so an unfocused visible pane can still be clicked. Neither flag owns
terminal lifetime or provider authority.

The review's dispatched Tab/Shift-Tab, arrows, Space and Return proof checks exact
focused labels, recursive/empty/XOR disclosure, unchanged PTY buffers and retained
surfaces/draft/replies. Neighboring palette, Monitor, paste and pointer checks
pass. Its unhosted fixture recalculates the key-view loop; that bounds the claim
and leaves installed key-loop behavior unverified. The fix does not establish
conditional mounting for the future room.

The disclosure list and diagram are two controls for the same state. Removing
one is an unselected design alternative, not a demonstrated simplification: the
list currently supplies recursive keyboard and empty-group access. Keep both
until a concrete interaction problem justifies changing them.

### Navigation retains the right owners; attempts still need theirs

Palette Return and highlighting use the same current selection
(`WorkspacePalette.swift:100`). Historical recents retain descriptors, while
`PodiumModel.swift:130` resolves missing Tasks through the exact reader. Identity
and repository checking at `:93` and Retry at `:146` preserve the selected intent.
`selectedTaskEvidence` remains the detail owner. Combining these into a historical
Task cache would introduce a second snapshot owner without improving the journey.

Normal template loading uses `resolve_flow` (`engine/flow.rs:407`) and the same
`flatten_resolved` execution expansion (`:233`). Catalog resolution failure
returns a named unavailable entry with no graph/template
(`engine/flow_graph.rs:167`); Task preview displays it (`TaskFlowView.swift:365`).
Historical execution cannot use a fresh catalog as a substitute for missing
capture. The existing **No Flow recorded** wording also correctly avoids treating
Started alone as proof of independent membership.

The remaining mismatch is already selected work, not a new redesign: the running
line uses Task update time and the first Task-matching active Run
(`TaskFlowView.swift:144`, `:156`). Exact attempt identity must replace both
through the shared projection. `session.rs:27` still lacks node, iteration and
ordered/current-attempt fields; `lf/mod.rs:736` still exposes no Session Bind.
These observations describe this checkout, not the current remote parent state.

## Remaining work and smallest next proof

No new product decision is required by this review. Current-Run-only bind and
the independent-Run/template treatment remain explicit implementation assumptions
in the approved design; they are not newly confirmed here.

The next bounded action is to assess the actual LOO-298 integration source and
fixtures against the existing checklist: all-kind stable Session/current Run,
typed Task/Wave/repository ancestry, historical Task lookup, confirmed write-once
bind fenced to the selected Run, shared Started, ordered/current attempts,
retained invocations and taskless legality. Record which contracts are present
and which are missing. Table presence alone cannot release dependent work; do
not rebuild the parent's storage here or infer its live status from local files.

Once available, the smallest attempt proof is failed Run A followed by current
Run B at one position, with independent Task Run C concurrently active. Node
detail, running line and Session chip must all identify B as attempt 2; choosing
A must retain its exact identity. Preserve loop-return, restart, child-invocation,
unknown-history and source-independent capture cases from the complete design.

Carry forward every unresolved slice-review obligation:

- Installed cold/warm Task links, configured provider and draft retention,
  live captures at 1440×900 and 1100×800, and Jack's installed-app verdict.
- Exact attempts and retained invocation/child history, including the independent
  Run counterexample and missing historical capture.
- Every null-Task orphan tiled, including Wave-only and non-bindable taskless
  reviews; shell-attached/shared terminals mounted once; no automatic takeover.
- One permanent-target confirmation from all three bind entry points, selected-Run
  replacement and competing bind, old polls, uncertain writes, completion,
  rename, repository-switch and Close/Undo races.
- Final deletion/documentation reconciliation and the inherited `wave_chapters`
  architecture-map failure. The latest checker receipt is 32/33 owners, not a
  passing gate.

The four-source Session list (`ops/human_session.rs:575`), unmatched-roadmap
classification (`WorkspaceProjection.swift:75`), reverse lookup and hidden mounted
checkout host (`SessionsView.swift:371`) still serve live consumers. Remove them
only with the approved working replacements. The keyboard eligibility repair
is not a substitute for the room's one-mount proof.

## Evidence limits

Inspected the active diff from `c832aaede` through `84d42b5e7`, the focused repair,
its tests, affected usage docs, governing design/feedback and Product memory.
The retained `return-review-swift.log` records nine tests in five suites passing;
`return-xcode.log` records successful build-for-testing. Only scratch notes changed
between the implementation at `792a3ce40` and the reviewed HEAD. Reuse those
receipts within their original local fixture/owned-PTY and compile scopes. Earlier
CLI/Rust/navigation evidence remains owned by the linked slice reviews.

Only this note changes. No product tests, resource recovery, rendering, installed
activation, parent integration, Home mutation, external message, publication or
Task disposition occurred. Documentation checks validate this note only. This
review returns evidence to the following deciding step and selects no navigation.
