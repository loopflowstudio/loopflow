# Wave Area Summaries — Review

## What was implemented

Pre-generated codebase area summaries that get loaded into every step's context automatically. Three pieces:

1. **Storage** — `.lf/summaries/{path_hash}.md` with YAML frontmatter (path, source_hash, tokens, generated_at, model). Path hash is SHA-256 of the normalized area path.

2. **Generation** — `lf summarize` CLI command. Enumerates files via `ignore` crate (respects `.gitignore`), computes content hash for staleness, decides preload vs path-only based on 50k token threshold, launches the agent with the builtin `summarize.md` step, writes output with frontmatter.

3. **Loading** — `gather_context()` calls `load_summaries()` which reads cached files, strips frontmatter, returns `Document` with category `"summaries"`. `trim_context_with_breakdown()` drops summaries before docs, after area_docs.

## Key choices

| Decision | Why |
|----------|-----|
| Files on disk, not SQLite | Summaries are repo-local. Work without lfd. Simpler. |
| Content hash, not timestamps | mtime can change without content changing. Hash is deterministic. |
| 50k token preload threshold | Below 50k: inline everything for better summaries. Above: pass paths, let agent read. Matches 75k default budget. |
| Gemini default model | Cost optimization. Summaries are reference material, don't need strongest model. |
| Manual regeneration only | Phase 1. `lf summarize --watch` or stimulus-driven is Phase 2. |
| Summaries counted under `docs` in ContextBreakdown | Simplest approach. Both share trimming category. Could add a dedicated field later if needed. |

## How it fits together

```
.lf/config.yaml (summaries config)
        │
        ▼
lf summarize ──▶ walk_area_files() ──▶ compute_source_hash()
        │               │                      │
        │               ▼                      ▼
        │        count_area_tokens()    is_summary_fresh()?
        │               │                      │
        │               ▼                      │
        │        build_summarize_prompt()       │
        │               │                      │
        │               ▼                      │
        │        launch_agent() ──▶ write_summary()
        │                               │
        ▼                               ▼
gather_context() ──▶ load_summaries() ──▶ .lf/summaries/{hash}.md
        │
        ▼
PromptComponents.summaries ──▶ format_prompt() ──▶ <lf:summaries>
```

## Risks and bottlenecks

- **Config loaded twice in some paths**: `gather_context()` loads config internally to get `summaries` config. Callers that already loaded config can't pass it through. Not a real performance issue (config loading is ~1ms) but worth noting for future refactoring of `GatherContextOpts`.
- **No config `exclude` in walk_area_files**: Design doc mentions respecting `exclude` patterns but the implementation only uses `.gitignore`. This is a reasonable simplification — exclude patterns are for context gathering scope, not summary source enumeration.
- **Large repos**: `compute_source_hash` reads all file contents to hash them. For very large areas this could be slow. Acceptable for Phase 1 manual usage.

## What's not included

- Webhook-triggered regeneration (Phase 2)
- Branch-specific summaries
- Automatic regeneration on file change
- Separate `summaries` field in `ContextBreakdown` (counted under `docs`)
- Config `exclude` patterns in `walk_area_files`
