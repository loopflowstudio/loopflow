# Consolidate UX Pipeline + Standardize on .md

Two changes:
1. Clean 3-task pipeline: `ux-research` → `ux-gaps` → `ux-fix`
2. Standardize on `.md` extension everywhere (removed `.lf` extension support)

## What was built

Merged `ux-review` into `ux-research`, deleted `ux-review`, renamed all `.lf` files to `.md`, and updated loopflow to only look for `.md` extensions.

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

## Implementation

### `gather_task()` search order

```python
def gather_task(repo_root: Path, name: str) -> TaskFile | None:
    """Search order:
    1. .claude/commands/{name}.md
    2. .lf/{name}.md
    3. templates/commands/{name}.md (builtin fallback)
    """
```

### Files changed

**Prompt files renamed:**
- `.lf/ux-research.lf` → `.claude/commands/ux-research.md` (merged ux-review content)
- `.lf/ux-review.lf` → DELETED
- `.lf/ux-gaps.lf` → `.claude/commands/ux-gaps.md`
- `.lf/ux-fix.lf` → `.claude/commands/ux-fix.md`
- `.lf/nux.lf` → DELETED

**Python code:**
- `src/loopflow/context.py` - Simplified `gather_task()`, `list_user_tasks()` to only find `.md`

**Documentation:**
- `README.md` - Updated examples
- `docs/config.md` - Updated task file references
- `docs/patterns.md` - Updated examples
- `docs/index.md`, `docs/maestro.md`, `docs/vision.md` - Updated examples

**Tests:**
- `tests/test_context.py` - Added test for ignoring non-.md extensions

## Constraints

- **Backwards compatibility**: Not maintained. Internal tool; just migrated everything.
- **`.lf/` directory stays**: Holds `config.yaml`, `voices/`, `summaries/`. Only task file extension changed.
- **`.claude/commands/` is primary**: Tasks go there for Claude Code compatibility.

## Done

All criteria met:
1. ✅ `lf ux-research` finds `.claude/commands/ux-research.md` and runs
2. ✅ `lf ux-gaps` and `lf ux-fix` work the same way
3. ✅ No `.lf` extension files remain in `.claude/commands/` or `.lf/`
4. ✅ `gather_task()` no longer searches for `.lf` extension
5. ✅ Docs updated to show `.md` examples only
