## Evaluate

```sh
uv run python scripts/test.py --reuse-passing
uv run python scripts/test.py --loopflow
uv run python scripts/desktop_performance.py run --output /tmp/desktop-review --samples 1
```

In the app, select a Task, reveal Monitor beside its Session and a companion shell,
then change outline presentation, resize/zoom and return. The exact terminal,
draft and companion should survive. Monitor must display only that Task's observed
Runs and distinguish unavailable evidence from a confirmed empty observation.

Current gate: **not ready to ship**. Affected suites stopped at resource preflight
(main's active build: 19.1 GiB / 12 GiB). Architecture, migration, Swift boundary,
Rust formatting and generated-document checks pass. Full suites, Clippy and Xcode
fallback compilation remain unverified for this tree. Six stale operating-document
fragments were updated in prompt goldens; the engine test remains required.

The archived capture/input baseline has 462/462 passing observations. Warm retained
Session return p50/p95 is 298.8/352.3 ms for the small population and 326.4/495.6 ms
for the large population. These include bitmap/OCR observer costs and are prior
measurements, not compositor latency or an improvement claim. No optimized
after-build comparison is available. A separate concurrent review records eight
passing report tests and 22/22 short native observations against pinned sources;
that receipt does not replace the blocked affected-suite gate.

## Why it matters

Work and conversations share one navigable outline while existing terminals remain
usable. Inspecting a Task or observing its Runs no longer requires changing between
competing work inventories or replacing the conversation's pane.

## What changed

- One compressible repo → Wave → Task → Session outline with retained selection,
  search, scroll, contextual inspection and scoped conversation creation.
- Monitor, Session and shell content in the existing checkout multiplexer, with
  retained pane choices and native keyboard ownership.
- Shared exact active-Run reads, required Session → Run identity, projected human
  actions, and explicit incomplete/stale observations.
- Internal chapter binding and deterministic rotation, direct Wave Task projections,
  retained historical attribution, and matching CLI/Swift contracts.
- Repeatable native capture/input journeys, report integrity checks, corrected
  architecture documentation and synchronized prompt snapshots.

## Risks / Not included

Active discovery still traverses retained history; automatic refresh is pending.
Objective/metric-target ownership reconciliation, compositor/hitch measurements,
configured budgets, cross-repository flat Session coverage and narrow-pane proof
remain open. Human canvas acceptance, positive configured activity, combined-pane
provider input and the required external workflow/edit trials are not complete.
No live chapter migration or installation is included. Rich Run history/output and
the two later optimization Tasks retain their separate scope.
