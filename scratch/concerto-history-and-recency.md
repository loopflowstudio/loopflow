# History and Recency Cues

Shipped: Wave sidebar now shows activity timestamps and groups recently-changed waves.

## What's there

1. **Activity timestamps on wave rows** — shows step name + relative time (e.g., "implement 2m ago") in the secondary metadata line
2. **Recent Activity section** — surfaces top 5 waves with activity in the last hour, excluding those already in Needs Attention or Open PRs
3. **Section counts** — each section header shows its count for at-a-glance scanning

Users can now answer "what happened since I last checked?" without opening any wave.

## Key decisions

**Computed properties on Wave.** Activity data already exists in `Wave.recentSteps`. Adding `lastActivityAt` and `lastActivityDescription` as computed properties keeps the model cohesive.

**Inline display, not separate panel.** A dedicated activity timeline would add complexity. Users want to see waves, not a separate feed.

**Italic serif for timestamps.** Per VISUAL_DESIGN.md, ephemeral context uses Cormorant Garamond italic — contextual whispers, not permanent metadata.

**Abbreviated visual, full VoiceOver.** Display shows "2m ago"; VoiceOver reads "2 minutes ago" for clarity.

**Recent Activity excludes duplicates.** Waves in Needs Attention or Open PRs don't appear in Recent Activity — they're already surfaced where they're most actionable.

## Known limitations

- Activity timestamps become stale if the app is completely idle (daemon poll keeps them fresh during normal use)
- Activity doesn't persist across app restarts — relies on daemon's ephemeral `recentSteps`
- No filtering or sorting by activity — existing section-based grouping preserved

## Files

- `Wave.swift` — `lastActivityAt`, `lastActivityDescription` computed properties
- `WaveRow.swift` — displays activity timestamp with accessibility
- `WaveSidebar.swift` — Recent Activity section, section counts
- `WaveTests.swift` — tests for activity properties
