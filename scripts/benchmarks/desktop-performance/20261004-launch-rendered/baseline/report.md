# Desktop launch, rendered

Release build of `58d3b5b3e`, `lf 0.13.0`, Apple M4 Max. Home copy: 966 MB database.
Budgets: first frame 400 ms, usable 1000 ms. Milliseconds from kernel process start; p95 needs 20 samples.

## uncached

3 launches, 3 reached the endpoint; saved workspace {'miss': 3}; usable from {'fresh': 3}.

| | samples | median | p95 | max |
|---|---|---|---|---|
| pre_main ms | 3 | 35 | — | 37 |
| restored ms | 3 | 68 | — | 72 |
| first_frame ms | 3 | 494 | — | 502 |
| usable ms | 3 | 10218 | — | 11039 |
| fresh ms | 3 | 17102 | — | 19142 |
| rss_mb | 3 | 123.0 | — | 123.1 |
| cpu_s | 3 | 1.0 | — | 1.0 |
| `home id` ms | 3 | 3702 | — | 3774 |
| `ps` ms | 3 | 3406 | — | 3478 |
| `refresh planning` ms | 3 | 16618 | — | 18647 |
| `refresh sessions` ms | 3 | 9711 | — | 10519 |
| `roadmap` ms | 3 | 16611 | — | 18639 |
| `session list` ms | 3 | 9710 | — | 10518 |
| `wave list` ms | 3 | 4935 | — | 4954 |

`lf` processes across these launches: {'home id': 3, 'ps --json': 3, 'roadmap --all': 3, 'session list': 6, 'wave list': 3, 'activity --since': 3}; failed reads: none; refused: {'repo ci': 3}; saved workspace kept: True.

## saved

20 launches, 20 reached the endpoint; saved workspace {'hit': 20}; usable from {'saved': 20}.

| | samples | median | p95 | max |
|---|---|---|---|---|
| pre_main ms | 20 | 33 | 38 | 38 |
| restored ms | 20 | 90 | 104 | 108 |
| first_frame ms | 20 | 736 | 934 | 949 |
| usable ms | 20 | 735 | 934 | 948 |
| fresh ms | 20 | 21354 | 24310 | 24861 |
| rss_mb | 20 | 145.2 | 146.9 | 147.2 |
| cpu_s | 20 | 1.6 | 1.8 | 1.8 |
| `home id` ms | 20 | 4380 | 5210 | 5409 |
| `ps` ms | 20 | 4055 | 4851 | 5073 |
| `refresh planning` ms | 20 | 20658 | 23571 | 23910 |
| `refresh sessions` ms | 36 | 10050 | 12104 | 13125 |
| `roadmap` ms | 20 | 20650 | 23563 | 23904 |
| `session list` ms | 36 | 10049 | 12103 | 13124 |
| `wave list` ms | 20 | 5421 | 6500 | 6792 |

`lf` processes across these launches: {'home id': 20, 'ps --json': 20, 'roadmap --all': 20, 'wave list': 20, 'session list': 40, 'activity --since': 20}; failed reads: none; refused: {'repo ci': 20}; saved workspace kept: True.

## saved_refresh_fails

3 launches, 3 reached the endpoint; saved workspace {'hit': 3}; usable from {'saved': 3}.

| | samples | median | p95 | max |
|---|---|---|---|---|
| pre_main ms | 3 | 32 | — | 35 |
| restored ms | 3 | 103 | — | 104 |
| first_frame ms | 3 | 872 | — | 896 |
| usable ms | 3 | 871 | — | 896 |
| fresh ms | 0 | — | — | — |
| rss_mb | 3 | 120.1 | — | 121.5 |
| cpu_s | 3 | 1.1 | — | 1.1 |

`lf` processes across these launches: {'home id': 3, 'ps --json': 3, 'roadmap --all': 3, 'wave list': 3, 'session list': 3}; failed reads: {'roadmap': 3, 'session list': 3, 'refresh sessions': 3}; refused: {'repo ci': 3}; saved workspace kept: True.

Not measured: main-thread stalls, on-glass presentation, launches with cold OS file caches.

Original 12 launches plus 14 resumed launches after the parent agent exited; same preserved app and Home snapshot. Concurrent development continued on this host.
