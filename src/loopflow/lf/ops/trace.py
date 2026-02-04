"""Ops tracing for parity testing.

When LF_TRACE=1, ops commands emit JSON traces instead of executing.
This enables comparison between Python and Rust implementations.
"""

from __future__ import annotations

import hashlib
import json
import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


def trace_enabled() -> bool:
    """Check if tracing is enabled via LF_TRACE env var."""
    return os.environ.get("LF_TRACE", "").lower() in ("1", "true", "yes")


@dataclass
class OpTrace:
    """A single traced operation."""

    op: str
    result: str | None = None
    args: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        d: dict[str, Any] = {"op": self.op}
        if self.result is not None:
            d["result"] = self.result
        if self.args:
            d.update(self.args)
        return d


@dataclass
class Tracer:
    """Collects operation traces during ops command execution."""

    command: str
    options: dict[str, Any] = field(default_factory=dict)
    operations: list[OpTrace] = field(default_factory=list)

    def trace(self, op: str, result: str | None = None, **kwargs: Any) -> None:
        """Record an operation."""
        self.operations.append(OpTrace(op=op, result=result, args=kwargs))

    def to_dict(self) -> dict[str, Any]:
        return {
            "command": self.command,
            "options": self.options,
            "operations": [op.to_dict() for op in self.operations],
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2)


def hash_prompt(prompt: str) -> str:
    """Hash a prompt for trace comparison (avoids large diffs)."""
    return hashlib.sha256(prompt.encode()).hexdigest()[:16]


def normalize_path(path: Path | str, repo_root: Path) -> str:
    """Normalize a path to be relative to repo root."""
    try:
        return str(Path(path).relative_to(repo_root))
    except ValueError:
        return str(path)


# Mock responses for trace mode
MOCK_RESPONSES = {
    "git:status": "dirty",
    "git:diff_cached": "has_changes",
    "git:has_upstream": True,
    "lint:run": "pass",
    "agent:commit_message": {"title": "mock commit", "body": ""},
    "agent:pr_message": {"title": "mock pr", "body": ""},
    "gh:pr_exists": False,
}


def mock_response(op: str) -> Any:
    """Get a mock response for an operation in trace mode."""
    return MOCK_RESPONSES.get(op)
