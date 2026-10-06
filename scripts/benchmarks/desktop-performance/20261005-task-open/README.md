# LOO-371 — October 5, 2026

Opening LOO-368 on the branch lands in its workspace with one window and no
Task-link sheet in 15/15 sampled attempts. Warm and reopen meet their 250 ms
budget in every sample; cold misses 5,000 ms. Host load was 32–65 on 16 CPUs
throughout, so the absolute timings are contaminated and no p95 exists.

## Same snapshot, same `lf`, same runner

Both runs used the frozen backup `/tmp/loo371-frozen-20261005` (342 Tasks, 604
conversations, 521,637 Session events; private, never committed), the `lf`
binary `4532649b…` and measurement source `ba306d5c…`. SwiftPM debug `-gnone`, five
samples each. The baseline swapped the four changed product sources and their
destination tests back to `8ea0bec9c`; the measurement test itself is identical.

| Scenario | Baseline median / max ms | Branch median / max ms | Budget ms | Sheet, baseline → branch | Max windows |
|---|---:|---:|---:|---:|---:|
| Cold workspace | 12,256 / 16,917 | 8,360 / 10,978 | 5,000 | 5/5 → 0/5 | 2 → 1 |
| Warm Task open | 9,059 / 16,235 | 152 / 163 | 250 | 5/5 → 0/5 | 2 → 1 |
| Reopen Task | 9,688 / 11,968 | 38 / 44 | 250 | 5/5 → 0/5 | 2 → 1 |

No attempt timed out in either run. The baseline ran second and load rose from
32 to 65 during it; the branch saw 32–38. That can inflate the baseline by some
factor, not by the sixty-fold warm difference: the baseline waits on a
`roadmap --task` read (median 9.3 s) for every open, and the branch reuses the
Task evidence it already holds.

Cold still waits on that one read. On the branch it took 8.2 s median and
`home id` 5.6 s; the cold budget cannot pass while a single `lf` read costs more
than the budget. `roadmap --all` exceeded the runner's 30-second read limit in
4/5 branch samples and 5/5 baseline samples and was reported as unavailable.
LOO-376's saved workspace is the real cold path and is not exercised here.

## The 9/9 timeout was the observer

`after-ocr-timeout/` keeps the earlier interrupted sweep: nine attempts, nine
45-second timeouts. The workspace had rendered each time. Fast text recognition
read the breadcrumb's mono identifier as `LOO- 368`, and the runner compared
recognized lines against `LOO-368` verbatim. It now compares with whitespace
removed and records which condition was unmet on a timeout. The two timeouts in
the October 4 baseline and the six in its after run used the same comparison,
so they may share this cause; those journals cannot confirm it.

## Limits

- Five samples per scenario; p95 needs twenty. One ordering, one host, heavy load.
- In-process bitmap capture plus OCR, not compositor presentation. Key window
  was `-1` in every observation, so focus is unproven.
- `ps`, `activity`, `flow list`, `task status` and `task comment` return
  unavailable against the copy in both runs; `session connect` never runs. Session
  usability and provider startup are unmeasured.
- A new Podium in a test window is not an OS launch. A Task deep link through
  `launch.py` remains to be added.
