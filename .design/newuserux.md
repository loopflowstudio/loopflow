# New User UX: MVP Prompter

Split Maestro into a focused MVP and beta features behind a flag.

## Implementation

### Flags.swift

```swift
enum Flags {
    static var beta: Bool {
        UserDefaults.standard.bool(forKey: "beta")
    }

    static func setBeta(_ enabled: Bool) {
        UserDefaults.standard.set(enabled, forKey: "beta")
    }
}
```

### What's hidden when `!Flags.beta`

| File | What's hidden |
|------|---------------|
| `WorktreeSidebar.swift` | PIPELINES and AGENTS sections in sidebar |
| `PromptLauncher.swift` | Pipelines in task dropdown (filteredPipelines returns empty) |
| `MaestroApp.swift` | Agents menu item and Cmd+Shift+A shortcut |
| `ContentView.swift` | PipelineEditor view - always shows PromptLauncher instead |

### Progressive disclosure in PromptLauncher

**Always visible:**
- Task selector dropdown (tasks only)
- Text input
- Auto / Interactive mode toggle
- Token count
- Run button

**Collapsed by default** (behind "Options" disclosure):
- Voice selector
- Context toggles (Docs, Files, Diff, Clipboard)
- File attachment drop zone

### Enabling beta mode

Menu: **Maestro > Beta Features** (toggle in app settings menu)

CLI: `defaults write com.loopflow.Maestro beta -bool true`

## Other changes

- `lfops pr` now auto-commits before creating/updating PR (removed `-a` flag)
- `lfops land --create-pr` opens PR in browser after creation
- Pipeline completion opens PR in browser when `pr: true`
