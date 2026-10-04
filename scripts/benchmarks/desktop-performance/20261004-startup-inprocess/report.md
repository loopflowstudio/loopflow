| Scenario | Samples | Usable from | Usable median / p95 / max ms | Init on main thread median / p95 ms | Settled median ms | Status | `lf` reads before usable | `lf` reads until settled |
|---|---|---|---|---|---|---|---|---|
| uncached | 20 | read | 13181.1 / 13186.7 / 13192.1 | 27.6 / 36.4 | 13181.1 | current | roadmap 1, session 2, wave 1 | activity 1, roadmap 1, session 2, wave 1 |
| saved | 20 | saved | 9.6 / 9.8 / 9.9 | 9.6 / 9.8 | 13163.3 | current | none | activity 1, roadmap 1, session 2, wave 1 |
| saved_offline | 20 | saved | 9.5 / 9.7 / 9.8 | 9.5 / 9.7 | 62.4 | failed | none | activity 1, roadmap 1, session 1, wave 1 |
| warm_reopen | 20 | saved | 6.5 / 6.7 / 6.7 | 6.5 / 6.7 | 13160.3 | current | none | activity 1, roadmap 1, session 2, wave 1 |
