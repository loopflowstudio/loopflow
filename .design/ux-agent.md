# ux-agent

UX improvements to Maestro: embedded terminal, progressive disclosure, accessibility, and first-run experience.

## Review

**Verdict: Needs work**

The committed changes (14 commits) are solid. The uncommitted changes contain valuable lfd improvements but need to be committed—currently the branch is in an inconsistent state.

### Issue: Uncommitted daemon improvements

7 files with uncommitted changes:
- `SetupService.swift` — Simplified to single `install()` method, added uv installation
- `SetupView.swift` — Removed multi-step progress, unified to single install flow
- `RepoWindow.swift` — Minor: uses `lfInstalled` instead of `allInstalled`
- `publish.md` — Added `lfd install` step to restart daemon after publish
- `lfd/__init__.py` — `install()` now restarts if already running
- `lfd/launchd.py` — Modern launchctl APIs (bootstrap/bootout/kickstart), PID file tracking
- `lfd/server.py` — Writes PID file on startup, cleans up on shutdown

These are good changes—the launchd migration from deprecated `load`/`unload` to modern `bootstrap`/`bootout` is overdue, and the PID file tracking enables reliable process management. But they should be committed.

### Issue: Inline import in launchd.py

Line 52-55 in `launchd.py`:
```python
def _generate_plist() -> str:
    """Generate the launchd plist XML."""
    import sys
    lfd_path = _find_lfd_executable()
```

STYLE.md: "Put imports at the top of the file." The `import sys` should be at module level. Same issue in `install()` (line 141) and `uninstall()` (line 179) with `import time`.

### Issue: Circular import workaround

The inline imports appear to be working around a circular import. The `_find_lfd_executable` function imports `sys` conditionally:
```python
def _find_lfd_executable() -> str:
    # ...
    import sys
    return sys.executable
```

This is valid but should be documented if intentional. If it's just copy-paste, move imports to top.

## What Changed

### Committed (14 commits)

**Embedded Terminal**
- `EmbeddedTerminalView.swift` — SwiftTerm wrapper for in-app terminal output
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

**Cleanup**
- UX prompt files simplified (removed redundant context directives)
- `WelcomeWindow.swift` — Concrete tagline

### Uncommitted

**Daemon Improvements**
- Modern launchctl APIs: `bootstrap`/`bootout` instead of deprecated `load`/`unload`
- `kickstart -k` for proper restart semantics
- PID file tracking (`~/.lf/lfd.pid`)
- `lfd install` restarts if already running

**Setup Simplification**
- Single `install()` method instead of multi-step
- Auto-installs uv if missing
- Removed wt status tracking (handled by lfops install)

## Style Compliance

Committed changes:
- No `Args:`/`Returns:` docstrings added
- Imports at top of files
- Private functions use `_` prefix where appropriate
- No backwards-compatibility shims

Uncommitted changes:
- Inline imports in `launchd.py` (3 occurrences) — should be fixed

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
