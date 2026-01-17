# ux-agent

UX improvements to Maestro: embedded terminal, progressive disclosure, accessibility, and first-run experience.

## What Changed

**Embedded Terminal**
- `EmbeddedTerminalView.swift` — SwiftTerm wrapper for in-app terminal output with proper PATH handling for GUI apps
- `TaskRunner.swift` — State management for embedded terminal
- `ResultsPanel.swift` — Shows embedded terminal when running in auto mode
- `Package.swift` — SwiftTerm dependency

**Progressive Disclosure**
- `PromptLauncher.swift` — Context bar and advanced options collapse by default, expand on demand
- `@AppStorage` for persisting user preferences (contextBarExpanded, showCommandPreview, useEmbeddedTerminal)

**Accessibility**
- `WorktreeSidebar.swift` — Stage badges have icons (lightbulb, hammer, magnifyingglass, sparkles) not just colors
- `RunningIndicator` — Text fallback ("Running") for users with reduced motion

**First-Run Experience**
- Default task selection ("implement")
- Mode descriptions ("Runs to completion" vs "Chat with the AI")
- "Workspaces" header with help text
- Optical centering in empty state

**Daemon Improvements**
- `lfd/launchd.py` — Modern launchctl APIs (`bootstrap`/`bootout` instead of deprecated `load`/`unload`), `kickstart -k` for restart
- `lfd/server.py` — PID file tracking (`~/.lf/lfd.pid`)
- `lfd/__init__.py` — `install()` restarts if already running

**Setup Simplification**
- `SetupService.swift` — Single `install()` method, auto-installs uv if missing
- `SetupView.swift` — Unified install flow

**Cleanup**
- UX prompt files simplified (removed redundant context directives)
- `WelcomeWindow.swift` — Concrete tagline
- Removed debug print statements from ResultsPanel

## Design Notes

**SwiftTerm choice**: Evaluated Ghostty and Warp. SwiftTerm is simplest—single dependency, MIT licensed, works with LocalProcessTerminalView for PTY handling.

**Progressive disclosure pattern**: Following Notion/Stripe. Simple surface (task, input, run button), expand for power features (model, voice, context toggles, command preview).

**Embedded terminal default**: Currently opt-in via "More options". Could be default for auto mode in future—decision captured in open questions.

## Open Questions

1. Should embedded terminal be on by default for auto mode?
2. What should first-time onboarding cover?
3. Slash commands (`/design`) as alternative to task dropdown?
4. `@src/auth.ts` mentions for context (Cursor pattern)?
5. Work-state grouping in sidebar (In Progress / Ready / Blocked)?
