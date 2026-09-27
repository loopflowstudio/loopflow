# Workspace navigation concept review

2026-09-26 · LOO-303 · reviewed HEAD `f20f06234`

The navigation model is clear: open a Task's details, open a Session's conversation,
or inspect a Flow template. Keep those destinations distinct and keep the existing
sidebar's single-Session shortcut. No redesign or new persistent owner is needed.
Two source-level gaps remain in repeat navigation: a disappearing highlighted row
can leave Return targeting an absent destination, and a visited historical Task
disappears from recents when its page is left. Return these for focused behavior
proof and repair; neither was exercised by this review.

The [slice review](workspace-navigation-review.md) remains authoritative for its
executed checks and outstanding acceptance. This review supplies concept evidence,
not a navigation decision or permission to publish or complete LOO-303.

## Intent and usage first

Jack requested keyboard access and exact Task links while preserving the installed
LOO-291 composition. Light mode only; no teardown. The approved design distinguishes
Project-owned Flow templates on Wave pages, Task-owned captured invocations, and
non-Task execution in monitoring. Session identity, terminal drafts, exact Run
attempts and permanent bind targets must survive the later integration.

The navigation usage in `swift/README.md:102` and the exact lookup example in
`docs/lf.md:703` already explain the normal path clearly. Keep them. No affected
skill needs rewriting: this change adds navigation, not a new agent workflow.
The following recovery wording is a proposal within the approved scope, not a
claim of implemented or installed behavior:

> Press ⌘K, search an issue identifier and press Return to open its Task page.
> Escape returns focus to the retained terminal. Recently visited Tasks remain
> available after leaving their pages, including Tasks from an earlier chapter.
> If a destination disappears during search, the palette shows a valid current
> selection; it never opens the removed item. A failed read keeps the last
> available results visibly stale. An unresolved Task link keeps the current
> workspace and offers Retry or repository-qualified choices.

Concrete journey: Jack opens `loopflow://task/LOO-303`, reads details rather than
entering its sole Session, visits a Wave, then returns through ⌘K. Inspection
never starts work. If exact lookup fails, Jack can cancel or retry without losing
the terminal. If a Session closes elsewhere while highlighted in the palette,
Return must have a visible, valid meaning rather than silently doing nothing.

The README's phrase “Task links and palette Task entries ... including Tasks
outside the current chapter” needs this boundary made explicit when the repair
lands: exact links resolve retained history; the palette currently adds only the
selected historical Task to its current inventory. General historical search is
not implemented and is not proposed here. Reconcile usage with the final bounded
recents behavior before shipping.

## Model in one screen

| Action / concept | Identity and owner | API / transition |
| --- | --- | --- |
| Open Task details | Planning Task ID plus owning repository/Wave; shared roadmap projection | `TaskLink` → `RegistryQuery.taskDestination` → `roadmap --task` → `openTaskDestination` |
| Open a named conversation | Session ID; existing Session reader and window terminal pool | Palette `.session` → existing `openSession`; explicit takeover remains separate |
| Inspect a template | Flow name in repository catalog; future resolved revision/group IDs | `.flow` → `WorkspaceNavigation.Palette.flow`; inspection does not start it |
| Choose execution | Exact Task and existing Flow draft/control | `.chooseFlow` → existing preview/restart picker; confirmation/legality stay there |
| Return to a destination | At most 20 disposable entries in per-repository `WorkspaceNavigation` | `remember`; no planning, Session or launch record is created |
| Resolve a link | One pending URL/window router plus generation-fenced model reading | One Podium receives it; a later selection invalidates the result |
| Retain a historical page | `selectedTaskEvidence`, separate from current plan membership | Current-plan refresh cannot eject the selected page |

Normal path inspection followed `LoopflowApp.swift:165`,
`WorkspaceDestination.swift:43`, `PodiumModel.swift:78`,
`RegistryQuery.swift:126` and `commands/waves.rs:610`. Exact lookup reads cached
planning and retained Tasks through the existing projector, without launch
resolution or an active-PR requirement. The failure path retains the selection,
records an unavailable reading and offers Retry in `WorkspacePalette.swift:146`.
`dismissTaskLink` and ordinary selection invalidate pending results.

## Findings and bounded simplifications

### 1. Current inventory is not the set of reachable recent destinations

Observed in source: `WorkspacePalette.swift:20` builds current Task rows and
`:27` adds only `selectedTaskEvidence`. `PodiumModel.swift:843` clears that evidence
when selecting a Wave. `searchDestinations` sorts existing rows by recent IDs; it
does not materialize a recent row absent from the inventory. At palette opening,
`WorkspacePalette.swift:130` removes recent IDs absent from those rows after a
successful reading. Therefore a linked historical Task can remain resolvable by
the shared exact reader but cease to be offered as a recent destination.

Current → proposed: after leaving an older Task, Jack must reuse its link → Jack
can return through recents, with exact readback when needed. A successful current
plan read establishes current membership, not deletion of historical Work.

Keep the selected-page evidence as the sole detail owner. Extend the bounded
recent entry only with the display and exact lookup information needed to revisit
it; use the existing reader and compare the returned stable identity before
navigation. Do not retain a second full Task snapshot cache, insert historical
Tasks into the current plan, or add a durable visit table. Unknown availability
must remain distinct from confirmed disappearance. This is a proposed repair,
not an approved new all-history search feature.

Smallest counterexample/proof: open historical Task A from an exact result absent
from the current roadmap; visit Wave B; refresh successfully; reopen the palette
and select A from recents. Require A's exact identity and details, unchanged
Run/event counts, and isolation from another repository's recents. Repeat with
failed readback and preserve the current workspace. The existing historical test
keeps A selected throughout; it does not prove this return journey.

### 2. A highlighted ID can outlive its row

Observed in source: `WorkspacePalette.swift:97` submits `highlighted` whenever
non-null. The visible highlight uses the same expression at `:115`; only search
text changes reset it (`:126`). A readings refresh can remove that row without
changing the search. `SessionsView.swift:471` correctly rejects the absent
destination, but no visible row then owns Return and no recovery is shown.

Current → proposed: Return silently targets an absent row → display and submit
one effective selection drawn from the current results. Preserve the selected
identity while present; otherwise show the first remaining result or the existing
empty state. Keep activation's fresh legality check. Do not add a tombstone store,
pending-action queue or permission to activate stale actions.

Smallest proof: in the mounted palette, highlight Session A, publish a successful
inventory without A but with B, then dispatch Return. Verify the visibly selected
destination opens, A is never opened, search reaches neither PTY, and Escape
still restores valid prior focus. Cover zero remaining rows and failure retaining
last-good rows. This instantiates the slice review's unresolved stale-row case;
the source finding is not a new executed AppKit failure receipt.

### 3. Keep the existing conceptual separations

`WorkspaceDestination` also carries UI actions, but one enum and switch are enough
for this small surface. Splitting it into a generic action registry adds no user
benefit. Search and Flow inspection already share one modal lifetime; the prior
compression removed the competing presentation flags. The slice review tested
natural focus restoration from Flow inspection, so no new responder machinery is
justified by this review.

Retain generation, selected Task evidence, terminal pool and pending cold URL:
they protect different lifetimes. A historical page surviving refresh is not
permission to keep it in the current plan. A Session surface remaining alive is
not authority to launch, transfer, complete or bind it.

The next template cut should preserve composition during the shared resolution
pass, as already approved. `engine/flow.rs:1053` flattens nested references through
`items.extend`; `flow_graph.rs:67` and Swift `TaskFlow.swift:149` expose only the
flat graph. Adjacent uses of the same sub-Flow cannot be recovered from equal
parent names. Separate local disclosure IDs and source revision from invocation,
node and iteration identity. Repeated composition, loop returns and Run retries
must not become three names for the same thing.

## Preserved obligations and next proof

No new product decision blocks these repairs or the independent template slice.
The current-Run-only bind assumption and independent-Run-without-invocation
presentation remain explicit in the [approved design](workspace-ux-on-data-model.md).
The later room must retain non-bindable taskless review Sessions with Rust's
explanation; it cannot relax nullable invocation/Task equality in Swift.

Carry forward the slice review's remaining work:

- Repeated Session activation, stale inventory, two mounted production windows,
  and two links racing a click; then installed cold/warm Launch Services `open`.
- Folded templates on Task and current-Project Wave pages: repeated/empty
  compositions, Xor alternatives, both returns, shared expansion equivalence,
  unavailable sources and matching Rust/Swift fixtures.
- Parent contract checklist before room/bind or attempts. `SessionCommand`
  still has no Bind (`lf/mod.rs:736`); `session.rs:27` lacks position/ordinal/current
  attempt projection. Table existence alone supplies none of those contracts.
- One mounted terminal per identity, shell attachment/shared-shell cases,
  unlimited orphan tiles, exact permanent-target confirmation and all bind races.
  Existing unmatched-roadmap grouping and hidden mounted checkout host remain
  live consumers; deleting them now would remove capability.
- Exact current Run in the status line and membership chip, retained invocation
  history, configured provider/draft proof, captures at both required widths and
  Jack's installed-app verdict. `TaskFlowView.swift:144` still uses Task update
  time and `:156` selects provider by Task; that remains later attempt work.
- Final deletion and documentation reconciliation; the inherited `wave_chapters`
  architecture-map gap remains as recorded in the slice review.

The smallest next navigation work is the disappearing-selection regression and
bounded historical-recents return proof above, followed by their implementation
review. The next independent feature remains shared template disclosure. These
are evidence recommendations; the following deciding step owns Flow navigation.

## Evidence limits

Inspected the active diff from `c832aaede` through `f20f06234`, the governing
design/review, Product memory, source callers and focused test bodies. Reused the
slice review's recorded 14 Rust and 8 Swift passes and Xcode compile only within
their stated scope; no product tests, builds, resource recovery or native capture
were repeated. Neither newly identified journey is claimed to have executed.

Only this review note changes. No prior executable proof is invalidated by that
edit. Repairs will require focused new behavior evidence; existing tests cannot
be relabeled as coverage of the new cases. No implementation, parent conversion,
live Home mutation, external message, publication, Task disposition or Flow
navigation was performed.

Review-note local links and whitespace checks pass. No product behavior is
established by those documentation checks.
