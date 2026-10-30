"""Decide which macOS CI jobs a change can reach.

Rust reaches the Swift app through the `lf --json` wire, which Linux tests hold
in lockstep with `tests/fixtures/dto/`, and through the linked `loopflow-ffi`
library, whose reads and writes Rust tests compare with their `lf` commands.
A wire change edits a fixture; a boundary change edits `loopflow-ffi` or the
`Read`/`Write` it serves. Each macOS job therefore runs only for its own inputs. CI changes,
this classifier, an unknown diff or a failed classifier run all of them.

    macos_inputs.py BASE_SHA [HEAD]   # writes one true|false output per job
"""

import os
import re
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path

SWIFT_TEST = "swift-test"
MACOS_PROCESS_TEST = "macos-process-test"
LOOPFLOW_UI_TEST = "loopflow-ui-test"
JOBS = (SWIFT_TEST, MACOS_PROCESS_TEST, LOOPFLOW_UI_TEST)

_EVERY_JOB = (".github/workflows/", "scripts/macos_inputs.py")
# The app sources, project.yml and GhosttyKit patches all live under swift/.
_UI_INPUTS = ("swift/", "scripts/build_xcode_ci.py", "scripts/swift_cache.py")
_SWIFT_TEST_INPUTS = (
    "swift/",
    "tests/fixtures/dto/",
    "scripts/swift_",
    "scripts/test_desktop.sh",
    "scripts/test_network.py",
    "scripts/desktop_performance.py",
    "scripts/desktop-",
    "scripts/check_swift_multiplatform_boundaries.py",
    "rust/loopflow-ffi/",
    "rust/loopflow/src/lf/commands/read.rs",
    "rust/loopflow/src/lf/commands/write.rs",
    # swift-test runs this check, and pytest loads any conftest beside it.
    "python/tests/test_desktop_performance.py",
    "python/tests/conftest.py",
)
_PROCESS_INPUTS = (
    "rust/loopflow/src/store/copy.rs",
    "rust/loopflow/src/journal/os_process.rs",
    "rust/loopflow/src/lf/commands/install",
)
_MACOS_CFG = re.compile(r'target_os\s*=\s*"macos"')


def _reaches_process_test(path: str, read: Callable[[str], str | None]) -> bool:
    if path.startswith(_PROCESS_INPUTS):
        return True
    if not path.endswith(".rs"):
        return False
    text = read(path)
    return text is not None and _MACOS_CFG.search(text) is not None


def affected_jobs(paths: list[str] | None, read: Callable[[str], str | None]) -> set[str]:
    """Name the macOS jobs that `paths` reach; `read` returns a changed file's text."""
    if not paths or any(path.startswith(_EVERY_JOB) for path in paths):
        return set(JOBS)
    jobs = set()
    if any(path.startswith(_UI_INPUTS) for path in paths):
        jobs.add(LOOPFLOW_UI_TEST)
    if any(path.startswith(_SWIFT_TEST_INPUTS) for path in paths):
        jobs.add(SWIFT_TEST)
    if any(_reaches_process_test(path, read) for path in paths):
        jobs.add(MACOS_PROCESS_TEST)
    return jobs


def _changed(base: str, head: str) -> list[str] | None:
    try:
        subprocess.run(["git", "fetch", "--no-tags", "--depth=1", "origin", base], check=True)
        result = subprocess.run(
            ["git", "diff", "--name-only", base, head], capture_output=True, text=True, check=True
        )
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"Changed paths unavailable ({error}); running the macOS jobs.", file=sys.stderr)
        return None
    return result.stdout.splitlines()


def _reader(base: str, head: str) -> Callable[[str], str | None]:
    # A deleted file's last text still says whether it was macOS code.
    def read(path: str) -> str | None:
        for rev in (head, base):
            result = subprocess.run(
                ["git", "show", f"{rev}:{path}"], capture_output=True, text=True
            )
            if result.returncode == 0:
                return result.stdout
        return None

    return read


def main() -> None:
    # Empty on events without a base, such as a manual run.
    base = sys.argv[1]
    head = sys.argv[2] if len(sys.argv) > 2 else "HEAD"
    paths = _changed(base, head) if base else None
    jobs = affected_jobs(paths, _reader(base, head))
    summary = (
        f"Running macOS jobs: {', '.join(sorted(jobs))}."
        if jobs
        else f"No macOS job reaches the {len(paths or [])} changed paths; skipping them."
    )
    print(summary)
    with Path(os.environ["GITHUB_OUTPUT"]).open("a") as output:
        for job in JOBS:
            output.write(f"{job}={'true' if job in jobs else 'false'}\n")
        output.write(f"any={'true' if jobs else 'false'}\n")
    with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as output:
        output.write(summary + "\n")


if __name__ == "__main__":
    main()
