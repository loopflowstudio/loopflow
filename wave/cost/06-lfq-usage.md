# 06: lfq Usage

**Finish line:** `lfq usage --wave engbot` prints token summary to terminal. `lfq providers` lists providers with models and auth status.

## What to build

```bash
lfq usage                    # global summary
lfq usage --wave engbot      # per-wave
lfq usage --model opus       # per-model
lfq usage --step implement   # per-step
lfq usage --prompt           # prompt composition view
lfq usage --from 2026-02-01  # time-filtered
lfq providers                # list providers with auth status and models
```

Reads from usage and providers APIs. Tabular terminal output, composable with other shell tools.

`lfq providers` reads from `GET /v0/providers` (Phase 03). `lfq auth zen` connects OpenCode Zen — the broker is ready, just needs a CLI subcommand.

