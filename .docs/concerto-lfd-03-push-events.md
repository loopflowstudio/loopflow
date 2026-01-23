# Project 3: Push-Based Worktree Events

Rich events that push full worktree status on changes.

**Status:** Phase 1 Complete

---

## Problem

Current worktree events are minimal:

```python
# Current: just branch name
Event("worktree.updated", {"branch": "feature-x", "reason": "draft_pr_created"})
Event("worktree.pruned", {"branch": "feature-x", "repo": "/path"})
```

When Concerto receives these, it must do a full re-fetch to get updated status. This defeats the purpose of events—we're still pulling.

---

## Solution

Events include full worktree status. Concerto can update in-place without re-fetching.

### Rich Event Format

```python
Event("worktree.updated", {
    "branch": "feature-x",
    "reason": "commit",  # or "push", "pr_created", "ci_updated", "status_changed"
    "worktree": {
        "path": "/path/to/worktree",
        "branch": "feature-x",
        "status": {
            "staged": 2,
            "modified": 5,
            "untracked": 0,
            "ahead": 4,  # was 3, now 4
            "behind": 0
        },
        "staleness": null,
        "ci": {
            "state": "pending",
            "url": "https://..."
        },
        "pr": {
            "number": 123,
            "state": "open"
        }
    }
})
```

### Event Types

| Event | When | Full Status? |
|-------|------|--------------|
| `worktree.created` | New worktree added | Yes |
| `worktree.updated` | Status changed | Yes |
| `worktree.removed` | Worktree deleted | No (just branch name) |
| `worktree.ci_updated` | CI status changed | Yes (or just CI fields) |

### Delta Updates (Optional Optimization)

For high-frequency updates, send only changed fields:

```python
Event("worktree.delta", {
    "branch": "feature-x",
    "changes": {
        "status.ahead": 4,
        "ci.state": "success"
    }
})
```

Concerto merges delta into existing state. Full event if worktree not in cache.

---

## Triggers

When does lfd emit worktree events?

| Trigger | Event |
|---------|-------|
| Git hook: post-commit | `worktree.updated` (reason: commit) |
| Git hook: post-checkout | `worktree.updated` (reason: checkout) |
| Git hook: post-merge | `worktree.updated` (reason: merge) |
| Push detected | `worktree.updated` (reason: push) |
| PR created | `worktree.updated` (reason: pr_created) |
| CI poll detects change | `worktree.ci_updated` |
| Staleness calculation changes | `worktree.updated` (reason: stale) |
| Worktree pruned | `worktree.removed` |

### Git Hook Integration

lfd installs git hooks that notify it of changes:

```bash
# .git/hooks/post-commit
#!/bin/bash
echo '{"method":"notify","params":{"event":"git.commit","data":{"branch":"'$(git branch --show-current)'"}}}' | nc -U ~/.lf/lfd.sock
```

Or: lfd watches `.git` directories for changes (fsevents on macOS).

---

## Concerto Changes

### Event Handler

```swift
func handleWorktreeEvent(_ event: LFDEvent) {
    switch event.name {
    case "worktree.updated", "worktree.created":
        if let worktree = event.worktree {
            updateWorktreeInPlace(worktree)
        }
    case "worktree.removed":
        removeWorktree(branch: event.branch)
    case "worktree.ci_updated":
        updateWorktreeCI(branch: event.branch, ci: event.ci)
    }
}

func updateWorktreeInPlace(_ updated: Worktree) {
    if let index = worktrees.firstIndex(where: { $0.branch == updated.branch }) {
        worktrees[index] = updated
    } else {
        worktrees.append(updated)
    }
}
```

### No More Full Refresh

```swift
// Before: event triggers full refresh
case .worktreeEvent:
    await listWorktrees()  // fetches everything

// After: event contains the update
case .worktreeEvent(let event):
    handleWorktreeEvent(event)  // in-place update
```

---

## Implementation

### Phase 1: Rich events from existing triggers

1. When lfd emits `worktree.updated`, include full status from cache
2. Concerto updates in-place when event includes `worktree` field
3. Fall back to refresh if `worktree` field missing

### Phase 2: Git hook integration

1. lfd installs git hooks on first connection
2. Hooks notify lfd of git operations
3. lfd refreshes affected worktree, emits event

### Phase 3: Filesystem watching

1. lfd watches `.git` directories with fsevents
2. Detects changes without hooks
3. More reliable than hooks (works for all git operations)

---

## Files to Modify

**Python (lfd):**
- `src/loopflow/lfd/daemon/server.py` — Emit rich events
- `src/loopflow/lfd/worktree_state.py` — Provide status for events
- `src/loopflow/lfd/git_watcher.py` — New: filesystem/hook integration

**Swift (Concerto):**
- `swift/LoopflowCore/Services/LFDEventService.swift` — Parse rich events
- `swift/Concerto/AppState.swift` — In-place update handler

---

## Done When

- [x] `worktree.updated` events include full status (when reason=draft_pr_created)
- [x] Concerto updates in-place (no full refresh when event includes worktree)
- [x] `worktree.pruned` events handled with in-place removal
- [x] `WorktreeEvent` struct includes optional `worktree: Worktree?` field
- [x] `AppState.handleWorktreeEvent()` updates in-place or falls back to refresh
- [ ] Git commits trigger status updates within 1 second (Phase 2: git hooks)
- [ ] CI status changes push to UI within poll interval (Phase 2)
- [ ] No flicker on status changes (mostly done - flicker only on events without status)

## Progress

### Phase 1: Rich events from existing triggers (Complete)

**Python:**
- `server.py` emits rich events with `worktree` field for `draft_pr_created`
- `WorktreeStateService.get_one()` retrieves single worktree status
- `worktrees.changed` method for CLI to notify daemon
- `_broadcast_worktree_event()` helper for consistent event emission

**Swift:**
- `WorktreeEvent` struct has `reason`, `repo`, `worktree` fields
- `LFDEventService.parseEvent()` parses full worktree from event data
- `AppState.handleWorktreeEvent()` updates in-place when status available
- Falls back to `listWorktrees()` if event doesn't include status

### Phase 2: Git hook integration (Future)

Not yet implemented. Requires:
- lfd installs git hooks on first connection
- Hooks notify lfd of git operations
- lfd refreshes affected worktree, emits rich event

### Phase 3: Filesystem watching (Future)

Not yet implemented. Alternative to hooks.
