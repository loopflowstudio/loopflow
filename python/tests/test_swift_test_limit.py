import json
import os
import sys
import time
from pathlib import Path

import pytest

from scripts import swift_test_limit

# Stands in for `swift test`: writes one test function's events to the stream
# it is given, then either finishes it or blocks the way a hung test does.
_FAKE_SWIFT_TEST = """
import json, sys, time
path = sys.argv[sys.argv.index("--event-stream-output-path") + 1]
test = "LoopflowTests.Suite/hangs()/Suite.swift:3:6"
def emit(stream, record):
    stream.write(json.dumps({"version": 0, **record}) + "\\n")
    stream.flush()
suite = {"id": "LoopflowTests.Suite", "kind": "suite", "name": "Suite"}
function = {"id": test, "kind": "function", "name": "hangs()", "displayName": "Builds a world"}
with open(path, "w") as stream:
    emit(stream, {"kind": "test", "payload": suite})
    emit(stream, {"kind": "test", "payload": function})
    for started in (suite["id"], test):
        emit(stream, {"kind": "event", "payload": {"kind": "testStarted", "testID": started}})
    if sys.argv[1] == "hang":
        # Like swift test's helper and Process fixtures: a descendant in its own group.
        import subprocess
        child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(600)"],
                                 start_new_session=True)
        open(sys.argv[3], "w").write(str(child.pid))
        time.sleep(600)
    if sys.argv[1] == "leak":
        sys.stdout.write("Test hangs() started.\\n")
        sys.stdout.write("SWIFT TASK CONTINUATION MISUSE: wait(_:) leaked its continuation\\n")
        sys.stdout.flush()
    emit(stream, {"kind": "event", "payload": {"kind": "testEnded", "testID": test}})
sys.exit(int(sys.argv[2]))
"""


def _command(mode: str, status: int, descendant: Path | None = None) -> list[str]:
    return [sys.executable, "-c", _FAKE_SWIFT_TEST, mode, str(status), str(descendant)]


def test_hung_test_is_named_and_stops_the_run(
    monkeypatch: pytest.MonkeyPatch, capsys, tmp_path: Path
) -> None:
    monkeypatch.setattr(swift_test_limit, "TIME_LIMIT_SECONDS", 1)
    descendant = tmp_path / "descendant.pid"
    started = time.monotonic()

    assert swift_test_limit.run(_command("hang", 0, descendant)) == 1

    assert time.monotonic() - started < 30
    with pytest.raises(ProcessLookupError):
        os.kill(int(descendant.read_text()), 0)
    assert (
        '"Builds a world" (LoopflowTests.Suite/hangs()/Suite.swift:3:6) exceeded'
        in capsys.readouterr().err
    )


@pytest.mark.parametrize("status", [0, 3])
def test_finished_run_keeps_its_exit_status(monkeypatch: pytest.MonkeyPatch, status: int) -> None:
    monkeypatch.setattr(swift_test_limit, "TIME_LIMIT_SECONDS", 1)

    assert swift_test_limit.run(_command("finish", status)) == status


def test_given_event_stream_path_is_kept(tmp_path: Path) -> None:
    path = tmp_path / "events.jsonl"

    assert (
        swift_test_limit.run([*_command("finish", 0), "--event-stream-output-path", str(path)]) == 0
    )

    records = [json.loads(line) for line in path.read_text().splitlines()]
    assert records[-1]["payload"]["kind"] == "testEnded"


def test_leaked_continuation_fails_a_passing_run_and_names_its_test(capsys) -> None:
    assert swift_test_limit.run(_command("leak", 0)) == 1

    assert "hangs() leaked a continuation" in capsys.readouterr().err
