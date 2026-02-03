"""Branch naming utilities for loopflow.

Generates branch names from wave name + config schema.
"""

import random
import re
import subprocess
from datetime import datetime
from pathlib import Path

from loopflow.lf.config import load_config

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
    parts = s.split("-", 1)
    if len(parts) != 2:
        return False
    return parts[0] in MAGICAL and parts[1] in MUSICAL


def generate_branch_name(wave_name: str, repo: Path) -> str:
    """Generate unique branch name for a wave using config schema.

    Schema placeholders:
        {user} - user identifier from config
        {name} - wave name
        {timestamp} - YYYYMMDD_HHMM format
        {words} - magical-musical word pair

    Default schema: "{user}.{name}.{timestamp}.{words}"
    Example: "jack-heart.concerto.20260202_1700.aurora-melody"
    """
    config = load_config(repo)

    # Get schema from config, or use default
    if config and config.branch_names:
        schema = config.branch_names.schema_
    else:
        schema = "{user}.{name}.{timestamp}.{words}"

    # Get user from config
    user = config.user if config else None
    if not user and "{user}" in schema:
        raise ValueError("Config missing 'user' field required by branch_names.schema")

    timestamp = generate_timestamp()

    for _ in range(100):
        words = generate_word_pair()
        candidate = schema.format(
            user=user or "",
            name=wave_name,
            timestamp=timestamp,
            words=words,
        )
        # Clean up any leading/trailing dots from empty placeholders
        candidate = candidate.strip(".")
        candidate = re.sub(r"\.+", ".", candidate)

        if not branch_exists(repo, candidate):
            return candidate

    raise ValueError(f"Could not generate unique branch for wave {wave_name}")
