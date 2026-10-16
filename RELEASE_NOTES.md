# v0.14.2

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.2 lets operators update the CLI while keeping Desktop and its bundled clients running. CLI-only installation previously could stop those clients even though it left the app installed; shutdown now follows what the installation will replace.

## Update the CLI without interrupting Desktop

Run `LF_INSTALL_CLI_ONLY=1 lf install` to update the CLI without selecting a Desktop replacement. The installer excludes app executables from process shutdown in this mode, preserving the clients already in use.

- The same process selection applies when recovering an interrupted installation.
- Full installations still stop the app and helpers being replaced.

## Operational notes

- Release guidance now clarifies that promotion does not restart Tasks or Flows.
- Regression coverage checks process selection for CLI-only and full installations. Recorded validation includes 20 focused installation tests, 2,643 passing Rust tests with 22 skips, and four separate clone tests; formatting, Clippy, architecture, and inventory checks also passed.
- Live client continuity remains unverified. After installing a published release containing this fix, the remaining check is a CLI-only update with Desktop and an active bundled client open, confirming that the same processes stay running and usable.