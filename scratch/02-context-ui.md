# Context UI in Concerto

## Problem

The context pipeline is opaque in Concerto. Users can't see what's eating tokens, which sources are included, or how close they are to budget. The CLI prints a context table to stderr — Concerto shows nothing.

Users managing waves need to know: is the diff crowding out area docs? Is the wave memory eating budget? Did the clipboard make it in? Today they have to run `lf` in a terminal to see this.

## Approach

The data path is already 80% built. `ContextSnapshot` is emitted as a `context_snapshot` session event during session startup. It's serialized via serde and streamed through SSE. Concerto receives it but drops it into the `.other` catch-all case.

**Three layers of work:**

1. **Swift model + event parsing** — Add `ContextSnapshot` model, parse the `context_snapshot` event, store it on `SessionState`
2. **Context bar** — Compact stacked bar at the top of the session view showing budget usage at a glance
3. **Context detail panel** — Expandable breakdown showing per-source token counts and metadata

### Rust: Enrich ContextSnapshot with display metadata

The current `ContextSnapshot` has `sources` (token counts), `budget`, `total`, and `diff_tier`. The CLI's `format_context_header` also displays step name, direction names, diff file count, area name, area doc count, and wave name — all from `ContextBreakdown` fields that don't survive the conversion to `ContextSnapshot`.

Add these fields to `ContextSnapshot`:

```rust
pub struct ContextSnapshot {
    pub sources: HashMap<String, u64>,
    pub budget: u64,
    pub total: u64,
    pub diff_tier: String,
    // New display metadata
    pub step_name: Option<String>,
    pub direction_names: Vec<String>,
    pub diff_file_count: u64,
    pub area_name: Option<String>,
    pub area_doc_count: u64,
    pub wave_name: Option<String>,
    pub has_clipboard: bool,
}
```

Update `From<&ContextBreakdown>` to carry these fields through. All new fields have natural defaults (None/empty/0/false) so this is backwards-compatible for any existing consumers.

### Swift: Parse and store

Add `ContextSnapshot` to `LoopflowCore/Models/AgentSession.swift`:

```swift
public struct ContextSnapshot: Sendable, Hashable {
    public let sources: [String: UInt64]
    public let budget: UInt64
    public let total: UInt64
    public let diffTier: String
    public let stepName: String?
    public let directionNames: [String]
    public let diffFileCount: UInt64
    public let areaName: String?
    public let areaDocCount: UInt64
    public let waveName: String?
    public let hasClipboard: Bool
}
```

Add `case contextSnapshot(ContextSnapshot)` to `AgentSessionEvent`. Parse `"context_snapshot"` in `LocalWaveService.parseSessionEventFromJSON`.

Store on `SessionState`:

```swift
@Observable final class SessionState {
    var contextSnapshot: ContextSnapshot?
    // ... existing fields
}
```

Set it when the event arrives during stream processing.

### Concerto: Context bar (summary)

A compact horizontal bar below the session header, always visible when a snapshot exists. Stacked segments colored by source category, proportional to token usage.

```
┌─────────────────────────────────────────────────────┐
│ ██████░░░░░░░░░░░░░░░░░░░░░░░░  15% of 75k         │
│ step  dir  diff  docs  area                         │
└─────────────────────────────────────────────────────┘
```

- Each segment gets a distinct color from the design palette (burgundy for step, info cyan for diff, success green for docs, etc.)
- Total percentage and absolute count displayed
- Tap to expand the detail panel
- Respects `reduceMotion` — no animated transitions if accessibility setting is on

### Concerto: Context detail panel (expandable)

Tapping the bar reveals a detail panel listing each source:

```
┌─────────────────────────────────────────────────────┐
│ Context                                    15% of 75k│
│─────────────────────────────────────────────────────│
│ ██ step          1,200   implement                  │
│ ██ direction       500   clarity, care              │
│ ██ system        3,000   loopflow                   │
│ ██ diff          5,000   unified (8 files)          │
│ ██ docs          1,200   3 files                    │
│ ██ scratch         800   2 files                    │
│ ██ area          2,000   src/ (12 files)            │
│ ██ wave            300   1 file                     │
│─────────────────────────────────────────────────────│
│ total           14,000                              │
└─────────────────────────────────────────────────────┘
```

- Sources sorted by token count descending
- Each row: color swatch + source name + token count + metadata annotation
- Metadata annotations from the enriched snapshot fields (step_name, direction_names, area_name + area_doc_count, diff_tier + diff_file_count, wave_name)
- Sources with 0 tokens omitted
- `.monospacedDigit()` on numbers for alignment
- Burgundy heading per design system
- Semantic spacing (Spacing.sm between rows, Spacing.lg padding)

### Source color mapping

| Source | Color | Rationale |
|--------|-------|-----------|
| step | `accent` (burgundy) | Primary instruction |
| direction | `statusWarning` | Guidance/orientation |
| diff | `statusInfo` (cyan) | Technical data |
| repo_doc | `statusSuccess` (green) | Reference material |
| scratch | `statusWarning` lighter | Working notes |
| wave | `accent` lighter | Wave context |
| wave_memory | `textSecondary` | Background context |
| summary | `textSecondary` | Supplementary |
| area | `statusSuccess` darker | Scoped reference |
| clipboard | `statusError` (orange) | User input |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| New REST endpoint `GET /sessions/:id/context` | Separate fetch, cache management | Data already streams via SSE — adding another endpoint is redundant |
| Pie chart | Familiar visualization | Poor for 8-10 small segments; stacked bar reads faster |
| Always-expanded detail view | No interaction needed | Wastes vertical space in session transcript; most users glance at the bar |
| Per-file breakdown (expandable sources showing individual files) | Maximum granularity | `ContextSnapshot` doesn't carry individual file paths — only `ContextBreakdown` has that via its `source_counts`. Adding file-level detail requires serializing the full document list. Defer to a follow-up; source-level granularity covers the core need |

## Key decisions

**Use the existing SSE event, not a new endpoint.** `context_snapshot` is already emitted during session start. Concerto just needs to parse it. No new API surface.

**Enrich ContextSnapshot rather than expose ContextBreakdown directly.** ContextBreakdown uses Rust-specific types (HashMap<DocumentSource, usize>). ContextSnapshot is the serialization boundary — flat, string-keyed, serde-friendly. Add display metadata to it rather than creating a parallel serialization path.

**No per-file detail in v1.** The wave item mentions "expanding a source shows the individual files included." This requires serializing the full document list from `ContextBreakdown` — file paths, individual token counts. The data isn't in `ContextSnapshot` today and adding it changes the event payload significantly. Source-level token counts (step: 1200, diff: 5000) cover the primary use case. File-level is a follow-up.

**Compact bar as default, detail on tap.** The context breakdown is diagnostic, not primary workflow. A compact bar gives the glance — "am I near budget?" — without eating session transcript space. Tap to see the full table when debugging context issues.

## Scope

- **In scope:** Swift ContextSnapshot model, event parsing, SessionState storage, context bar view, context detail panel, Rust ContextSnapshot enrichment with display metadata
- **Out of scope:** Per-file breakdown (requires serializing document list — follow-up), summaries (experimental, excluded by wave vision), new REST endpoints, changes to prompt XML structure

## Done when

- `cargo test --all` passes with enriched ContextSnapshot fields
- `swift test --package-path swift` passes with new model and event parsing
- Concerto session view shows a stacked context bar when a session has a context snapshot
- Tapping the bar expands a detail panel with per-source token counts and metadata
- Bar shows budget percentage at a glance
- Data comes from the `context_snapshot` SSE event, not hardcoded
- Works on both macOS and iOS (compact layout adapts to horizontal size class)
- `reduceMotion` respected for expand/collapse animation
