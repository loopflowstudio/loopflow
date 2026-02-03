---
status: todo
phase: 1
persona: concerto
order: 6
sources: [improviser]
---

# Quick experiment path without creating a wave

Run a step directly from the main repo without creating a wave.

## Problem

The current flow requires: Create Wave → Pick Area → Select Step → Run. For quick experiments, this is three steps too many.

Improvisers want to paste an error and run `lf debug`, or kick off a design session, without committing to a named workstream.

## Approach

Add quick step buttons to two locations:

### 1. Empty state (sidebar)

When no waves exist, show step buttons directly:

```
Quick Experiment
Run a step without creating a wave

[design] [review] [debug] [implement]
```

### 2. Detail panel when no wave is selected

When the sidebar has waves but none is selected, show the same quick experiment UI:

```
┌─────────────────────────────────────────────┐
│  Quick Experiment                           │
│                                             │
│  Run a step on the entire codebase          │
│                                             │
│  [design] [review] [debug] [implement]      │
│                                             │
│  ───────────────────────────────────────    │
│                                             │
│  Or select a wave from the sidebar          │
└─────────────────────────────────────────────┘
```

### Implementation

Quick experiment launches terminal in the **main repo directory** (not a worktree) with the selected step:

```swift
func launchQuickExperiment(step: String) {
    guard let repo = repoState.currentRepo else { return }

    // Launch terminal in main repo
    let command = "cd \(repo.path) && lf \(step)"
    terminalLauncher.launchWithCommand(command)
}
```

No wave is created. No worktree is created. The step runs on the current main branch state.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Create "scratch" wave that auto-deletes | Still creates DB records, worktrees, branches | Overkill for exploration |
| Run from existing worktree if one exists | Confuses which state you're working on | Implicit behavior is bad |
| Add quick experiment to command palette | Discoverability lower than visible buttons | First-time users won't find it |

## Key decisions

**Run on main repo, not worktree.** Quick experiments don't need branch isolation. If the step makes changes, the user can commit them to a new branch manually or create a wave.

**No session tracking.** Quick experiments don't appear in wave history. They're truly fire-and-forget.

**Same four steps as StepRunner.** `design`, `review`, `implement`, `debug` are the common entry points. Advanced users know to run `lf <step>` directly.

## Scope

- In scope: Empty state quick buttons, detail panel placeholder state
- Out of scope: Quick experiment for existing waves, clipboard paste detection, auto-wave promotion

## Done when

```swift
// From empty state
QuickExperimentButtons(repo: repoState.currentRepo) { step in
    terminalLauncher.launchQuickExperiment(step: step, in: repo)
}
```

User clicks "debug" → terminal opens in repo directory → `lf debug` runs → no wave created.
