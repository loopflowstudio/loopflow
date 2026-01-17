# Review: ux-agent

**Verdict: Ready to ship**

This branch delivers substantial UX improvements to Maestro with 13 focused commits. The changes address first-run experience, accessibility, and progressive disclosure without over-engineering.

## Summary

The branch adds:
1. **Embedded terminal via SwiftTerm** — Output streams in-app instead of context-switching to external Terminal
2. **Progressive disclosure** — Context bar and options collapse by default, expand on demand
3. **Accessibility fixes** — Stage badges have icons (not just colors), running state has text fallback for reduced motion
4. **First-run improvements** — Default task, better placeholder, friendlier terminology
5. **Persistence** — User preferences saved via @AppStorage

## What Changed

### New Files
- `TaskRunner.swift` — Lightweight service for embedded terminal state
- `EmbeddedTerminalView.swift` — NSViewRepresentable wrapper for SwiftTerm

### Modified Files
- `Package.swift` — Added SwiftTerm dependency
- `AppState.swift` — Added TaskRunner to environment
- `PromptLauncher.swift` — Default task, collapsible context/options, mode descriptions, embedded terminal toggle
- `ResultsPanel.swift` — Empty state, embedded terminal view, overflow menu
- `WorktreeSidebar.swift` — "Workspaces" header, stage icons, optical centering, selection highlight
- `ContentView.swift` — Removed redundant toolbar item
- `WelcomeWindow.swift` — Concrete tagline

### Design Documents Created
Research and specs that informed the implementation:
- Terminal embedding options (SwiftTerm vs Ghostty vs Warp)
- User profile simulations (New Developer, Power User, Designer/PM)
- Gap analysis comparing to Figma/Cursor/Notion patterns
- Progressive disclosure audit

## Issues Found

### Minor: Typography inconsistency
Mixed use of `.caption` vs `.caption2` across views. Not blocking—the UI is readable and functional.

### Minor: Context bar can overflow
With many attachments, the five chips can extend beyond viewport. Edge case.

### Minor: Token count orphaned
Neither prominent nor hidden. Floating in UI without clear purpose for new users.

### Not blocking but noted
- Embedded terminal toggle is in "More options" — users may not discover it
- No onboarding flow for first-time users (documented as future work)
- Interactive mode still launches external terminal (documented constraint)

## Style Compliance

- No `Args:`/`Returns:` docstrings added
- Imports at top of files
- No backwards-compatibility shims
- Mocks not introduced (no new tests in this branch)
- Private functions use `_` prefix where appropriate

## Code Quality

The implementation is clean:

```swift
// EmbeddedTerminalView.swift — Simple wrapper, no over-engineering
struct EmbeddedTerminalView: NSViewRepresentable {
    let command: String
    let workingDirectory: URL
    let onTerminate: () -> Void
    // ... 83 lines total
}

// TaskRunner.swift — Minimal state management
@Observable
final class TaskRunner {
    var isRunning = false
    var currentCommand: String?
    // ... 32 lines total
}
```

The SwiftTerm integration is straightforward:
- Uses `LocalProcessTerminalView` for PTY handling
- Proper process termination via delegate
- Environment configured for 256-color support

## Open Questions

Consolidated from research, for future consideration:

1. **Embedded terminal default** — Should it be on by default for auto mode? Currently requires discovery.

2. **Onboarding** — What should a first-time walkthrough cover? Candidates: purpose, workspaces, first task, results.

3. **Permission dialog** — Bundle identifier shows gibberish. Developer-only feature, but erodes trust.

4. **Slash commands** — `/design`, `/review` as alternative to dropdown. Aligns with Notion patterns.

5. **@ mentions** — `@src/auth.ts` in prompt to add context. Cursor pattern.

6. **Work-state grouping** — Organize sidebar by In Progress / Ready / Blocked.

7. **Whimsical names** — "floral-tiger" confuses newcomers. Consider task-based naming.

## Recommendation

Ship as-is. The branch improves the baseline experience significantly:
- New users get sensible defaults
- Power users get command preview persistence
- Accessibility users get text fallbacks
- Everyone gets in-app output streaming (when enabled)

The remaining gaps (onboarding, slash commands, @ mentions) are documented and can be addressed in follow-up work. Nothing in this branch introduces regressions or technical debt.
