"""Run `swift test` and stop the run when one test outlives the time limit.

Swift Testing's `.timeLimit` records an issue but still waits for a test that
ignores cancellation, such as one blocked on a child process, so a hung test
held CI until the step timeout without naming itself. This reads the run's
event stream, names the overrunning test and ends every process the run started.

    swift_test_limit.py swift test --skip-build ...
"""

import json
import os
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

TIME_LIMIT_SECONDS = 300
_STREAM_FLAG = "--event-stream-output-path"


def _with_event_stream(command: list[str], directory: Path) -> tuple[list[str], Path]:
    if _STREAM_FLAG in command:
        return command, Path(command[command.index(_STREAM_FLAG) + 1])
    path = directory / "events.jsonl"
    return [*command, _STREAM_FLAG, str(path), "--event-stream-version", "0"], path


class _Stream:
    """Test functions running now, read incrementally from the event stream."""

    def __init__(self, path: Path) -> None:
        self.path = path
        self.offset = 0
        self.partial = b""
        self.names: dict[str, str] = {}
        self.running: dict[str, float] = {}

    def read(self, now: float) -> None:
        try:
            with self.path.open("rb") as stream:
                stream.seek(self.offset)
                data = stream.read()
        except FileNotFoundError:
            return
        self.offset += len(data)
        *lines, self.partial = (self.partial + data).split(b"\n")
        for line in lines:
            try:
                record = json.loads(line)
            except ValueError:
                continue
            payload = record.get("payload", {})
            if record.get("kind") == "test" and payload.get("kind") == "function":
                self.names[payload["id"]] = payload.get("displayName") or payload["name"]
            elif record.get("kind") == "event":
                test = payload.get("testID")
                if test not in self.names:
                    continue
                if payload.get("kind") == "testStarted":
                    self.running[test] = now
                elif payload.get("kind") == "testEnded":
                    self.running.pop(test, None)

    def overrun(self, now: float, limit: float) -> str | None:
        for test, started in self.running.items():
            if now - started > limit:
                return f'"{self.names[test]}" ({test})'
        return None


def _tree(root: int) -> list[int]:
    """`root` and its descendants. swift test starts its test helper, and Process
    starts each fixture child, in a new process group, so a group kill misses them."""
    table = subprocess.run(["ps", "-A", "-o", "pid=,ppid="], capture_output=True, text=True)
    children: dict[int, list[int]] = {}
    for line in table.stdout.splitlines():
        pid, ppid = (int(field) for field in line.split())
        children.setdefault(ppid, []).append(pid)
    tree = [root]
    for pid in tree:
        tree.extend(children.get(pid, []))
    return tree


def _signal(pids: list[int], sig: signal.Signals) -> None:
    for pid in pids:
        try:
            os.kill(pid, sig)
        except ProcessLookupError:
            pass


def _stop(process: subprocess.Popen) -> None:
    tree = _tree(process.pid)
    _signal(tree, signal.SIGTERM)
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        pass
    _signal(tree, signal.SIGKILL)
    process.wait()


def run(command: list[str]) -> int:
    with tempfile.TemporaryDirectory(prefix="swift-test-") as directory:
        command, path = _with_event_stream(command, Path(directory))
        stream = _Stream(path)
        process = subprocess.Popen(command)
        try:
            while process.poll() is None:
                time.sleep(1)
                now = time.monotonic()
                stream.read(now)
                test = stream.overrun(now, TIME_LIMIT_SECONDS)
                if test is not None:
                    print(
                        f"swift test: {test} exceeded the {TIME_LIMIT_SECONDS}-second "
                        "test time limit; stopping the run.",
                        file=sys.stderr,
                        flush=True,
                    )
                    _stop(process)
                    return 1
            return process.returncode
        finally:
            if process.poll() is None:
                _stop(process)


if __name__ == "__main__":
    sys.exit(run(sys.argv[1:]))
