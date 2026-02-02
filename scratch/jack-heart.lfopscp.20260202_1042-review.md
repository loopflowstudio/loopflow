# Design Review: lf ops cp simplification and context consolidation

## What was implemented

1. **Simplified `lf ops cp` output**: Outputs raw file content wrapped in `<lf:file>` tags without instructional prompts (`<lf:loopflow>`, `<lf:docs>`, etc.). This makes the output suitable for direct paste into web LLM clients.

2. **Consolidated lfdocs gathering**: New `gather_lfdocs()` function in `design.py` gathers scratch/, roadmap/<wave>/, and root .md files in a consistent order. Both `cp.py` and `context.py` use this shared function.

3. **Removed automatic parent doc inclusion from `gather_files()`**: Files are now gathered without automatically pulling in parent directory READMEs. Parent docs are handled separately via `gather_area()` and `gather_ancestral_docs()` when needed.

4. **Config resolution helpers**: Added `resolve_flag()` and `extend_list()` to `config.py` for clean flag/list merging.

5. **Output formatting helpers**: Added `format_files_raw()` to `files.py` that returns just the `<lf:files>` block. `format_files()` uses it internally.

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| `cp` builds output directly | Needs raw file content without system prompts. Reusing `format_prompt()` would require stripping them out. | `raw=True` parameter adds complexity for single use case |
| `gather_lfdocs()` in `design.py` | Gathers design artifacts (scratch/, roadmap/) which is the domain of `design.py` | `context.py` already large |
| Removed `_gather_docs()` from `files.py` | Parent docs now explicit via `gather_ancestral_docs()`. Implicit gathering was surprising and hard to control. | Implicit parent docs made exclusion patterns unpredictable |
| Shared `gather_diff_files()` | `cp` now uses `context.py:gather_diff_files()` with dynamic base ref via `get_default_base_ref()` | Duplicate implementation in `cp.py` |

## How it fits together

```
lf ops cp            lf step run
    │                     │
    ▼                     ▼
gather_files()      gather_prompt_components()
gather_lfdocs()           │
gather_diff_files()       ▼
    │              gather_lfdocs()
    │              gather_area()
    │              gather_ancestral_docs()
    │                     │
    ▼                     ▼
format_files_raw()  format_prompt()
<lf:files>          (full prompt with system docs)
(raw content)
```

The key insight: `cp` needs *just* the files. `step` needs a complete prompt. They share `gather_lfdocs()` and `gather_diff_files()` for consistency but diverge in how they format output.

## Known limitations

- **Clipboard flag unused**: The `-c` flag in `cp` is defined for interface consistency with `lf step` but not wired up. Clipboard image support requires additional encoding work.

## What's not included

- No changes to `step.py` config resolution patterns (could use `resolve_flag()`/`extend_list()` if desired later)
- No changes to the raw diff output (`--diff-mode` flag removed from `cp` since it's for web clients)

## Aligned areas

**GatherResult**: Clean data structure separating text files from images. The `(Path, str)` tuple pattern works well.

**TokenTree**: Token counting and display is centralized. Both `cp.py` and `step.py` use the same analysis and warning infrastructure.

**File exclusion**: The `_compile_exclude_patterns` + `_is_excluded_by_paths` pattern efficiently handles gitignore and explicit excludes with O(patterns) glob operations.
