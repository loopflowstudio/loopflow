# lintbuiltin: Make lint a built-in step

## What to build

Add `lint` as a built-in step so `lfops land` works out of the box without requiring per-repo lint.md files.

## Background

"In my other repo loopflowstudio it's complaining that there's no lf lint."

The `run_lint` helper in `lfops land` falls back to `lf lint -a` when the fast ruff check fails or can't run. But `lint` isn't a built-in step—it only exists in the loopflow repo's `.claude/commands/lint.md`.

## Data structures

No new data structures. Just moving `lint.md` to the built-in templates.

## Key functions

No new functions. The existing built-in step discovery (`_get_builtin_step` in `context.py`) handles everything.

## Changes

### 1. Add lint.md to built-in templates

Copy `.claude/commands/lint.md` → `src/loopflow/templates/steps/lint.md`

### 2. Remove repo-local lint.md

Delete `.claude/commands/lint.md` (now redundant).

### 3. Update docs

Update `docs/lf.md` to include `lint` in the built-in steps list:

```
Built-in steps: debug, design, implement, lint, polish, review
```

Also in `docs/workflow.md` if it mentions built-ins.

## Constraints

- The lint step content should work generically (ruff on src/ and tests/) — current content is fine
- Must handle repos that don't use ruff (the step instructs the agent, fast-path in `_check_lint` handles auto-detection)

## Done when

```bash
# From loopflowstudio (or any repo without local lint.md):
lf --list | grep lint
# Shows lint in built-in steps

# Fast check passes if ruff clean:
lfops land --create-pr
# "Lint passed" or runs the agent successfully
```
