"""Wave context determination for prompt assembly."""

from dataclasses import dataclass
from pathlib import Path


@dataclass
class WaveContext:
    """Wave context for prompt assembly."""

    name: str  # e.g., "rust"
    source: str  # "explicit" | "inferred" | "worktree" | "lfd"


def _lookup_wave_from_lfd(worktree: Path, repo: Path | None = None) -> str | None:
    """Query lfd database for wave name by worktree path.

    Returns wave name if found, None otherwise.
    Fails silently if lfd database is not available.
    """
    try:
        from loopflow.lf.git import find_main_repo
        from loopflow.lfd.wave import get_wave_by_worktree

        # Resolve main repo from worktree if not provided
        main_repo = repo or find_main_repo(worktree)
        wave = get_wave_by_worktree(worktree, repo=main_repo)
        return wave.name if wave else None
    except Exception:
        return None


def determine_wave(
    repo_root: Path,
    explicit_wave: str | None = None,
) -> WaveContext | None:
    """Determine wave context.

    Priority:
    1. Explicit wave name (from lfd or --wave flag)
    2. Query lfd database for wave by worktree path
    3. Wave worktree pattern (<repo>.<wave>.main indicates lfd-created wave)
    4. Infer from worktree directory name (only if roadmap/<candidate>/ exists)

    An explicit wave is always honored, even without a roadmap folder.
    Wave worktrees (created by lfd) are recognized by the .main suffix.
    Other inference requires a matching roadmap folder to avoid false positives.
    """
    if explicit_wave:
        return WaveContext(name=explicit_wave, source="explicit")

    # Query lfd database for wave by worktree path
    lfd_wave = _lookup_wave_from_lfd(repo_root)
    if lfd_wave:
        return WaveContext(name=lfd_wave, source="lfd")

    worktree_name = repo_root.name
    parts = worktree_name.split(".")

    # Check for lfd wave worktree pattern: <repo>.<wave>.main
    # Example: loopflow.rust.main → wave = "rust"
    if len(parts) >= 3 and parts[-1] == "main":
        wave_name = parts[-2]
        # Skip if it looks like a date or common suffix
        if wave_name and not wave_name.isdigit() and wave_name not in ("main", "master"):
            return WaveContext(name=wave_name, source="worktree")

    # Fall back to roadmap-based inference
    for candidate in _extract_wave_candidates(worktree_name):
        roadmap_path = repo_root / "roadmap" / candidate
        if roadmap_path.is_dir():
            return WaveContext(name=candidate, source="inferred")

    return None


def _extract_wave_candidates(worktree_name: str) -> list[str]:
    """Extract wave candidates from worktree/branch names.

    Examples:
    - loopflow.rust-protocol -> ['rust', 'rust-protocol']
    - jack.rust-protocol.20260127 -> ['rust', 'rust-protocol']
    - loopflow.lfflow -> ['lfflow']
    - feature-enterprise-auth -> ['enterprise', 'enterprise-auth']
    """
    candidates = []

    # Split on dots to extract middle parts
    # Pattern: prefix.wave-suffix.date or prefix.wave
    parts = worktree_name.split(".")

    # Try middle parts (skip first which is usually username/repo prefix)
    for part in parts[1:]:
        # Skip date-like suffixes (8 digits)
        if part.isdigit() and len(part) == 8:
            continue

        # Add the full part
        if part and part not in candidates:
            candidates.append(part)

        # Also try first segment before hyphen
        if "-" in part:
            first_segment = part.split("-")[0]
            if first_segment and first_segment not in candidates:
                candidates.append(first_segment)

    # Also try the full name minus common prefixes
    for prefix in ["feature-", "fix-", "chore-"]:
        if worktree_name.startswith(prefix):
            remainder = worktree_name[len(prefix) :]
            if remainder and remainder not in candidates:
                candidates.append(remainder)
            if "-" in remainder:
                first_segment = remainder.split("-")[0]
                if first_segment and first_segment not in candidates:
                    candidates.append(first_segment)

    return candidates
