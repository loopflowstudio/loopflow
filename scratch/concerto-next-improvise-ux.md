# Improvise UX

Manual wave exploration through area selection, step execution, and guided transition to autonomous mode.

## Problem

Conduct mode assumes you know what you want: waves exist, flows are chosen, you're landing PRs. Improvise is the opposite: you're discovering what to build.

Today, creating a new wave in Concerto dumps you at an idle screen with a disabled "Start" button. No guidance on picking an area. No way to run individual steps. No path from exploration to autonomous work.

Improvise needs to answer: "I want to poke around src/api/ and see what this agent can do."

## Approach

**One unified detail panel that adapts to wave state.** No separate "Improvise mode"—the WaveDetailPanel detects an unconfigured wave and shows the setup flow. Configured waves show the existing Conduct UI.

The progression:
1. **No area** → Show AreaPicker (prominent, full-panel)
2. **Has area, no steps run** → Show StepRunner (configure + run)
3. **Has steps, still exploring** → Show StepRunner + history
4. **Add stimulus** → Transition complete, shows Conduct view

### Three New Components

**AreaPicker** — Shown when `wave.area == nil`. Dominant UI. Three options:
- Recent areas (per-repo, persisted)
- Browse (NSOpenPanel folder picker)
- Infer from branch diff (if worktree has changes)

**StepRunner** — Shown after area is set. The improvisation cockpit:
- Current area display with "Change" button
- Direction pills (editable inline)
- Step/flow grid (most common: review, design, implement, debug)
- Prompt text field for ad-hoc instructions
- Big "Run ▶" button

**TransitionBar** — Shown after any steps run successfully. Sticky footer:
- "Set Stimulus" → Opens stimulus picker sheet
- "Create PR" → Runs `lf ops pr`
- "Archive" → Moves to archive/hidden

### Key Data Flow

```
User creates wave (sidebar +)
    ↓
WaveDetailPanel sees wave.area == nil
    ↓
Shows AreaPicker (full panel)
    ↓
User picks area → repoState.updateWave(wave, area: [...])
    ↓
WaveDetailPanel sees wave.area != nil && wave.recentSteps.isEmpty
    ↓
Shows StepRunner (ready to improvise)
    ↓
User picks step, enters optional prompt, hits Run
    ↓
sessionState.launchInteractiveSession(...) for interactive steps
    OR repoState.runWave(...) for auto steps
    ↓
Step completes → recentSteps populated
    ↓
TransitionBar appears: "3 steps run — Set Stimulus to run autonomously"
    ↓
User sets stimulus → wave.stimulus.kind != .manual
    ↓
WaveDetailPanel shows Conduct view (existing UI)
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Separate Improvise tab/mode | Clear separation | Doubles UI surface, forces mode declaration before starting |
| Wizard-style multi-step setup | Guided flow | Feels heavy for "just poke around" use case |
| Keep AreaPicker in sidebar | Less intrusive | Buries the critical first step, confusing for new users |
| Inline area editing (click to edit) | Compact | Doesn't teach users what areas are or why they matter |

## Key decisions

**Full-panel AreaPicker, not a dialog.** When you create a wave, the first thing you see is area selection taking over the detail panel. This makes the requirement obvious and the action prominent. A sheet/dialog feels like an interruption; a full panel feels like "this is what you do next."

**Steps as grid, not dropdown.** The FlowPicker uses a dropdown—fine for power users, invisible for discovery. A grid of buttons (review, design, implement, debug) teaches what's available. Common steps first, "More..." expands to full list.

**Direction as pills, not a text field.** Directions are composable ("product-engineer, security"). Pills make this visual. Click pill to remove, click "+" to add from preset list.

**Prompt field always visible.** Even if running a named step like "review", users can add context: "focus on auth endpoints". This makes every step invocation customizable.

**TransitionBar sticky at bottom.** Not inline with content. The question "should this run autonomously?" persists until answered. User can keep improvising, but the option to graduate is always visible.

**Recent areas stored per-repo in UserDefaults.** Key: `recentAreas.<repo-hash>`. Max 5 entries. Sorted by recency.

**"Current branch" area inference.** If the worktree has uncommitted changes, offer to use the paths of changed files as the area. Common pattern: user makes manual edits, wants agent to continue.

## Scope

**In scope:**
- AreaPicker component with recent/browse/infer
- StepRunner component with step grid + direction pills + prompt
- TransitionBar with stimulus/PR/archive actions
- WaveDetailPanel state machine detecting unconfigured waves
- RecentAreasService with UserDefaults persistence
- Direction editing via inline pills

**Out of scope:**
- Custom flow editor (use existing flows, custom via CLI)
- Fine-grained area selection (whole directories only)
- Prompt history (single field, no recall)
- Step templates ("my review always uses these directions")
- Remote execution (Phase 2)

## Done when

```bash
# Create new wave
# → AreaPicker shows, not disabled Start button

# Pick area from recent or browse
# → StepRunner appears with step grid

# Run "review" step
# → Interactive session launches in terminal
# → After completion, TransitionBar appears

# Set stimulus to "loop"
# → Detail panel switches to Conduct view
# → Wave sidebar shows running indicator

# Time from creating wave to running first step: < 15 seconds
```

## File changes

**New files:**
- `swift/Concerto/Views/Improvise/AreaPicker.swift` — Area selection component
- `swift/Concerto/Views/Improvise/StepRunner.swift` — Step/flow execution UI
- `swift/Concerto/Views/Improvise/TransitionBar.swift` — Stimulus/PR/archive actions
- `swift/Concerto/Views/Improvise/DirectionPills.swift` — Inline direction editing
- `swift/Concerto/Services/RecentAreasService.swift` — UserDefaults persistence

**Modified files:**
- `WaveDetailPanel.swift` — Add state machine for unconfigured/improvise/conduct views
- `RepoState.swift` — Add `updateWaveDirection(_:, directions:)` method
- `WaveService.swift` — Ensure area/direction update APIs work

## Wave alignment

Following concerto-next principles from `roadmap/concerto-next/`:

> "Improvise: Create wave, run steps manually, discover"

This design makes Improvise the default experience for new waves. No configuration required upfront—just create and start exploring.

> "Same UI, Different Workflows. Conduct and Improvise aren't tabs or modes in the UI."

The design uses a single WaveDetailPanel that adapts based on wave state. Users don't declare "I want Improvise mode"—they just create a wave and the UI guides them.

> "Quick steps: common single steps"

StepRunner shows a grid of common steps (review, design, implement, debug) prominently, with flows available but secondary.
