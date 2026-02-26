# 06: lfq Usage

CLI interface for usage data.

## What to build

```bash
lfq usage                    # global summary
lfq usage --wave engbot      # per-wave
lfq usage --model opus       # per-model
lfq usage --step implement   # per-step
lfq usage --prompt           # prompt composition view
lfq usage --from 2026-02-01  # time-filtered
```

Reads from the usage API. Tabular terminal output, composable with other shell tools.

## Done when

`lfq usage --wave engbot` prints token summary to terminal.
