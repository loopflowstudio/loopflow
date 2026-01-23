# Project 2: Worktree State Service

Move worktree status calculation from Concerto into lfd.

**Status:** In Progress (Python done, Swift pending)

---

## Problem

Concerto calculates worktree status through multiple async passes:

```
Pass 1: wt list --json           → branch, basic status (~200ms)
Pass 2: detectStaleness()        → 4+ git commands/worktree (~500ms/wt)
Pass 3: fetchCIStatus()          → gh pr checks/worktree (~1-2s/wt)
Pass 4: loadSessions()           → session history/worktree
```

Each pass updates the UI, causing flicker. With 5 worktrees, full status takes 10+ seconds.

### Current Data Flow

```
Concerto                           CLI/Git
   │                                  │
   ├─► WorktreeService.list() ───────►│ wt list --json
   │◄─────────────────────────────────┤ (basic status)
   │                                  │
   ├─► detectStaleness() ────────────►│ git diff, git branch --merged, etc.
   │◄─────────────────────────────────┤ (per worktree, sequential)
   │                                  │
   ├─► fetchCIStatus() ──────────────►│ gh pr checks
   │◄─────────────────────────────────┤ (per worktree, sequential)
   │                                  │
   └─► UI re-renders on each pass
```

---

## Solution

lfd maintains worktree state. Concerto asks lfd instead of running git commands.

### New lfd Endpoint

```python
# Request
{"method": "worktrees.list", "params": {"repo": "/path/to/repo"}}

# Response
{
    "ok": true,
    "result": {
        "worktrees": [
            {
                "path": "/path/to/worktree",
                "branch": "feature-x",
                "status": {
                    "staged": 2,
                    "modified": 5,
                    "untracked": 0,
                    "ahead": 3,
                    "behind": 0
                },
                "staleness": {
                    "stale": true,
                    "reason": "merged",  // or "remote_deleted", "inactive"
                    "age_days": 7
                },
                "ci": {
                    "state": "success",  // or "pending", "failure"
                    "url": "https://github.com/..."
                },
                "pr": {
                    "number": 123,
                    "state": "open",
                    "url": "https://github.com/..."
                },
                "last_session": {
                    "step": "review",
                    "status": "completed",
                    "timestamp": "2026-01-23T10:00:00Z"
                }
            }
        ],
        "cache_age_ms": 150
    }
}
```

### lfd State Management

```python
class WorktreeStateService:
    """Maintains worktree status, refreshes on demand or events."""

    def __init__(self):
        self._cache: dict[str, WorktreeStatus] = {}
        self._cache_time: dict[str, float] = {}
        self._ci_poll_task: asyncio.Task | None = None

    async def list(self, repo: Path) -> list[WorktreeStatus]:
        """Return cached status, refresh if stale."""
        ...

    async def refresh(self, repo: Path, branch: str | None = None) -> None:
        """Refresh status for one or all worktrees."""
        ...

    async def _poll_ci_status(self) -> None:
        """Background task: poll CI status every 60s."""
        ...

    def invalidate(self, repo: Path, branch: str) -> None:
        """Mark a worktree as needing refresh."""
        ...
```

### Cache Invalidation Triggers

| Event | Action |
|-------|--------|
| `worktrees.list` request | Return cache if <5s old, else refresh |
| Git hook (post-commit, post-checkout) | Invalidate affected worktree |
| Step run completes | Invalidate that worktree |
| CI poll interval (60s) | Refresh CI status only |
| `worktrees.refresh` request | Force refresh |

### Concerto Changes

Replace multi-pass loading with single request:

```swift
// Before
let worktrees = try await worktreeService.list(in: repo)
await detectStaleness()  // 4+ git commands/worktree
await fetchCIStatus()    // gh pr checks/worktree

// After
let worktrees = try await lfdService.request("worktrees.list", params: ["repo": repo])
// Done. All status included.
```

---

## Implementation

### Phase 1: Add endpoint, keep CLI fallback

1. Add `worktrees.list` method to lfd server
2. Implement `WorktreeStateService` with basic caching
3. Concerto tries lfd first, falls back to CLI if unavailable

### Phase 2: Full status calculation

1. Move staleness detection into lfd
2. Move CI polling into lfd
3. Include session history in response

### Phase 3: Remove CLI fallback

1. Require lfd for worktree status
2. Remove `detectStaleness()` and `fetchCIStatus()` from Concerto
3. Simplify `WorktreeService` to thin wrapper

---

## Files to Create/Modify

**Python (lfd):**
- `src/loopflow/lfd/worktree_state.py` — New state service
- `src/loopflow/lfd/daemon/server.py` — Add `worktrees.list` handler
- `src/loopflow/lfd/daemon/protocol.py` — Document new method

**Swift (Concerto):**
- `swift/LoopflowCore/Services/LFDService.swift` — Add `worktreesList()` method
- `swift/Concerto/AppState.swift` — Replace multi-pass loading

---

## Done When

- [x] `worktrees.list` endpoint returns full status
- [x] Staleness calculated server-side
- [ ] CI status included in response (needs CI polling in lfd)
- [ ] Concerto loads worktree list in single request (needs Swift changes)
- [ ] No flicker on initial load
- [ ] Status updates within 1 second of changes

## Progress

### Completed
- `WorktreeStateService` in `src/loopflow/lfd/worktree_state.py`
- `worktrees.list` endpoint in daemon server
- Staleness detection using existing `is_merged()` logic
- Recent steps included from step_run database
- Caching with 5-second TTL

### Remaining
- Swift: Add request-response capability to LFDEventService
- Swift: Add `listFromLFD()` method to WorktreeService
- Swift: Update AppState to try lfd first, fall back to CLI
