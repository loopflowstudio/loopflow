"""Outcome metadata for disposable test artifacts; absent evidence is unknown."""

import json
from pathlib import Path

MARKER = ".cleanup-evidence.json"


def evidence_outcome(root: Path) -> str:
    if (root / ".cleanup-keep").exists():
        return "saved"
    try:
        marker = root / MARKER
        if marker.is_symlink() or marker.stat().st_size > 4096:
            return "unknown"
        record = json.loads(marker.read_text())
        if record.get("saved") is True:
            return "saved"
        outcome = record.get("outcome")
        return outcome if outcome in {"passed", "failed"} else "unknown"
    except (OSError, ValueError, AttributeError, TypeError):
        return "unknown"


def record_outcome(root: Path, passed: bool) -> None:
    if not root.is_dir() or evidence_outcome(root) == "saved":
        return
    marker = root / MARKER
    temporary = marker.with_suffix(".tmp")
    temporary.write_text(json.dumps({"outcome": "passed" if passed else "failed"}) + "\n")
    temporary.replace(marker)
