# Launch, rendered, 2026-10-04

```sh
uv run python scripts/benchmarks/desktop-performance/launch.py run --work /tmp/launch/base --output /tmp/launch/base/run
uv run python scripts/benchmarks/desktop-performance/launch.py run --work /tmp/launch/candidate --home /tmp/launch/base/home --output /tmp/launch/candidate/run
```

Real launches of the release app on Jack Heart's machine (Apple M4 Max, `lf`
0.13.0) against one private copy of his Home: a 966 MB database. `baseline/` is
`58d3b5b3e`; `candidate/` adds the navigator building its workspace projection
once per render instead of once per row menu. Each has `report.md`,
`report.json` and every sample in `journal.ndjson`.

Returning launch with a saved workspace, 20 samples each, milliseconds from
kernel process start:

| | baseline median / p95 | candidate median / p95 | budget |
|---|---|---|---|
| first frame | 736 / 934 | 506 / 540 | 400 |
| usable workspace | 735 / 934 | 506 / 540 | 1000 |
| everything fresh | 21354 / 24310 | 19330 / 20984 | — |
| CPU seconds at the endpoint | 1.6 / 1.8 | 1.0 / 1.1 | — |

- A returning launch is usable inside its budget in both builds and never
  waits on a read: with planning and Sessions reads failing it is usable at
  590 ms (3 samples) and the saved workspace is kept.
- First frame misses 400 ms. The saved workspace is restored by 89 ms; the
  remaining ~420 ms is the window's first render.
- A launch with nothing saved waits for `lf`: usable at 10–12 s, fresh at
  17–22 s (3 samples each; the difference between builds is within their
  spread). Every `lf` read costs at least 3.7 s on this Home, `home id`
  included; `roadmap --all` takes 19–21 s; `session list` runs twice per launch.
- The app started no `lf` command outside the startup reads except `repo ci`,
  which the bundle refused.

Limits. The host was under concurrent development load throughout, so absolute
numbers are pessimistic and the two runs did not see identical load. The
baseline is 12 launches from one run and 14 from a resumed run over the same
bundle and Home copy, after the first run's parent process exited. The
candidate's Home was copied from the baseline's. First frame is the commit to
the render server, not on-glass presentation. Main-thread stalls and launches
with cold OS file caches are not measured.
