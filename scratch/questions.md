# Open design questions

October 15, 2026 — LOO-462

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
Jack accepted two hierarchy levels, one Work selection, keyboard-only history,
sidebar-owned collapse and drag resizing. Prototype 04 explores Wave breadcrumbs
and conditional workspace headers. Add/replace gestures, mixed Work layouts and
cross-launch restoration remain draft; no outer-multiplexer implementation launched.
