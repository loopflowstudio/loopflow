# Desktop measurement review — 2026-09-24

The capture/input runner advances the accepted canvas design. A fresh public
command passes all 22 short observations using the actual native outline,
SessionsView, multiplexer and three owned cat PTYs per population. Review found
and fixed false completion in journal reconstruction. Full rendering performance
and Task acceptance remain open; publication is pending.

## Finding fixed

Report completion counted unique attempt IDs rather than coverage of the declared
population/scenario/attempt plan. In a copy of the recorded 462-observation journal,
replacing the final large combined-restore observation with an unplanned scenario
still made `report` exit zero. Duplicate observations with different IDs could
likewise replace a missing planned sample. Duplicate ends, orphan ends and end
records naming a different subject were accepted or silently ignored.

The reporter now accounts for exact planned observations and records duplicate or
inconsistent begin/end evidence as journal errors. Missing samples remain counted
even when an unrelated extra sample keeps the total unchanged. Reports with such
errors are incomplete and cannot supply a comparison. Raw journals remain intact.
README wording now distinguishes failure rates among attempted observations from
the separate count of observations never started.

Five regression cases fail before the change; all eight report tests pass after.
The public counterexample command now exits one, reports one missing observation,
and identifies the unplanned record. Reconstructing a copy of the original valid
baseline still yields complete, 462 attempts, zero missing/errors, and identical
scenario statistics. The checked-in baseline was not rewritten.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior and proof | Result |
|---|---|---|---|
| Two repeatable journeys | Hierarchy and retained Task workspace, fixed small/large populations | Public short command: 11 scenarios × 2 populations, 22 passing observations | pass at capture/input scope |
| Native retained input | Exact original Session surfaces, draft and responding companion through navigation | Mounted production views; first responder, surface identity, draft and cat replies checked | pass with fixture reads and real owned PTYs |
| Truthful result coverage | Failures, interruptions and missing attempts cannot become completion | Five before failures; eight after tests; copied-journal public counterexample now exits one | pass after correction |
| Comparable baseline | First/warm separated, sufficient samples for p95, source/host/endpoint recorded | Existing 462-attempt baseline reconstructed unchanged; fresh short sample has no warm p95 | pass for declared endpoint, no optimization claim |
| One product authority | Reuse existing inventory and pane owners | One Podium reader per inventory, one root workspace registry; test-only capture/journal | pass by source trace |
| Rendered/usable performance | Compositor presentation, hitches, scrolling during refresh, correlated phases and budgets | Capture/OCR plus PTY replies only; those broader observations remain absent | gap |
| Complete canvas acceptance | Bounded active discovery, automatic refresh, both builds, configured/human proof | Current reader still walks retained Run directories; no new configured or human trial | gap |

Fresh command:

```sh
uv run python scripts/desktop_performance.py run \
  --output /tmp/loo291-review21-native --samples 1
```

Exit zero; 22/22 observations; source fingerprints before/after match. The native
test took 8.478 seconds. This is an in-process bitmap/OCR and input endpoint with
intrusive observer cost, not compositor paint time. The active-Run transport is
synthetic; it does not measure Rust discovery, configured providers or network.
No installed application or existing user client was moved or stopped.

Concurrent architecture/Swift README edits followed the native run. Its recorded
before/after source hashes match each other; the full current checkout fingerprint
has since changed. The measured Swift test and corrected reporter hashes remain
pinned separately. Those other documentation edits are preserved.

`uv run pytest -q python/tests/test_desktop_performance.py` passes eight tests.
Ruff check/format and `git diff --check` pass. No Swift or Rust executable change
was made by this review, and no broader suite was rerun. Source hashes and fresh
artifacts are pinned in [the receipt](desktop-performance-review/receipt.json).

## Source and intent

Read the current cached LOO-291 governing directive through `lf pm show --wave
product --no-sync --json`, Task status, Wave memory, the canvas amendment, current
slice and Done When, and prior integration review. The cache is not a fresh PM
sync. The complete tracked base-to-worktree patch at review entry is retained in
`/tmp/loo291-review21-tracked.patch`: 728 sections, 680 identical to the prior
Wave review patch and 48 changed/new. Reviewed the measurement files in full and
the changed executable/fixture sections separately. Existing navigation fixes,
Task routing-field deletion, baseline archival and Wave curation remain their
preceding contributions.

Traced native actions into the existing Podium/outline, SessionsView, retained
workspace registry, multiplexer and Ghostty ownership. Capture text and timing
belong to the test window; journal begin/end records describe observations. They
introduce no production navigation, Run liveness or terminal authority. Searches
retain one production capture-binding writer and one Podium caller per inventory.
SessionScope, FlowResolutionAction, requestedSessionId and routing_project_id
remain absent from source/current fixtures. Native discovery still traverses Run
directories; no bounded-cost or automatic-refresh claim follows from the benchmark.

## Next boundary

Continue the existing core with compositor/hitch and scrolling-during-refresh
measurement, correlated phases, configured costs and measured budgets. Bound
native discovery while preserving old live clients before automatic Monitor
refresh. Integrate the complete sibling objective/target change and obtain both
build-path verdicts. Preserve the recorded five-pane clipping limitation and
finish the human composition/retention demo, configured positive Run changes,
external trials and authorized edit. These are not optimization follow-ups or
grounds to complete LOO-291, LOO-251 or LOO-293.

The [review-slice skill](/Users/jack/.agents/skills/review-slice/SKILL.md) permits
publication “When all applicable `Done when` claims hold and the slice is coherent.”
The broader rendering and configured/human claims above remain unmet. No
publication, landing, Task completion, installation or PM mutation occurred.
