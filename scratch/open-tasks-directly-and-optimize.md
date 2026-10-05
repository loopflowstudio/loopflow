# Direct Task opening — LOO-371

Jack Heart authorized autonomous local implementation and measurement on October 3,
2026. Jack's October 4 steer authorizes direct lf execution and landing when ready,
without restarting the Task worker or historical Flows. LOO-368 is the reproduction
case; LOO-372 owns the default interactive unfinished Session population. LOO-376
owns cold startup, persistent caching and unified refresh; integrate its work
instead of adding another cache. This Task retains routing, window churn and the runner.

## Intended cut

Keep one workspace window and its retained panes. Resolve unknown Task identities
without presenting a successful-path sheet; show actual missing/ambiguous errors.
Reuse exact loaded Task/Session evidence for warm navigation. Prefer an existing
window holding the destination over an unrelated key window. Preserve historical
records and retained drafts. No copied Session may launch a provider or control a
live process.

Delete — do not maintain: successful-path TaskLinkView loading presentation and
unconditional full-inventory rereads for already retained destinations; the separate
SwiftPM test-launch path for fixture measurements. These cuts are implemented. Preserve
missing/ambiguous selection and retry, generation fencing and exact membership.

## Measurement

Extend scripts/desktop_performance.py and the native experience instruments. Use
SQLite online backup of the current Home, retain all rows, and permit only read
commands against the copy. Record snapshot hash/counts, process reads, windows,
focus and intermediate states. Baseline precedes production edits. Native bitmap
capture is not compositor presentation; owned local PTYs are not provider startup.
The first native baseline observed cold workspace captures at 12.1–15.2 seconds
and warm Task captures at 4.5–4.9 seconds, with a transient sheet window. Reopening
also exposed a 45-second unresolved-reading timeout. Before production UI edits,
the October 4 targets are cold workspace ≤5,000 ms and warm/reopen ≤250 ms, with
zero successful-path sheets or extra windows. These are optimization targets from
the observed baseline, not claimed product acceptance or compositor budgets.
Session readiness has no scored budget until a credible baseline exists.

## Remaining

Routing and cached reopen changes are implemented. Compression removed the early
Session-page exit: publishing a partial inventory could remove retained panes on
later pages. Unknown Session links retain full enumeration. Planning/runtime IDs
both preserve a selected Task's Session. LOO-372 owns filtering; LOO-304 owns
polling/projection. No lifecycle or history migration is part of this change.

The [dated evidence](../scripts/benchmarks/desktop-performance/20261004-task-open/README.md)
preserves all final journals, snapshot identity, before/after medians, timeouts,
window observations and limits. Interrupted SwiftPM children continued writing;
overlap invalidates latency comparison. The runner now owns the native helper
directly and drains reads between samples. SIGTERM and timeout now stop its owned
process group during both build and measurement, including children remaining
after the leader exits. Focused subprocess tests cover interruption and timeout;
the revised native measurement still needs execution. Compression routes both
fixture and snapshot measurements through that same owned native-process path,
with an exact test filter. Archived launcher runs require fresh baselines.

Capacity still blocks fresh native builds/measurement; the dated evidence records
the latest observation. A capable host still needs the latest Swift
build/focused tests and Clippy, then comparable baseline/after samples. OS cold
launch/URL dispatch, Session deep-link/input usability, reliable focus, and absence
of an unrelated cold landing screen remain acceptance work, with startup changes
owned by LOO-376. No target or KR passes.

Checks: `uv run pytest python/tests/test_desktop_performance.py -q` 13 passed; Ruff passed; earlier cargo fmt and Swift syntax
passed; earlier resolver tests 2 and destination tests 16 passed; latest Swift
behavior/build and Clippy deferred to a host with capacity, broader suites to gate.
