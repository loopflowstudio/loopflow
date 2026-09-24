# Connected Task diagram — 2026-09-24

The existing snapshot now renders as connected stage nodes above the passive
output feed. This is one desktop slice of LOO-293; full Task acceptance remains
open. No command, DTO, reader, store authority or provider behavior changes.

## Behavior and review

The diagram reads the selected immutable invocation directly from TaskWatchStore.
Adjacent ancestry changes introduce group headings. Native button anchors supply
geometry, keyed by step index within the selected invocation; the invocation ID
resets transient geometry and focus when switching histories. Dashed lines show
plan order. Solid arrow paths come only from retained transitions, including
Iterate returns and same-stage retries. Repeated transitions may share a path;
their individual iteration links and attempt cards remain inspectable.

Selecting a node applies the existing exact stage filter; Up/Down moves between
expanded stages, independent of their names. Iterate and retry controls select
their recorded destination. Current-stage indication, recorded state and attempt
count remain separate facts. Completed invocation settlement does not turn an
unentered stage into a completed stage. Empty old plans show their missing evidence.
Selection scrolls without animation, and geometry never takes keyboard focus.

Review traced TaskWatchView → diagram → retained Store selection and the existing
output filter. Removed the old List and its duplicate current-stage helper.
No new Session inventory, flow reducer, transcript writer or read task exists.
Canvas is decorative and excluded from accessibility; native nodes, links and
attempt details carry the textual evidence. The first render exposed too little
plan space; compacted the node labels and increased the plan's minimum height.
The resulting integrated image shows the return path and attempt cards above
the output feed. Longer plans still scroll inside the resizable plan pane.

## Focused verification

```sh
LF_WATCH_RENDER_PATH=/tmp/loo293-diagram.png swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'TaskWatchTests/(diagramNavigation|selectionAndStaleEvidence|renderSnapshot)'
```

Three tests pass, including two render cases; Mac product compilation/linking
passes. [Receipt](watch-diagram-evidence/tests.log). The new behavioral test
drives actual view actions and arrow-command callbacks with repeated skill names,
then inspects exact Iterate/retry targets, separate Run attempts, completion and
an older missing plan. The existing stale-selection proof also passes.

Inspected the [complete diagram](watch-diagram-evidence/plan.png) and
[integrated workspace](watch-diagram-evidence/workspace.png). Both are window-backed
fixture renders. A test-only attempt to override the read-only reduced-motion
environment did not compile and was removed; the production diagram contains no
animation. No configured reduced-motion or physical keyboard/scroll trial is
claimed. No executable edits followed the passing receipt. Whitespace checks pass.

Automatic visible polling, bounded discovery/initialization/retained state,
contiguous cross-Run observation ordering, complete capture, exact human Session
navigation and the configured human demo remain required in this same Task/PR.
No provider, installed app, lifecycle state or Project KR was mutated. Nothing
was published, landed or marked complete in this slice.
