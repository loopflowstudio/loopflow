"""Branch name generation with configurable schemas."""

import os
import re
import subprocess
from datetime import datetime
from typing import TYPE_CHECKING

from loopflow.lf.naming import generate_word_pair

if TYPE_CHECKING:
    from loopflow.lf.config import BranchNameConfig


def _get_git_username() -> str:
    """Get username from git config user.name or $USER env var."""
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
    # Replace spaces and common special chars with hyphens
    result = re.sub(r"[\s@#$%^&*()+=\[\]{}|\\:;\"'<>,?!~`]+", "-", s)
    # Remove leading/trailing hyphens
    result = result.strip("-")
    # Collapse multiple hyphens
    result = re.sub(r"-+", "-", result)
    # Convert to lowercase for consistency
    return result.lower()


def format_branch_name(short_name: str, config: "BranchNameConfig | None") -> str:
    """Transform short name into full branch name using schema.

    Available placeholders:
        {name}  - the short name provided
        {user}  - git username
        {ts}    - timestamp (YYYYMMDD_HHMM)
        {date}  - date (YYYYMMDD)
        {words} - magical-musical word pair (e.g., wisp-forte)
    """
    if config is None:
        return short_name

    # Access schema_ for Pydantic model (uses alias="schema" in YAML)
    schema = config.schema_
    if schema == "{name}":
        return short_name

    now = datetime.now()
    placeholders = {
        "name": short_name,
        "user": _get_git_username(),
        "ts": now.strftime("%Y%m%d_%H%M"),
        "date": now.strftime("%Y%m%d"),
        "words": generate_word_pair(),
    }

    result = schema
    for key, value in placeholders.items():
        result = result.replace(f"{{{key}}}", value)

    return result
