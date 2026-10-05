# Desktop launch, rendered

Release build of `58d3b5b3e-dirty`, `lf 0.13.0`, Apple M4 Max. Home copy: 967 MB database.
Budgets: first frame 400 ms, usable 1000 ms. Milliseconds from kernel process start; p95 needs 20 samples.

## uncached

3 launches, 3 reached the endpoint; saved workspace {'miss': 3}; usable from {'fresh': 3}.

| | samples | median | p95 | max |
|---|---|---|---|---|
| pre_main ms | 3 | 34 | — | 489 |
| restored ms | 3 | 87 | — | 533 |
| first_frame ms | 3 | 624 | — | 984 |
| usable ms | 3 | 12104 | — | 12672 |
| fresh ms | 3 | 22005 | — | 23567 |
| rss_mb | 3 | 122.8 | — | 123.1 |
| cpu_s | 3 | 1.0 | — | 1.1 |
| `home id` ms | 3 | 4235 | — | 4506 |
| `ps` ms | 3 | 3910 | — | 4193 |
| `refresh planning` ms | 3 | 21019 | — | 22942 |
| `refresh sessions` ms | 6 | 9547 | — | 12015 |
| `roadmap` ms | 3 | 21008 | — | 22931 |
| `session list` ms | 6 | 9546 | — | 12013 |
| `wave list` ms | 3 | 5160 | — | 5859 |

`lf` processes across these launches: {'home id': 3, 'ps --json': 3, 'roadmap --all': 3, 'session list': 6, 'wave list': 3, 'activity --since': 3}; failed reads: none; refused: {'repo ci': 3}; saved workspace kept: True.

## saved

20 launches, 20 reached the endpoint; saved workspace {'hit': 20}; usable from {'saved': 20}.

| | samples | median | p95 | max |
|---|---|---|---|---|
| pre_main ms | 20 | 36 | 40 | 41 |
| restored ms | 20 | 89 | 95 | 100 |
| first_frame ms | 20 | 506 | 540 | 616 |
| usable ms | 20 | 506 | 540 | 616 |
| fresh ms | 20 | 19330 | 20984 | 21114 |
| rss_mb | 20 | 144.7 | 145.8 | 145.9 |
| cpu_s | 20 | 1.0 | 1.1 | 1.1 |
| `home id` ms | 20 | 3968 | 4317 | 4410 |
| `ps` ms | 20 | 3726 | 4067 | 4150 |
| `refresh planning` ms | 20 | 18821 | 20472 | 20644 |
| `refresh sessions` ms | 39 | 8328 | 10854 | 12071 |
| `roadmap` ms | 20 | 18815 | 20466 | 20636 |
| `session list` ms | 39 | 8327 | 10853 | 12070 |
| `wave list` ms | 20 | 4886 | 5516 | 5780 |

`lf` processes across these launches: {'home id': 20, 'ps --json': 20, 'session list': 40, 'wave list': 20, 'roadmap --all': 20, 'activity --since': 20}; failed reads: none; refused: {'repo ci': 20}; saved workspace kept: True.

## saved_refresh_fails

3 launches, 3 reached the endpoint; saved workspace {'hit': 3}; usable from {'saved': 3}.

| | samples | median | p95 | max |
|---|---|---|---|---|
| pre_main ms | 3 | 38 | — | 40 |
| restored ms | 3 | 99 | — | 102 |
| first_frame ms | 3 | 590 | — | 634 |
| usable ms | 3 | 590 | — | 634 |
| fresh ms | 0 | — | — | — |
| rss_mb | 3 | 118.6 | — | 118.6 |
| cpu_s | 3 | 0.7 | — | 0.7 |

`lf` processes across these launches: {'home id': 3, 'ps --json': 3, 'wave list': 3, 'roadmap --all': 3, 'session list': 3}; failed reads: {'roadmap': 3, 'session list': 3, 'refresh sessions': 3}; refused: {'repo ci': 3}; saved workspace kept: True.

Not measured: main-thread stalls, on-glass presentation, launches with cold OS file caches.
