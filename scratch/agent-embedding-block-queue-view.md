# 01: Block Queue View

## Problem

Concerto currently centers on wave management — pick a wave, see its detail. There's no single screen that answers "what needs me right now?" across all waves. The human has to check each wave individually to find what's stuck.

The block queue makes Concerto a conductor: one screen showing every place the system is waiting for human judgment. Advancing two wave goals:
- "Primary Concerto screen is a block queue, not a chat view"
- "Clicks from 'I see a problem' to 'I'm acting on it' (target: <=2)"

## Approach

Introduce a `Block` data model that represents anything requiring human attention. The block queue is a new primary view in Concerto that lists unresolved blocks sorted by urgency. Tapping a block opens an inline detail panel with enough context to decide and act without navigating elsewhere.

### New data model: `Block`

A block is a durable record that says "this wave needs human judgment." It has a lifecycle: surfaced → viewed → decided → resolved.

```rust
// rust/loopflow/src/lfd/types/block.rs
pub struct Block {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub run_id: Option<LfdId>,
    pub kind: BlockKind,
    pub status: BlockStatus,
    pub title: String,
    pub summary: String,
    pub context: serde_json::Value,
    pub surfaced_at: OffsetDateTime,
    pub viewed_at: Option<OffsetDateTime>,
    pub decided_at: Option<OffsetDateTime>,
    pub resolved_at: Option<OffsetDateTime>,
    pub decision: Option<String>,
    pub decision_detail: Option<String>,
}

pub enum BlockKind {
    DesignReview,   // gate step: design needs approval before implement
    CodeReview,     // gate step: code needs review before land
    Calibration,    // tend flow: chord mutations need approval
    QueueFailure,   // merge queue: rebase conflict, missing PR, etc.
    StepFailure,    // step failed and couldn't self-heal
}

pub enum BlockStatus {
    Surfaced,   // system created it, human hasn't seen it
    Viewed,     // human opened the detail
    Decided,    // human made a choice
    Resolved,   // system acted on the decision
}
```

**Why not extend `QueueBlock`?** QueueBlock is mechanical — it tracks merge queue promotion failures and is keyed per-run. The new Block model tracks human judgment needs across the entire wave lifecycle. Different lifecycles, different consumers, different actions. Merging them would overload QueueBlock's narrow purpose.

**Why `context: serde_json::Value`?** Each block kind needs different context (diff stats for code review, chord mutations for calibration, conflict files for queue failures). A flexible JSON field avoids a parallel hierarchy of context structs while keeping the core model uniform. The Swift side decodes into typed context structs per-kind.

### Block creation points

| Source | BlockKind | When created |
|--------|-----------|--------------|
| `gate` step | `CodeReview` | Gate produces a PR-ready assessment |
| `review-design` step / `kickoff` step | `DesignReview` | Design doc written to scratch/ |
| `tend/draft-chord` step | `Calibration` | Chord mutations drafted |
| Queue reconciliation | `QueueFailure` | `QueueBlock` created (rebase conflict, etc.) |
| Step executor | `StepFailure` | Step fails after retry attempts |

Blocks are created by `lfd` — either by step hooks (post-step block creation based on step name and output) or by existing queue reconciliation logic. The step executor checks if the completed step is a block-producing step and creates the appropriate block.

### Block resolution

When the human decides, the decision flows back:
- **CodeReview → ship**: `lf ops land` is triggered for the wave
- **CodeReview → iterate**: Wave is restarted with feedback injected into next run context
- **DesignReview → approve**: Implementation step is unblocked
- **DesignReview → redirect**: Design doc updated, wave restarted from design
- **Calibration → approve**: Chord mutations applied via `tend/apply-chord`
- **QueueFailure → resolve**: Queue reconciliation re-triggered
- **StepFailure → resolve**: Wave restarted from failed step

Resolution is a two-phase commit: the human's decision is recorded immediately (status → Decided), then the system acts on it asynchronously and marks Resolved when the action completes.

### API endpoints

```
GET    /v0/blocks?repo=&status=&kind=     List blocks (default: unresolved)
GET    /v0/blocks/:id                      Block detail with context
PATCH  /v0/blocks/:id                      Update status (viewed)
POST   /v0/blocks/:id/decide              Submit decision + detail
GET    /v0/blocks/history?repo=&limit=     Resolved blocks
```

WebSocket events:
```
block_created   { block: BlockDto }
block_updated   { block: BlockDto }
block_resolved  { block: BlockDto }
```

### Database

```sql
CREATE TABLE blocks (
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
    decided_at TEXT,
    resolved_at TEXT,
    decision TEXT,
    decision_detail TEXT
);
CREATE INDEX idx_blocks_wave_id ON blocks(wave_id);
CREATE INDEX idx_blocks_status ON blocks(status);
```

### Swift data model

```swift
// LoopflowCore/Models/Block.swift
public struct Block: Identifiable, Sendable, Hashable {
    public let id: String
    public let waveId: String
    public let runId: String?
    public let kind: BlockKind
    public var status: BlockStatus
    public let title: String
    public let summary: String
    public let context: BlockContext
    public let surfacedAt: Date
    public var viewedAt: Date?
    public var decidedAt: Date?
    public var resolvedAt: Date?
    public var decision: String?
    public var decisionDetail: String?
}

public enum BlockKind: String, Sendable, CaseIterable {
    case designReview = "design_review"
    case codeReview = "code_review"
    case calibration
    case queueFailure = "queue_failure"
    case stepFailure = "step_failure"
}

public enum BlockStatus: String, Sendable, CaseIterable {
    case surfaced, viewed, decided, resolved
}
```

### Swift UI

**BlockQueueView** — the new default content area in `ContentView`. When no wave is selected (or always, as the primary view), this replaces the wave detail panel.

```
ContentView (NavigationSplitView)
├── WaveSidebar          ← unchanged, still shows wave list
└── detail
    ├── BlockQueueView   ← NEW: default when no wave selected
    │   ├── BlockRow × N ← compact list, sorted by urgency
    │   └── EmptyQueueView ← "Nothing needs you."
    ├── WaveDetailPanel  ← shown when wave selected from sidebar
    └── ...
```

**BlockQueueView** layout:
- Header: "Queue" with count badge and filter chips (all / review / calibration / failures)
- List of `BlockRow` items sorted by: status (surfaced first), then kind priority (calibration > code review > design review > failures), then recency
- Each `BlockRow` shows: wave name pill, block kind icon, title, time-since-surfaced, status dot

**BlockDetailView** — shown when a `BlockRow` is tapped. Pushes onto the navigation stack (or expands inline on wide layouts).

Content per kind:
- **CodeReview**: PR diff stats, gate assessment summary, recent commits. Actions: Ship / Iterate / Reject
- **DesignReview**: Design doc content from scratch/, alternatives table. Actions: Approve / Request Changes / Redirect
- **Calibration**: Chord mutations list, wave health summary. Actions: Approve Mutations / Modify / Override
- **QueueFailure**: Conflict files, error message, what the system tried. Actions: Resolve / Defer
- **StepFailure**: Step name, error output, retry count. Actions: Retry / Skip / Abort

**BlockStore** — new `@Observable` state class, similar to `WaveStore`. Holds block dictionary, derives sorted queue. Updated via WebSocket events and HTTP polling fallback.

Add `blockStore: BlockStore` to `RepoState`.

### Urgency sort

Urgency is derived, not stored. Sort key:

1. **Status weight**: surfaced (0) < viewed (1) — decided/resolved filtered out
2. **Kind weight**: calibration (0) < codeReview (1) < designReview (2) < stepFailure (3) < queueFailure (4)
3. **Age**: older blocks surface higher (FIFO within same priority)

Calibration sorts highest because chord mutations affect multiple waves — a stale calibration blocks the whole system.

### Empty state

When the block queue is empty:

> **Nothing needs you.**
> Waves are running.

Centered, muted text. The burgundy wave count in the sidebar still shows activity. This is the goal state — the system is autonomous.

### Migration from current default

Currently `ContentView` shows `CatchWaveView` (empty state encouraging wave selection) when no wave is selected. Replace this with `BlockQueueView`. When blocks exist, the queue is shown. When empty, the empty state replaces `CatchWaveView`.

The wave sidebar remains. Selecting a wave still navigates to `WaveDetailPanel`. The block queue is what you see when you're in conductor mode — surveying, not diving into a specific wave.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Extend `QueueBlock` with human-judgment reasons | Minimal new code, reuses existing infra | QueueBlock is keyed per-run, tightly coupled to merge queue mechanics. Human judgment blocks (calibration, design review) aren't per-run — they're per-wave or per-chord. Wrong abstraction level. |
| Notification feed instead of queue | Familiar pattern, easy to build | Notifications are passive. A queue implies items that must be processed. The UX goal is "a machine waiting for you," not "things that happened." |
| Blocks as wave status (add `blocked` status with reason) | No new model, just extend WaveStatus | A wave can have multiple simultaneous blocks (code review + queue failure). Status is singular. Also, blocks need their own lifecycle (surfaced → resolved) independent of wave status. |
| Separate block tables per kind | Type-safe context, no JSON | Five tables for one concept. The query "show me all blocks" becomes a UNION across tables. The UI treats them uniformly — the data model should too. |

## Key decisions

**Block is a first-class entity, not a wave annotation.** Blocks have their own table, API, lifecycle, and store. This lets the block queue exist independently of wave selection. A block references a wave but isn't owned by the wave's detail view.

**JSON context field over typed variants.** The block model is uniform; the context varies by kind. The Swift side decodes context into typed structs (`CodeReviewContext`, `CalibrationContext`, etc.) based on `kind`. This keeps the data layer simple while giving the UI type safety where it matters.

**Block queue replaces the default view, doesn't add a tab.** No new navigation concept. The queue IS the home screen. Opening a wave is drilling in. This matches the conductor mental model: you start broad, narrow when something needs attention.

**Two-phase resolution (Decided → Resolved).** The human's decision is recorded instantly for responsive UI. The system acts on it async. If the action fails, the block can resurface. This avoids the human waiting for `lf ops land` to complete before seeing their decision acknowledged.

**QueueFailure blocks mirror QueueBlocks.** When a `QueueBlock` is created during queue reconciliation, a corresponding `Block` with kind `QueueFailure` is also created. This means the block queue shows mechanical issues alongside judgment calls. The `QueueBlock` still exists for its internal purpose; the `Block` is the human-facing representation.

## Scope

### In scope
- `Block` data model (Rust types, DB migration, store operations)
- Block creation hooks in step executor and queue reconciliation
- HTTP API for blocks (list, detail, decide)
- WebSocket events for block lifecycle
- Swift `Block` model and `BlockStore`
- `BlockQueueView` as default content area
- `BlockDetailView` with per-kind context rendering
- Decision actions that flow back to the system
- Empty state
- Block history endpoint (resolved blocks)

### Out of scope
- Terminal embedding (wave item 02)
- Portfolio-level block aggregation across repos (future: portfolio view)
- Push notifications for new blocks (future: platform notifications)
- Block assignment to specific humans (single-user for now)
- Custom urgency rules or manual priority override
- Block creation from external sources (GitHub comments, Slack)

## Done when

- `cargo test -p loopflow` passes with block store tests
- `swift test --package-path swift` passes with Block model tests
- Block queue is the default view when opening a repo window with no wave selected
- Creating a wave, running `gate`, and seeing a CodeReview block appear in the queue
- Deciding "ship" on a CodeReview block triggers `lf ops land`
- Empty queue shows "Nothing needs you. Waves are running."
- Resolved blocks disappear from queue, appear in history endpoint
- `GET /v0/blocks?repo=` returns blocks sorted by urgency
