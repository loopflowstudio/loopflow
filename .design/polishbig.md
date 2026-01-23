# polishbig: Support flags without step name

Make `lf -m codex` and `lf --web` work without requiring a step/prompt argument.

## What to build

`lf` with flags but no step name should launch an interactive session with those flags applied. Currently requires a step name when any flags are present.

## Current behavior

```bash
lf                    # ✓ works → lf run --interactive
lf review             # ✓ works → lf run review
lf -m codex           # ✗ fails → "No step or flow named '-m'"
lf --web              # ✗ fails → "No step or flow named '--web'"
lf -m codex review    # ✓ works (step comes after flag)
```

## Desired behavior

```bash
lf -m codex           # launch interactive codex session
lf --web              # copy docs context to clipboard, open claude.ai
lf -m codex --web     # copy docs context to clipboard, open chatgpt.com
lf -v                 # interactive session with clipboard pasted
```

## Data structures

No new types needed. The change is in argument parsing.

## Key functions

```python
# src/loopflow/lf/__init__.py

def main():
    ...
    if len(sys.argv) > 1:
        first_arg = sys.argv[1]

        # NEW: If first arg is a flag, prepend "run" so typer parses it
        if first_arg.startswith("-"):
            sys.argv.insert(1, "run")
        elif first_arg == ":":
            ...
```

The `run` command already handles `step=None` via `_launch_interactive_default()`.

## Constraints

- Must preserve existing behavior for all current patterns
- `lf :` must still work for inline prompts
- `lf step: args` must still work
- Flag order shouldn't matter: `lf -m codex -v` and `lf -v -m codex` both work

## Done when

```bash
# All pass:
lf -m codex           # launches codex interactive (or exits 0 with token breakdown if --copy implied)
lf --web              # copies to clipboard, opens claude.ai
lf -m codex --web     # copies to clipboard, opens chatgpt.com
lf -v                 # interactive with clipboard
lf -c                 # shows token breakdown, copies to clipboard
lf                    # still works (interactive claude)
lf review             # still works
lf review -m codex    # still works
```
