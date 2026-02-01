# Attention Grouping and Counts

Surface "what needs attention" at a glance without clicking into waves.

## Problem

The sidebar has grouping infrastructure (Needs Attention, Open PRs, Active, Idle) but:
1. **No counts** — section headers show labels but not how many waves are in each
2. **No header summary** — the "Waves" header has no badge showing total attention items
3. **Sections vanish when empty** — users can't build a mental model of the structure

Conductors and returning users can't answer "anything need me?" without scanning every visible wave. The 5-second check-in becomes a 30-second scan.

## Approach

Add counts everywhere attention matters. Make the structure visible even when empty.

### 1. Header attention badge

Add a count badge next to "Waves" showing items needing attention:

```
Waves  ●2  [+] [🔍]
```

The badge shows `blockedWaves.count + prWaves.count` — the waves that need human action. Only appears when count > 0. Uses gold/amber to complement the burgundy palette.

### 2. Section header counts

Add counts to all section headers:

```
▲ Needs Attention (2)
● Active (3)
○ Idle (1)
```

Counts appear inline after the section name. No count for empty sections (the "(0)" is visual noise).

### 3. Progressive section visibility

Sections appear when they have content, disappear when empty:

```
▲ Needs Attention (2)
  auth-feature     blocked: PR limit
  data-migration   waiting: review
● Active (2)
  swift-falcon     [ship]
  crystal-melody   [ship]
```

No empty headers, no placeholder text. The structure reveals itself through use — returning conductors see only what matters.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Single badge only (no section counts) | Simpler UI, less visual noise | Users still need to scan to find which waves need attention |
| Always-visible headers | Teaches mental model | Clutter for returning users; violates progressive disclosure |
| Ghost/faint empty headers | Shows structure without clutter | Added complexity; users learn structure through use anyway |
| Collapsible sections with chevrons | User control over visibility | Over-engineering for 4 sections; adds interaction without value |
| Notification-style badges (red circles) | High urgency signaling | Too alarming for normal workflow; "attention" isn't an emergency |

## Key decisions

**Gold for attention, not red.** Following the concerto design principle that "attention" is normal workflow, not emergency. Red would create anxiety on a dashboard meant for regular check-ins. Gold/amber complements the burgundy palette while remaining distinct.

**Counts on sections, summary in header.** The header badge answers "anything need me?" (macro). Section counts answer "how many of each?" (micro). Both are needed for different user moments.

**Progressive disclosure over teaching.** Empty sections hide completely. Users learn structure through use, not empty scaffolding. Returning conductors see only what matters — less scanning, faster check-ins.

**No interaction for sections.** Collapsible sections add complexity without value. Four sections fit on screen. If they didn't, we'd have bigger problems.

## Scope

- In scope: Header badge, section counts, progressive section visibility
- Out of scope: Filtering, search, section reordering, notification sounds

## Done when

1. Header shows attention badge (blocked + PR count) when > 0
2. Section headers show counts when > 0
3. Empty sections hidden (progressive disclosure)
4. User can answer "anything need attention?" in < 5 seconds from sidebar alone

## Implementation notes

The wave categorization logic already exists in `WaveSidebar.swift`:

```swift
private var blockedWaves: [Wave]  // status == .error
private var prWaves: [Wave]       // has pending open PR
private var activeWaves: [Wave]   // running/waiting, no PR
private var idleWaves: [Wave]     // idle, no PR
```

Changes are purely in the view layer:
1. Update `header` to include attention badge
2. Update `sectionHeader()` to accept count parameter
3. Render sections only when they have content (current behavior, keep it)
