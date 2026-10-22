"""Name the Swift tests that never finished and the slowest ones, as Markdown.

Reads a Swift Testing event stream (`--event-stream-output-path`, version 0).
A step timeout leaves only "timed out" in the log; this says which tests ran.
"""

import json
import sys
from pathlib import Path

SLOWEST = 10


def report(events: Path) -> str:
    names: dict[str, str] = {}
    started: dict[str, float] = {}
    durations: dict[str, float] = {}
    first: float | None = None
    for line in events.read_text(errors="replace").splitlines():
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        payload = record.get("payload", {})
        if record.get("kind") == "test":
            names[payload.get("id", "")] = payload.get("displayName") or payload.get("name", "")
            continue
        test = payload.get("testID")
        at = payload.get("instant", {}).get("absolute")
        if test is None or at is None:
            continue
        first = at if first is None else min(first, at)
        if payload.get("kind") == "testStarted":
            started[test] = at
        elif payload.get("kind") in ("testEnded", "testSkipped") and test in started:
            durations[test] = at - started.pop(test)

    def label(test: str) -> str:
        return f"`{names.get(test) or test}` ({test.split('/')[0]})"

    lines = ["### Swift tests"]
    if started:
        lines.append("Started and never finished:")
        lines += [
            f"- {label(t)}: started {at - (first or at):.0f}s into the run"
            for t, at in sorted(started.items(), key=lambda item: item[1])
        ]
    else:
        lines.append("Every started test finished.")
    slow = sorted(durations.items(), key=lambda item: -item[1])[:SLOWEST]
    if slow:
        lines.append("Slowest finished:")
        lines += [f"- {seconds:.1f}s {label(t)}" for t, seconds in slow]
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    path = Path(sys.argv[1])
    print(report(path) if path.exists() else "### Swift tests\nNo event stream was written.\n")
