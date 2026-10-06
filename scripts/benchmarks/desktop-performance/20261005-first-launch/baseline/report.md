# Desktop launch, rendered

Release build of `bee8dcc49-dirty, 28.5 MB executable`, `lf 0.13.3`, Apple M4 Max. Home copy: 1698 MB database.
Budgets: first frame 400 ms, usable 1000 ms. Milliseconds from kernel process start; p95 needs 20 samples.

## saved_first_launch

20 launches, 20 reached the endpoint; saved workspace {'hit': 20}; usable from {'saved': 20}.

| | samples | median | p95 | max |
|---|---|---|---|---|
| pre_main ms | 20 | 427 | 524 | 645 |
| restored ms | 20 | 587 | 678 | 809 |
| first_frame ms | 20 | 832 | 930 | 1062 |
| usable ms | 20 | 832 | 930 | 1061 |
| fresh ms | 0 | — | — | — |
| rss_mb | 20 | 116.0 | 116.6 | 117.0 |
| cpu_s | 20 | 0.4 | 0.5 | 0.5 |

`lf` processes across these launches: {'home id': 20, 'session list': 20, 'ps --json': 20, 'roadmap --all': 20, 'wave list': 20}; failed reads: none; refused: {'repo ci': 20}; saved workspace kept: True.

## saved

4 launches, 3 reached the endpoint; saved workspace {'hit': 4}; usable from {'saved': 4}.

| | samples | median | p95 | max |
|---|---|---|---|---|
| pre_main ms | 4 | 35 | — | 37 |
| restored ms | 4 | 184 | — | 190 |
| first_frame ms | 4 | 430 | — | 441 |
| usable ms | 4 | 429 | — | 441 |
| fresh ms | 3 | 25530 | — | 27619 |
| rss_mb | 4 | 139.1 | — | 146.0 |
| cpu_s | 4 | 0.8 | — | 0.9 |
| `home id` ms | 4 | 7728 | — | 37476 |
| `ps` ms | 4 | 7276 | — | 30878 |
| `refresh planning` ms | 3 | 25130 | — | 27174 |
| `refresh sessions` ms | 6 | 10732 | — | 13966 |
| `roadmap` ms | 3 | 25124 | — | 27167 |
| `session list` ms | 6 | 10732 | — | 13965 |
| `wave list` ms | 4 | 8552 | — | 53924 |

`lf` processes across these launches: {'home id': 4, 'wave list': 4, 'roadmap --all': 4, 'ps --json': 4, 'session list': 10, 'activity --since': 3}; failed reads: none; refused: {'repo ci': 4}; saved workspace kept: True.

Not measured: main-thread stalls, on-glass presentation, launches with cold OS file caches.
