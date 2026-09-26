# Direction D in the native app — 2026-09-25

Jack asked for polish direction D in the Swift app: A's sidebar, B's center structure,
C's mood. This pass restyles the existing owners only. No Rust, DTO, control legality,
navigation or terminal ownership changed. The concurrent parent tuple work in
`TaskFlow.swift`, `TaskFlowView`'s header tuple (`pinned.iterations`) and
`TaskFlowProofTests` belongs to the parent and was preserved. Nothing was committed.

## Changed files

- `LoopflowMac/Design/WorkspaceStyle.swift` (new). Palette-derived tokens
  (`textTertiary`, `borderStrong`, `selectionTint`), `WorkspaceSectionHeading` (the
  single 11pt tracked-caps heading), `WorkspaceTone` (shared state colors; fills are
  opaque), `WorkspaceChip`, the `workspacePanel()` card, and `WorkspaceOutlineButtonStyle`
  (primary actions as a burgundy outline).
- `TaskFlowView.swift`. The Flow lives in a panel, with the status line and a tone dot
  beneath it. The diagram is rebuilt: numbered, content-sized mono chips; connectors
  between chips; and loop endpoints that always share a row.
  - **Wide layout:** once and loops share one row, and the tail sits on an indented
    `↳ then` row.
  - **Narrow layout:** separate `once` / `loops` / `then` rows. Only an overwide loop
    row scrolls.
  - **Loops:** each loop gets its own padding, return lane and label row. Arrowheads
    land on opposite sides of the shared target, and the second loop's border is dashed.
  - **Labels:** each loop labels its own return count from `FlowReturn.traversals`,
    e.g. "Loop 1 · 2 returns". "Loop N" is shown for previews.
  - **Header chip:** holds the tuple. The parent now feeds it from `pinned.iterations`.
- `WorkSurfaceView.swift`
  - Page is 1160 wide.
  - Wave: 46pt serif title and 18pt serif objective. The planning gap is a compact tinted
    note with **Details**, which discloses the reason and the copyable recovery command.
    Chapter history sits beside Current KRs.
  - Plan rows sit in a panel with rank, state dot, title, state chip and mono ID columns.
    The chip reads the shared Flow execution, completion and started state.
  - Task: 32pt serif title, and New session as an outline button. The Flow heading is
    gone because the panel names itself.
  - Sessions sit in a panel with a state chip. Description uses the shared heading.
  - Wave controls live in a quiet disclosure.
- `WaveDetailPane.swift`. KRs use ring marks, filled green when a KR holds. The Metrics
  title uses the shared heading.
- `WorkspaceNavigator.swift`
  - Serif 25 repository name in burgundy; presentation glyph in a 28pt hover target.
  - Rows are 30pt and Wave names are serif 19. Tasks are 13.5 with a state dot.
  - The selection tint has a 2px burgundy edge.
  - Session counts sit in a fixed 40pt column.
  - The search field at the bottom is rounded.
- `WorkspaceBreadcrumbBar.swift`. Quiet ancestors, and the issue ID in burgundy mono,
  underlined. The Session name is bold 14. The membership is a mono chip.
- `SessionsView.swift`: navigator width 272 → 264.
- `WorkspaceProjection.swift`: `WorkspaceNavigation.expandedNotices`, the presentation
  state for notice Details (next to `expandedComments`).

## Tests

Updated only where copy or structure changed. `TaskFlowProofTests` now expects
"Loop 1"/"Loop 2" in preview, "Loop N · 1 return" when pinned, and the `task-flow-iteration`
chip. `WorkspaceNavigationTests` finds the one-line notice, opens Details, then finds
the reason and recovery.

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel --filter \
 'WorkspaceNavigationTests|TaskFlowTests|TaskFlowProofTests|WorkspaceNavigationProofTests/namedSessionDrillDownRetainsTerminal|WorkspaceNavigationProofTests/workspaceRetainsNativeSplit|TaskMonitorProofTests|PodiumModelTests'
```

The final run passed: **49 tests in 7 suites**, log at `/tmp/loo291-polish-swift.log`.
It includes the real Ghostty PTY retention proofs.

Earlier runs:

- The first pass failed on three expectations, all from the structural changes above:
  the uppercased heading text (fixed with `.textCase`), and the notice detail now
  sitting behind Details.
- One build broke mid-way on the parent's in-progress `FlowReturn.iteration` removal.
  It compiled once they finished.

No Xcode fallback build or hosted UI run was done.

## Captures (fixture data, real PTYs, hidden window)

`LOOPFLOW_FLOW_CAPTURE_DIR` / `LOOPFLOW_OUTLINE_CAPTURE_DIR` wrote
`task-flow-pinned.png`, `task-flow-running.png`, `wave-{false,true}.png` and the three
outline presentations. I inspected them against `?v=d`:

- **Matches D:** cream sidebar with a burgundy edge mark; serif titles; caps headings;
  the plan panel with aligned chips; the tinted planning note; the Flow panel with
  numbered chips; a whole Feature-shaped Flow including `pr land -c` with no clipping at
  1500pt; separated lanes and landings; per-loop labels at opposite corners; the tuple
  chip; and opaque green and yellow inside the tints.
- **Fixed after the first inspection:**
  - System-blue links (the breadcrumb Wave, Worktree/PR, Stop & restart) are now
    burgundy or quiet.
  - A clipped sidebar count.
  - The filled system button in the restart confirmation.

## Remaining gaps

- The fixture Flow has 8 nodes; the real 13-node Feature was not rendered natively.
  Width is estimated from the 0.6em mono advance, so the 1100pt fallback to three rows
  is exercised by the layout code but not captured.
- There is no Session-surface capture with the new breadcrumb. Session rows still lack
  a provider/summary line (that is a data gap, not a polish gap).
- The Metrics portfolio body is still the older card, not D's single table row. Comments
  and the Monitor pane are unstyled.
- Wave notices have no per-visit Dismiss.
- Dark palettes derive tokens by opacity but were not captured.
- No human acceptance yet.
