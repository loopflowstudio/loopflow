# YAML Frontmatter for Task Configuration

This branch adds per-task configuration via YAML frontmatter in task files.

## What Changed

### New: Per-Task Frontmatter

Task files (`.claude/commands/*.md` and `.lf/*.lf`) can now include YAML frontmatter:

```markdown
---
interactive: true
include:
  - tests/**
model: claude:opus
---
Task content here...
```

### Supported Fields

| Field | Type | Description |
|-------|------|-------------|
| `interactive` | bool | Override run mode for this task |
| `include` | list[str] | Glob patterns to include in context |
| `exclude` | list[str] | Glob patterns to exclude |
| `model` | string | Override model for this task |

### Config Resolution Order

1. CLI flags (`-i`, `-a`, `-m`)
2. Task frontmatter
3. Global config (`.lf/config.yaml`)
4. Defaults

### Migrated: `include_tests_for`

The global `include_tests_for` config is deprecated. Use frontmatter instead:

```yaml
# Old (.lf/config.yaml)
include_tests_for:
  - polish
  - implement

# New (per-task frontmatter)
---
include:
  - tests/**
---
```

## Files Changed

- `src/loopflow/frontmatter.py` - New module for parsing frontmatter
- `src/loopflow/context.py` - Updated to return `TaskFile` with config
- `src/loopflow/cli/run.py` - Uses `resolve_task_config()` for merged config
- `src/loopflow/config.py` - Deprecation warning for `include_tests_for`
- `src/loopflow/publish.py` - New module for publish workflow helpers
- `.claude/commands/*.md` - Updated prompts with frontmatter

## Also in This Branch

- Moved research docs from `files/` to `.research/`
- Updated publish command documentation
- Added `refine` skill for iterative text refinement
- Removed deprecated `include_tests_for` from `.lf/config.yaml`
