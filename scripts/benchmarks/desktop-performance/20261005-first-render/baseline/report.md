# Desktop launch, rendered

Release build of `8ea0bec9c`, `lf 0.13.3`, Apple M4 Max. Home copy: 1304 MB database.
Budgets: first frame 400 ms, usable 1000 ms. Milliseconds from kernel process start; p95 needs 20 samples.

## saved

20 launches, 20 reached the endpoint; saved workspace {'hit': 20}; usable from {'saved': 20}.

| | samples | median | p95 | max |
|---|---|---|---|---|
| pre_main ms | 20 | 35 | 40 | 44 |
| restored ms | 20 | 101 | 119 | 122 |
| first_frame ms | 20 | 850 | 1087 | 1177 |
| usable ms | 20 | 850 | 1087 | 1177 |
| fresh ms | 20 | 42197 | 47244 | 52327 |
| rss_mb | 20 | 154.8 | 156.1 | 176.6 |
| cpu_s | 20 | 1.9 | 2.5 | 2.6 |
| `home id` ms | 20 | 6598 | 7986 | 7994 |
| `ps` ms | 20 | 6717 | 7680 | 7780 |
| `refresh planning` ms | 20 | 41290 | 46326 | 50913 |
| `refresh sessions` ms | 38 | 17075 | 27453 | 27863 |
| `roadmap` ms | 20 | 41260 | 46315 | 50725 |
| `session list` ms | 38 | 17072 | 27438 | 27862 |
| `wave list` ms | 20 | 9112 | 10559 | 12688 |

`lf` processes across these launches: {'home id': 20, 'ps --json': 20, 'session list': 54, 'roadmap --all': 20, 'wave list': 20, 'activity --since': 19}; failed reads: {'task comment': 20, 'flow list': 20, 'task status': 20}; refused: {'repo ci': 20, 'task comment': 20, 'flow list': 20, 'task status': 20}; saved workspace kept: True.

Not measured: main-thread stalls, on-glass presentation, launches with cold OS file caches.
