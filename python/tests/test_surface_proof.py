from __future__ import annotations

import json
import os
import signal
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
APP = """#!/usr/bin/env python3
import json
import os
import time
from pathlib import Path

root = Path(os.environ["CAPTURE_EVIDENCE"])
width = os.environ["LOOPFLOW_UI_TEST_WIDTH"]
state = os.environ["LOOPFLOW_UI_TEST_DETAIL_STATE"] or os.sys.argv[-1]

def record(event):
    with (root / "events").open("a") as output:
        output.write(json.dumps({"event": event, "pid": os.getpid()}) + "\\n")

record("start")
(root / (state + width)).touch()
other_width = "1440" if width == "900" else "900"
deadline = time.monotonic() + 5
while not (root / (state + other_width)).exists():
    if time.monotonic() >= deadline:
        raise SystemExit("the other capture never started")
    time.sleep(0.01)
if os.environ.get("CAPTURE_CASE") == "hold":
    while True:
        time.sleep(1)
if not (os.environ.get("CAPTURE_CASE") == "missing" and width == "900"):
    Path(os.environ["LOOPFLOW_UI_TEST_SNAPSHOT_PATH"]).write_text(state + width)
record("finish")
"""
MD5 = """#!/usr/bin/env python3
import hashlib
import sys
from pathlib import Path
print(hashlib.md5(Path(sys.argv[-1]).read_bytes()).hexdigest())
"""


def _fixture(root: Path, case: str = "complete") -> tuple[Path, dict[str, str]]:
    script = root / "scripts/prove_wave_surface_states.sh"
    script.parent.mkdir()
    script.write_text((ROOT / "scripts/prove_wave_surface_states.sh").read_text())
    app = root / "swift/.build/debug/LoopflowMac"
    app.parent.mkdir(parents=True)
    app.write_text(APP)
    app.chmod(0o755)
    md5 = root / "bin/md5"
    md5.parent.mkdir()
    md5.write_text(MD5)
    md5.chmod(0o755)
    evidence = root / "evidence"
    evidence.mkdir()
    return script, {
        **os.environ,
        "PATH": f"{md5.parent}:{os.environ['PATH']}",
        "CAPTURE_EVIDENCE": str(evidence),
        "CAPTURE_CASE": case,
    }


def _events(root: Path) -> list[dict]:
    path = root / "evidence/events"
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def test_captures_overlap_in_pairs_and_keep_all_eight_images(tmp_path: Path) -> None:
    script, env = _fixture(tmp_path)
    result = subprocess.run(
        ["bash", str(script)], env=env, capture_output=True, text=True, timeout=15
    )
    assert result.returncode == 0, result.stdout + result.stderr
    assert "all 8 captures" in result.stdout
    active = peak = count = 0
    for event in _events(tmp_path):
        if event["event"] == "start":
            count += 1
            active += 1
            peak = max(peak, active)
        else:
            active -= 1
    assert (count, peak, active) == (8, 2, 0)


def test_missing_capture_keeps_the_proof_red(tmp_path: Path) -> None:
    script, env = _fixture(tmp_path, "missing")
    result = subprocess.run(
        ["bash", str(script)], env=env, capture_output=True, text=True, timeout=15
    )
    assert result.returncode == 1
    assert "FAIL — no snapshot" in result.stdout
    assert "PASS — all" not in result.stdout


def test_interrupt_reaps_owned_capture_children(tmp_path: Path) -> None:
    script, env = _fixture(tmp_path, "hold")
    process = subprocess.Popen(["bash", str(script)], env=env, stdout=subprocess.DEVNULL)
    children: list[int] = []
    try:
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            children = [event["pid"] for event in _events(tmp_path) if event["event"] == "start"]
            if len(children) == 2:
                break
            time.sleep(0.01)
        assert len(children) == 2
        process.terminate()
        assert process.wait(timeout=5) == 143
        for pid in children:
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                continue
            raise AssertionError(f"owned capture child {pid} survived interruption")
        children = []
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        for pid in children:
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
