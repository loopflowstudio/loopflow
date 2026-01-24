"""Branch naming utilities for loopflow.

Provides word lists and functions for generating branch names with magical-musical
suffixes, used by both `lfops next` and agent creation.
"""

import random
import subprocess
from pathlib import Path

# Word lists for generating unique branch names

MAGICAL = [
    "aurora",
    "cascade",
    "crystal",
    "drift",
    "echo",
    "ember",
    "fern",
    "flume",
    "frost",
    "glade",
    "grove",
    "haze",
    "ivy",
    "jade",
    "luna",
    "mist",
    "nova",
    "opal",
    "petal",
    "prism",
    "rain",
    "ripple",
    "sage",
    "shade",
    "spark",
    "star",
    "stone",
    "storm",
    "tide",
    "vale",
    "wave",
    "wisp",
    "wren",
    "zephyr",
]

MUSICAL = [
    "allegro",
    "aria",
    "ballad",
    "cadence",
    "canon",
    "chord",
    "coda",
    "duet",
    "forte",
    "fugue",
    "harmony",
    "hymn",
    "lilt",
    "lyric",
    "melody",
    "motif",
    "opus",
    "prelude",
    "refrain",
    "rondo",
    "sonata",
    "tempo",
    "trill",
    "tune",
    "verse",
    "waltz",
]


def generate_word_pair() -> str:
    """Generate a random magical-musical pair like 'aurora-melody'."""
    magical = random.choice(MAGICAL)
    musical = random.choice(MUSICAL)
    return f"{magical}-{musical}"


def branch_exists(repo: Path, branch: str) -> bool:
    """Check if a branch exists locally or on origin."""
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"refs/heads/{branch}"],
        cwd=repo,
        capture_output=True,
    )
    if result.returncode == 0:
        return True
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"refs/remotes/origin/{branch}"],
        cwd=repo,
        capture_output=True,
    )
    return result.returncode == 0


def parse_branch_base(branch: str) -> str:
    """Extract base branch name (agent name) for next iteration.

    If branch ends with .word1-word2 pattern, strip it.
    If branch ends with .main, strip it.
    Otherwise use as-is.

    Examples:
        'my-feature.main' → 'my-feature'
        'my-feature.nova-waltz' → 'my-feature'
        'my-feature' → 'my-feature'
    """
    # Check if branch ends with .main
    if branch.endswith(".main"):
        return branch[:-5]
    # Check if branch ends with .word1-word2 pattern
    if "." in branch:
        base, suffix = branch.rsplit(".", 1)
        if "-" in suffix:
            word1, word2 = suffix.split("-", 1)
            if word1 in MAGICAL and word2 in MUSICAL:
                return base
    return branch


def generate_next_branch(base: str, repo: Path) -> str:
    """Generate unique branch name for next iteration.

    Appends .word1-word2 suffix, retries if exists.

    Examples:
        'my-feature' → 'my-feature.nova-waltz'
    """
    for _ in range(100):
        candidate = f"{base}.{generate_word_pair()}"
        if not branch_exists(repo, candidate):
            return candidate

    raise ValueError(f"Could not generate unique branch from {base}")
