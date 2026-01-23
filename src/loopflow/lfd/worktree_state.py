"""Worktree state service for lfd daemon.

Provides cached worktree status including staleness detection.
Concerto calls this instead of running multiple git commands.
"""

import time
from pathlib import Path

from loopflow.lf.worktrees import is_merged, list_all
from loopflow.lfd.step_run import load_step_runs_for_worktree


class WorktreeStateService:
    """Maintains worktree status with caching."""

    def __init__(self, cache_ttl_seconds: float = 5.0):
        self._cache: dict[str, list[dict]] = {}  # repo_path -> worktrees JSON
        self._cache_time: dict[str, float] = {}
        self._cache_ttl = cache_ttl_seconds

    def list_worktrees(self, repo: Path) -> list[dict]:
        """Return worktree list with staleness and recent steps.

        Uses cache if fresh, otherwise recalculates.
        Returns JSON-serializable dicts matching Swift's expected format.
        """
        repo_key = str(repo.resolve())
        now = time.time()

        # Check cache
        if repo_key in self._cache:
            cache_age = now - self._cache_time.get(repo_key, 0)
            if cache_age < self._cache_ttl:
                return self._cache[repo_key]

        # Refresh
        result = self._compute_worktrees(repo)
        self._cache[repo_key] = result
        self._cache_time[repo_key] = now
        return result

    def get_one(self, repo: Path, branch: str) -> dict | None:
        """Get status for a single worktree by branch name.

        Returns cached status if available, otherwise refreshes cache.
        Returns None if branch not found.
        """
        worktrees = self.list_worktrees(repo)
        for wt in worktrees:
            if wt["branch"] == branch:
                return wt
        return None

    def invalidate(self, repo: Path, branch: str | None = None) -> None:
        """Invalidate cache for a repo (optionally just one branch)."""
        repo_key = str(repo.resolve())
        if repo_key in self._cache:
            del self._cache[repo_key]
            self._cache_time.pop(repo_key, None)

    def _compute_worktrees(self, repo: Path) -> list[dict]:
        """Compute full worktree status including staleness."""
        worktrees = list_all(repo)
        result = []

        for wt in worktrees:
            # Compute staleness
            staleness = None
            staleness_days = None

            if is_merged(wt, repo):
                staleness = "merged"
            elif not wt.on_origin and wt.branch not in ("main", "master"):
                staleness = "remote_deleted"
            # Note: inactive detection would need timestamp tracking

            # Get recent steps
            recent_steps = []
            try:
                step_runs = load_step_runs_for_worktree(str(wt.path), limit=5)
                recent_steps = [
                    {
                        "id": sr.id,
                        "step": sr.step,
                        "status": sr.status.value,
                        "startedAt": sr.started_at.isoformat() if sr.started_at else None,
                        "endedAt": sr.ended_at.isoformat() if sr.ended_at else None,
                    }
                    for sr in step_runs
                ]
            except Exception:
                pass  # Step run lookup can fail for various reasons

            # Build response matching Swift's WorktreeJSON + extensions
            wt_dict = {
                "branch": wt.branch,
                "path": str(wt.path),
                "base_branch": wt.base_branch,
                "working_tree": {
                    "staged": wt.has_staged,
                    "modified": wt.has_modified,
                    "untracked": wt.has_untracked,
                    "diff_vs_main": {
                        "added": wt.lines_added,
                        "deleted": wt.lines_removed,
                    },
                },
                "main": {
                    "ahead": wt.ahead_main,
                    "behind": wt.behind_main,
                },
                "main_state": wt.main_state,
                "remote": {
                    "name": "origin" if wt.on_origin else None,
                    "ahead": wt.ahead_remote,
                    "behind": wt.behind_remote,
                },
                "operation_state": (
                    "rebase" if wt.is_rebasing else ("merge" if wt.is_merging else None)
                ),
                "ci": {
                    "source": "pr" if wt.pr_url else None,
                    "url": wt.pr_url,
                    "state": wt.pr_state,
                },
                # Extensions beyond wt CLI output
                "prunable": staleness == "merged",
                "staleness": staleness,
                "staleness_days": staleness_days,
                "recent_steps": recent_steps,
            }
            result.append(wt_dict)

        return result


# Global instance for the daemon
_service: WorktreeStateService | None = None


def get_worktree_state_service() -> WorktreeStateService:
    """Get or create the global worktree state service."""
    global _service
    if _service is None:
        _service = WorktreeStateService()
    return _service
