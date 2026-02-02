"""Branch naming utilities for loopflow.

Provides word lists and functions for generating branch names with magical-musical
suffixes, used by both `lfops next` and agent creation.
"""

import random
import re
import subprocess
from datetime import datetime
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


def generate_timestamp() -> str:
    """Generate timestamp in YYYYMMDD_HHMM format."""
    return datetime.now().strftime("%Y%m%d_%H%M")


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


def _is_timestamp(s: str) -> bool:
    """Check if string matches YYYYMMDD_HHMM format."""
    return bool(re.match(r"^\d{8}_\d{4}$", s))


def _is_word_pair(s: str) -> bool:
    """Check if string is a magical-musical word pair."""
    if "-" not in s:
        return False
    word1, word2 = s.split("-", 1)
    return word1 in MAGICAL and word2 in MUSICAL


def _remove_doubled_prefix(s: str) -> str:
    """Remove doubled prefix from a dot-separated string.

    Examples:
        'jack-heart.concerto.jack-heart.concerto' → 'jack-heart.concerto'
        'foo.bar.foo.bar.baz' → 'foo.bar.baz'
        'foo.bar' → 'foo.bar'
    """
    parts = s.split(".")
    n = len(parts)
    # Check for doubled prefix of length 1, 2, 3, etc.
    for prefix_len in range(1, n // 2 + 1):
        prefix = parts[:prefix_len]
        next_segment = parts[prefix_len : prefix_len * 2]
        if prefix == next_segment:
            # Found a doubled prefix, remove it
            return ".".join(parts[prefix_len:])
    return s


def parse_branch_base(branch: str) -> str:
    """Extract base branch name (wave name) for next iteration.

    Strips suffixes: .main, .timestamp.words, or .timestamp (recursively)
    Also removes any doubled wave name prefix.

    Examples:
        'foo.main' → 'foo'
        'foo.20260127_2204.wisp-forte' → 'foo'
        'foo.20260127_2204' → 'foo'
        'foo.20260127_2204.20260127_2205.wisp-forte' → 'foo'
        'foo' → 'foo'
        'foo.bar.foo.bar.20260127_2204' → 'foo.bar'
    """
    if branch.endswith(".main"):
        return branch[:-5]

    parts = branch.split(".")
    if len(parts) >= 3:
        maybe_words = parts[-1]
        maybe_timestamp = parts[-2]
        if _is_word_pair(maybe_words) and _is_timestamp(maybe_timestamp):
            # Recursively strip in case of nested timestamps
            result = parse_branch_base(".".join(parts[:-2]))
            return _remove_doubled_prefix(result)

    # Also strip trailing timestamp without word pair (from branch naming schema)
    if len(parts) >= 2 and _is_timestamp(parts[-1]):
        result = ".".join(parts[:-1])
        return _remove_doubled_prefix(result)

    return _remove_doubled_prefix(branch)


def extract_iteration_suffix(branch: str) -> str | None:
    """Extract iteration suffix (timestamp.words) from branch name.

    Examples:
        'rust.20260127_1234.wisp-forte' → '20260127_1234.wisp-forte'
        'jack.rust.20260127_1234.wisp-forte' → '20260127_1234.wisp-forte'
        'rust' → None
    """
    parts = branch.split(".")
    if len(parts) >= 3:
        maybe_words = parts[-1]
        maybe_timestamp = parts[-2]
        if _is_word_pair(maybe_words) and _is_timestamp(maybe_timestamp):
            return f"{maybe_timestamp}.{maybe_words}"
    return None


def generate_next_branch(base: str, repo: Path) -> str:
    """Generate unique branch name for next iteration.

    Appends .timestamp.word1-word2 suffix, retries if exists.

    Examples:
        'foo' → 'foo.20260127_2204.wisp-forte'
    """
    timestamp = generate_timestamp()
    for _ in range(100):
        candidate = f"{base}.{timestamp}.{generate_word_pair()}"
        if not branch_exists(repo, candidate):
            return candidate

    raise ValueError(f"Could not generate unique branch from {base}")
