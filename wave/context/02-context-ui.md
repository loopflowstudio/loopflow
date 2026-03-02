# 02: Concerto Context UI

**Finish line:** Concerto shows the context breakdown for a session — what's in the prompt, how many tokens each source uses.

Surface the same `ContextBreakdown` data that the CLI prints to stderr as a visual panel in Concerto.

## What to build

### Context panel in session view

When viewing a session, show the context breakdown:
- Token counts per source (step, direction, scratch, wave, docs, diff, area, clipboard)
- Percentage of budget used
- Which specific files are included (expandable)

### Data path

`ContextBreakdown` is already computed during prompt assembly. It tracks per-source token counts (`source_tokens: HashMap<DocumentSource, usize>`) and per-source file counts (`source_counts: HashMap<DocumentSource, usize>`). The `source_key()` function in `types.rs` maps each `DocumentSource` variant to a serialization key (e.g., `Scratch` -> `"scratch"`, `Wave` -> `"wave"`).

lfd needs to:
1. Store the breakdown alongside the session (or return it in the session start response)
2. Expose it via HTTP API — `GET /waves/:id/runs/:id/context` or similar
3. Concerto fetches and renders it

### Concerto views

- **Summary bar**: compact token usage (like the CLI table but visual — maybe a stacked bar or segmented gauge)
- **Detail panel**: expandable list of sources, each showing file paths and token counts
- Respect the design system — burgundy for headings, semantic spacing, reduce motion support

## Constraints

- `ContextBreakdown` currently lives in Rust only. Need to serialize it (serde) and expose via the HTTP API.
- Concerto is SwiftUI — the rendering is straightforward once the data is available.
- Don't block session start on context data — fetch it async after the session begins.

## Done when

- Concerto session view shows context breakdown with per-source token counts
- Expanding a source shows the individual files included
- Budget percentage is visible at a glance
- Data comes from lfd HTTP API, not hardcoded
- Works on both macOS and iOS
