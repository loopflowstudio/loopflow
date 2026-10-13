import os
import signal
import subprocess
import sys
import time
from pathlib import Path

import pytest

from scripts import build_xcode_ci
from scripts.build_xcode_ci import _run_builds, _stop_xcode_compilers


@pytest.mark.skipif(sys.platform != "darwin", reason="Xcode entrypoint uses macOS time")
@pytest.mark.parametrize("status", [0, 11])
def test_exact_cache_hit_runs_only_ordinary_xcode(tmp_path: Path, status: int) -> None:
    (tmp_path / "swift").mkdir()
    xcode = tmp_path / "xcodebuild"
    xcode.write_text(f'#!/bin/sh\n[ "$1" = build-for-testing ] || exit 99\nexit {status}\n')
    xcode.chmod(0o755)
    result = subprocess.run(
        [
            sys.executable,
            "-c",
            "from scripts import build_xcode_ci; from pathlib import Path; "
            f"build_xcode_ci.REPO_ROOT=Path({str(tmp_path)!r}); "
            "build_xcode_ci.main()",
        ],
        env=dict(os.environ, PATH=str(tmp_path), XCODE_CACHE_HIT="true"),
        capture_output=True,
    )
    # No preparation script exists here; resolving settings also fails.
    assert result.returncode == status, result.stderr.decode()


@pytest.fixture(autouse=True)
def _checkout(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(build_xcode_ci, "REPO_ROOT", tmp_path)


def _command(tmp_path: Path, name: str, status: int, descendant: bool = False) -> list[str]:
    script = tmp_path / f"{name}.py"
    peer = tmp_path / ("xcode.ready" if name == "control" else "control.ready")
    alive = tmp_path / f"{name}.alive"
    child = (
        "import signal,time; from pathlib import Path; "
        "signal.signal(signal.SIGTERM,signal.SIG_IGN); "
        f"Path({str(alive)!r}).touch(); time.sleep(60)"
    )
    script.write_text(
        "import os,signal,subprocess,sys,time\n"
        "from pathlib import Path\n"
        + (
            f"p=subprocess.Popen([sys.executable,'-c',{child!r}])\n"
            f"while not Path({str(alive)!r}).exists(): time.sleep(.01)\n"
            f"pidfile=Path({str(tmp_path / (name + '.pid'))!r})\n"
            "pending=pidfile.with_suffix('.pending')\n"
            "pending.write_text(str(p.pid)); pending.rename(pidfile)\n"
            if descendant
            else ""
        )
        + f"Path({str(tmp_path / (name + '.ready'))!r}).touch()\n"
        + f"peer=Path({str(peer)!r})\n"
        + "deadline=time.monotonic()+5\n"
        + "while not peer.exists() and time.monotonic()<deadline: time.sleep(.01)\n"
        + "assert peer.exists(), 'other build never started'\n"
        + "time.sleep(.1)\n"
        + ("time.sleep(60)\n" if status == -1 else f"sys.exit({status})\n")
    )
    return [sys.executable, str(script)]


def _assert_descendants_stopped(tmp_path: Path) -> None:
    for path in tmp_path.glob("*.pid"):
        pid = int(path.read_text())
        state = subprocess.run(
            ["ps", "-o", "stat=", "-p", str(pid)], capture_output=True, text=True
        ).stdout.strip()
        assert not state or state.startswith("Z"), f"owned descendant {pid} still running"


@pytest.mark.parametrize("statuses", [(0, 0), (7, -1), (-1, 9), (7, 9)])
def test_builds_overlap_and_require_both_success(tmp_path: Path, statuses: tuple[int, int]) -> None:
    control, xcode = statuses
    result = _run_builds(
        _command(tmp_path, "control", control, True),
        _command(tmp_path, "xcode", xcode, True),
        tmp_path,
        dict(os.environ),
    )
    assert result == 0 if statuses == (0, 0) else result in {control, xcode} - {-1, 0}
    _assert_descendants_stopped(tmp_path)


def test_launch_failure_cleans_started_build(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    popen = subprocess.Popen

    def launch(command: list[str], **kwargs: object) -> subprocess.Popen:
        if command[0].endswith("missing-command"):
            deadline = time.monotonic() + 5
            while not (tmp_path / "control.ready").exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            assert (tmp_path / "control.ready").exists()
        return popen(command, **kwargs)

    monkeypatch.setattr(subprocess, "Popen", launch)
    result = _run_builds(
        _command(tmp_path, "control", -1, True),
        [str(tmp_path / "missing-command")],
        tmp_path,
        dict(os.environ),
    )
    assert result == 1
    _assert_descendants_stopped(tmp_path)


@pytest.mark.parametrize("signum", [signal.SIGTERM, signal.SIGINT])
def test_cancellation_cleans_both_groups(tmp_path: Path, signum: int) -> None:
    control = _command(tmp_path, "control", -1, True)
    xcode = _command(tmp_path, "xcode", -1, True)
    driver = [
        sys.executable,
        "-c",
        "from scripts import build_xcode_ci; import os,sys; "
        f"from pathlib import Path; build_xcode_ci.REPO_ROOT=Path({str(tmp_path)!r}); "
        f"sys.exit(build_xcode_ci._run_builds({control!r}, {xcode!r}, "
        f"Path({str(tmp_path)!r}), dict(os.environ)))",
    ]
    proc = subprocess.Popen(driver)
    try:
        deadline = time.monotonic() + 5
        while len(list(tmp_path.glob("*.pid"))) < 2 and time.monotonic() < deadline:
            time.sleep(0.01)
        assert len(list(tmp_path.glob("*.pid"))) == 2
        proc.send_signal(signum)
        assert proc.wait(timeout=5) == 128 + signum
        _assert_descendants_stopped(tmp_path)
    finally:
        if proc.poll() is None:
            proc.kill()
        proc.wait()


def test_compiler_cleanup_preserves_other_outputs_and_preexisting_work(tmp_path: Path) -> None:
    source = tmp_path / "compiler.c"
    source.write_text(
        "#include <signal.h>\n#include <stdio.h>\n#include <unistd.h>\n"
        'int main(void) { signal(SIGTERM, SIG_IGN); puts("ready"); fflush(stdout); '
        "for (;;) pause(); }\n"
    )
    compiler = tmp_path / "swift-frontend"
    subprocess.run(["cc", str(source), "-o", str(compiler)], check=True)
    output = tmp_path / "swift/.build/xcode-derived-data/Build/module.pcm"
    processes = [
        subprocess.Popen([str(compiler), "-o", str(path)], stdout=subprocess.PIPE, text=True)
        for path in (output, output, tmp_path / "other-build/module.pcm")
    ]
    try:
        for proc in processes:
            assert proc.stdout.readline().strip() == "ready"
        _stop_xcode_compilers({processes[0].pid})
        assert processes[1].wait(timeout=2) == -signal.SIGKILL
        assert processes[0].poll() is None
        assert processes[2].poll() is None
    finally:
        for proc in processes:
            if proc.poll() is None:
                proc.kill()
            proc.wait()
