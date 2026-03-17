# 01: Attention Queue View

## Problem

Concerto currently centers on wave management — pick a wave, see its detail. There's no single screen that answers "what needs me right now?" across all waves. The human has to check each wave individually to find what's stuck.

The attention queue makes Concerto a conductor: one screen showing every place the system needs human judgment. Advancing two wave goals:
- "Primary Concerto screen is an attention queue, not a chat view"
- "Clicks from 'I see a problem' to 'I'm acting on it' (target: <=2)"

## Approach

Introduce an `AttentionItem` data model that represents anything requiring human attention. The attention queue is a new primary view in Concerto that lists unresolved items sorted by urgency. Tapping an item opens an inline detail panel with enough context to act without navigating elsewhere.

### New data model: `AttentionItem`

An attention item is a durable record that says "this wave needs human judgment." It has a lifecycle: surfaced → viewed → resolved.

```rust
// rust/loopflow/src/lfd/types/attention.rs
pub struct AttentionItem {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub run_id: Option<LfdId>,
    pub kind: AttentionKind,
    pub status: AttentionStatus,
    pub title: String,
    pub summary: String,
    pub context: serde_json::Value,
    pub surfaced_at: OffsetDateTime,
    pub viewed_at: Option<OffsetDateTime>,
    pub resolved_at: Option<OffsetDateTime>,
}

pub enum AttentionKind {
    DesignReview,   // gate step: design needs approval before implement
    CodeReview,     // gate step: code needs review before land
    Calibration,    // tend flow: chord mutations need approval
    QueueFailure,   // merge queue: rebase conflict, missing PR, etc.
    StepFailure,    // step failed after exhausting retries
}

pub enum AttentionStatus {
    Surfaced,   // system created it, human hasn't seen it
    Viewed,     // human opened the detail
    Resolved,   // condition cleared (human acted via domain API, or system self-healed)
}
```

### Attention items are projections, not action endpoints

An attention item is a *view* of something that needs the human. The human acts through the domain API, not through the attention item itself. The item resolves as a consequence.

| Kind | Human action | Domain API |
|------|-------------|------------|
| CodeReview → ship | Land the wave | `POST /v0/waves/:id/land` |
| CodeReview → iterate | Restart with feedback | `POST /v0/waves/:id/run` (with context) |
| DesignReview → approve | Continue flow | `POST /v0/waves/:id/run` |
| DesignReview → redirect | Restart from design | `POST /v0/waves/:id/run` (from step) |
| Calibration → approve | Apply chord | existing tend/apply-chord flow |
| QueueFailure | Fix root cause | human acts externally, reconciler detects |
| StepFailure → retry | Restart from step | `POST /v0/waves/:id/run` (from step) |

An attention reconciler periodically checks whether each item's condition still holds. When the condition clears — because the human landed the PR, approved the design, or the system self-healed — the item moves to Resolved.

This means no "Decided but not Resolved" limbo state. If a domain action fails, the item stays Surfaced/Viewed and the reconciler keeps checking.

### Subsumes QueueBlock

`AttentionItem` replaces `QueueBlock`. The existing queue reconciler (`reconcile_wave_queue`) creates `AttentionItem { kind: QueueFailure }` instead of `QueueBlock`. The queue projection logic (`project_queue_views`) reads from `attention_items` filtered by kind. The `wave_queue_blocks` table is migrated and dropped.

QueueBlock was already a projection of queue state — created when conditions fail, deleted when they clear, never directly interacted with by humans. AttentionItem generalizes this pattern to all human attention needs.

**Migration path:**
1. Add `attention_items` table
2. Migrate existing `wave_queue_blocks` rows into `attention_items` with `kind = 'queue_failure'`
3. Update `reconcile_wave_queue` to create/resolve `AttentionItem` instead of `QueueBlock`
4. Update `project_queue_views` to read from `attention_items`
5. Drop `wave_queue_blocks` table

**Why `context: serde_json::Value`?** Each kind needs different context (diff stats for code review, chord mutations for calibration, conflict files for queue failures). A flexible JSON field avoids a parallel hierarchy of context structs while keeping the core model uniform. The Swift side decodes into typed context structs per-kind.

### Creation points

| Source | Kind | When created |
|--------|------|--------------|
| `gate` step | `CodeReview` | Gate produces a PR-ready assessment |
| `review-design` step / `kickoff` step | `DesignReview` | Design doc written to scratch/ |
| `tend/draft-chord` step | `Calibration` | Chord mutations drafted |
| Queue reconciliation | `QueueFailure` | Rebase conflict, missing PR, scratch dirty, etc. |
| Step executor | `StepFailure` | Step fails after exhausting retries |

Items are created by `lfd` — either by step hooks (post-step creation based on step name and output) or by the queue reconciler. The step executor checks if the completed step produces an attention item and creates the appropriate one.

### Resolution

Items resolve when their condition clears. An attention reconciler runs periodically (piggybacks on the existing 60-second queue reconciliation cycle) and checks each open item:

- **CodeReview**: wave's PR has been landed or closed → Resolved
- **DesignReview**: wave has advanced past the design gate step → Resolved
- **Calibration**: chord mutations have been applied or discarded → Resolved
- **QueueFailure**: queue condition cleared (same logic as current QueueBlock deletion) → Resolved
- **StepFailure**: wave has been restarted or the step succeeded on retry → Resolved

Resolved items are kept for history. The queue view filters them out.

### API endpoints

```
GET    /v0/attention?repo=&status=&kind=    List items (default: unresolved)
GET    /v0/attention/:id                     Item detail with context
PATCH  /v0/attention/:id                     Update status (viewed)
GET    /v0/attention/history?repo=&limit=    Resolved items
```

Read-mostly. No decide endpoint — the human acts through domain APIs (land, run, etc.) and the item resolves as a consequence.

WebSocket events:
```
attention_created    { item: AttentionItemDto }
attention_updated    { item: AttentionItemDto }
attention_resolved   { item: AttentionItemDto }
```

### Database

```sql
CREATE TABLE attention_items (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id),
    run_id TEXT REFERENCES wave_runs(id),
    kind TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'surfaced',
    title TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    context TEXT NOT NULL DEFAULT '{}',
    surfaced_at TEXT NOT NULL,
    viewed_at TEXT,
    resolved_at TEXT
);
CREATE INDEX idx_attention_items_wave_id ON attention_items(wave_id);
CREATE INDEX idx_attention_items_status ON attention_items(status);
```

### Swift data model

```swift
// LoopflowCore/Models/AttentionItem.swift
public struct AttentionItem: Identifiable, Sendable, Hashable {
    public let id: String
    public let waveId: String
    public let runId: String?
    public let kind: AttentionKind
    public var status: AttentionStatus
    public let title: String
    public let summary: String
    public let context: AttentionContext
    public let surfacedAt: Date
    public var viewedAt: Date?
    public var resolvedAt: Date?
}

public enum AttentionKind: String, Sendable, CaseIterable {
    case designReview = "design_review"
    case codeReview = "code_review"
    case calibration
    case queueFailure = "queue_failure"
    case stepFailure = "step_failure"
}

public enum AttentionStatus: String, Sendable, CaseIterable {
    case surfaced, viewed, resolved
}
```

### Swift UI

**AttentionQueueView** — the new default content area in `ContentView`. When no wave is selected, this replaces the wave detail panel.

```
ContentView (NavigationSplitView)
├── WaveSidebar              ← unchanged, still shows wave list
└── detail
    ├── AttentionQueueView   ← NEW: default when no wave selected
    │   ├── AttentionRow × N ← compact list, sorted by urgency
    │   └── EmptyQueueView   ← "Nothing needs you."
    ├── WaveDetailPanel      ← shown when wave selected from sidebar
    └── ...
```

**AttentionQueueView** layout:
- Header: "Queue" with count badge and filter chips (all / review / calibration / failures)
- List of `AttentionRow` items sorted by urgency
- Each `AttentionRow` shows: wave name pill, kind icon, title, time-since-surfaced, status dot

**AttentionDetailView** — shown when a row is tapped. Pushes onto the navigation stack.

Content per kind:
- **CodeReview**: PR diff stats, gate assessment summary, recent commits. Actions: Ship (calls land) / Iterate (restarts wave with feedback)
- **DesignReview**: Design doc content from scratch/, alternatives table. Actions: Approve (continues flow) / Redirect (restarts from design)
- **Calibration**: Chord mutations list, wave health summary. Actions: Approve (applies chord) / Modify
- **QueueFailure**: Conflict files, error message, what the system tried. Context only — human fixes root cause externally.
- **StepFailure**: Step name, error output, retry count. Actions: Retry (restarts from step) / Skip / Abort

Action buttons call domain APIs directly. The attention item resolves when the reconciler detects the condition has cleared.

**AttentionStore** — new `@Observable` state class, similar to `WaveStore`. Holds item dictionary, derives sorted queue. Updated via WebSocket events and HTTP polling fallback.

Add `attentionStore: AttentionStore` to `RepoState`.

### Urgency sort

Urgency is derived, not stored. Sort key:

1. **Status weight**: surfaced (0) < viewed (1) — resolved filtered out
2. **Kind weight**: calibration (0) < codeReview (1) < designReview (2) < stepFailure (3) < queueFailure (4)
3. **Age**: older items surface higher (FIFO within same priority)

Calibration sorts highest because chord mutations affect multiple waves — a stale calibration blocks the whole system.

### Empty state

When the attention queue is empty:

> **Nothing needs you.**
> Waves are running.

Centered, muted text. The burgundy wave count in the sidebar still shows activity. This is the goal state — the system is autonomous.

### Migration from current default

Currently `ContentView` shows `CatchWaveView` (empty state encouraging wave selection) when no wave is selected. Replace this with `AttentionQueueView`. When items exist, the queue is shown. When empty, the empty state replaces `CatchWaveView`.

The wave sidebar remains. Selecting a wave still navigates to `WaveDetailPanel`. The attention queue is what you see when you're in conductor mode — surveying, not diving into a specific wave.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Extend `QueueBlock` with human-judgment kinds | Minimal new code, reuses existing infra | QueueBlock is keyed per-run, tightly coupled to merge queue mechanics. Human judgment needs (calibration, design review) aren't per-run — they're per-wave or per-chord. Wrong abstraction level. Better to subsume QueueBlock into the new model. |
| Notification feed instead of queue | Familiar pattern, easy to build | Notifications are passive. A queue implies items that must be processed. The UX goal is "a machine waiting for you," not "things that happened." |
| Attention items as wave status | No new model, just extend WaveStatus | A wave can have multiple simultaneous attention needs (code review + queue failure). Status is singular. Also, items need their own lifecycle independent of wave status. |
| Separate tables per kind | Type-safe context, no JSON | Five tables for one concept. The query "show me all items" becomes a UNION across tables. The UI treats them uniformly — the data model should too. |
| Decide endpoint on attention items | Human acts on the item directly | Backwards — attention items are projections of state, not action endpoints. The human acts through domain APIs (land, run, apply chord). The item resolves as a consequence when the reconciler detects the condition has cleared. |

## Key decisions

**AttentionItem is a projection, not an action endpoint.** Items describe what needs human attention. Humans act through domain APIs (land wave, restart flow, apply chord). Items resolve when the reconciler detects the underlying condition has cleared. No decide endpoint, no two-phase commit, no "Decided but not Resolved" limbo.

**AttentionItem subsumes QueueBlock.** QueueBlock was already a projection of queue state — created when conditions fail, deleted when they clear. AttentionItem generalizes this to all human attention needs. One model, one table, one lifecycle. The `wave_queue_blocks` table is migrated and dropped.

**AttentionItem is a first-class entity, not a wave annotation.** Items have their own table, API, lifecycle, and store. This lets the attention queue exist independently of wave selection. An item references a wave but isn't owned by the wave's detail view.

**JSON context field over typed variants.** The model is uniform; the context varies by kind. The Swift side decodes context into typed structs (`CodeReviewContext`, `CalibrationContext`, etc.) based on `kind`. This keeps the data layer simple while giving the UI type safety where it matters.

**Attention queue replaces the default view, doesn't add a tab.** No new navigation concept. The queue IS the home screen. Opening a wave is drilling in. This matches the conductor mental model: you start broad, narrow when something needs attention.

## Scope

### In scope
- `AttentionItem` data model (Rust types, DB migration, store operations)
- Subsume `QueueBlock` — migrate `wave_queue_blocks` into `attention_items`, drop old table
- Attention creation hooks in step executor and queue reconciliation
- Attention reconciler (resolves items when conditions clear)
- HTTP API for attention items (list, detail, mark viewed)
- WebSocket events for attention lifecycle
- Swift `AttentionItem` model and `AttentionStore`
- `AttentionQueueView` as default content area
- `AttentionDetailView` with per-kind context rendering and domain API action buttons
- Empty state
- Attention history endpoint (resolved items)

### Out of scope
- Terminal embedding (wave item 02)
- Portfolio-level attention aggregation across repos (future: portfolio view)
- Push notifications for new items (future: platform notifications)
- Attention assignment to specific humans (single-user for now)
- Custom urgency rules or manual priority override
- Attention creation from external sources (GitHub comments, Slack)
- Signal-based attention items (drift detection, staleness) — future expansion of AttentionKind

## Done when

- `cargo test -p loopflow` passes with attention store tests
- `swift test --package-path swift` passes with AttentionItem model tests
- `wave_queue_blocks` table migrated and dropped
- Attention queue is the default view when opening a repo window with no wave selected
- Creating a wave, running `gate`, and seeing a CodeReview attention item appear in the queue
- Tapping "Ship" on a CodeReview item calls `POST /v0/waves/:id/land`, item resolves when reconciler detects PR landed
- Empty queue shows "Nothing needs you. Waves are running."
- Resolved items disappear from queue, appear in history endpoint
- `GET /v0/attention?repo=` returns items sorted by urgency
