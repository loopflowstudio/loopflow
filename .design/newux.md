# Consolidate UX Pipeline + Standardize on .md

Two changes:
1. Clean 3-task pipeline: `ux-research` → `ux-gaps` → `ux-fix`
2. Standardize on `.md` extension everywhere (remove `.lf` extension)

## What to build

Merge `ux-review` into `ux-research`, delete `ux-review`, and rename all `.lf` files to `.md`. Update loopflow to stop looking for `.lf` extension.

## Pipeline Design

```
ux-research          ux-gaps              ux-fix
┌────────────────┐   ┌────────────────┐   ┌────────────────┐
│ Generate shots │   │ Compare to     │   │ Implement      │
│ Review UI      │ → │ best-in-class  │ → │ priority fixes │
│ Simulate users │   │ tools          │   │                │
└────────────────┘   └────────────────┘   └────────────────┘
     ↓                    ↓                    ↓
.design/             .design/             .design/
ux-research.md       ux-gaps.md           ux-fixes.md
screenshots/
```

## Data structures

No new types. Code changes in `context.py` to simplify lookup.

## Key functions

### `gather_task()` in context.py

Current search order:
```python
1. .claude/commands/{name}.md
2. .lf/{name}.lf           # Remove
3. .lf/{name}.md
4. .lf/{name}.*            # Remove
5. .lf/{name}              # Remove
6. templates/commands/{name}.md
```

New search order:
```python
def gather_task(repo_root: Path, name: str) -> TaskFile | None:
    """Search order:
    1. .claude/commands/{name}.md
    2. .lf/{name}.md
    3. templates/commands/{name}.md (builtin fallback)
    """
```

### `list_tasks()` in context.py

Update to only find `.md` files, not `.lf`.

## Files to change

### Prompt files (rename .lf → .md)

```
.claude/commands/ux-research.lf  → ux-research.md  (+ merge ux-review content)
.claude/commands/ux-review.lf    → DELETE
.claude/commands/ux-gaps.lf      → ux-gaps.md
.claude/commands/ux-fix.lf       → ux-fix.md
.lf/nux.lf                       → DELETE or move to .claude/commands/nux.md
```

### Python code

```
src/loopflow/context.py    # Simplify gather_task(), list_tasks()
```

### Documentation

```
README.md           # Update examples from .lf extension to .md
docs/config.md      # Update task file references
docs/patterns.md    # Update examples
```

## ux-research.md content

Merge screenshot review into user research:

```markdown
---
context:
  - .design/screenshots/
  - Maestro/Maestro/Views/
voice: customer
---
# UX Research

## Part 1: Screenshot Capture

Use Maestro's debug capture (⌘⇧C) to generate screenshots of key states:
- Welcome/setup screen
- Empty repo state
- Prompt input with various toggle states
- Running state
- Error states

Save to .design/screenshots/ with descriptive names.

## Part 2: Visual Review

For each screenshot:
- Alignment and spacing issues
- Typography hierarchy
- Color contrast and accessibility
- Unclear affordances
- macOS convention violations

## Part 3: User Profile Simulation

Walk through as three personas:
1. Curious Beginner - "What can I even ask?"
2. CLI Convert - expects feature parity
3. Prompt Explorer - knows ChatGPT, not worktrees

For each: first impression, first action, first obstacle, recovery, verdict.

## Output

Write to .design/ux-research.md:
- Screenshots captured (paths)
- Visual issues found
- Per-profile friction points
- Top 5 priority issues
```

## Constraints

- **Backwards compatibility**: Not required. This is an internal tool; just migrate everything.
- **`.lf/` directory stays**: It holds `config.yaml`, `voices/`, `summaries/`. Only task file extension changes.
- **`.claude/commands/` is primary**: Tasks go there for Claude Code compatibility.

## Done when

1. `lf ux-research` finds `.claude/commands/ux-research.md` and runs
2. `lf ux-gaps` and `lf ux-fix` work the same way
3. No `.lf` extension files remain in `.claude/commands/` or `.lf/`
4. `gather_task()` no longer searches for `.lf` extension
5. Docs updated to show `.md` examples only
