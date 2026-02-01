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

The badge shows `blockedWaves.count + prWaves.count` — the waves that need human action. Only appears when count > 0. Uses orange color to match "Needs Attention" section.

### 2. Section header counts

Add counts to all section headers:

```
▲ Needs Attention (2)
● Active (3)
○ Idle (1)
```

Counts appear inline after the section name. No count for empty sections (the "(0)" is visual noise).

### 3. Always-visible section structure

Show all four section headers even when empty, with muted styling:

```
▲ Needs Attention
● Open PRs
● Active (2)
  swift-falcon     [ship]
  crystal-melody   [ship]
○ Idle
```

Empty sections show header only (no "No waves" text — that's noise). The structure teaches the mental model: "this is where things go when they need me."

### 4. Collapsed empty sections

Empty sections collapse to just the header line. Populated sections expand naturally. No expand/collapse interaction needed — purely visual hierarchy.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Single badge only (no section counts) | Simpler UI, less visual noise | Users still need to scan to find which waves need attention |
| Hide empty sections (current) | Cleaner when empty | Destroys mental model; returning users don't know where things go |
| Collapsible sections with chevrons | User control over visibility | Over-engineering for 4 sections; adds interaction without value |
| Notification-style badges (red circles) | High urgency signaling | Too alarming for normal workflow; "attention" isn't an emergency |

## Key decisions

**Orange for attention, not red.** Following the concerto design principle that "attention" is normal workflow, not emergency. Red would create anxiety on a dashboard meant for regular check-ins.

**Counts on sections, summary in header.** The header badge answers "anything need me?" (macro). Section counts answer "how many of each?" (micro). Both are needed for different user moments.

**Structure always visible.** Per concerto-vision: "Dashboard shows everything and directs the work." Hiding structure when empty defeats the learning curve. New users need to see where waves will appear before they have waves.

**No interaction for sections.** Collapsible sections add complexity without value. Four sections fit on screen. If they didn't, we'd have bigger problems.

## Scope

- In scope: Header badge, section counts, always-visible structure
- Out of scope: Filtering, search, section reordering, notification sounds

## Done when

1. Header shows attention badge (blocked + PR count) when > 0
2. Section headers show counts when > 0
3. All four sections visible even when empty
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
3. Update `waveList` to always render all four sections
