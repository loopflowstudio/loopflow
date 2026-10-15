# Open questions and assumptions

## LOO-487 assumptions — 2026-10-15

- Jack Heart accepted implementation for hands-on review on October 15: use an
  overrideable `minimum-contrast = 3` and finite 160 ms cell emphasis using the
  existing motion token. Original-color mode remains `1`; final visual judgment
  awaits the demo.
- LOO-462's delivered copy/Find work was integrated through `lf sync --manual`
  at merge `213136cd79`, preserving its UI owners. Its separate Workflow-loading
  landing hold remains live; no parent repair is selected.
- Contrast preserves channel ordering, not exact hue or arbitrary color separation.
  Faint alpha, graphics, images and unknown transparent backdrops retain explicit
  limits. Review the fixture results before accepting the default.
## LOO-462 parent design context — 2026-10-15

- Jack Heart redirected this design conversation to sidebar exploration. The
  [sidebar design](sidebar-exploration.md) owns the current request; Workflow
  loading proposals remain separate, unaccepted drafts.
- The completion investigation records a roughly 1.2-second command; historical
  confirmation and UI delay remain unmeasured. No cause or latency target is accepted.
- Proposed two-phase progress wording in
  [the current design](make-loopflow-a-work-first.md), including a ten-second
  confirmation deadline independent of heartbeats, is unreviewed.
- Existing feedback leaves Fantasia → manabot ambiguous between folder rename
  and display naming. Studio/Silencio portfolio removal was an interpretation,
  not authorization for deleting repository directories.

Sidebar: Jack Heart approved implementing prototype 03 in the app, with program
headers, a collapsible sidebar without Machine grouping, and a compact attention
queue above the Wave / Task columns. Context switching remains the priority.
Native visual review still needs to judge width, metadata density and focus
restoration. Implementation is authorized; landing and native acceptance are not.

Separate work: LOO-487 captures animation/shader/color polish, including cell-level
effects and automatic terminal contrast. LOO-488 owns whole-app theming; its
feature design edge is launched, with theme architecture/import choices still
draft. These scopes do not extend the current pane/navigation implementation.

Native review continued into [Work multiplexer / Task workspaces](task-workspaces.md).
Jack Heart selected Ink on Paper and authorized Swift implementation of the full
prototype, including mirrored selection in Needs you. Command-click adds adjacent
Work; ordinary click replaces the focused slot; close hides while retaining panes.
These prototype gestures carry into implementation. Cross-launch outer layout
restoration remains outside this change. The existing reader watches focused Task
Workflow detail; inactive workspaces retain live panes and offer focus for actions.
Native demo judgment and landing remain distinct from implementation authorization.
