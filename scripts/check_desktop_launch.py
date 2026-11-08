#!/usr/bin/env python3
"""Launch Desktop in -ui-test-mode and fail unless it snapshots and quits.

Usage:
    uv run python scripts/check_desktop_launch.py [executable]

Defaults to the debug `LoopflowMac` that `swift build` leaves in
`swift/.build/debug`. The release package runs `check_launch` against its
signed copy.
"""

from __future__ import annotations

import os
import signal
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
DEBUG_EXECUTABLE = REPO_ROOT / "swift" / ".build" / "debug" / "LoopflowMac"
LAUNCH_TIMEOUT = 30


def check_launch(executable: Path, work_dir: Path) -> None:
    """Raise unless the app writes its snapshot into `work_dir` and exits 0."""
    if not executable.is_file():
        raise RuntimeError(f"No Desktop executable at {executable}; run `swift build` first")
    snapshot = work_dir / "snapshot.png"
    _launch(
        [str(executable), "-ui-test-mode", "empty-workspaces"],
        env={
            **os.environ,
            "LOOPFLOW_UI_TEST_SNAPSHOT_PATH": str(snapshot),
            "LOOPFLOW_UI_TEST_DELAY": "0.2",
            "LOOPFLOW_UI_TEST_WIDTH": "900",
            "LOOPFLOW_UI_TEST_HEIGHT": "600",
        },
        sample_path=work_dir / "hang-sample.txt",
    )
    if not snapshot.is_file() or snapshot.stat().st_size == 0:
        raise RuntimeError("App quit without writing its -ui-test-mode snapshot")


def _launch(cmd: list[str], env: dict[str, str], sample_path: Path) -> None:
    """Run the app; raise with its output if it fails, plus a stack sample if it hangs."""
    print(f"$ {' '.join(cmd)}", flush=True)
    process = subprocess.Popen(
        cmd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        start_new_session=True,
    )
    try:
        output, _ = process.communicate(timeout=LAUNCH_TIMEOUT)
        if process.returncode != 0:
            diagnostic = output.strip()[-2000:]
            detail = f": {diagnostic}" if diagnostic else ""
            raise RuntimeError(f"App exited {process.returncode} from -ui-test-mode{detail}")
        return
    except subprocess.TimeoutExpired:
        # macOS `sample`; without it the error still carries the app's output.
        try:
            subprocess.run(
                ["sample", str(process.pid), "2", "-mayDie", "-file", str(sample_path)],
                capture_output=True,
                timeout=30,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired):
            pass
        # Children the app started hold the output pipe; end the whole group.
        os.killpg(process.pid, signal.SIGKILL)
        output, _ = process.communicate()
    sample = sample_path.read_text(errors="replace") if sample_path.is_file() else ""
    call_graph = sample.partition("Call graph:")[2]
    stacks = "\n".join(call_graph.splitlines()[:120]) or "(no stack sample)"
    raise RuntimeError(
        f"App did not exit within {LAUNCH_TIMEOUT}s of "
        f"-ui-test-mode.\nOutput:\n{output.strip()[-2000:] or '(none)'}\n"
        f"Stacks:\n{stacks}"
    )


def main(argv: list[str]) -> int:
    executable = Path(argv[0]) if argv else DEBUG_EXECUTABLE
    try:
        with tempfile.TemporaryDirectory(prefix="loopflow-launch-check-") as temp:
            check_launch(executable, Path(temp))
    except RuntimeError as error:
        print(error, file=sys.stderr)
        return 1
    print("Desktop launched, wrote its snapshot and quit", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
