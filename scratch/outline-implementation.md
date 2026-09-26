# Native outline contribution — 2026-09-24

The canvas navigation is now native code. `WorkspaceProjection.outline` produces
visible rows from the existing typed planning/Session joins. `WorkspaceNavigation`
retains presentation, selected Session, folding, search and scroll per repository
and window. No additional inventory, query owner, terminal pool or layout tree.

Compact is the default. Full shows structural rows; Sessions promotes leaves.
Compression requires readable, complete planning and never removes Task identity.
Folded structural rows remain visible so their descendants can be deliberately
hidden. Presentation changes preserve saved folds. Context menus expose omitted
ancestors, inspection and scoped conversation creation. Unknown ancestry remains
explicit. Equal Session labels use distinguishing ancestry and, when needed, IDs.
Repository roots select the existing repository reading; this increment does not
introduce a machine-wide Session inventory or claim cross-repository flat coverage.

Removed the Podium repository/status band, its unused instrument and Session
button, the Active/All Task selector, All work/list visibility toolbar, and the
second set of subject Session links above details. Upcoming Tasks are visible.
Ordinary shells and scoped conversations remain contextual outline actions;
retained terminals are reachable from its presentation menu. Task selection still
opens existing details pending Monitor integration. Task and Project headings now
use the native system family. Shared human actions remain inside Session panes.

## Evidence

`outline-evidence/native.log` records the focused SwiftPM proof:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter \
  'WorkspaceNavigationTests|WorkspaceNavigationProofTests/(sessionRowRestoresWorktree|workspaceRetainsNativeSplit|navigatorRetainsScroll)'
```

The same native proof can emit captures by setting
`LOOPFLOW_OUTLINE_CAPTURE_DIR` to an existing directory. Its `/bin/cat` PTYs are
real; planning, Session inventory and resolution replies are fixtures. It proves
exact Session leaf selection, compression/missingness, upcoming Work, folding,
viewport retention, retained native draft/input and companion output, repository
return, and completion cleanup. It does not prove configured provider continuation
or human acceptance. Full/compact/Session captures were visually inspected.

The required `uv run python scripts/test.py --loopflow` attempt stopped at resource
preflight: main's build occupied 19.1 GiB against its 12 GiB budget. No product
suite or Xcode fallback compilation ran. The diagnostic is retained in
`outline-evidence/fallback.log`; no other checkout's build was removed.

## Review findings applied

- A folded singleton keeps its disclosure row; compression cannot silently undo
  the human's fold. Failed/partial planning cannot authorize compression.
- Same-titled Tasks can yield identical ancestry text even with different IDs;
  ambiguous Session rows now include the Session ID.
- Searching a Task identifier also retains its Session leaves.
- Selected Session identity survives presentations/repository return and clears
  on authoritative disappearance or resolution, without clearing Task selection.
- Updated hosted UI assertions and user documentation to the new outline; the
  hosted suite itself remains unrun.

## Remaining core

Monitor pane content, exact active-Run projection, Task pane restoration and both
repeatable performance measures remain unimplemented by this contribution.
No timing budget or rendering-latency claim follows from native test duration.
The configured human demo, external trials/edit, long-lived-registry samples and
fallback compilation remain required by the full canvas design.

Concurrent edits in this checkout are implementing required Session → Run identity
and updating the marked contract slice. Those Rust/DTO/fixture edits were preserved,
not claimed or checkpointed here. This contribution adds only native navigation,
its focused proof and documentation. No PM mutation, publication, landing or Task
completion occurred.
