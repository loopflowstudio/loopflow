# Design Review: lf ops cp simplification and context consolidation

## What was implemented

1. **Simplified `lf ops cp` output**: Now outputs raw file content wrapped in `<lf:file>` tags without instructional prompts (`<lf:loopflow>`, `<lf:docs>`, etc.). This makes the output suitable for direct paste into web LLM clients.

2. **Consolidated lfdocs gathering**: New `gather_lfdocs()` function in `design.py` that gathers scratch/, roadmap/<wave>/, and root .md files in a consistent order. Replaces the scattered logic that was in `context.py`.

3. **Removed automatic parent doc inclusion from `gather_files()`**: Files are now gathered without automatically pulling in parent directory READMEs. Parent docs are handled separately via `gather_area()` and `gather_ancestral_docs()` when needed.

4. **Concerto UI: history and recency cues**: Added `lastActivityAt` and `lastActivityDescription` to Wave model. WaveSidebar now shows a "Recent Activity" section for waves active in the last hour.

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| `cp` builds output directly instead of using `format_prompt()` | `cp` needs raw file content without system prompts, direction, or step context. Reusing `format_prompt()` would require stripping those out. | Could have added a `raw=True` parameter to `format_prompt()` but that adds complexity for a single use case |
| `gather_lfdocs()` lives in `design.py` | It gathers design artifacts (scratch/, roadmap/) which is the domain of `design.py` | Could be in `context.py` but that module is already large |
| Removed `_gather_docs()` from `files.py` | Parent doc gathering is now explicit via `gather_ancestral_docs()` when area is set. Implicit gathering was surprising and hard to control. | Keep implicit parent docs but that made exclusion patterns unpredictable |
| Recent Activity section in Concerto uses 1-hour window and max 5 waves | Balances visibility of recent activity without cluttering the sidebar. Executive users want quick triage, not a full activity log. | Could make configurable but adds complexity |

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

- **TODO: clipboard support**: The `-c` flag in `cp` is defined but not wired up. This is intentional - clipboard image support requires additional work. The TODO marks this known gap.

- **Wave activity tracking relies on `recentSteps`**: If a wave has no steps recorded, it won't appear in Recent Activity even if other activity occurred.

## What's not included

- **Full consolidation of context gathering**: The design doc identifies three simplification opportunities. This branch addresses Opportunity 2 (output formatting) partially. Opportunities 1 and 3 are noted for future work.

- **Clipboard image support in `cp`**: The `-c` flag accepts but ignores the value. Implementing this requires clipboard image reading and encoding.

- **Reports directory changes**: The branch removes `reports/` from automatic inclusion. This is intentional - reports are now accessed via `--area` when relevant.
