# v0.13.17

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.17 keeps conversations, files, and published PRs together in retained Task workspaces. Navigation opens when needed, switching companions preserves drafts and shells, and quieter terminal feedback keeps text readable while working.

## Return to the work in place

Repository launch opens its configured conversation, and Waves organize Tasks without a separate Session inventory. Work navigation starts closed, opens with Command-K, and can be pinned when browsing needs more room.

- Waves stay in place and reveal Tasks inline. Available Tasks appear once above the hierarchy with their Wave context; search filters both sections.
- Available uses the same definition in Desktop and `lf session list --available`.
- Command-click opens Work beside the current destination; ordinary click replaces or focuses it. Hiding panes retains their contents and drafts.
- Each retained Work layout has independent pane splits. Pane controls support alternating terminal splits, Claude/Codex/OpenCode switching, and Session reset.

## Keep files and review beside the conversation

New Task workspaces open with Session and Files side by side at a 61.8/38.2 split. Companion menus replace content in place, so inspecting a file or PR does not require abandoning the conversation.

- Open the Files chooser with Command-P and pin it when needed. Files shows scratch and changes against the recorded parent commit.
- Resize dividers with constrained golden-ratio snapping. Switching companions retains file drafts and hidden shells.
- Inspect a published GitHub PR's status, checks, requested reviewers, description, change totals, and expandable patches in the compact PR viewer.
- Missing GitHub access and incomplete patches remain explicit. Removing the Task's PR link clears its retained PR page.

## Read terminal output with less interruption

Copy and Find feedback now stays brief, while terminal contrast handling preserves more of the original color. Shortcut handling also leaves provider commands with extra modifiers available.

- Successful clipboard writes show confirmation in the pane header without decorating terminal cells.
- Find sits outside the terminal viewport and briefly emphasizes the current match, respecting Reduce Motion.
- Exact Command-C, Command-V, and Command-F handling preserves provider shortcuts that add modifiers.
- Ghostty composes shell backgrounds before contrast correction. Low-contrast text retains more color; `minimum-contrast = 1` preserves original colors.

## Operational notes

- Layouts persist within the window, not across app relaunch.
- The bundled Ghostty lf7 artifact is checksum-pinned. Contrast correction does not guarantee exact hue, separation of nearly identical colors, or contrast through transparent backgrounds and custom postprocessing.
- The supplied record includes headless Desktop builds, broad Swift test runs, and passing focused Rust and Python checks. Final navigation-test repairs still required exact-head CI validation; local verification of the later change was limited by disk space. A completed final full-matrix pass is not established here.
- Paired native performance and all-provider input scenarios remain unmeasured; this release makes no performance improvement claim.

## Small changes

- Task Process history loads only when Debug expands, keeping diagnostics out of normal Task frames.
- Task lifecycle controls separate command execution from refresh: successful commands no longer leave confirmation busy indefinitely, and refresh failures do not repeat the mutation.
- Task sync delivers state before fields so its own title update cannot stale the state read.
- Terminal teardown no longer initializes Find while its view is being deallocated.