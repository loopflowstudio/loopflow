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


def _get_git_username() -> str:
    """Get username from git config user.name or $USER env var."""
    import os

    result = subprocess.run(
        ["git", "config", "user.name"],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return _sanitize_for_branch(result.stdout.strip())

    return _sanitize_for_branch(os.environ.get("USER", "user"))


def _sanitize_for_branch(s: str) -> str:
    """Replace spaces, special chars with hyphens for valid git branch names."""
    result = re.sub(r"[\s@#$%^&*()+=\[\]{}|\\:;\"'<>,?!~`]+", "-", s)
    result = result.strip("-")
    result = re.sub(r"-+", "-", result)
    return result.lower()


def parse_branch_base(branch: str, repo: Path) -> str:
    """Extract wave name from branch using the schema to know what to strip.

    Inverse of generate_branch_name. Uses the config schema to identify
    which parts are {user}, {ts}/{timestamp}, {words} and strips them,
    leaving only the {name} portion.

    Gets user from git config (same source as branch generation).

    Examples (with schema="{user}.{name}.{ts}", user="jack-heart"):
        'jack-heart.concerto.20260202_1700' → 'concerto'
        'jack-heart.concerto' → 'concerto'
        'concerto.20260202_1700' → 'concerto'
        'concerto' → 'concerto'
    """
    config = load_config(repo)

    if config and config.branch_names:
        schema = config.branch_names.schema_
    else:
        schema = "{user}.{name}.{timestamp}.{words}"

    # Get user from git config (same source as branch generation)
    user = _get_git_username()

    # Build a regex from the schema to extract {name}
    pattern = re.escape(schema)

    # {user} matches the git username
    pattern = pattern.replace(r"\{user\}", re.escape(user))

    # {name} is what we want to capture
    pattern = pattern.replace(r"\{name\}", r"(?P<name>.+?)")

    # {ts} and {timestamp} match YYYYMMDD_HHMM
    pattern = pattern.replace(r"\{ts\}", r"\d{8}_\d{4}")
    pattern = pattern.replace(r"\{timestamp\}", r"\d{8}_\d{4}")

    # {date} matches YYYYMMDD
    pattern = pattern.replace(r"\{date\}", r"\d{8}")

    # {words} matches magical-musical word pairs
    word_pattern = r"(?:" + "|".join(MAGICAL) + r")-(?:" + "|".join(MUSICAL) + r")"
    pattern = pattern.replace(r"\{words\}", word_pattern)

    # Try matching with full pattern first
    match = re.fullmatch(pattern, branch)
    if match and "name" in match.groupdict():
        return match.group("name")

    # Fallback: strip known suffixes manually
    parts = branch.split(".")

    # Strip trailing word pair and timestamp
    if len(parts) >= 2 and _is_word_pair(parts[-1]) and _is_timestamp(parts[-2]):
        parts = parts[:-2]
    elif len(parts) >= 1 and _is_timestamp(parts[-1]):
        parts = parts[:-1]

    # Strip leading user prefix
    if parts and parts[0] == user:
        parts = parts[1:]

    return ".".join(parts) if parts else branch


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
