# Desktop feed proof contribution — 2026-09-24

The shared implementation now renders output below the existing Task plan using
RegistryQuery.taskOutput. This contribution adds behavioral proof and fixes
three bounded presentation defects; it adds no CLI, DTO or service API.

- Follow live now selects current progress on subsequent snapshot refreshes.
  Explicit inspection still retains the selected stage and Run.
- Adjacent text deltas fold together without moving text across an intervening
  command, message or other event. The first regression run reproduced reordered
  prose around a command.
- Historical pages no longer hide a failed live start. Separate lane evidence
  retains `tail_unavailable` until explicit reload.

`TaskWatchFeedTests` covers independent history/live continuation, overlapping
mutable revisions, auxiliary Runs, exact stage/Run filters and Follow live,
reset/reload, failed and superseded reads, cancellation, source text order and
live-start gap retention. Existing inspection tests use the same explicit stage
selection action as the view. Existing workspace rendering now loads the output
fixture as well as the plan fixture.

Focused command:
`LF_WATCH_RENDER_PATH=/tmp/loo293-watch-feed.png swift test --package-path swift
--filter 'TaskWatchFeedTests|TaskWatchTests'` — 12 tests passed. Receipt:
`/tmp/loo293-watch-feed-validation.log`. Inspected
`/tmp/loo293-watch-feed.workspace.png`: the integrated light workspace shows the
plan and output together, readable provider/Run/stage labels, tool input/result,
auxiliary prose, paging controls and explicit manual-update status. This is a
rendered fixture, not configured live arrival or an installed-app demonstration.
`git diff --check` passed. No Rust code changed or broad gate ran in this contribution.

The primary implementation run owns the subsequent correction that anchors
following on the most recently updated Run, plus its final integrated proof.
The above receipt precedes that correction and must not be presented as proof
of it. Shared store/view sources were left to that run after this contribution.

Automatic polling, bounded discovery/state, full capture, exact human Session
navigation, cross-Run observation ordering and the configured human demonstration
remain required for the complete Task. This contribution is neither publication
nor final Task acceptance.

## Final integrated slice proof

The primary implementation now follows the most recently changed Run, including
arrivals in a Run above a quiet later group. Quiet pages and repeated revisions
do not change that target. This retains the slice's explicit Run grouping;
contiguous cross-Run observation-order groups remain required for the complete
feed. No temporal ordering is inferred between providers.

Final command:

```sh
LF_WATCH_RENDER_PATH=/tmp/loo293-watch-feed.png swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'TaskWatchTests|TaskWatchFeedTests|TaskWatchOutputTests'
```

All 14 tests passed, exit 0; `/tmp/loo293-feed-verified.log`. The Mac product
compiled and linked. Added proof covers the Follow control restoring all Runs,
arrivals targeting an earlier Run, quiet-page target retention, and completed
tool snapshots replacing accumulated output without replay. Existing cancelled
and superseded read, source reset/reload, text order, filtering and independent
continuation proofs remain included. Both window-backed render cases passed;
inspected `/tmp/loo293-watch-feed.png` and its `.workspace.png` counterpart. The
plan, Run labels, tool input/output, auxiliary prose and paging controls fit
without horizontal clipping. Scrolling and long-output expansion use native
controls but have no configured input verdict in this receipt.

The first primary build stopped because the concurrent contribution edited a
compiled test. A subsequent build caught duplicate fixture setup added by both
writers. The duplicate was removed; neither failed build supplies a behavioral
verdict. The source was stable for the final pass. The other writer's proof and
corrections were retained. No provider or registry was mutated for these tests.

Review findings fixed: historical pages could erase live-start gap evidence;
text deltas crossed commands; Follow failed to advance the selected stage;
and the overall scroll bottom could hide arrivals from another Run. Ownership
remains one navigation-retained TaskWatchStore; view tasks own query cancellation,
RegistryQuery owns typed transport, and Rust owns attribution/capture. There is
no Swift provider-file reader, durable output store, Session inventory, provider
launch path or lifecycle mutation. README describes manual updates precisely.

The full Task still needs bounded discovery/initialization/retained state before
polling, cross-Run observation-order groups, complete provider capture, connected
diagrams, exact checkpoint Session navigation and the configured human demo.
This slice does not establish publication readiness, Task completion or landing.
