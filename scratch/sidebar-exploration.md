# Sidebar exploration

Directions for Jack Heart, October 15, 2026. Existing LOO-462 / Product context.
Jack selected Wave / Task navigator and subsequently authorized native
implementation with program headers, collapsible sidebar and a compact attention
queue. [The native design](native-wave-task-navigator.md) owns that implementation;
the alternatives below retain the exploration evidence.

## What to build

Explore a sidebar where Repo → Wave → Task feels continuous, with generous
selection targets and enough information to choose the next piece of work.

Jack Heart: “I dont love the chevrons”; “both the tasks AND the waves could take
more space, have more info, be more clickaable”; “repo needs selectable state.”
“Brand New UX encouraged, not limited to today's exact feature set.”

## Three inspirations

1. **Notion: containers are destinations.** Its nested pages retain the same
   capabilities as parent pages. Borrow equal selection semantics for repo,
   Wave and Task, with meaningful summaries at each level. Avoid inheriting its
   repetitive toggle-and-indent treatment.
   Sources: [subpages](https://www.notion.com/en-gb/help/create-a-subpage),
   [sidebar](https://www.notion.com/help/navigate-with-the-sidebar).
2. **Workflowy: make depth a change of focus.** A bullet can become the current
   view; breadcrumbs return to any ancestor. Borrow scope navigation so row width
   does not shrink at each level. Its actual product still has disclosure arrows;
   removing them is this exploration's proposal.
   Source: [navigation](https://workflowy.com/help/navigate-around).
3. **Figma: connect the hierarchy to what is selected.** Layers map to objects;
   hover highlights the object, and collapse-all can retain the selected branch.
   Borrow spatial continuity and separate ancestor context from active selection.
   Figma is a nested object manager, not an unlimited document outliner.
   Sources: [selection](https://help.figma.com/hc/en-us/articles/360040449873-Select-layers-and-objects),
   [navigation](https://help.figma.com/hc/en-us/articles/360039831974-Explore-the-navigation-bar-and-left-sidebar).

Product facts above come from official documentation read October 15. The
transfers below are design interpretations, not claims about those products.

## The demo

Open [the gallery](sidebar-prototypes/index.html), compare the same sample work
in three interactive layouts, select the repo, a Wave and several Tasks. Type a
draft, navigate away and Back: each destination retains its own draft. Filter
work, change appearance and resize the window. Fictional statuses and prose
illustrate information hierarchy; these pages do not connect to live work.

| Direction | Distinct mechanism | What it tests | Cost |
|---|---|---|---|
| Continuous outline | One connected vertical rail; burgundy Wave headings; explicit count/fold buttons | Can hierarchy read through typography and continuity rather than arrows? | Tall sections need scrolling |
| Focus sidebar | A Wave becomes the sidebar's scope; ancestor stack stays clickable | Can generous rows replace deep indentation? | Sibling Waves require stepping out |
| Wave / Task navigator | Two adjacent columns; Wave selection determines the Task column | Can stable spatial placement make switching feel effortless? | More horizontal space, less overview |

## Interaction and data

One selected destination (`repo`, `wave`, `task`); the selected object's ancestors
are context, never additional selections. Scope and folding are separate from
selection. Whole titles/summary areas are buttons; disclosure uses labeled text
controls, not a competing tiny triangle. Burgundy identifies Wave structure;
neutral fill and edge identify selection; status has separate labels and colors.

Prototypes share `waves`, `tasks`, `selectedId`, `scopeId`, `collapsed`, navigation
history and per-destination drafts. Plain local JavaScript renders from these
values; no production persistence or execution. New UX includes richer Wave
objectives, Task next-step summaries and column navigation.

## Current system / Delete — do not maintain

Current `WorkNavigator.header` gives the repo identity but no equivalent selected
row. `outlineRow` uses chevrons, 28px Wave rows and 44px Task rows. Selection is
already Work-owned and must survive any production redesign, as must metadata
links, truthful activity, retained clients and drafts.

The selected native direction replaces the existing `header`/`outlineRow`
presentation and its exclusive view assertions,
not add a second navigator or a second Work selection owner. Prototype alternatives
are deliberate comparison artifacts, not compatibility implementations.

## Constraints and forbidden outcomes

No migrations, app replacement or invented activity. No selection
that highlights all ancestors identically. No Wave click that unexpectedly launches
execution. No automatic collapse that loses the current destination. Current
burgundy-heading conventions are intentionally open for this experiment.

## Prototype boundary / done when

The exploration produced three local HTML prototypes plus a cited research gallery. Each
supports repo/Wave/Task selection, local draft retention, search, Back and appearance
switching. Browser verification covers those interactions, layout at desktop and
narrow widths, and absence of JavaScript errors. No Swift or Rust tests are needed.
The native implementation is specified separately in the linked design.

Jack Heart selected “lets try Wave/Task Navigator” and emphasized “switching back
and forth across contexts is key.” The refined prototype remembers the last
selected destination in each Wave, its draft, conversation scroll and Task-list
scroll. Wave-column buttons restore that destination; the heading in the Task
column explicitly opens the Wave conversation. Back/Forward traverse visited
destinations. Option-1/2/3 switch Waves in this prototype; production shortcuts
remain unchosen. Repo selection preserves each Wave's remembered destination.

These are per-page in-memory values, not persisted workspaces or native clients.
No selection alone launches execution. Open judgment: sidebar width, persistent
metadata, and whether Wave switching should restore focus as well as position.

Review: preserve one Work selection owner; separate an ancestor's scope treatment
from selected destination styling. Columns trade width for stable sibling access;
focus trades overview for width. Rich rows are intentionally taller, not more
deeply indented. The subsequent explicit native request authorizes implementation;
prototype behavior alone does not establish native acceptance.

Check: `uv run --with playwright python /tmp/loo462-check-sidebar.py` and
`/tmp/loo462-preview-sidebar.py` — all three pass selection, Back, draft retention,
search and appearance checks; desktop/narrow layouts have no horizontal overflow
or JavaScript errors. Screenshots visually reviewed. Gallery opened and verified
in Chrome; served locally at port 8846, with file-based entry also supported.

Context refinement check: `uv run --with playwright python /tmp/loo462-check-contexts.py`
PASS — per-Wave Task/draft/two-scroll restoration, explicit Wave conversation,
repo return, Back/Forward and keyboard switching; baseline prototype checks also
PASS. A new-tab opening attempt was interrupted by concurrent Chrome changes;
the updated page remains available from its local URL.
