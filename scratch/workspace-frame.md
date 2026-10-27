# Workspace frame — LOO-608

## Accepted outcome — 2026-10-26

Jack Heart requested square workspace top corners, retained rounded lower
corners, and a softer edge that fades like a shadow. Pane focus styling, layout,
terminal sizing and input behavior remain unchanged. Visual acceptance and
merge approval remain pending.

## Implemented; visual tuning provisional — 2026-10-27

Both top clip radii are zero. The lower radii still derive from pane radius plus
inset, preserving LOO-492's nested-corner geometry. The hard `borderActive`
workspace outline is replaced by the theme's shadow color at 30% opacity, with
a two-point stroke blurred by three points. These values are an implementation
choice awaiting visual review, not an accepted palette specification.

The noninteractive overlay follows the content clip and does not change layout.
The clearance test now measures pane outlines against that clip rather than the
fading decoration, retaining the five-point minimum gap. Its four Light/Dark ×
split-axis cases inspect production shapes and modifiers; they do not establish
rendered shadow quality or native terminal sizing/input continuity.

Reconciliation on October 27 found no bounded code mismatch. Current main's
Dashboard/sidebar changes retain the same workspace panel and pane geometry;
no alternate frame mechanism is needed. The README describes the new frame.

## Remaining acceptance

A rendered before/after comparison in Light and Dark remains for demo in the
primary Task conversation: square alignment against titlebar/sidebar, a softer
edge, retained lower-corner clearance, and unchanged pane focus and terminal
behavior. Broader affected suites remain with gate. No product decision is
needed before that review; shadow tuning depends on the rendered result.

Check: `scripts/test_desktop.sh --filter TerminalPaneLayoutTests -Xswiftc -gnone` — prior PASS (Desktop build and four cases), reused without code changes; rendered comparison: demo.

## Concept review — 2026-10-27

Outcome: no design change and no open product choice. Jack Heart's Task brief
and the accepted outcome agree: the workspace boundary is decorative; the pane
owns focus. The former hard `borderActive` outline gave the whole workspace a
competing emphasis. The implemented fading edge removes that mismatch without
introducing a frame setting, state, or lifecycle.

Normal path: open a Task, focus its conversation, then resize the split beside
its shell. `workspacePanel` renders the shared clip and decoration;
`focusPane` calls `MultiplexerStore.setFocusedPane`, and the divider calls
`updateRatio`. `SessionsWorkspace` still owns the retained layout under its
`WorkWorkspaceKey`. Only the frame's appearance changes. `WorkPaneLayout`,
RegistryQuery APIs, Rust ownership, SQLite persistence and provider execution
need no changes. The README already expresses the accepted appearance; no
usage action or skill instruction needs adding.

Failure and recovery: if saving a resized layout fails, the existing
“Layout not saved” message and “Reload saved intent” action remain inside the
same workspace. `loadPaneIntent` retains current edits while merging saved
content; the frame adds no recoverable data. The noninteractive decoration
must leave that action and terminal pointer input reachable. This is source
inspection, not a new runtime recovery proof.

Counterexamples to check at demo: a fading edge that disappears or becomes a
hard halo in either theme; blurred ink that visually crowds the lower pane
corners despite geometric clearance; or changed native sizing/input. Any of
these requires a repair within the accepted outcome, not a new product choice.
The next proof remains the rendered comparison and interaction checks above;
no new implementation direction or additional test run follows from this review.
