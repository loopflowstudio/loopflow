# Direct Task opening — LOO-371

Jack Heart authorized autonomous local implementation and measurement on October 3,
2026. Jack's October 4 steer authorizes direct lf execution, publication and landing
when ready, without restarting the Task worker or historical Flows and without
interactive reviews. LOO-368 is the reproduction case. This Task retains routing,
window churn and the snapshot runner.

## Neighbors (reconciled October 5 against main `8ea0bec9c`)

- LOO-372 landed (#1421): ordinary navigation shows unfinished interactive Sessions.
  This branch sits on it; no filtering belongs here.
- LOO-376 landed real launch measurement (#1432, #1436): `launch.py` launches the
  release bundle against a private Home copy and reads the app's own launch journal.
  It owns cold startup, the saved workspace and unified refresh. No cache is added here.
- LOO-304 owns steady-state polling and projection; nothing of it is on main yet.

## Implemented (accepted cut)

One workspace window and its retained panes. Unknown Task identities resolve without
a successful-path sheet; `TaskLinkView` remains only for missing/ambiguous links and
retry. Exact loaded Task/Session evidence is reused for warm navigation, and a window
already holding the destination wins over an unrelated key window. Planning and
runtime IDs both preserve a selected Task's Session. Unknown Session links keep full
enumeration: publishing a partial inventory could remove retained panes on later
pages. Checkout-root resolution canonicalizes before launching Git (333 → 29 probes
in one profiled Session read). No lifecycle or history migration; no copied Session
launches a provider or controls a live process.

The runner (`scripts/desktop_performance.py`) takes an SQLite online backup, permits
only reads against the copy, and records snapshot hash/counts, reads, windows and
intermediate states. Fixture and snapshot modes share one owned native-process path;
SIGTERM and timeout stop the whole process group during build and measurement, and
reads drain between samples.

## Evidence and its limits

The [dated evidence](../scripts/benchmarks/desktop-performance/20261004-task-open/README.md)
keeps every journal, the snapshot identity, medians, timeouts and window observations.
Baseline: cold 12.1–15.2 s, warm 4.5–4.9 s, a 45 s reopen timeout, and the Task-link
sheet in 9/9 attempts. First implementation: one window and no sheet in 9/9 sampled
attempts. Its latency run overlapped a benchmark child that survived its launcher,
under host load, with a different `lf` build mode: no speedup can be inferred.
Targets chosen from the baseline before UI edits (October 4): cold workspace
≤5,000 ms, warm/reopen ≤250 ms, zero successful-path sheets or extra windows.
None is met or scored. Session readiness has no budget until a credible baseline
exists. Bitmap capture is not compositor presentation; owned PTYs are not provider
startup; key-window observations were always `-1`, so focus is unproven.

The coordinating Wave interrupted the earlier run at load 292 on 16 CPUs; no product
failure caused it. On October 5 free disk is 135 GiB (the 32 GiB reserve is no longer
a blocker) and one-minute load is 120, which still contaminates timing.

## Remaining

- Latest Swift build, focused destination tests and Clippy: the retention and runner
  revisions have syntax checking only. Owner: gate/CI.
- Comparable baseline and after samples with the revised runner, same snapshot and
  `lf` build mode, on a quiet host. Archived launcher runs cannot be reused.
- OS cold launch with a Task deep link, and absence of an unrelated landing screen:
  extend LOO-376's `launch.py` (real bundle, private Home, launch journal) with a
  link scenario instead of a third launch path. Its Home copy and this runner's
  `_snapshot` duplicate one SQLite backup; share one when that scenario is added.
- Session deep-link and retained-input usability, and reliable focus observation.

No target, acceptance bullet beyond sheet removal, or KR passes.

Checks: `uv run pytest python/tests/test_desktop_performance.py -q` 13 passed (reused, content unchanged); Swift build/tests and Clippy deferred to gate/CI.
