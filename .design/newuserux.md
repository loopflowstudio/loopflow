# New User UX: MVP Prompter

Split Maestro into a focused MVP and beta features behind a flag.

## What to build

A feature flag system that hides agents and pipelines, leaving only the core prompter: task selector, text input, and run mode toggle.

## Data structures

```swift
// Maestro/Maestro/Flags.swift
enum Flags {
    static var beta: Bool {
        UserDefaults.standard.bool(forKey: "beta")
    }

    static func setBeta(_ enabled: Bool) {
        UserDefaults.standard.set(enabled, forKey: "beta")
    }
}
```

## Key functions

```swift
// No new functions needed - this is conditional rendering
// in existing views based on Flags.beta
```

## UI changes

### Hide when `!Flags.beta`

| Location | What to hide |
|----------|--------------|
| `WorktreeSidebar.swift:273-324` | "PIPELINES" section |
| `WorktreeSidebar.swift:326-381` | "AGENTS" section |
| `PromptLauncher.swift:195-232` | Pipelines in task dropdown |
| `MaestroApp.swift` | AgentWindow menu item / shortcut |
| `ContentView.swift` | PipelineEditor view switching |

### Progressive disclosure in PromptLauncher

Current layout shows everything at once. Change to:

**Always visible:**
- Task selector dropdown (tasks only, no pipelines)
- Big text input
- Auto / Interactive mode toggle
- Run button

**Collapsed by default** (disclosure chevron):
- Voices selector
- Context toggles (Docs, Files, Diff, Clipboard)
- File attachment drop zone
- Token count

### Enable beta mode

Add to app menu: **Maestro > Enable Beta Features** (toggles `Flags.beta`)

Or: detect `defaults write com.loopflow.Maestro beta -bool true` for power users.

## Default context (no changes needed)

Already correct in both loopflow and Maestro:

| Setting | Default | Why |
|---------|---------|-----|
| Docs | ON | Repo-wide `.md` files give essential context |
| Diff Files | ON | Full content of changed files is primary context |
| Raw Diff | OFF | Redundant when diff files are included |
| Clipboard | OFF | Explicit opt-in for pasting |

These are the right defaults for new users. Advanced toggles are collapsed but respect these defaults.

## Constraints

- "Beta" must feel intentional, not broken. Hiding features cleanly, no dangling references.
- User's `selectedPipeline` state should be cleared when beta is disabled mid-session.
- Keyboard shortcut Cmd+Shift+A (agents window) should be no-op when `!Flags.beta`.

## Done when

1. Fresh launch shows only: task dropdown, text input, mode toggle, run button
2. Context options (voices, docs/files/diff/clipboard) hidden behind disclosure
3. No pipelines or agents visible anywhere in default mode
4. `defaults write com.loopflow.Maestro beta -bool true` + relaunch shows full UI
5. Menu item exists to toggle beta mode
