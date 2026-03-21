---
requires: current wave context or explicit wave name
produces: local wave state exported to PM
---
Push the current wave into PM.

## Workflow

Run exactly one command:

```bash
lf ops pm export
```

If the current wave cannot be auto-detected, run the same command with an explicit wave name.
