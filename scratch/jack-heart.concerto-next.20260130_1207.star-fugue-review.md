# Design Review: Improvise UX

Branch: `jack-heart.concerto-next.20260130_1207.star-fugue`

## What was implemented

### 1. Area Picker

New wave creation flow that guides users to select an area (scope) before running steps.

**File:** `swift/Concerto/Views/Improvise/AreaPicker.swift`

Three options for area selection:
- **Recent areas** — Persisted per-repo in UserDefaults, shows last 5 selections
- **From current changes** — Infers directories from diff when wave has uncommitted changes
- **Browse** — NSOpenPanel folder picker with relative path conversion
- **Entire repository** — Quick option to use `.` as area

The picker appears when `wave.area == nil`, taking over the entire detail panel. This makes area selection the obvious first step for new waves.

### 2. Step Runner

Improvisation cockpit for running individual steps on a wave.

**File:** `swift/Concerto/Views/Improvise/StepRunner.swift`

Features:
- **Area header** with "Change" button to go back to AreaPicker
- **Direction pills** inline editing (via DirectionPills component)
- **Step grid** showing common steps (review, design, implement, debug) prominently
- **Flow dropdown** for running named flows as secondary option
- **Prompt field** for optional additional context
- **Run button** that launches interactive or auto execution

Step selection uses a 4-column grid for common steps. "More..." expands to show all available steps from the repo.

### 3. Direction Pills

Inline direction editing with visual pills.

**File:** `swift/Concerto/Views/Improvise/DirectionPills.swift`

- Click pill to remove direction
- Plus button opens popover with available directions
- Auto-saves via `repoState.updateWave()` on change
- Visual feedback with accent-colored capsules

### 4. Transition Bar

Sticky footer guiding transition from Improvise to Conduct mode.

**File:** `swift/Concerto/Views/Improvise/TransitionBar.swift`

Three actions after steps have run:
- **Set Stimulus** — Opens StimulusPicker sheet (loop, once, watch, cron)
- **Create PR** — Runs `lf ops pr` via WorktreeService
- **Archive** — Sets stimulus to manual and pauses wave

The bar appears when `wave.recentSteps.count > 0`.

### 5. Stimulus Picker

Sheet for choosing wave trigger mode.

**File:** `swift/Concerto/Views/Improvise/TransitionBar.swift` (nested view)

Options:
- **Loop** — Run continuously until stopped
- **Once** — Run one time then stop
- **Watch** — Run when files change on main
- **Schedule (cron)** — Run on a schedule with cron expression input

### 6. Recent Areas Service

UserDefaults persistence for area selections.

**File:** `swift/Concerto/Services/RecentAreasService.swift`

- Key: `recentAreas.<repo-path-hash>`
- Max 5 entries, most recent first
- Moves existing entries to front on re-selection

### 7. Wave Detail Panel State Machine

**File:** `swift/Concerto/Views/WaveDetailPanel.swift`

New `ViewMode` enum determines what to show:
1. `.interactive` — Active session takes over
2. `.areaPicker` — No area set, show AreaPicker
3. `.stepRunner` — Has area, idle, show StepRunner + TransitionBar
4. `.conduct` — Running/waiting/completed/error, show Conduct view

This makes Improvise the default experience for new waves without forcing a mode declaration.

### 8. Branch Naming Fix

**Files:** `src/loopflow/lf/naming.py`, `tests/test_naming.py`

`parse_branch_base()` now handles nested timestamps recursively:

```
jack-heart.concerto-next.20260129_2255.20260129_2318.aurora-rondo → jack-heart.concerto-next
```

This happens when waves are created from other waves' branches (stacking iterations).

## Key choices

| Decision | Why |
|----------|-----|
| Full-panel AreaPicker | Makes area selection obvious and prominent for new waves |
| Step grid, not dropdown | Teaches available steps, better for discovery |
| Direction pills, not text field | Shows composability visually, enables quick editing |
| Sticky TransitionBar | "Should this run autonomously?" persists until answered |
| Per-repo recent areas | Different repos have different relevant areas |
| ViewMode enum | Clean state machine for panel behavior |

## How it fits together

```
User creates wave (sidebar +)
    ↓
WaveDetailPanel sees wave.area == nil
    ↓
Shows AreaPicker (full panel)
    ↓
User picks area → repoState.updateWave(wave, area: [...])
    ↓
WaveDetailPanel sees wave.area != nil && wave.status == .idle
    ↓
Shows StepRunner (ready to improvise)
    ↓
User picks step, enters optional prompt, hits Run
    ↓
sessionState.launchInteractiveSession(...) for steps
    OR repoState.runWave(...) for flows
    ↓
Step completes → wave.recentSteps populated
    ↓
TransitionBar appears: "1 step run — Set Stimulus to run autonomously"
    ↓
User sets stimulus → stimulus.kind != .manual
    ↓
wave.status changes to .running
    ↓
WaveDetailPanel shows Conduct view (existing UI)
```

## Risks and bottlenecks

**NSOpenPanel blocking** — Folder picker runs modally on the main thread. Could freeze UI if file system is slow.

**Recent areas persistence** — Uses hash of repo path as key, which could collide (unlikely). Clears on app reinstall.

**Direction sync** — Silent failure on save error. User sees stale state until next refresh.

**Stimulus picker sheet** — Fixed size (400x500) may not work well on smaller screens.

## What's not included

- Custom flow editor (use existing flows via CLI)
- Fine-grained area selection (whole directories only)
- Prompt history (single field, no recall)
- Step templates ("my review always uses these directions")
- Remote execution (Phase 2)

## Test coverage

| Suite | Result |
|-------|--------|
| Python | 668 passed |
| Swift package | 61 passed |
| Concerto build | succeeded |

## Files changed

| Category | Files |
|----------|-------|
| **New (Swift)** | `AreaPicker.swift`, `StepRunner.swift`, `DirectionPills.swift`, `TransitionBar.swift`, `RecentAreasService.swift` |
| **New (Python)** | Tests for nested timestamp handling |
| **Modified (Swift)** | `WaveDetailPanel.swift`, `WaveSidebar.swift`, `RepoState.swift`, `ConcertoApp.swift`, `Direction.swift` |
| **Modified (Python)** | `naming.py`, `land.py` |
| **Scratch docs** | Design docs for improvise-ux and local notifications |

## Alignment with concerto-next roadmap

From `roadmap/concerto-next/04-improvise-ux.md`:

> "Start by picking where to work"

AreaPicker is the first thing users see for new waves.

> "Quick steps: common single steps"

StepRunner shows grid of common steps prominently.

> "TransitionBar sticky at bottom"

TransitionBar appears after any steps run, persists until stimulus is set.

> "Time from creating wave to running first step: < 15 seconds"

Flow is: create wave → pick area → pick step → run. Four clicks.
