"""Measure what SQLite history keeps of retained capture events.

Replays every `runs/*/*/events.jsonl` under a Machine through the recorder's
increment rule and prints rows and bytes before and after. Read-only.

    python3 scripts/benchmarks/storage/replay_history.py [HOME]
"""

import json
import sys
from pathlib import Path

INCREMENTS = ("text_delta", "reasoning_delta", "item_updated")


def _is_provider_increment(event: dict) -> bool:
    if event.get("type") != "provider_output" or event.get("stream") != "notification":
        return False
    try:
        method = json.loads(event["line"]).get("method", "")
    except (ValueError, AttributeError, KeyError):
        return False
    return isinstance(method, str) and (method.endswith("elta") or method == "turn/diff/updated")


def replay(path: Path) -> tuple[int, int, int, int]:
    """Return rows and bytes in the file, then rows and bytes history keeps."""
    rows = size = kept_rows = kept_size = 0
    run: tuple[tuple, int] | None = None
    diff: tuple[str, int] | None = None

    def keep(length: int) -> None:
        nonlocal kept_rows, kept_size
        kept_rows += 1
        kept_size += length

    with path.open("rb") as lines:
        for line in lines:
            rows += 1
            size += len(line)
            try:
                event = json.loads(line)
            except ValueError:
                keep(len(line))
                continue
            if _is_provider_increment(event):
                continue
            detail = event.get("event") if event.get("type") == "conversation" else None
            kind = detail.get("type") if isinstance(detail, dict) else None
            if kind == "diff_updated":
                if diff and diff[0] != detail.get("turn_id"):
                    keep(diff[1])
                diff = (detail.get("turn_id"), len(line))
                continue
            if kind in INCREMENTS:
                data = detail.get("data") or {}
                key = (kind, detail.get("turn_id"), detail.get("item_id"), data.get("type"))
                content = len((detail.get("content") or data.get("content") or "").encode())
                if run and run[0] == key:
                    run = (key, run[1] + content)
                    continue
                if run:
                    keep(run[1])
                run = (key, len(line))
                continue
            if run:
                keep(run[1])
                run = None
            if kind == "turn_completed" and diff:
                keep(diff[1])
                diff = None
            keep(len(line))
    for held in (run, diff):
        if held:
            keep(held[1])
    return rows, size, kept_rows, kept_size


def main() -> None:
    home = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.home() / ".lf"
    files = 0
    totals = [0, 0, 0, 0]
    for path in home.glob("runs/*/*/events.jsonl"):
        files += 1
        totals = [total + part for total, part in zip(totals, replay(path))]
    rows, size, kept_rows, kept_size = totals
    print(
        json.dumps(
            {
                "files": files,
                "before": {"rows": rows, "bytes": size},
                "after": {"rows": kept_rows, "bytes": kept_size},
                "rows_kept_pct": round(100 * kept_rows / rows, 1) if rows else None,
                "bytes_kept_pct": round(100 * kept_size / size, 1) if size else None,
            }
        )
    )


if __name__ == "__main__":
    main()
