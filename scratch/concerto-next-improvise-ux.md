# Improvise UX

Build the Improvise mode UI for Concerto: area picker, step runner, and transition to Conduct.

## Context

Concerto has two modes:
- **Conduct**: Dashboard-first, connect when needed, land PRs (mostly built)
- **Improvise**: Create wave, run steps manually, discover (not built)

Phase 1 requires both modes. Conduct is functional. Improvise is missing.

## What to Build

### 1. Area Picker

When a wave is selected and idle, show area selection options:

```
┌─────────────────────────────────────────────────────┐
│ Explore                                             │
├─────────────────────────────────────────────────────┤
│ Recent areas:                                       │
│   src/api/          last: 2h ago                    │
│   swift/Concerto/   last: yesterday                 │
│                                                     │
│ [Browse...]  [From clipboard]  [Current branch]     │
└─────────────────────────────────────────────────────┘
```

- Track recent areas per repo
- Browse opens file picker
- From clipboard uses clipboard as area hint
- Current branch infers area from diff

### 2. Step Runner

Once area is set:

```
┌─────────────────────────────────────────────────────┐
│ src/api/auth/                          [Change Area]│
├─────────────────────────────────────────────────────┤
│ Direction: product-engineer, security    [Edit]     │
├─────────────────────────────────────────────────────┤
│ Quick steps:                                        │
│   [review]  [design]  [implement]  [debug]          │
├─────────────────────────────────────────────────────┤
│ Or run flow:                                        │
│   [ship]  [grind]  [research]  [custom...]          │
├─────────────────────────────────────────────────────┤
│ Prompt:                                             │
│ ┌─────────────────────────────────────────────────┐ │
│ │ add rate limiting to the auth endpoints        │ │
│ └─────────────────────────────────────────────────┘ │
│                                        [Run ▶]      │
└─────────────────────────────────────────────────────┘
```

- Quick steps: common single steps
- Flows: multi-step workflows
- Prompt: optional stimulus text
- Run: launches the selected step/flow

### 3. Transition to Conduct

After running steps, wave has history. Show transition options:

```
┌─────────────────────────────────────────────────────┐
│ auth-feature (4 steps run)                          │
│                                                     │
│ [Add Stimulus]  [Create PR]  [Archive]              │
└─────────────────────────────────────────────────────┘
```

- Add Stimulus: set loop/watch/cron for autonomous execution
- Create PR: opens PR for manual review
- Archive: removes wave from active list

## Implementation Approach

### File Changes

**New files:**
- `swift/Concerto/Views/ImprovisePanel.swift` — Main Improvise view
- `swift/Concerto/Views/AreaPicker.swift` — Area selection component
- `swift/Concerto/Views/StepRunner.swift` — Step/flow selection + prompt
- `swift/Concerto/Services/RecentAreasService.swift` — Track recent areas

**Modified files:**
- `WaveDetailPanel.swift` — Show ImprovisePanel for idle waves without area
- `RepoState.swift` — Add area tracking, step execution
- `LoopflowCore/Models/Wave.swift` — Ensure area field is exposed

### Data Flow

1. User creates wave → idle, no area
2. User picks area → wave.area updated via lfd
3. User picks step + enters prompt → lfd runs step
4. Step completes → wave shows results
5. User can run more steps or transition to Conduct

### lfd Integration

Uses existing lfd commands:
- `lfd area <wave> <paths>` — Set wave area
- `lfd direction <wave> <directions>` — Set wave directions
- `lfd run <wave> <step>` — Run step on wave (may need to add prompt parameter)

## Success Criteria

- Can create wave and pick area
- Can run individual steps with optional prompt
- Can transition to Conduct mode (add stimulus)
- Recent areas persist across app launches

## Out of Scope

- Custom flow editor (use existing FlowPicker)
- Remote execution (Phase 2)
- Fine-grained area selection (just paths for now)
