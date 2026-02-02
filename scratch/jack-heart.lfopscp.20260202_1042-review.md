# Design Review: lf ops cp simplification and context consolidation

## What was implemented

1. **Simplified `lf ops cp` output**: Now outputs raw file content wrapped in `<lf:file>` tags without instructional prompts (`<lf:loopflow>`, `<lf:docs>`, etc.). This makes the output suitable for direct paste into web LLM clients.

2. **Consolidated lfdocs gathering**: New `gather_lfdocs()` function in `design.py` that gathers scratch/, roadmap/<wave>/, and root .md files in a consistent order. Replaces the scattered logic that was in `context.py`.

3. **Removed automatic parent doc inclusion from `gather_files()`**: Files are now gathered without automatically pulling in parent directory READMEs. Parent docs are handled separately via `gather_area()` and `gather_ancestral_docs()` when needed.

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| `cp` builds output directly instead of using `format_prompt()` | `cp` needs raw file content without system prompts, direction, or step context. Reusing `format_prompt()` would require stripping those out. | Could have added a `raw=True` parameter to `format_prompt()` but that adds complexity for a single use case |
| `gather_lfdocs()` lives in `design.py` | It gathers design artifacts (scratch/, roadmap/) which is the domain of `design.py` | Could be in `context.py` but that module is already large |
| Removed `_gather_docs()` from `files.py` | Parent doc gathering is now explicit via `gather_ancestral_docs()` when area is set. Implicit gathering was surprising and hard to control. | Keep implicit parent docs but that made exclusion patterns unpredictable |

## How it fits together

```
lf ops cp            lf step run
    │                     │
    ▼                     ▼
gather_files()      gather_prompt_components()
gather_lfdocs()           │
    │                     ▼
    │              gather_lfdocs()
    │              gather_area()
    │              gather_ancestral_docs()
    │                     │
    ▼                     ▼
<lf:files>          format_prompt()
(raw content)       (full prompt with system docs)
```

The key insight: `cp` needs *just* the files. `step` needs a complete prompt. They share `gather_lfdocs()` for consistency but diverge in how they format output.

## Risks and bottlenecks

- **`_gather_diff_file_paths()` duplicates `gather_diff_files()`**: The design doc notes this as "Opportunity 1" for future consolidation. Current duplication is acceptable because `cp` uses `origin/main...HEAD` while `context.py` uses a dynamic base ref.

- **Clipboard flag unused**: The `-c` flag in `cp` is defined for interface consistency with `lf step` but not wired up. Clipboard image support requires additional encoding work.

## What's not included

- **Full consolidation of context gathering**: The design doc identifies three simplification opportunities. This branch addresses Opportunity 2 (output formatting) partially. Opportunities 1 and 3 are noted for future work.

- **Clipboard image support in `cp`**: The `-c` flag accepts but ignores the value. Implementing this requires clipboard image reading and encoding.
