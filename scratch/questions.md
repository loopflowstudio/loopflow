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
Jack Heart selected Ink on Paper and authorized Swift implementation of the full
prototype, including mirrored selection in Needs you. Command-click adds adjacent
Work; ordinary click replaces the focused slot; close hides while retaining panes.
These prototype gestures carry into implementation. Cross-launch outer layout
restoration remains outside this change. The existing reader watches focused Task
Workflow detail; inactive workspaces retain live panes and offer focus for actions.
Native demo judgment and landing remain distinct from implementation authorization.

October 15 native feedback: Jack Heart rejected selection jumping from a Wave
row to the header. Replacement is unresolved: stable Wave rows with inline Tasks,
or a persistent Wave index above a scoped Task section. Preserve Ink on Paper,
one Work selection and Needs you mirroring. Neither replacement is authorized
by this design discussion.

Dashboard exploration is separate [LOO-489](https://linear.app/loopflow/issue/LOO-489),
stacked on LOO-462 at 7d10108eaf. Jack Heart clarified that a Wave dashboard is
strictly query-derived (Wave conversation plus open Tasks), offered on Wave click
by default or alongside conversation-only. Expandable sections remain a proposal.
Only the pinned repo dashboard is customizable; it initially contains the repo
session. Tasks have no dashboards. The child owns `scratch/dashboard-design.md`
in its checkout. Feature's task-design/kickoff edge is running; its primary
Session is open in the existing cmux window as LOO-489 Dashboards.
