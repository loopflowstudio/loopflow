# Summary fallback

`context.py` checks for summaries but doesn't generate them if missing. Users must run `lfops summarize` explicitly, which breaks the "just works" experience.

**Problem**: Large files exceed budget and get dropped entirely. No summary exists because the user didn't know to run `lfops summarize` first.

**Solution**: Auto-generate summaries on demand when a file exceeds its budget allocation. Cache summaries in `.lf/cache/summaries/` for reuse.

**Effort**: Medium (integrate summary generation into context assembly, add caching layer)
