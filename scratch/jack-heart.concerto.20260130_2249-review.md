# Design Review: History, Recency, and Actionable Waiting States

Branch: `jack-heart.concerto.20260130_2249`

## What was implemented

Two roadmap items for Concerto Phase 1:

1. **History and recency cues** — Wave rows now show activity timestamps ("implement 2m ago") and a "Recent Activity" section surfaces waves with activity in the last hour.

2. **Actionable waiting states** — Blocked waves show a contextual card with the specific reason ("2/5 PRs open") and a "Review PRs" button that opens the repo's GitHub PR list.

Both items were moved from `roadmap/concerto/` to `scratch/` with implementation notes.

## Key choices

### Activity timestamps on Wave model

Added `lastActivityAt` and `lastActivityDescription` as computed properties on `Wave`. Data already exists in `recentSteps`—no API changes needed, just computed views of existing data.

**Alternative rejected:** Separate activity timeline view. Added complexity for minimal benefit; users want to see waves, not a separate feed.

### Recent Activity section

Surfaces top 5 waves with activity in the last hour, excluding those already in "Needs Attention" or "Open PRs" sections to avoid duplication. Uses existing Wave data without new API calls.

### WaitingReason enum

```swift
public enum WaitingReason: Sendable, Hashable {
    case prLimitReached(open: Int, limit: Int)
}
```

Extensible for future reasons (e.g., `behindMain`, `ciBlocked`). Daemon API surfaces `waiting_reason` and `open_prs` fields when wave status is WAITING.

### PR list URL construction

WaitingStateCard extracts owner/repo from git remote URL (supports both SSH and HTTPS formats) to construct the GitHub PR list URL. Falls back to path-based extraction if git fails.

**Alternative rejected:** Fetch individual PR details from GitHub API. Adds complexity, rate limits, and latency for v1.

## How it fits together

```
                     ┌─────────────────────┐
                     │   Wave API (Python) │
                     │                     │
                     │  waiting_reason     │
                     │  open_prs           │
                     └─────────┬───────────┘
                               │
                               ▼
                     ┌─────────────────────┐
                     │  WaveService.swift  │
                     │                     │
                     │  Parses JSON into   │
                     │  WaitingReason enum │
                     └─────────┬───────────┘
                               │
          ┌────────────────────┼────────────────────┐
          ▼                    ▼                    ▼
   ┌─────────────┐    ┌────────────────┐    ┌─────────────┐
   │  WaveRow    │    │ WaveDetailPanel │    │ WaveSidebar │
   │             │    │                 │    │             │
   │  Activity   │    │  Waiting card   │    │  Recent     │
   │  timestamps │    │  with action    │    │  Activity   │
   └─────────────┘    └────────────────┘    │  section    │
                                            └─────────────┘
```

## Risks and bottlenecks

**Activity timestamps become stale** if the app is completely idle. The daemon poll keeps them fresh during normal use, but a wave's "2m ago" could become "30m ago" without visual indication of staleness.

**Git remote extraction** runs synchronously on the main thread when opening PR list. For most repos this is instant (<50ms), but slow disk I/O could cause a UI hitch. Consider moving to async if observed in practice.

**Recent Activity section** filters on a 1-hour window. Waves with activity 61 minutes ago won't appear, which could surprise users expecting to see "recent" work. Window is hardcoded; could expose as preference if feedback indicates.

## What's not included

- **Individual PR links** — requires GitHub API integration, out of scope for v1
- **Other waiting reasons** — only `prLimitReached` implemented; `behindMain`, `ciBlocked` deferred
- **Sidebar reorganization** — waiting waves remain in "Active" section, not a separate "Blocked" section
- **Activity persistence** — relies on daemon's ephemeral `recentSteps`, doesn't persist across app restarts

## Test coverage

| Component | Tests |
|-----------|-------|
| `WaitingReason.description` | Shows count fraction ("2/5 PRs open") |
| `WaitingReason.accessibilityDescription` | Shows full text ("2 of 5 PRs open") |
| `Wave.waitingReason` | Stores and retrieves correctly |
| `Wave.lastActivityAt` | Returns nil, endedAt, or falls back to startedAt |
| `Wave.lastActivityDescription` | Returns nil or includes step name |

All tests pass: Python (674), Swift package (79), Concerto UI (78).

## Files changed

| File | Change |
|------|--------|
| `swift/LoopflowCore/Models/Wave.swift` | `WaitingReason` enum, `waitingReason` field, activity computed properties |
| `swift/LoopflowCore/Services/WaveService.swift` | Parse `waiting_reason`, `open_prs` from JSON |
| `swift/Concerto/Views/WaitingStateCard.swift` | New file - waiting state UI with action |
| `swift/Concerto/Views/WaveDetailPanel.swift` | Use `WaitingStateCard` for waiting status |
| `swift/Concerto/Views/WaveRow.swift` | Display activity timestamp |
| `swift/Concerto/Views/WaveSidebar.swift` | "Recent Activity" section |
| `swift/ConcertoTests/WaveTests.swift` | Tests for new functionality |
| `src/loopflow/lfd/daemon/http_server.py` | Add `waiting_reason`, `open_prs` to API response |
| `scratch/concerto-history-and-recency.md` | Design doc (moved from roadmap) |
| `scratch/concerto-waiting-state-actionable.md` | Design doc (moved from roadmap) |
