# Startup, in-process, 2026-10-04

```sh
uv run python scripts/benchmarks/desktop-performance/startup.py capture --repo ~/src/loopflow --output /tmp/startup-capture
uv run python scripts/benchmarks/desktop-performance/startup.py run --capture /tmp/startup-capture --output /tmp/startup-run
```

Twenty samples per scenario, debug build, replaying one capture of Jack Heart's
real Home (`lf` 0.12.32) with each read's recorded wall time. [report.md](report.md)
has the table; `report.json` and `journal.ndjson` hold every sample.

- `uncached` is also the baseline: before the saved workspace, every launch
  took this path. Its 28 ms on the main thread at init is the `git` check of a
  repository no saved workspace names.
- `saved` is a new process reading the file; `warm_reopen` is a second window
  in the process that saved.
- `saved_offline` settles as failed with the saved content kept.

The endpoint is the model holding outline content. This is lower-level timing:
no frame was rendered, and process launch, pre-main, CPU, memory and
main-thread stalls after init are not measured. The capture itself holds
private planning text and is not kept here.
