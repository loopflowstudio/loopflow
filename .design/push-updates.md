# Push-Based Updates: Git Hooks + PR Polling

Complete the push-based architecture so Concerto always reflects current state.

---

## Problem

Concerto updates on initial load and when lfd emits events from existing triggers (like `draft_pr_created`). But:

1. **Local git operations go unnoticed** — commits, checkouts, merges don't trigger updates
2. **CI status changes are invisible** — no way to know when CI passes/fails
3. **Remote merges aren't detected** — PR merged on GitHub, worktree still shows as active

---

## Solution

Two new notification sources feeding into the existing rich event infrastructure:

| Source | What it catches | Latency |
|--------|-----------------|---------|
| **Git hooks** | commit, checkout, merge, rebase | Immediate |
| **PR poller** | CI status, merge status | 30s-5min |

Both trigger the same flow: invalidate cache → refresh state → emit `worktree.updated` event.

---

## 1. Git Hooks

### Hook Types

| Hook | Trigger | What changed |
|------|---------|--------------|
| `post-commit` | After commit | HEAD, staged files |
| `post-checkout` | After checkout/switch | HEAD, working tree |
| `post-merge` | After merge | HEAD, working tree |
| `post-rewrite` | After rebase/amend | Commit history |

### Hook Template

Each hook is a shell script that sends JSON to lfd:

```bash
#!/bin/bash
# Loopflow git hook - notifies lfd of git operations

SOCKET="$HOME/.lf/lfd.sock"
REPO="$(git rev-parse --show-toplevel)"
BRANCH="$(git branch --show-current)"
EVENT="$1"  # commit, checkout, merge, rewrite

# Only notify if socket exists (lfd running)
[ -S "$SOCKET" ] || exit 0

# Send notification (fire and forget)
echo '{"method":"notify","params":{"event":"git.'"$EVENT"'","data":{"repo":"'"$REPO"'","branch":"'"$BRANCH"'"}}}' | nc -U "$SOCKET" &
```

### Hook Installation

**When:** First time lfd sees a repo (via `worktrees.list` or `worktrees.changed`)

**How:** Append to existing hooks (don't replace):
```bash
# If hook exists, append
echo "" >> .git/hooks/post-commit
echo "# Loopflow notification" >> .git/hooks/post-commit
echo '/path/to/lf-hook.sh commit' >> .git/hooks/post-commit

# If hook doesn't exist, create
```

**Safety:**
- Check for existing hooks, preserve them
- Add marker comment so we can identify our additions
- `lfd hooks uninstall <repo>` to remove

### Server Handler

Add to `server.py` dispatch:

```python
elif method == "notify" and params.get("event", "").startswith("git."):
    return await self._handle_git_event(params)
```

Handler:
```python
async def _handle_git_event(self, params: dict) -> Response:
    event = params.get("event")  # git.commit, git.checkout, etc.
    data = params.get("data", {})
    repo = data.get("repo")
    branch = data.get("branch")

    if not repo:
        return error("Missing repo in git event")

    repo_path = Path(repo)
    service = get_worktree_state_service()
    service.invalidate(repo_path)

    # Emit rich event if we have a branch
    if branch:
        worktree_status = service.get_one(repo_path, branch)
        await self._broadcast(Event("worktree.updated", {
            "branch": branch,
            "reason": event.replace("git.", ""),  # "commit", "checkout", etc.
            "repo": str(repo_path),
            "worktree": worktree_status,
        }))

    return success({"event": event})
```

---

## 2. PR State Poller

### What We Poll

`gh pr view --json state,statusCheckRollup,mergedAt,url` returns:
- `state`: OPEN, MERGED, CLOSED
- `statusCheckRollup`: CI status (SUCCESS, PENDING, FAILURE)
- `mergedAt`: When merged (null if not)

One call gives us both CI status and merge status.

### Polling Strategy

**Smart intervals based on state:**

| Current State | Poll Interval | Rationale |
|---------------|---------------|-----------|
| CI pending | 30s | Actively waiting for results |
| CI success/failure | 5min | Stable, just watching for merge |
| PR merged/closed | Stop | Nothing more to watch |
| No PR | Don't poll | Nothing to check |

**Track state to detect changes:**
```python
@dataclass
class PRState:
    branch: str
    repo: Path
    pr_number: int
    ci_state: str | None  # SUCCESS, PENDING, FAILURE
    pr_state: str  # OPEN, MERGED, CLOSED
    last_poll: float
    next_poll: float
```

### When to Start/Stop Polling

**Start polling:**
- When `draft_pr_created` event fires (existing trigger)
- When lfd starts, check existing worktrees for open PRs

**Stop polling:**
- PR state becomes MERGED or CLOSED
- Worktree is pruned/deleted

### Implementation

New file `src/loopflow/lfd/pr_poller.py`:

```python
class PRPoller:
    """Polls PR state for worktrees with open PRs."""

    def __init__(self):
        self._tracked: dict[str, PRState] = {}  # branch -> state
        self._task: asyncio.Task | None = None

    def track(self, repo: Path, branch: str, pr_number: int) -> None:
        """Start tracking a PR."""
        ...

    def untrack(self, branch: str) -> None:
        """Stop tracking a PR."""
        ...

    async def poll_once(self) -> list[tuple[str, dict]]:
        """Poll all tracked PRs due for check. Returns (branch, change) pairs."""
        ...

    async def run(self, broadcast_fn) -> None:
        """Background loop - call broadcast_fn with events when state changes."""
        while True:
            await asyncio.sleep(10)  # Check every 10s which PRs need polling
            for branch, change in await self.poll_once():
                # change = {"ci_state": "success", "pr_state": "open", ...}
                await broadcast_fn(Event("worktree.updated", {
                    "branch": branch,
                    "reason": "ci_updated" if "ci_state" in change else "pr_updated",
                    "repo": str(change["repo"]),
                    "worktree": change.get("worktree"),
                }))
```

### Integration with Server

In `server.py`:

```python
def __init__(self, socket_path: Path):
    ...
    self.pr_poller = PRPoller()

async def start(self) -> None:
    ...
    self._poller_task = asyncio.create_task(
        self.pr_poller.run(self._broadcast)
    )

# When draft_pr_created fires:
self.pr_poller.track(repo, branch, pr_number)

# When worktree pruned:
self.pr_poller.untrack(branch)
```

---

## Files

```
src/loopflow/lfd/
├── git_hooks.py      # NEW: Hook templates and installation
├── pr_poller.py      # NEW: PR state polling
├── worktree_state.py # Existing: Add method to get PR number from worktree
└── daemon/
    └── server.py     # Modify: Handle git events, manage poller
```

---

## Edge Cases

### Git Hooks

1. **Existing hooks** — Append, don't replace. Use marker comments.
2. **Hook permissions** — Ensure executable bit set.
3. **Submodules** — Don't install hooks in submodules.
4. **Worktrees share hooks** — Hooks are in main `.git`, shared across worktrees.

### PR Poller

1. **Rate limits** — `gh` has rate limits. Space out calls, back off on 429.
2. **No gh CLI** — Skip polling if `gh` not available.
3. **Auth issues** — Skip polling if `gh auth status` fails.
4. **Multiple repos** — Track PRs per-repo, poll independently.

---

## Success Criteria

- [x] `git commit` triggers Concerto update within 1s
- [x] `git checkout` triggers Concerto update within 1s
- [x] CI completion triggers Concerto update within 30s
- [x] PR merge triggers worktree marked stale within 5min
- [x] Hooks coexist with existing hooks
- [x] No errors when lfd not running (hooks exit gracefully)

---

## Testing

### Git Hooks

```bash
# Install hooks
lfd hooks install /path/to/repo

# Make a commit, verify event
echo "test" >> file && git add file && git commit -m "test"
# Should see worktree.updated event in lfd logs

# Uninstall
lfd hooks uninstall /path/to/repo
```

### PR Poller

```bash
# Create PR, verify tracking starts
gh pr create --draft

# Wait for CI, verify update
# Should see worktree.updated with ci_state change

# Merge PR, verify tracking stops
gh pr merge
# Should see worktree.updated with pr_state=MERGED, staleness=merged
```

---

## Out of Scope

- Filesystem watching (fsevents) — hooks are sufficient
- Webhook integration — would need server infrastructure
- Cross-repo polling — only poll repos lfd knows about
