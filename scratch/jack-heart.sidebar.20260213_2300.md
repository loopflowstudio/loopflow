# Sidebar cleanup

Simplify the wave sidebar — fix broken selected state, remove noise, flatten groups.

## What to build

A cleaner sidebar with two groups (Active/Idle), no live output, and a clear selected state.

## Changes

### WaveRow.swift

**Remove:**
- `SidebarLiveOutput` (lines 145-148) and the `SidebarLiveOutput` struct at bottom of file
- Keyboard focus border overlay (the accent-color rounded rect stroke, lines 156-160)
- Iteration count display (lines 101-108 — the "iter N" text + dot separator)
- PR limit indicator (lines 124-131 — the "PR limit" text + dot separator)

**Keep:**
- Name + flow badge + PR badge
- Area display
- Activity timestamp (italic serif)
- Stimulus label (loop/watching/cron)
- Selected fill (white 0.2) and hover fill (white 0.08)

### WaveStore.swift

**Collapse 5 groups → 2:**
- `active`: all non-idle waves (running, waiting, failed)
- `idle`: idle waves

Remove `blocked`, `pr`, `recentActivity` from `WaveGroups`.

### WaveSidebar.swift

**Update `waveList`** to render just two sections: Active, Idle.

Remove the `isKeyboardFocused` parameter from `WaveRow` calls (border is gone).

### WaveRow.swift cleanup

Remove `isKeyboardFocused` property since the overlay is gone.

## Done when

- Selecting a wave highlights only that row (white fill, no borders on others)
- No live output in sidebar rows
- No iteration count in sidebar rows
- Sidebar shows two groups: Active and Idle
- Existing tests pass
