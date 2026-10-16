# v0.14.3

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.3 makes returning to work smoother: interrupted Codex Sessions reopen without a scan of historical process receipts, and Desktop terminals give text and pane borders more room. Upgrade to avoid the long “Preparing codex” wait on installations with large receipt histories and keep terminal text clear of rounded corners as panes resize.

## Return to interrupted Sessions sooner

Reopening a Codex Session now reads the receipt for the relevant Process directly. The change removes work that grew with accumulated receipts while retaining the checks used to establish process identity and attachment ownership. (#1575)

- In a controlled fixture with 234,483 synthetic historical receipts, median reconnect time fell from 53.72 to 1.85 seconds for a short history and from 54.35 to 1.79 seconds for a 56,766-byte context.
- All 12 benchmark samples preserved native identity, captured input and every pre-existing event. No context, history or receipts were deleted.
- Missing, corrupt or mismatched receipts and reused process identities retain their existing handling; recovery does not require permission to enumerate the receipt directory.

## Keep terminal text inside the pane

Desktop now reserves space inside terminal borders before Ghostty measures its surface, so columns and wrapping fit within the visible clearance. Adjusted outer spacing also keeps lower pane outlines clear of the window’s rounded corners. (#1574)

- Terminal content has an 8-point inset, with a 10-point gutter along the Work layout’s sides and bottom.
- Nested corner curves stay evenly spaced, and existing split-divider spacing is retained.
- Recorded frame and grid checks cover both split directions and light and dark themes.

## Operational notes

- The reconnect measurements are warm results from a shared host. They reproduce the delayed reconnect stage; they do not measure the original incident or establish cold-cache startup acceptance. Independent startup acceptance remains pending.
- A CLI-only installation preserves active Desktop clients but does not update their explicitly pinned bundled helper. Desktop needs its updated bundle to use the reconnect fix; plan adoption without interrupting live work.

## Small changes

- Updated the Python development dependency Ruff from 0.16.10 to 0.17.0. (#1572)