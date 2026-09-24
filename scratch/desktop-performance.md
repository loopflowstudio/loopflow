# Native desktop measurement runner — 2026-09-24

## Remaining measurement contract

The complete canvas still needs compositor presentation/frame-hitch observations,
scrolling during shared refresh, correlated production phases, configured registry
and provider costs, published budgets and the human composition/retention demo.
Bounded active discovery and automatic shared refresh remain core implementation
work. This first capture/input runner does not satisfy those claims or the original
human-selected external trials and authorized directive edit. No optimization Task
is created; this pass contains no production optimization.

## Implemented measurement boundary

`scripts/desktop_performance.py run --output <new-directory>` runs the opt-in
`DesktopPerformanceTests` against the existing SessionsView, Podium, outline,
workspace registry, multiplexer and Ghostty surface pool. It consumes the current
Wave/direct-Task fixture contract introduced by the concurrent integration. No
production inventory, identity, pane owner, configuration flag or reader is added.

The versioned fixture has one repository/Wave, 8 Tasks/4 Sessions or 256 Tasks/128
Sessions, two checkout placements, two retained Session PTYs and a companion PTY.
Each population owns its `/bin/cat` clients. Active-Run rows are explicitly synthetic
shared DTOs, including another Task's Run that must not appear in the selected
Monitor; they do not prove Rust discovery or vendor-provider behavior.

Eleven scenarios exercise full/compact/flat presentation, fold/expand/filter,
active/empty Monitor, retained Session return, and combined-pane zoom/restore.
The first and warm observations are separate. Default sampling provides one first
interaction plus twenty warm attempts for every scenario/population. Control
lookup and fixture setup stay outside measured actions. The native return verifies
exact selection, original surfaces, actual first responder, retained draft echoed
and returned through the PTY, and companion response. Checkouts and all three
surfaces survive the empty-Task visit.

The endpoint is **captured native pixels verified through text recognition**, plus
actual PTY input/replies for Session return. It is deliberately not a model update,
view creation or onAppear timestamp. The SwiftPM host rendered the window but
returned no SwiftUI accessibility descendants. Its unsuccessful AX trials remain
failed harness evidence, not false product latency values. The capture timestamp
and subsequent verification cost are recorded separately; total duration includes
this intrusive observer overhead. No on-screen compositor or hitch claim follows.

Begin/end JSONL records are synchronized per attempt so interruption remains an
unfinished attempt. Reports retain failures and not-started scenarios, require
20 successful comparable samples for p95, and refuse comparison across different
hosts, endpoints, populations, measurement/fixture source or build modes. A source
change during the run invalidates its completion. Missing tools/host support,
compile failures, timeouts and interrupted subprocesses do not become passes.
Report-only mode reconstructs evidence after interruption without another UI run.

## Review findings

- The first native compilation overlapped the other writer's rebase and existing
  conflict repair. It has no behavioral verdict. Their integration and corrections
  are preserved, not claimed as this contribution.
- AppKit AX proxies do not all conform to NSAccessibilityProtocol; following their
  selectors still found no descendants under the test's NSHostingView. A native
  bitmap confirmed the expected UI existed. The runner now names its actual
  capture/verification endpoint instead of relabeling model readiness as paint.
- Five adjacent panes at the first attempted width clipped the empty-state text.
  That trial remains `/tmp/loo291-desktop-perf-empty/`; it is a real composition
  limitation, not a timing pass. The fixed measured journey opens the empty Task
  in its separate existing checkout workspace and returns to all original panes.
  It does not claim five-column readability or resolve that layout limitation.
- Complete evidence is scoped to the runner's declared endpoint. Rendering budgets
  cannot be justified by these instrumented values alone. Ordinary test runs skip
  this opt-in benchmark; `TESTING.md` gives the short command for relevant UI work.

## Receipts

`uv run python scripts/desktop_performance.py run --output
/tmp/loo291-desktop-performance-integrated` exits zero. **462/462 observations
pass**, comprising 11 scenarios × 2 populations × 21 attempts. Source fingerprints
before/after match. The one first interaction is not pooled with the twenty warm
samples. No production change was made to improve these values.

The measured source still matches at receipt creation.

| Scenario | Small warm p50 / p95 ms | Large warm p50 / p95 ms |
|---|---:|---:|
| Full hierarchy | 261.4 / 322.3 | 261.6 / 377.9 |
| Flat Sessions | 265.0 / 325.4 | 282.7 / 416.1 |
| Active Monitor | 269.8 / 868.2 | 363.4 / 859.8 |
| Retained Session return | 298.8 / 352.3 | 326.4 / 495.6 |
| Combined pane restore | 287.7 / 361.1 | 303.5 / 419.8 |

These are captured/verified-content and input durations including observer costs,
not compositor paint budgets or a diagnosis of the Monitor tail. The baseline
precedes optimization. An earlier 462/462 pass preceded the concurrent Podium/DTO
correction; it remains tied to its own source. Comparison with that earlier run
correctly reports **unavailable: different measurement source**, because the shared
fixture contract changed. No optimized after-build or improvement is claimed.

Full per-attempt records, host/build/source identity, native log and reports are
retained in [the ignored evidence directory](../.lf/tmp/desktop-performance/integrated-baseline/).
The [curated receipt](desktop-performance-receipt.json) pins their hashes and all
warm results. Earlier development attempts remain under `/tmp/loo291-desktop-perf-*`;
none contributes to this final distribution.

`uv run pytest -q python/tests/test_desktop_performance.py` passes three focused
report tests: interrupted/unstarted journeys, first/warm percentile eligibility,
and comparison refusal across source drift or differing endpoints. Ruff and
`git diff --check` pass. The native command compiled the measured Swift source;
no broad gate, fallback build, configured provider interaction, publication or
Task completion occurred.
