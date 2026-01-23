# Project 5: Simplified WorktreeRow

Remove hover buttons. Show essential info at a glance.

**Status:** Future (can start anytime, but benefits from Projects 2-4)

---

## Problem

Current WorktreeRow has too much:

### Hover Buttons (6 total)
1. View Diff
2. PR (create/open)
3. Open Terminal
4. Open IDE
5. Land PR
6. Abandon/Trash

Six buttons compete for attention. Most aren't needed on every row.

### Summary Info (Current)
- Branch name (display name from path)
- "PR #X (State) · N ahead · M behind"
- CI status icon
- Last step badge
- Staleness badge

The format is dense and hard to scan. Ahead/behind counts aren't the most important signal.

---

## Design Goals

1. **Scannable** — Glance at list, know what needs attention
2. **Minimal chrome** — No buttons unless acting
3. **Clear signals** — What's ready to ship? What's blocked?
4. **Information hierarchy** — Most important info most visible

---

## Proposed Design

### Row Layout (Always Visible)

```
┌─────────────────────────────────────────────────────────────┐
│ [CI] feature-name                            [status] 2d ago│
│      PR #123 · needs review                                 │
└─────────────────────────────────────────────────────────────┘
```

**Elements:**
- **CI indicator** — Green/yellow/red dot (or checkmark/spinner/x)
- **Branch name** — Primary identifier
- **Status pill** — Single word: "ready", "dirty", "stale", "blocked"
- **Age** — Time since last activity
- **Secondary line** — PR info if exists, otherwise uncommitted changes count

### Status Hierarchy

| Status | Meaning | Visual |
|--------|---------|--------|
| `ready` | Clean, CI passing, can ship | Green pill |
| `review` | PR open, awaiting review | Blue pill |
| `dirty` | Uncommitted changes | Yellow pill |
| `blocked` | CI failing or merge conflict | Red pill |
| `stale` | Inactive >7 days or merged | Gray pill |

Only one status shown. Priority order: blocked > dirty > review > ready > stale.

### No Hover Buttons

Actions move to:
1. **Right-click context menu** — All actions available
2. **Keyboard shortcuts** — Power users
3. **Double-click** — Primary action (open in IDE? open PR?)
4. **Selection + toolbar** — Batch actions

### Context Menu

```
Open in IDE           ⌘O
Open in Terminal      ⌘T
────────────────────────
View Diff             ⌘D
────────────────────────
Create PR...
Open PR in GitHub     ⌘⇧O
Land PR...
────────────────────────
Abandon Worktree...
```

---

## Information Shown

### Always Visible
- Branch name (primary)
- Status pill (single clear signal)
- Age (time since last activity)
- CI state (dot/icon)

### On Selection / Expanded
- Full PR title
- Ahead/behind counts
- Last step run info
- Uncommitted file count

### On Hover (Minimal)
- Tooltip with full branch name if truncated
- Maybe: single primary action button (the one you'd most likely click)

---

## Implementation

### Phase 1: Remove hover buttons

1. Delete hover button overlay
2. Add right-click context menu with all actions
3. Add keyboard shortcuts for common actions

### Phase 2: Simplify row content

1. Remove "N ahead · M behind" from default display
2. Add status pill with single-word status
3. Add age indicator
4. Secondary line for PR info only

### Phase 3: Selection-based detail

1. Selected row shows expanded info
2. Or: detail panel beside list (master-detail)
3. Batch selection for multi-worktree actions

---

## Visual Reference

### Before (Current)

```
┌─────────────────────────────────────────────────────────────┐
│ feature-branch-name                                         │
│ PR #123 (open) · 3 ahead · 0 behind                        │
│ [CI ✓] [review ✓]                     [staleness: merged]  │
│                                                             │
│ [HOVER: diff] [pr] [term] [ide] [land] [trash]             │
└─────────────────────────────────────────────────────────────┘
```

### After (Proposed)

```
┌─────────────────────────────────────────────────────────────┐
│ ● feature-branch-name                    [review]     2d   │
│   PR #123 · awaiting review                                 │
└─────────────────────────────────────────────────────────────┘
```

---

## Files to Modify

**Swift (Concerto):**
- `swift/Concerto/Views/WorktreeSidebar.swift` — Rewrite WorktreeRow
- `swift/Concerto/Views/WorktreeContextMenu.swift` — New: context menu
- `swift/LoopflowCore/Models/Worktree.swift` — Add computed `displayStatus`

---

## Done When

- [ ] No buttons appear on hover
- [ ] Right-click shows full context menu
- [ ] Keyboard shortcuts work for common actions
- [ ] Single status pill per row
- [ ] Age visible at a glance
- [ ] Secondary line shows PR info or change count
- [ ] List is scannable—can quickly identify what needs attention
