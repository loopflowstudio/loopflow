---
requires: current wave context or explicit wave name
produces: refreshed local wave files from PM
---
Pull PM state into the current wave.

## Workflow

Run exactly one command:

```bash
lf ops pm pull
```

If the current wave cannot be auto-detected, run the same command with an explicit wave name.
