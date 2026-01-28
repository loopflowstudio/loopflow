"""Wave context determination for prompt assembly."""

from dataclasses import dataclass
from pathlib import Path


@dataclass
class WaveContext:
    """Wave context for prompt assembly."""

    name: str  # e.g., "rust"
    source: str  # "explicit" | "inferred"


def determine_wave(
    repo_root: Path,
    explicit_wave: str | None = None,
) -> WaveContext | None:
    """Determine wave context.

    Priority:
    1. Explicit wave name (from lfd or --wave flag)
    2. Infer from worktree directory name (only if roadmap/<candidate>/ exists)

    An explicit wave is always honored, even without a roadmap folder.
    Inference requires a matching roadmap folder to avoid false positives.
    """
    if explicit_wave:
        return WaveContext(name=explicit_wave, source="explicit")

    # Infer from worktree name — requires matching roadmap folder
    worktree_name = repo_root.name
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
