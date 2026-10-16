# LOO-492 native corner review — 2026-10-15

## Delivery direction — 2026-10-16

After the revised native demonstration and clarification that it was not yet
landed, Jack Heart said, “ok, ship it”. This authorizes publication and merge of
the current corner fix; the earlier pending-review disposition is superseded.
Finish affected verification, land through `lf land`, verify the merge, and
complete LOO-492. No additional visual approval is required.

## Reopen — 2026-10-16

Jack Heart requested reopening the demo. The installed development helper was
still v0.14.0 and rejected the shared database's v0.14.1 migration. The checkout
had already advanced to v0.14.2, but restoring local edits left AGENTS.md
unmerged. Combined the compatible headless-view and display-only frame-test
guidance, staged that resolution, and completed `lf sync --manual` successfully.
The retained corner implementation is unchanged. Rebuilt the app and its
matching v0.14.2 helper successfully; log: `/tmp/loo-492-demo-reopen.log`.
Repository opening passed. Reopened and raised Loopflow Dev on LOO-492, added
a shell, and observed the native split layout with clear bottom-corner gutters.
Jack Heart's current Session remains attached to its original client. Visual
acceptance remains pending; no Flow or additional conversation was started.
Check: `lf sync --manual` — PASS; no unmerged index entries, diff checks PASS.
Disposition: acted; fresh Task status remains open/idle with both earlier Flows
stopped/completed and no PR.

## Previous demonstration

Jack Heart requested a native demonstration and rejected the first built app:
“The Looopflow Dev i saw didnt have a fix”. The pane still crowded the native
window's lower corner. This supersedes the earlier inner-workspace geometry
preview as evidence of a solution.

Disk capacity recovered above the 32 GiB reserve. The supported development
build completed with the matching validation-only CLI and launched Loopflow Dev.
Native shell panes and a horizontal split were displayed without transferring
Jack Heart's active conversation. The missing-helper and capacity blockers from
the initial preview are resolved. Build log: `/tmp/loo-492-demo-build.log`.

The earlier implementation corrected only workspace/pane nesting. The current
repair adds a 10-point horizontal/bottom gutter around the Work layout; combined
with the existing 6-point pane gutter, pane borders are 16 points from the native
window edges. The headless check covers the outer gutter, and the native frame
check covers left/right/bottom terminal clearance. The rebuilt app was observed
with a single shell and a horizontal shell split: both lower pane borders now
sit visibly inside the window curves, with a continuous surrounding gutter.
Loopflow Dev is left open on that split for Jack Heart's review. Jack Heart has
not yet accepted this revised appearance. The unrelated active-conversation
banner remains visible because the demo intentionally did not transfer the
primary Session from its current client.

Check: `uv run --no-sync python scripts/loopflow-dev.py run` — PASS; matching app/helper built and native app launched.
Check: `scripts/test_desktop.sh --filter 'TerminalPaneLayoutTests|PaneResizeTests'` — PASS; both split axes/light-dark geometry and resizing.
Check: `LOOPFLOW_NATIVE_TESTS=1 swift test --skip-build --package-path swift --filter TerminalPaneFrameTests --no-parallel` — PASS; four native frame/grid cases, light/dark and both split axes.
Check: `git diff --check` — PASS.

Review: keep the window gutter on WorkLayoutView so it applies with one or
multiple workspaces, outside terminal measurement. Retain independent split
spacing; inset changes must not widen internal dividers. No new store, setting,
or helper is introduced. Remaining affected gate checks are still pending.

Design: `scratch/loo-492.md`; initial direction: `scratch/corner-direction.md`.
The SVG at `scratch/demo/corners.html` illustrates only the earlier inner pair;
it is not the native demo or proof of this correction. No Flow was restarted by
the demo. Next: show the revised native corner, collect Jack Heart's judgment,
then finish the remaining gate and delivery.
