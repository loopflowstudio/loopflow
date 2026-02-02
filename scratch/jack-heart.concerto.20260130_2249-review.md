# Review: History and Recency Cues for Wave Sidebar

## What was implemented

Added three UI elements to answer "what happened since I last checked?" at a glance:

1. **Relative timestamps on wave rows** — shows when each wave last changed (e.g., "implement 2m ago")
2. **Recent Activity section** — surfaces the top 5 waves with activity in the last hour
3. **Attention badge** — header shows count of waves needing attention (blocked + open PRs)

## Key choices

**Computed properties on Wave rather than separate service.** Activity data already exists in `Wave.recentSteps`. Adding `lastActivityAt` and `lastActivityDescription` as computed properties keeps the model cohesive and avoids a separate state manager.

**Recent Activity section excludes duplicates.** Waves already shown in "Needs Attention" or "Open PRs" don't appear in "Recent Activity". This prevents the same wave from cluttering multiple sections while still surfacing it where it's most actionable.

**Italic serif for timestamps.** Per VISUAL_DESIGN.md, ephemeral context uses Cormorant Garamond italic. Timestamps are precisely this — contextual whispers, not permanent metadata.

**Abbreviated time format for display, full format for VoiceOver.** Visual display uses "2m ago" for scannability; VoiceOver reads "2 minutes ago" for clarity.

**Section headers include counts.** Each section now shows its count (e.g., "Idle (3)"), making the sidebar scannable at a glance.

## How it fits together

```
Wave.recentSteps (existing)
    → Wave.lastActivityAt (new computed property)
    → Wave.lastActivityDescription (new computed property)
        → WaveRow displays activity timestamp
        → WaveSidebar filters for recentActivityWaves (last hour, top 5)
        → WaveSidebar computes attentionCount for header badge
```

## Risks and bottlenecks

**Performance of computed properties.** `recentActivityWaves` is computed multiple times per render (once for the array, once for the ID set used in filtering). For typical wave counts (< 50) this is negligible. If wave counts grow significantly, consider caching.

**Relative time updates.** Timestamps like "2m ago" become stale if the view isn't refreshed. The daemon poll interval (typically 1s) keeps this fresh, but a completely idle app would show stale times until the next refresh.

**Test infrastructure.** Swift tests failed to run due to missing GhosttyKit XCFramework — this is a pre-existing dependency issue unrelated to this branch.

## What's not included

- **Activity persistence across app restarts** — relies on daemon's `recentSteps`, which is ephemeral
- **Filtering or sorting by activity** — the existing section-based grouping is preserved
- **Push notifications for activity** — out of scope for Phase 1
- **Separate activity timeline panel** — deliberately rejected in favor of inline display

## Files changed

| File | Change |
|------|--------|
| `swift/LoopflowCore/Models/Wave.swift` | Added `lastActivityAt` and `lastActivityDescription` computed properties |
| `swift/Concerto/Views/WaveRow.swift` | Display activity timestamp with accessibility support |
| `swift/Concerto/Views/WaveSidebar.swift` | Added Recent Activity section, attention badge, section counts |
| `swift/ConcertoTests/WaveTests.swift` | Tests for activity tracking properties |
| `roadmap/concerto/README.md` | Updated Phase 1 ordered set (marked item 01 complete) |
| `roadmap/concerto/20260131-01-*.md` | Removed (shipped) |
| `roadmap/concerto/20260131-02-*.md` | Removed (moved to scratch/) |
| `scratch/concerto-history-and-recency.md` | Design doc for this feature |

## Verification

1. Build succeeds (tested locally)
2. Wave rows show activity timestamps when `recentSteps` is populated
3. Recent Activity section appears for waves with activity in the last hour
4. Attention badge shows correct count in header
5. VoiceOver reads activity as "implement, 2 minutes ago" (full format)
6. Duplicate waves don't appear in both Recent Activity and other sections
