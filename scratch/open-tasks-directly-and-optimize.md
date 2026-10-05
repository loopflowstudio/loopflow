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

[October 5 evidence](../scripts/benchmarks/desktop-performance/20261005-task-open/README.md):
same frozen snapshot, `lf` binary and runner, five samples each, load 32–65.

| Scenario | Base `8ea0bec9c` median ms | Branch median / max ms | Budget | Sheet |
|---|---:|---:|---:|---:|
| Cold workspace | 12,256 | 8,360 / 10,978 | 5,000 — missed | 5/5 → 0/5 |
| Warm Task | 9,059 | 152 / 163 | 250 — met 5/5 | 5/5 → 0/5 |
| Reopen Task | 9,688 | 38 / 44 | 250 — met 5/5 | 5/5 → 0/5 |

One window on the branch, two on the base. No timeouts. p95 is unavailable (needs
twenty samples) and load contaminates absolute timings; the warm/reopen difference
is the removed `roadmap --task` read, not load. Cold still waits on that read
(8.2 s median) and cannot meet 5,000 ms while one `lf` read costs more than the
budget; real cold launches go through LOO-376's saved workspace, unmeasured here.

The earlier 9/9 timeout sweep was the observer: fast OCR read the breadcrumb as
`LOO- 368` and the runner compared verbatim. It now ignores whitespace and names
the unmet condition on timeout. October 4 timeouts may share that cause; see the
[October 4 record](../scripts/benchmarks/desktop-performance/20261004-task-open/README.md)
for the budgets' origin and the interrupted runs. Bitmap capture is not compositor
presentation; owned PTYs are not provider startup; key-window observations were
always `-1`, so focus is unproven.

## Remaining

- Cold budget: needs a Task deep-link scenario in LOO-376's `launch.py` (real
  bundle, private Home, saved workspace) and faster `lf` reads, which no Task owns.
  Its Home copy and this runner's `_snapshot` duplicate one SQLite backup; share one
  when that scenario is added.
- Twenty samples per scenario on a quiet host for p95.
- Session deep-link and retained-input usability, and reliable focus observation.

Sheet removal, single window and the warm/reopen budgets hold in sampled attempts.
The cold budget, Session readiness and every KR remain unmet or unscored.

Checks (after compression: loaded links open through one match lookup instead of re-filtering a snapshot; runner sources changed, so new runs do not compare against the October 5 reports): `swift test --filter WorkspaceDestinationTests` 16 passed; `uv run pytest python/tests/test_desktop_performance.py -q` 13 passed; ruff clean. Rust unchanged (`cargo fmt --check`, Clippy reused). Snapshot sweep not rerun; gate owns affected suites.
