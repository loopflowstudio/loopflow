# Simplification Opportunities

## Product intent

`lf ops cp` gathers file context and copies it to clipboard for use with web-based LLM clients. It's a lightweight alternative to running full agent sessions—just the context gathering, no execution.

## Opportunity 1: cp duplicates context.py's gather_prompt_components

**Misalignment**: `cp.py` builds its own file gathering pipeline instead of reusing the existing `gather_prompt_components` → `format_prompt` flow that `step.py` uses.

**Symptom**:
- `cp.py:_gather_diff_file_paths()` duplicates `context.py:gather_diff_files()`
- `cp.py` manually merges paths, excludes, and lfdocs instead of using `ContextConfig`
- `cp.py` builds its own output format instead of using `format_files()`
- The TODO comment `# TODO: clipboard support if needed` suggests awareness of missing feature parity

**Realignment**: `cp` should be a thin wrapper around `gather_prompt_components` with `DiffMode.NONE` and no step, similar to how `--web` mode works in `step.py:run()` (lines 285-293).

**Cascade**:
- Delete `_gather_diff_file_paths()` (~20 lines)
- Simplify cp() to ~15 lines: gather components, format, copy
- Automatic feature parity: clipboard images, area support, budgets
- One place to fix bugs in context gathering

## Opportunity 2: Output formatting spread across three modules

**Misalignment**: The product wants consistent XML-tagged output for LLM consumption, but formatting logic lives in three places with different conventions.

**Symptom**:
- `files.py:format_files()` wraps with `<lf:files>` and adds a header outside the tag
- `cp.py` builds `<lf:file>` tags manually with different structure (no header)
- `context.py:format_prompt()` uses `<lf:tag>` but with inline doc comments

**Realignment**: Single `format_files()` in `files.py` that returns just the tagged content. Callers add context-appropriate headers.

**Cascade**:
- `cp.py` uses `format_files()` directly
- Remove duplicate tag-building code from `cp.py`
- Consistent output structure across all context-gathering paths

## Opportunity 3: Config merging duplicated in cp and step

**Misalignment**: The product has a clear config resolution order (CLI > frontmatter > global > defaults) but each command reimplements it.

**Symptom**:
- `cp.py` lines 65-82: manual config loading and flag resolution
- `step.py` lines 241-254: same pattern for clipboard/docs/diff_mode
- `step.py` lines 400-412: same pattern again in `inline()`

**Realignment**: Already partially solved: `resolve_step_config()` in `frontmatter.py` handles much of this. `cp` should use it or a simpler variant for commands without steps.

**Cascade**:
- `cp.py` becomes a straightforward command that delegates to existing infrastructure
- New ops commands follow the same pattern
- Config resolution bugs fixed once

## Aligned areas

**GatherResult**: Clean data structure separating text files from images. The `(Path, str)` tuple pattern works well.

**TokenTree**: Token counting and display is centralized. Both `cp.py` and `step.py` use the same analysis and warning infrastructure.

**File exclusion**: The `_compile_exclude_patterns` + `_is_excluded_by_paths` pattern efficiently handles gitignore and explicit excludes with O(patterns) glob operations.
