"""Decide whether a change can affect the Desktop CI jobs.

swift-test builds and drives the source `lf`, decodes CLI JSON fixtures and
runs scripts, so Rust, fixtures, scripts and CI changes all reach Desktop.
Only the paths below cannot; any other path, or an unknown diff, runs Desktop.

    desktop_inputs.py BASE_SHA    # writes desktop=true|false to $GITHUB_OUTPUT
"""

import os
import subprocess
import sys
from pathlib import Path

_UNAFFECTED = ("docs/", "website/", "scratch/", ".lf/wave/", "python/loopflow/", "python/tests/")
# swift-test runs this check, and pytest loads any conftest beside it.
_STILL_AFFECTED = ("python/tests/test_desktop_performance.py", "python/tests/conftest.py")


def _affects(path: str) -> bool:
    if path in _STILL_AFFECTED:
        return True
    if "/" not in path and path.endswith(".md"):
        return False
    return not path.startswith(_UNAFFECTED)


def affects_desktop(paths: list[str]) -> bool:
    return not paths or any(_affects(path) for path in paths)


def _changed(base: str) -> list[str] | None:
    try:
        subprocess.run(["git", "fetch", "--no-tags", "--depth=1", "origin", base], check=True)
        result = subprocess.run(
            ["git", "diff", "--name-only", base, "HEAD"], capture_output=True, text=True, check=True
        )
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"Changed paths unavailable ({error}); running Desktop.", file=sys.stderr)
        return None
    return result.stdout.splitlines()


def main() -> None:
    # Empty on events without a base, such as a manual run.
    base = sys.argv[1]
    paths = _changed(base) if base else None
    desktop = paths is None or affects_desktop(paths)
    summary = (
        "Desktop inputs changed; running the Desktop jobs."
        if desktop
        else f"Only {len(paths)} non-Desktop paths changed; skipping the Desktop jobs."
    )
    print(summary)
    with Path(os.environ["GITHUB_OUTPUT"]).open("a") as output:
        output.write(f"desktop={'true' if desktop else 'false'}\n")
    with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as output:
        output.write(summary + "\n")


if __name__ == "__main__":
    main()
