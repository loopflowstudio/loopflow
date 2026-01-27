---
area: cli
---
# CLI UX Polish

Small improvements to help text and diagnostics.

## Items

### Help text for `--direction`

`lf run --help` shows `--direction -d,-D TEXT Direction to apply (repeatable, or comma-separated)` with no explanation of what directions are or where they come from.

Add context: "Direction from .lf/directions/ or built-in (e.g., product-engineer)"

### Doctor output clarity

`lfops doctor` shows "no task files (run: lf init)" which is cryptic. Should say ".lf/steps/ or .claude/commands/ not found".

### Help text formatting

`lfd loop --help` example shows word-wrap artifacts. Use actual newlines in help strings.

## Lower priority

### Uppercase flag aliases

`--auto -a,-A` and similar show uppercase aliases that add visual noise. Consider removing in future cleanup.
