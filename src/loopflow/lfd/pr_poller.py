"""PR state poller for lfd daemon.

Polls GitHub PR state (CI status + merge status) for worktrees with open PRs.
Uses smart intervals: faster polling when CI is pending, slower when stable.
"""

import asyncio
import json
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Coroutine


@dataclass
class PRState:
    """Tracked state for a PR."""

    repo: Path
    branch: str
    pr_number: int
    ci_state: str | None = None  # SUCCESS, PENDING, FAILURE, None
    pr_state: str = "OPEN"  # OPEN, MERGED, CLOSED
    last_poll: float = 0
    next_poll: float = 0


# Polling intervals in seconds
INTERVAL_PENDING = 30  # CI running, poll frequently
INTERVAL_STABLE = 300  # CI done, poll less often
INTERVAL_INITIAL = 5  # First poll after tracking


def _get_poll_interval(ci_state: str | None, pr_state: str) -> float:
    """Determine next poll interval based on current state."""
    if pr_state in ("MERGED", "CLOSED"):
        return float("inf")  # Stop polling
    if ci_state == "PENDING":
        return INTERVAL_PENDING
    return INTERVAL_STABLE


def _gh_available() -> bool:
    """Check if gh CLI is available and authenticated."""
    try:
        result = subprocess.run(
            ["gh", "auth", "status"],
            capture_output=True,
            timeout=5,
        )
        return result.returncode == 0
    except Exception:
        return False


def _poll_pr(repo: Path, branch: str) -> dict | None:
    """Poll PR state for a branch.

    Returns dict with pr_number, pr_state, ci_state, or None if no PR.
    """
    try:
        result = subprocess.run(
            [
                "gh",
                "pr",
                "view",
                branch,
                "--json",
                "number,state,statusCheckRollup,mergedAt,url",
            ],
            cwd=repo,
            capture_output=True,
            text=True,
            timeout=30,
        )
        if result.returncode != 0:
            return None

        data = json.loads(result.stdout)

        # Extract CI state from statusCheckRollup
        ci_state = None
        checks = data.get("statusCheckRollup", [])
        if checks:
            # Aggregate: any pending = PENDING, any failure = FAILURE, else SUCCESS
            states = [c.get("state", c.get("conclusion", "")) for c in checks]
            if any(s in ("PENDING", "IN_PROGRESS", "QUEUED") for s in states):
                ci_state = "PENDING"
            elif any(s in ("FAILURE", "ERROR", "CANCELLED", "TIMED_OUT") for s in states):
                ci_state = "FAILURE"
            elif states:
                ci_state = "SUCCESS"

        return {
            "pr_number": data.get("number"),
            "pr_state": data.get("state", "OPEN"),  # OPEN, MERGED, CLOSED
            "ci_state": ci_state,
            "url": data.get("url"),
        }
    except Exception:
        return None


class PRPoller:
    """Polls PR state for worktrees with open PRs."""

    def __init__(self):
        self._tracked: dict[str, PRState] = {}  # key = "repo:branch"
        self._gh_available: bool | None = None

    def _key(self, repo: Path, branch: str) -> str:
        return f"{repo}:{branch}"

    def track(self, repo: Path, branch: str, pr_number: int) -> None:
        """Start tracking a PR."""
        key = self._key(repo, branch)
        now = time.time()
        self._tracked[key] = PRState(
            repo=repo,
            branch=branch,
            pr_number=pr_number,
            last_poll=0,
            next_poll=now + INTERVAL_INITIAL,
        )

    def untrack(self, repo: Path, branch: str) -> None:
        """Stop tracking a PR."""
        key = self._key(repo, branch)
        self._tracked.pop(key, None)

    def is_tracked(self, repo: Path, branch: str) -> bool:
        """Check if a PR is being tracked."""
        return self._key(repo, branch) in self._tracked

    def list_tracked(self) -> list[PRState]:
        """List all tracked PRs."""
        return list(self._tracked.values())

    async def poll_due(self) -> list[tuple[PRState, dict]]:
        """Poll PRs that are due for checking.

        Returns list of (state, changes) where changes contains any changed fields.
        """
        if self._gh_available is None:
            self._gh_available = _gh_available()
        if not self._gh_available:
            return []

        now = time.time()
        results = []

        for key, state in list(self._tracked.items()):
            if now < state.next_poll:
                continue

            # Poll in executor to not block
            loop = asyncio.get_event_loop()
            pr_data = await loop.run_in_executor(None, _poll_pr, state.repo, state.branch)

            state.last_poll = now

            if pr_data is None:
                # PR not found or error - keep tracking but back off
                state.next_poll = now + INTERVAL_STABLE
                continue

            # Detect changes
            changes = {}
            if pr_data["ci_state"] != state.ci_state:
                changes["ci_state"] = pr_data["ci_state"]
                state.ci_state = pr_data["ci_state"]

            if pr_data["pr_state"] != state.pr_state:
                changes["pr_state"] = pr_data["pr_state"]
                state.pr_state = pr_data["pr_state"]

            # Update next poll time
            interval = _get_poll_interval(state.ci_state, state.pr_state)
            if interval == float("inf"):
                # PR closed/merged - stop tracking
                self._tracked.pop(key, None)
            else:
                state.next_poll = now + interval

            if changes:
                results.append((state, changes))

        return results

    async def run(
        self,
        broadcast_fn: Callable[[any], Coroutine],
        get_worktree_fn: Callable[[Path, str], dict | None],
    ) -> None:
        """Background loop - polls PRs and broadcasts events.

        Args:
            broadcast_fn: Async function to broadcast events
            get_worktree_fn: Function to get full worktree status for events
        """
        from loopflow.lfd.daemon.protocol import Event

        while True:
            try:
                await asyncio.sleep(10)  # Check every 10s which PRs need polling

                for state, changes in await self.poll_due():
                    # Determine event reason
                    if "pr_state" in changes and changes["pr_state"] == "MERGED":
                        reason = "merged"
                    elif "pr_state" in changes:
                        reason = "pr_state_changed"
                    else:
                        reason = "ci_updated"

                    # Get full worktree status for rich event
                    worktree_status = get_worktree_fn(state.repo, state.branch)

                    await broadcast_fn(
                        Event(
                            "worktree.updated",
                            {
                                "branch": state.branch,
                                "reason": reason,
                                "repo": str(state.repo),
                                "worktree": worktree_status,
                                "changes": changes,
                            },
                        )
                    )

            except asyncio.CancelledError:
                break
            except Exception:
                # Don't let polling errors crash the daemon
                pass
