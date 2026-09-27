# Workspace navigation and template concept review

2026-09-27 · LOO-303 · reviewed HEAD `587d8fe2f`

Keep the current model: inspect a Task, open a Session, or inspect a Flow
template; captured execution remains a separate reading. The prior navigation
findings are repaired with local proof. One narrow template mismatch remains:
folding can make a return target's detail display an internal node key instead
of its name. Return that case for focused reproduction and repair. It requires
no new product object, resolver, persistence or workspace redesign.

The [template slice review](workspace-template-review.md) owns executed checks
and remaining acceptance. This concept review supplies source evidence and a
bounded proposal, not a Flow navigation decision or permission to publish or
complete LOO-303.

## Intent and usage first

Jack requested keyboard access, exact Task links, understandable templates and
later exact attempts and permanent Session binding. Preserve LOO-291's composition,
light mode, retained terminals/drafts, and the existing sidebar's single-Session
shortcut. The approved model places the Project's default template on its Wave
page and captured invocations on Task pages; non-Task execution stays in overall
monitoring. Current-Run-only bind remains an explicit inherited assumption.

Keep the navigation and template guidance in `swift/README.md:102` and the exact
lookup example in `docs/lf.md:706`. The README now correctly bounds historical
recents: visited Tasks can be revisited after leaving their pages; this is not
all-history search. No affected skill needs a new workflow or instruction.

Concrete journey: Jack opens `loopflow://task/LOO-303`, reads Task details rather
than entering its sole Session, visits a Wave, and returns through ⌘K. A recent
historical Task is read back by exact identity; failure preserves the workspace
and offers Retry. A removed highlighted row gives Return the first remaining
visible result, or no action when empty. Those repairs have local evidence in
the slice review; installed link delivery still needs proof.

For templates, retain the documented interaction: expand a composed Flow from
its group or disclosure control; collapse without changing execution; inspect
both return edges. Add this proposed detail expectation to the next focused
implementation, not as a claim already met:

> Inspect a return step to see the named step it returns to, even while that
> step's containing Flow is folded. Expanding the group reveals the same target.

This stays inside the approved template semantics. No new user choice is needed.
Keyboard access through nested disclosure remains unverified; naming it here
does not close that existing acceptance gap.

## Model in one screen

| User action / concept | Identity and owner | API or transition |
| --- | --- | --- |
| Open Task details | Stable planning Task ID and repository; shared roadmap projection | `TaskLink` / palette → exact `roadmap --task` → `openTaskDestination` |
| Revisit a destination | At most 20 display/lookup descriptors per window/repository | `remember`, `openPaletteTask`; exact readback for history absent from current readings |
| Retain the selected historical page | `selectedTaskEvidence`, outside current-plan membership | Selection and successful planning refresh; no second snapshot cache |
| Open a conversation | Session ID; existing reader and window terminal pool | Existing `openSession`; explicit client takeover remains separate |
| Inspect a Flow template | Resolved composition plus graph, with content revision and local group IDs | Rust `resolve_flow` → catalog → Swift `FlowTemplateView` |
| Expand a group | Repository navigation's revision-keyed set of group IDs | Disclosure and diagram click update the same set |
| Inspect execution | Captured invocation/node/iteration; later exact Run attempt | Existing captured graph; parent contracts supply missing attempt authority |
| Choose another Flow | Task's existing draft and legal controls | Preview or confirmed restart; inspection itself never starts work |

Normal template path: `engine/flow.rs:403` flattens the resolved tree for execution;
`engine/flow_graph.rs:141` uses the same resolution for catalog graph and disclosure
structure. Every composition use gets a distinct ID, including empty uses and
groups inside XOR alternatives. Revision identifies resolved content, not an
invocation or loop pass. Swift uses one shared view on Task, Wave and catalog
surfaces; pinned execution takes precedence at `TaskFlowView.swift:360`.

Failure path: catalog resolution errors yield a named unavailable entry with no
graph/template (`flow_graph.rs:167`); Task preview displays that reason
(`TaskFlowView.swift:365`). It does not manufacture an executable partial graph.
Historical execution cannot be reconstructed from this current catalog. Retained
history and exact attempts still depend on the parent integration below.

## Findings and simplifications

### 1. Folding should not change the name of a return target

**Observed in source, not executed in this review:**

- `TaskFlow.swift:224` replaces a collapsed group's children with one visible
  group node. Other nodes retain their original `returnsTo` (`:232`).
- `TaskFlowView.swift:469` correctly remaps the diagram's return endpoints through
  `visibleKeys`. The return arrow can therefore terminate at the folded group.
- `TemplateDiagram` passes the projected graph to `FlowDiagram` (`:533`), which
  passes that same graph to `FlowNodeDetail` (`:614`).
- Detail resolves the target only among that graph's visible steps, falling back
  to the raw target key when absent (`:967`).

Smallest source counterexample: a composition contains `implement` at key `0`;
an outer `loop-decide` at key `1` returns to `0`. Fold the composition. The
diagram maps `0` to `group-0`, but the detail lookup finds no visible step keyed
`0`, so its expression produces **Iterate returns to 0**. Expanding restores the
name. The target still exists in the catalog; disclosure alone lost its label.
This is a presentation defect, not evidence of a changed execution target.

Current → proposed: Jack must interpret a numeric key when a target is hidden →
the detail always names the semantic target while the arrow attaches to its
visible containing group. Resolve descriptive facts from the original graph;
keep the projected graph for layout. If helpful, identify the containing group
alongside the step name, but do not substitute the group for the execution target.
Reuse the original graph already passed into `TemplateDiagram`; no copied label
store, wire field, YAML lookup or rewritten execution edge is needed.

Smallest next proof: mount a template with the target inside a folded composition
and two return steps outside it. Inspect each return and require the original
target name and distinct edge labels; expand/collapse and require unchanged
meaning. Repeat the shape inside an XOR alternative. Assert inspection issues no
Flow control and keeps terminal input isolated. The current fixture puts both
returns and their target in the same `pursue` group, so its full-expansion and
folded-loop checks do not exercise this crossing (`TaskFlowProofTests.swift:53`).

The existing topology and resolver passes remain valid within their scope. A
repair to shared detail rendering needs focused template and captured-detail
proof before it is treated as complete; no new passing behavior is claimed here.

### 2. The previous navigation repairs keep the right owners

`WorkspacePalette.swift:100` now derives both visible highlighting and Return from
current results. `SessionsView.swift:469` retains activation's fresh inventory
check. There is no tombstone, pending-action queue or permission to activate an
absent row. The slice review also executed Return during a failed reading with
last-good inventory, preserving the exact retained terminal.

Recent Task descriptors are materialized independently of current membership
(`WorkspacePalette.swift:33`). `PodiumModel.swift:123` bounds them at 20;
`:130` resolves absent historical Tasks through the existing exact reader.
Expected identity and repository are checked before selection (`:93`); Retry
retains the expected ID (`:146`). Confirmed absence removes the recent entry;
transport failure does not. The historical Task → Wave → refresh → recent Task
journey and repository isolation are now covered at
`WorkspaceDestinationTests.swift:61`, with failure/mismatch cases at `:91`.

Keep descriptors and selected evidence separate: one permits returning to a
destination, the other owns its current detail. Combining them into a historical
Task cache would add another truth source. The old concept-review findings are
resolved locally, not reasons to repeat their implementation.

### 3. Preserve the template / invocation / attempt distinctions

Composition is an authored group; a return is an edge; one pass has an iteration
value; a Run is an attempt. Local disclosure IDs and template revisions cannot
identify execution. The shared resolution/flattening split preserves that model
without a second loader. Keep graph-referencing template items rather than
duplicating node policy into the tree.

The slice review's **No Flow recorded** repair correctly avoids calling Started
proof of independent membership (`TaskFlowView.swift:407`). Keep it until exact
Run evidence can support a stronger statement. The running line's current Task
timestamp/provider selection remains later attempt work, not a new reliable
Run projection (`TaskFlowView.swift:144`, `:156`).

The disclosure tree and diagram offer two controls for the same expansion set.
That is not a second lifecycle. Whether the extra disclosure rows feel redundant
is a visual question, not a source-confirmed usability defect. Do not remove them
before proving recursive keyboard access and empty-group reachability. No broader
template redesign or generic action registry is selected by this review.

## Remaining work and decision boundary

The smallest independent action is the crossing-boundary return-detail proof and
repair above, followed by the already owed dispatched recursive keyboard proof.
Preserve all outstanding slice-review and full-design obligations:

- Installed cold/warm Launch Services Task links, configured providers/drafts,
  live captures at 1440×900 and 1100×800, and Jack's installed-app verdict.
- The explicit LOO-298 integration checklist: all-kind stable Session/current Run,
  typed ancestry/repository, historical Task lookup, confirmed selected-Run bind,
  shared Started, ordered/current attempts, retained invocations and taskless
  legality. This source still has no Bind in `SessionCommand` (`lf/mod.rs:736`)
  and no node/iteration/attempt projection on `Run` (`session.rs:27`).
- Exact attempts in node detail, status and Session chip; retained historical and
  child invocations; source-independent capture; independent-Run counterexample.
- Every orphan tiled, including Wave-only and non-bindable taskless reviews;
  shell-attached/shared terminals mounted once; one permanent-target confirmation
  across three entry points; bind/replacement/poll/repository/Undo races.
- Final deletion and documentation reconciliation. Unmatched-roadmap grouping
  (`WorkspaceProjection.swift:75`) and the hidden mounted checkout host
  (`SessionsView.swift:371`) remain live dependencies until their replacements
  work. The inherited `wave_chapters` architecture-map gap remains open.

No new product decision is needed for the focused repair. Current-Run-only bind
and the approved handling of independent Runs remain explicit assumptions in the
[design](workspace-ux-on-data-model.md). Parent integration cannot be inferred
from this review or from table existence. The following deciding step owns Flow
navigation; this note makes no navigation or disposition choice.

## Evidence limits

Reviewed the active diff from `c832aaede` through `587d8fe2f`, the latest review
changes, governing design/feedback, Product memory, source consumers and focused
test bodies. Inspected the retained passing logs for the five affected Flow
checks, failed-inventory activation and Xcode compilation. Reuse the slice
review's other CLI/Rust/Swift receipts only within their named byte and behavior
scope; nothing was rerun or relabeled as installed acceptance.

Only this review note changes. No product test, resource recovery, native render,
installed activation, parent conversion, live Home mutation, external message,
publication or Task completion occurred. The new return-label case is a source
counterexample awaiting behavior proof. Local link and whitespace checks validate
the note only; they establish no product behavior.
