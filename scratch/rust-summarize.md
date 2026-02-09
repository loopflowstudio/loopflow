# Wave Area Summaries

Pre-generated codebase area summaries loaded into every step's context automatically. Agents start with a mental model of the whole codebase, not just their corner.

## Architecture

Three pieces: storage, generation, loading.

```
.lf/config.yaml (summaries config)
        |
        v
lf summarize --> walk_area_files() --> compute_source_hash()
        |               |                      |
        |               v                      v
        |        count_area_tokens()    is_summary_fresh()?
        |               |                      |
        |               v                      |
        |        build_summarize_prompt()       |
        |               |                      |
        |               v                      |
        |        launch_agent() --> write_summary()
        |                               |
        v                               v
gather_context() --> load_summaries() --> .lf/summaries/{hash}.md
        |
        v
PromptComponents.summaries --> format_prompt() --> <lf:summaries>
```

**Storage** — `.lf/summaries/{path_hash}.md` with YAML frontmatter (path, source_hash, tokens, generated_at, model). Path hash is SHA-256 of the normalized area path.

**Generation** — `lf summarize` CLI command. Enumerates files via `ignore` crate (respects `.gitignore`), computes content hash for staleness, decides preload vs path-only based on 50k token threshold, launches the agent with the builtin `summarize.md` step, writes output with frontmatter.

**Loading** — `gather_context()` calls `load_summaries()` which reads cached files, strips frontmatter, returns `Document` with category `"summaries"`. `trim_context_with_breakdown()` drops summaries before docs, after area_docs.

## Key decisions

| Decision | Why |
|----------|-----|
| Files on disk, not SQLite | Summaries are repo-local. Work without lfd. Simpler. |
| Content hash, not timestamps | mtime can change without content changing. Hash is deterministic. |
| 50k token preload threshold | Below 50k: inline everything for better summaries. Above: pass paths, let agent read. Matches 75k default budget. |
| Gemini default model | Cost optimization. Summaries are reference material, don't need strongest model. |
| Manual regeneration only | Phase 1. `lf summarize --watch` or stimulus-driven is Phase 2. |
| Summaries counted under `docs` in ContextBreakdown | Simplest approach. Both share trimming category. Could add a dedicated field later if needed. |

## Known gaps

- `gather_context()` loads config internally — callers that already loaded config can't pass it through (~1ms, not a real issue)
- `walk_area_files` uses `.gitignore` only, not config `exclude` patterns (reasonable simplification)
- `compute_source_hash` reads all file contents — could be slow for very large areas (acceptable for Phase 1)

## Out of scope (Phase 2+)

- Webhook-triggered regeneration
- Branch-specific summaries
- Automatic regeneration on file change
- Separate `summaries` field in `ContextBreakdown`
- Config `exclude` patterns in `walk_area_files`
