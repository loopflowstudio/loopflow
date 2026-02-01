# Add history and recency cues

Show what changed since last check-in without opening each wave.

## Problem

Returning users (listeners, executives, conductors after being away) cannot tell what happened while they were gone. The sidebar shows wave names, statuses, and iteration counts, but not when things changed or what the most recent transition was. Users must click into each wave and reconstruct context manually.

This matters most for:
- **Listeners**: Checking in periodically to see overall progress
- **CEOs**: Quick status assessment without drilling in
- **Conductors**: Returning after a meeting to see what moved

## Approach

Add three UI elements that together answer "what happened since I last checked?" at a glance:

1. **Relative timestamps on wave rows** — show when each wave last changed
2. **Activity summary in iteration display** — show what the last transition was
3. **Recent Activity section header** — surface the most recently changed waves

The data already exists: `Wave.recentSteps` contains `StepRun` objects with `startedAt` and `endedAt` timestamps. We compute and display, not fetch.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Separate activity timeline panel | More screen real estate, dedicated view | Adds complexity; users want to see waves, not a separate feed |
| Activity notifications/badges | Attention-grabbing | Already have attention badges for errors/PRs; don't want badge fatigue |
| Toast notifications on change | Real-time awareness | Interrupts flow; users may not be watching |
| Sort waves by recency | Shows what's hot | Loses stable mental model of wave positions; conductors want consistent grouping |

## Key decisions

**Timestamps in secondary line, not primary.** The wave name and status are still the primary information. Timestamps appear in the secondary metadata line (where area and iteration already live) to maintain hierarchy.

**Relative time, not absolute.** "2m ago" is more scannable than "2:34 PM". Use `RelativeDateTimeFormatter` for consistency with existing `StepRun.relativeTime`.

**Most recent step name, not generic "updated".** Instead of "2m ago", show "implement 2m ago" — users know what was running, not just that something happened.

**Group recently-active waves visually.** Add a "Recent Activity" section that surfaces waves with activity in the last hour. This helps returning users without disrupting the existing attention-based grouping (Needs Attention, Open PRs, Active, Idle).

**Italic serif timestamp for warmth.** Per VISUAL_DESIGN.md, use italic serif for special moments. Timestamps are precisely this — ephemeral, contextual whispers rather than permanent metadata.

Following wave's Phase 1 principles from the roadmap:
- "A user can answer 'what happened since I last checked?' without opening any wave"
- Focus on listeners, CEOs, conductors — people checking in, not diving deep

## Scope

**In scope:**
- Relative timestamps on wave rows (secondary line)
- Step name + time in iteration display (e.g., "iter 3 • implement 2m ago")
- "Recent Activity" section header for waves changed in last hour
- `lastActivityAt` computed property on Wave from recentSteps

**Out of scope:**
- Separate activity timeline view (Phase 2 maybe)
- Activity persistence across app restarts (relies on daemon's recentSteps)
- Filtering/sorting by activity
- Push notifications for activity

## Implementation

### 1. Add `lastActivityAt` computed property to Wave

```swift
// Wave.swift
public var lastActivityAt: Date? {
    recentSteps.first?.endedAt ?? recentSteps.first?.startedAt
}

public var lastActivityDescription: String? {
    guard let step = recentSteps.first else { return nil }
    let formatter = RelativeDateTimeFormatter()
    formatter.unitsStyle = .abbreviated
    let time = formatter.localizedString(for: step.endedAt ?? step.startedAt, relativeTo: Date())
    return "\(step.step) \(time)"
}
```

### 2. Update WaveRow secondary line

```swift
// WaveRow.swift secondary info line
HStack(spacing: 4) {
    Text(wave.areaDisplay)
        .font(.caption)
        .foregroundStyle(.white.opacity(0.5))

    if !wave.iterationText.isEmpty {
        Text("•")
        Text(wave.iterationText)
    }

    // NEW: Activity timestamp
    if let activity = wave.lastActivityDescription {
        Text("•")
        Text(activity)
            .font(.custom("Cormorant Garamond", size: 11))
            .italic()
            .foregroundStyle(.white.opacity(0.4))
    }

    // Existing PR limit, cron display...
}
```

### 3. Add "Recent Activity" section to WaveSidebar

```swift
// WaveSidebar.swift
private var recentActivityWaves: [Wave] {
    let hourAgo = Date().addingTimeInterval(-3600)
    return repoState.waves
        .filter { wave in
            guard let lastActivity = wave.lastActivityAt else { return false }
            return lastActivity > hourAgo
        }
        .filter { wave in
            // Exclude waves already in blocked/PR sections
            wave.status != .error && pendingPR(for: wave) == nil
        }
        .sorted { ($0.lastActivityAt ?? .distantPast) > ($1.lastActivityAt ?? .distantPast) }
        .prefix(5)  // Top 5 most recent
        .map { $0 }
}

// In waveList, add section after Open PRs:
if !recentActivityWaves.isEmpty {
    sectionHeader("Recent Activity", icon: "clock.arrow.circlepath", color: .cyan, count: recentActivityWaves.count)
    waveRows(Array(recentActivityWaves))
}
```

### 4. Typography: italic serif for timestamps

Use Cormorant Garamond italic per VISUAL_DESIGN.md. The timestamp is a "special moment" — ephemeral context, not permanent metadata.

```swift
Text(activity)
    .font(.custom("Cormorant Garamond", size: 11))
    .italic()
    .foregroundStyle(.white.opacity(0.4))
```

## Done when

1. Each wave row shows its last activity time in the secondary line
2. The activity shows step name + relative time (e.g., "implement 2m ago")
3. Waves with recent activity appear in a "Recent Activity" section
4. A user can answer "what happened since I last checked?" without opening any wave detail
5. VoiceOver reads activity timestamps correctly

## Verification

```bash
# Build and run
cd swift && xcodegen generate && xcodebuild -scheme Concerto -destination 'platform=macOS'

# Verify with mock data
# Run app, create waves, run some steps
# After running, wave rows should show "implement 2m ago" style timestamps
# "Recent Activity" section should appear for recently-changed waves
```

## Test scenarios

1. **Fresh waves**: No activity — no timestamp shown (graceful nil handling)
2. **Recently active**: Shows step name + "2m ago" in secondary line
3. **Old activity**: Still shows but with "3d ago", "1w ago" style
4. **Multiple sections**: Wave appears in "Recent Activity" AND "Active" — shows in Recent only (no duplication)
5. **Accessibility**: VoiceOver reads "implement, 2 minutes ago" not raw timestamp
