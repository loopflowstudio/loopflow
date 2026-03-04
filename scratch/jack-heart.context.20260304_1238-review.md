# Context UI — Design Review

## What was implemented

Context snapshot visualization for Concerto. Users can now see what's consuming their token budget at a glance — a stacked bar with per-source breakdown, expandable to show individual documents.

**Rust:** Enriched `ContextSnapshot` with `source_counts`, `documents` (per-file entries), and display metadata (`step_name`, `direction_names`, `area_name`, `wave_name`, `has_clipboard`). Updated `From<&ContextBreakdown>` to carry these through. All new fields have serde defaults — backwards-compatible for existing consumers.

**Swift model + parsing:** Added `DocumentEntry` and `ContextSnapshot` to `AgentSession.swift`, parsed `context_snapshot` SSE event in `LocalWaveService`, stored on `SessionState`.

**Concerto UI:** `SessionContextView` with compact stacked bar (always visible when snapshot exists) and expandable detail panel. Per-source rows sorted by token count, with metadata annotations. Per-document drill-down within each source (top 10, with "...N more" overflow). Mounted in `WaveSessionView` above the transcript.

## Key choices

| Decision | Why |
|----------|-----|
| Enrich `ContextSnapshot` rather than expose `ContextBreakdown` directly | `ContextBreakdown` uses Rust-specific types (`HashMap<DocumentSource, usize>`). `ContextSnapshot` is the serialization boundary — flat, string-keyed, serde-friendly. |
| `source_counts` derived from documents in `ContextBreakdown` | Single source of truth for document counts. Previous commit collapsed the builder logic to derive counts from the document list. |
| Per-document entries included in v1 | Goes beyond the design doc's "no per-file detail in v1" — but the data was already available in `ContextBreakdown.documents`, so serializing it was low-cost and completes the drill-down story. |
| Compact bar as default, detail on tap | Context breakdown is diagnostic, not primary workflow. Bar gives the glance ("am I near budget?") without eating transcript space. |
| Document slice capped at 10 entries | Prevents long lists from overwhelming the detail panel. Overflow count shown as "...N more". |

## How it fits together

```
ContextBreakdown (Rust engine)
  → From<&ContextBreakdown> → ContextSnapshot (Rust types)
    → SessionEvent::ContextSnapshot → SSE stream
      → LocalWaveService.parseContextSnapshot() → ContextSnapshot (Swift model)
        → SessionState.contextSnapshot
          → SessionContextView (bar + detail panel)
```

The data flows through one existing path (SSE events) with no new endpoints or API surface.

## Risks and bottlenecks

- **Large document lists:** If a session has hundreds of documents, the `ContextSnapshot` event payload grows. Currently unbounded on the Rust side — the Swift side caps display at 10 per source. Not a concern at current scale but worth watching.
- **Stacked bar precision:** Very small sources (<1% of total) may render as zero-width segments. Acceptable for the bar's purpose (glance at major consumers).

## What's not included

- **Per-file detail beyond top 10:** The document list is fully serialized but only the top 10 per source are displayed. Full list is a follow-up.
- **New REST endpoint:** Data comes from the existing `context_snapshot` SSE event, not a separate fetch.
- **Budget customization display:** The bar shows budget from `ContextSnapshot.budget` (currently `DEFAULT_CONTEXT_BUDGET`). No UI for changing the budget.
