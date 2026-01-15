# Live Update

Live-refreshing worktree list via push notifications from lfd, plus main branch protection.

## Summary

Maestro now receives push notifications from lfd when worktrees are created or removed, instead of polling. When running from the main branch, Maestro auto-creates a new worktree with a random name.

## Architecture

```
┌─────────────────┐    post-create     ┌─────────────────┐
│   worktrunk     │ ─────────────────► │  lfd notify     │
│   (wt switch)   │    pre-remove      │  CLI command    │
└─────────────────┘                    └────────┬────────┘
                                                │
                                                ▼
                                       ┌─────────────────┐
                                       │   lfd daemon    │
                                       │  Unix socket    │
                                       │  ~/.lf/lfd.sock │
                                       └────────┬────────┘
                                                │ broadcast
                                                ▼
                                       ┌─────────────────┐
                                       │    Maestro      │
                                       │  (subscribed)   │
                                       └─────────────────┘
```

## What was built

### Python: lfd notify command

`lfd notify <event> [--branch] [--path]` sends events to the daemon for broadcast to subscribers. Fails silently if daemon not running (safe for use in hooks).

### Python: lfd server notify handler

The daemon's `_handle_notify()` method accepts external events and broadcasts them to subscribed clients.

### Swift: LFDEventService

Unix socket client that subscribes to worktree events and calls a callback when events arrive. Uses Network framework's `NWConnection` for async socket communication.

### Swift: AppState integration

`startEventSubscription()` connects to lfd on repo open and refreshes worktrees on any `worktree.*` event.

### Swift: Main branch protection

`PromptLauncher.launchInTerminal()` detects when main is selected and auto-creates a worktree with a random name (e.g. "aurora-allegro") before launching the task.

### Swift: Selection preservation

`refreshWorktrees()` now preserves the selected worktree by matching on branch name after refresh.

## Setup required

Users must configure worktrunk hooks to send notifications:

```toml
# ~/.config/worktrunk/config.toml

[post-create]
lfd-notify = "lfd notify worktree.created --branch '{{ branch }}' --path '{{ worktree_path }}'"

[pre-remove]
lfd-notify = "lfd notify worktree.removed --branch '{{ branch }}'"
```

## Constraints

- **Silent failures**: `lfd notify` catches all errors to avoid breaking worktrunk hooks
- **No daemon = no events**: If lfd isn't running, Maestro won't get live updates
- **Idempotent refresh**: Multiple events may arrive; `refreshWorktrees()` is safe to call repeatedly

## Files changed

| File | Change |
|------|--------|
| `src/loopflow/lfd/server.py` | Add `_handle_notify()` method |
| `src/loopflow/lfd/__init__.py` | Add `notify` CLI command |
| `Maestro/Maestro/Services/LFDEventService.swift` | New - Unix socket client |
| `Maestro/Maestro/Services/NameGenerator.swift` | New - Random name generator |
| `Maestro/Maestro/AppState.swift` | Event subscription, selection preservation |
| `Maestro/Maestro/Views/PromptLauncher.swift` | Main branch protection |
| `Maestro/Maestro/Views/WorktreeSidebar.swift` | Removed polling (now uses push) |
