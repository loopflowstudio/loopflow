# Desktop launch, rendered

Release build of `8ea0bec9c-dirty`, `lf 0.13.3`, Apple M4 Max. Home copy: 1304 MB database.
Budgets: first frame 400 ms, usable 1000 ms. Milliseconds from kernel process start; p95 needs 20 samples.

## saved

20 launches, 20 reached the endpoint; saved workspace {'hit': 20}; usable from {'saved': 20}.

| | samples | median | p95 | max |
|---|---|---|---|---|
| pre_main ms | 20 | 35 | 38 | 488 |
| restored ms | 20 | 220 | 284 | 747 |
| first_frame ms | 20 | 617 | 880 | 1320 |
| usable ms | 20 | 617 | 880 | 1320 |
| fresh ms | 20 | 43107 | 49131 | 54768 |
| rss_mb | 20 | 150.9 | 154.5 | 166.9 |
| cpu_s | 20 | 1.2 | 1.7 | 1.8 |
| `home id` ms | 20 | 6989 | 9260 | 9749 |
| `ps` ms | 20 | 6893 | 10704 | 11737 |
| `refresh planning` ms | 20 | 42412 | 47800 | 53875 |
| `refresh sessions` ms | 39 | 16881 | 27066 | 29652 |
| `roadmap` ms | 20 | 42398 | 47676 | 53533 |
| `session list` ms | 39 | 16880 | 27064 | 29649 |
| `wave list` ms | 20 | 10049 | 13059 | 15334 |

`lf` processes across these launches: {'home id': 20, 'ps --json': 20, 'roadmap --all': 20, 'session list': 54, 'wave list': 20, 'activity --since': 20}; failed reads: {'task status': 20, 'flow list': 20, 'task comment': 20}; refused: {'repo ci': 20, 'task status': 20, 'flow list': 20, 'task comment': 20}; saved workspace kept: True.

## saved_refresh_fails

3 launches, 3 reached the endpoint; saved workspace {'hit': 3}; usable from {'saved': 3}.

| | samples | median | p95 | max |
|---|---|---|---|---|
| pre_main ms | 3 | 34 | — | 35 |
| restored ms | 3 | 215 | — | 238 |
| first_frame ms | 3 | 591 | — | 679 |
| usable ms | 3 | 591 | — | 676 |
| fresh ms | 0 | — | — | — |
| rss_mb | 3 | 125.7 | — | 126.5 |
| cpu_s | 3 | 0.7 | — | 0.8 |

`lf` processes across these launches: {'home id': 3, 'ps --json': 3, 'session list': 3, 'roadmap --all': 3, 'wave list': 3}; failed reads: {'task status': 3, 'flow list': 3, 'task comment': 3, 'session list': 3, 'refresh sessions': 3, 'roadmap': 3}; refused: {'task status': 3, 'repo ci': 3, 'task comment': 3, 'flow list': 3}; saved workspace kept: True.

Not measured: main-thread stalls, on-glass presentation, launches with cold OS file caches.
