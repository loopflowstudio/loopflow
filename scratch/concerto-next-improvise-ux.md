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
| Show StepRunner immediately with area field | Less friction | Hides the importance of area selection; users skip it |
| Modal for area selection | Focused attention | Feels interruptive; panel flow is more natural |
| Direction selection required before running | Ensures intent | Most users want defaults; friction kills exploration |

## Key decisions

**Full-panel AreaPicker, not a dialog.** When you create a wave, the first thing you see is area selection taking over the detail panel. This makes the requirement obvious and the action prominent. A sheet/dialog feels like an interruption; a full panel feels like "this is what you do next."

**Steps as grid, not dropdown.** The FlowPicker uses a dropdown—fine for power users, invisible for discovery. A grid of buttons (review, design, implement, debug) teaches what's available. Common steps first, "More..." expands to full list.

**Direction as pills, not a text field.** Directions are composable ("product-engineer, security"). Pills make this visual. Click pill to remove, click "+" to add from preset list.

**Prompt field always visible.** Even if running a named step like "review", users can add context: "focus on auth endpoints". This makes every step invocation customizable.

**TransitionBar sticky at bottom.** Not inline with content. The question "should this run autonomously?" persists until answered. User can keep improvising, but the option to graduate is always visible.

**Recent areas stored per-repo in UserDefaults.** Key: `recentAreas.<repo-hash>`. Max 5 entries. Sorted by recency.

**"Current branch" area inference.** If the worktree has uncommitted changes, offer to use the paths of changed files as the area. Common pattern: user makes manual edits, wants agent to continue.

**ViewMode enum in WaveDetailPanel.** Four states: `interactive`, `areaPicker`, `stepRunner`, `conduct`. Priority order determines which view shows. Interactive sessions always win. This eliminates complex conditional logic.

## Wild Success

Six months out, Improvise is how everyone starts. The pattern:

1. User has a vague idea: "auth feels janky"
2. Creates wave, sees AreaPicker, picks `src/auth/`
3. Runs `review` — agent produces a thorough analysis
4. Reads it, thinks "actually, the token refresh is the issue"
5. Runs `design` with prompt: "focus on token refresh edge cases"
6. Agent produces a design they love
7. Hits "Set Stimulus → Loop" — wave runs autonomously
8. Next morning, PR is ready

What made it great:
- **Zero config** — creating a wave took one click
- **Discoverable** — the step grid taught them what's possible
- **Low commitment** — they could poke around before committing to autonomous work
- **Smooth transition** — going from exploration to production felt like one continuous workflow

Users say: "I used to overthink what flow to use. Now I just start with review and see where it goes."

## Wild Failure

Six months out, users avoid Improvise entirely. The pattern:

1. User creates wave, sees AreaPicker, confused
2. Picks "Entire repository" because they're not sure what to do
3. Runs `review` — agent produces unfocused analysis (too broad)
4. Tries `design` — agent asks clarifying questions, user doesn't know how to answer
5. Abandons wave, goes back to CLI: `lf design -a src/auth/`

What went wrong:
- **Area selection felt like a gate** — users didn't understand why it mattered
- **Steps grid didn't explain anything** — users clicked randomly, got random results
- **Prompt field was ignored** — users didn't realize they could/should add context
- **No feedback loop** — after running a step, users didn't know if it worked or what to do next
- **TransitionBar was noise** — users didn't understand "stimulus" and ignored it

The lesson: Improvise failed because it assumed users understood the loopflow mental model. It optimized for speed over understanding.

## Refinements to Avoid Failure

### 1. AreaPicker Needs Context

Current: "Choose where to focus this wave."

Better: Show a visual hint of what areas are available. If the repo has `src/`, `tests/`, `docs/`, show them as quick options with file counts. "src/ (847 files)" tells users this is a substantial directory.

**Decision:** Add a "Common directories" section below Recent that shows top-level directories from the repo with file counts. Makes area selection feel informed, not arbitrary.

### 2. Step Grid Needs Descriptions

Current: Grid of buttons: `review`, `design`, `implement`, `debug`

Better: On hover (or always visible on first use), show one-line descriptions:
- **review** — analyze architecture, complexity, quality
- **design** — interactive session to plan changes
- **implement** — build from a design
- **debug** — fix an error (paste it in the prompt)

**Decision:** Add `.help()` tooltips to step buttons. First-time users see them; power users ignore them.

### 3. Prompt Field Should Guide

Current: "Additional context (optional)" with placeholder "e.g., focus on auth endpoints"

Better: When specific steps are selected, the prompt guidance changes:
- **debug** → "Paste the error message or describe the bug"
- **review** → "What aspect to focus on? (e.g., security, performance)"
- **design** → "What are you trying to build or change?"
- **implement** → "Any specific requirements or constraints?"

**Decision:** Dynamic prompt placeholder based on selected step. Teaches users what kind of input is useful.

### 4. After Step Completion, Show Actionable Summary

Current: TransitionBar shows "3 steps run" with generic buttons.

Better: After a step completes, show a summary card:
- For `review`: "Analysis complete. Key findings: X, Y, Z. → Run design to plan fixes"
- For `design`: "Design written to scratch/. → Run implement to build it"
- For `implement`: "Changes made. → Run review to verify, or set stimulus to loop"

**Decision:** Add a `StepSummaryCard` below StepRunner that shows context-aware next steps. Bridges the gap between "step done" and "what now?"

### 5. TransitionBar Should Explain

Current: "Set Stimulus" button without explanation.

Better: "Make this wave run automatically" with a brief explanation:
- Loop: "Keep running until you stop it"
- Watch: "Run when files change on main"
- Schedule: "Run on a schedule (9am daily)"

**Decision:** Move explanations into TransitionBar preview, not just the sheet. Users should understand the choice before clicking.

## Scope

**In scope:**
- AreaPicker component with recent/browse/infer
- StepRunner component with step grid + direction pills + prompt
- TransitionBar with stimulus/PR/archive actions
- WaveDetailPanel state machine detecting unconfigured waves
- RecentAreasService with UserDefaults persistence
- Direction editing via inline pills
- Step button tooltips with descriptions
- Dynamic prompt placeholder based on selected step

**Out of scope:**
- Custom flow editor (use existing flows, custom via CLI)
- Fine-grained area selection (whole directories only)
- Prompt history (single field, no recall)
- Step templates ("my review always uses these directions")
- Remote execution (Phase 2)
- StepSummaryCard (good idea, but adds complexity—revisit after basic flow works)
- Common directories with file counts (nice-to-have, not essential)

## Done when

```bash
# Create new wave
# → AreaPicker shows, not disabled Start button

# Pick area from recent or browse
# → StepRunner appears with step grid

# Hover over step button
# → Tooltip shows what the step does

# Select "debug" step
# → Prompt placeholder changes to "Paste the error message..."

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
- `WaveDetailPanel.swift` — Add ViewMode enum and state machine for unconfigured/improvise/conduct views
- `RepoState.swift` — Ensure `updateWave(_:, area:)` and `updateWave(_:, direction:)` methods work
- `WaveService.swift` — Ensure area/direction update APIs work

## Wave alignment

Following concerto-next principles from `roadmap/concerto-next/`:

> "Improvise: Create wave, run steps manually, discover"

This design makes Improvise the default experience for new waves. No configuration required upfront—just create and start exploring.

> "Same UI, Different Workflows. Conduct and Improvise aren't tabs or modes in the UI."

The design uses a single WaveDetailPanel that adapts based on wave state. Users don't declare "I want Improvise mode"—they just create a wave and the UI guides them.

> "Quick steps: common single steps"

StepRunner shows a grid of common steps (review, design, implement, debug) prominently, with flows available but secondary.

## Implementation Status

All components implemented:
- [x] AreaPicker with recent/browse/infer
- [x] StepRunner with step grid, direction pills, prompt field
- [x] TransitionBar with stimulus picker sheet
- [x] DirectionPills with add/remove
- [x] RecentAreasService with UserDefaults persistence
- [x] WaveDetailPanel ViewMode state machine
- [x] Step button tooltips (`.help()` modifiers)
- [x] Dynamic prompt placeholder based on selected step
