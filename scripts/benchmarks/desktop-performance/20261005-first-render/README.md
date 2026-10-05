# First render, 2026-10-05

```sh
uv run python scripts/benchmarks/desktop-performance/launch.py run --work /tmp/launch/base --built --output /tmp/launch/alt/base-1 --uncached 0 --saved 4 --failed 0
uv run python scripts/benchmarks/desktop-performance/launch.py run --work /tmp/launch/candidate --built --home /tmp/launch/base/home --output /tmp/launch/alt/cand-1 --uncached 0 --saved 4 --failed 0
```

Real launches of the release app on Jack Heart's machine (Apple M4 Max, `lf`
0.13.3) against one private copy of his Home: a 1304 MB database. `baseline/` is
`8ea0bec9c`; `candidate/` is this change. Five rounds of four baseline then four
candidate launches, so both builds saw the same host load; each directory joins
its rounds into `report.md`, `report.json` and every sample in `journal.ndjson`.

Returning launch with a saved workspace, 20 samples each, milliseconds from
kernel process start:

| | baseline median / p95 | candidate median / p95 | budget |
|---|---|---|---|
| first frame | 850 / 1087 | 617 / 880 | 400 |
| usable workspace | 850 / 1087 | 617 / 880 | 1000 |
| CPU seconds at the endpoint | 1.9 / 2.5 | 1.2 / 1.7 | — |

Median first frame per round, baseline → candidate: 1016 → 867, 1038 → 775,
744 → 559, 705 → 599, 851 → 602.

A Time Profiler trace of one baseline launch put 923 ms of main-thread work
before the first frame. The window's views asked the model for the current
repository's Waves many times per render; every answer
standardized three file URLs per Wave (170 ms), and every answer that needed
the workspace matched each Task against every Session. The candidate keeps a
repository's identity until another origin is recorded, indexes Sessions by
Task in one pass, reads the selected Task once for the Task's Session list,
and no longer decodes the saved workspace for the Portfolio window at every
launch. The same trace of the candidate: 764 ms, 26 ms of it in those getters.

- `restored` moves from 101 to 220 ms because it now marks the window that is
  shown. Before, the unopened Portfolio window's model marked it first, so the
  20261004 receipt's "restored by 89 ms" understated the visible window.
- With planning and Sessions reads failing the candidate is usable at 591 ms
  (3 samples) and keeps the saved workspace.
- First frame still misses 400 ms on this host. What remains before it is
  mostly SwiftUI building and laying out the window (about 290 ms under the
  profiler), 45 ms decoding the saved workspace, 44 ms building the main menu
  and about 26 ms of font lookups.
- Each launch still runs `session list` two or three times, and every `lf`
  read costs 7 s or more here.

Limits. Load averages were 30 to 100 throughout from unrelated builds and
training, so absolute numbers are pessimistic and are not comparable with
20261004's; the comparison is within this run. Profiler numbers include its
overhead and come from one launch per build. First frame is the commit to the
render server, not on-glass presentation. No launch without a saved workspace,
quiet host, cold OS file cache or hitch measurement is in this run. The
restored selection made the app ask for `task status`, `flow list` and
`task comment`, which the benchmark `lf` refuses in both builds.
