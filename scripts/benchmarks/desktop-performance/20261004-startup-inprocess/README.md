# Startup, in-process, 2026-10-04

```sh
uv run python scripts/benchmarks/desktop-performance/startup.py capture --repo ~/src/loopflow --output /tmp/startup-capture
uv run python scripts/benchmarks/desktop-performance/startup.py run --capture /tmp/startup-capture --output /tmp/startup-run
```

Five samples per scenario, debug build, replaying one capture of Jack Heart's
real Home (`lf` 0.12.32) with each read's recorded wall time. `uncached` is
also the baseline: before the saved workspace, every launch took this path.

| Scenario | Usable from | Usable median / max ms | Settled median ms | Status |
|---|---|---|---|---|
| uncached | read | 16371 / 16555 | 16371 | current |
| saved | saved | 11.7 / 13.7 | 16381 | current |
| saved_offline | saved | 10.1 / 11.8 | 63 | failed, content kept |

The endpoint is the model holding outline content. This is lower-level timing:
no frame was rendered, and process launch, pre-main, CPU, memory and
main-thread stalls after init are not measured. Five samples give no p95. The
capture itself holds private planning text and is not kept here.
