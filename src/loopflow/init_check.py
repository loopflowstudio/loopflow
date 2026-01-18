"""Check repository initialization status."""

import shutil
from dataclasses import dataclass
from pathlib import Path


@dataclass
class InitStatus:
    """What's configured in the current repo."""

    has_lf_dir: bool
    has_config: bool
    has_commands: bool
    missing_deps: list[str]


def check_init_status(repo_root: Path) -> InitStatus:
    """Return what's configured in this repo."""
    lf_dir = repo_root / ".lf"
    commands_dir = repo_root / ".claude" / "commands"

    return InitStatus(
        has_lf_dir=lf_dir.exists(),
        has_config=(lf_dir / "config.yaml").exists(),
        has_commands=any(commands_dir.glob("*.md")) if commands_dir.exists() else False,
        missing_deps=_check_deps(),
    )


def _check_deps() -> list[str]:
    """Return list of missing required dependencies."""
    from loopflow.lf.launcher import check_claude_available

    missing = []
    if not check_claude_available():
        missing.append("claude")
    if not shutil.which("wt"):
        missing.append("wt")
    return missing
