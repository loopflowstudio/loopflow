"""Auto-prune merged worktrees for lfd daemon."""

import logging
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path

from loopflow.lf.config import AutopruneConfig, load_config
from loopflow.lf.git import get_default_branch
from loopflow.lf.worktrees import find_merged, find_worktree_root, remove

log = logging.getLogger(__name__)


@dataclass
class PruneState:
    """Track prune state per repo."""

    repo: Path
    last_check: datetime | None = None


@dataclass
class AutopruneManager:
    """Manage auto-pruning across repos."""

    poll_interval_seconds: int = 60
    _states: dict[Path, PruneState] = field(default_factory=dict)

    def check_and_prune(self, repo: Path) -> list[str]:
        """Check for merged worktrees and prune them. Returns pruned branch names."""
        config = load_config(repo)
        if not config:
            return []

        autoprune = config.autoprune
        if isinstance(autoprune, bool):
            if not autoprune:
                return []
            autoprune = AutopruneConfig(enabled=True)

        if not autoprune.enabled:
            return []

        # Check if poll interval has elapsed
        state = self._states.get(repo)
        now = datetime.now()
        if state and state.last_check:
            elapsed = (now - state.last_check).total_seconds()
            if elapsed < autoprune.poll_interval_seconds:
                return []

        # Update last check time
        if not state:
            state = PruneState(repo=repo)
            self._states[repo] = state
        state.last_check = now

        # Find and prune merged worktrees
        try:
            base_branch = get_default_branch(repo)
            merged = find_merged(repo, base_branch)
        except Exception as e:
            log.warning(f"Failed to find merged worktrees in {repo}: {e}")
            return []

        pruned = []
        for wt in merged:
            if wt.is_dirty:
                continue
            try:
                if remove(repo, wt.branch):
                    pruned.append(wt.branch)
                    log.info(f"Auto-pruned worktree: {wt.branch}")
            except Exception as e:
                log.warning(f"Failed to prune {wt.branch}: {e}")

        return pruned


def get_repos_to_check() -> list[Path]:
    """Get list of repos that might need auto-pruning.

    Returns repos that have worktrees (i.e., main repo roots).
    """
    from loopflow.lfd.step_run import load_step_runs
    from loopflow.lfd.wave import list_waves

    repos = set()

    # Get repos from active step runs
    for step_run in load_step_runs(active_only=True):
        if step_run.repo:
            repos.add(Path(step_run.repo))

    # Get repos from waves
    for wave in list_waves():
        if wave.repo:
            repos.add(wave.repo)

    # For each repo, find the worktree root (main repo)
    main_repos = set()
    for repo in repos:
        try:
            root = find_worktree_root(repo)
            if root:
                main_repos.add(root)
        except Exception:
            pass

    return list(main_repos)
