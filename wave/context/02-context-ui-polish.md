# 02: Context UI Polish

**Finish line:** Concerto's context panel shows all files (not capped at 10), budget is customizable from the UI, and small sources render visibly in the stacked bar.

Polish items from the context UI sprint. The data path is already wired — `ContextSnapshot` serializes the full document list per source. These are display-layer improvements.

## What to build

1. **Per-file detail beyond top 10.** `SessionContextView` caps `documents` display at 10 per source. The data is already serialized — remove the cap or add pagination/scroll.
2. **Budget customization display.** The stacked bar shows `DEFAULT_CONTEXT_BUDGET`. Surface the actual budget value and let users see (not necessarily change) what it's set to.
3. **Stacked bar precision.** Sources under ~1% of total render as zero-width segments. Add a minimum visible width or collapse them into an "other" segment.

## Done when

- Expanding a source in Concerto shows all included files, not just the first 10
- Budget bar labels the actual budget value
- All non-zero sources are visible in the stacked bar
