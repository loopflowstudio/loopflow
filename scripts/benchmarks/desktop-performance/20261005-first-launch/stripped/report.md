# Desktop launch, rendered

Release build of `bee8dcc49-dirty, 17.5 MB executable without local symbols`, `lf 0.13.3`, Apple M4 Max. Home copy: 1698 MB database.
Budgets: first frame 400 ms, usable 1000 ms. Milliseconds from kernel process start; p95 needs 20 samples.

## saved_first_launch

20 launches, 20 reached the endpoint; saved workspace {'hit': 20}; usable from {'saved': 20}.

| | samples | median | p95 | max |
|---|---|---|---|---|
| pre_main ms | 20 | 389 | 454 | 569 |
| restored ms | 20 | 538 | 617 | 750 |
| first_frame ms | 20 | 779 | 878 | 997 |
| usable ms | 20 | 778 | 876 | 997 |
| fresh ms | 0 | — | — | — |
| rss_mb | 20 | 116.0 | 116.6 | 116.6 |
| cpu_s | 20 | 0.4 | 0.5 | 0.5 |

`lf` processes across these launches: {'home id': 20, 'ps --json': 20, 'wave list': 20, 'roadmap --all': 20, 'session list': 20}; failed reads: none; refused: {'repo ci': 20}; saved workspace kept: True.

## saved

4 launches, 4 reached the endpoint; saved workspace {'hit': 4}; usable from {'saved': 4}.

| | samples | median | p95 | max |
|---|---|---|---|---|
| pre_main ms | 4 | 33 | — | 36 |
| restored ms | 4 | 190 | — | 192 |
| first_frame ms | 4 | 438 | — | 457 |
| usable ms | 4 | 438 | — | 457 |
| fresh ms | 4 | 26364 | — | 26919 |
| rss_mb | 4 | 145.7 | — | 150.5 |
| cpu_s | 4 | 0.9 | — | 0.9 |
| `home id` ms | 4 | 7742 | — | 8717 |
| `ps` ms | 4 | 7273 | — | 8281 |
| `refresh planning` ms | 4 | 25910 | — | 26484 |
| `refresh sessions` ms | 8 | 11092 | — | 14502 |
| `roadmap` ms | 4 | 25903 | — | 26478 |
| `session list` ms | 8 | 11091 | — | 14501 |
| `wave list` ms | 4 | 8580 | — | 9590 |

`lf` processes across these launches: {'home id': 4, 'ps --json': 4, 'roadmap --all': 4, 'session list': 12, 'wave list': 4, 'activity --since': 4}; failed reads: none; refused: {'repo ci': 4}; saved workspace kept: True.

Not measured: main-thread stalls, on-glass presentation, launches with cold OS file caches.
