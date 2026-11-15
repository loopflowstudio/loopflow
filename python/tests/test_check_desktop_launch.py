from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

SCRIPT_PATH = Path(__file__).resolve().parents[2] / "scripts" / "check_desktop_launch.py"
spec = importlib.util.spec_from_file_location("_check_desktop_launch_script", SCRIPT_PATH)
if spec is None or spec.loader is None:
    raise RuntimeError(f"failed to load module spec from {SCRIPT_PATH}")
check_desktop_launch = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = check_desktop_launch
spec.loader.exec_module(check_desktop_launch)


def _app(tmp_path: Path, script: str) -> Path:
    executable = tmp_path / "LoopflowMac"
    executable.write_text("#!/bin/sh\nset -eu\n" + script)
    executable.chmod(0o755)
    return executable


def test_passes_when_the_app_snapshots_and_quits(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    app = _app(
        tmp_path, 'test "$1" = -ui-test-mode\nprintf png > "$LOOPFLOW_UI_TEST_SNAPSHOT_PATH"\n'
    )

    assert check_desktop_launch.main([str(app)]) == 0
    assert "quit" in capsys.readouterr().out


@pytest.mark.parametrize("script", ["exit 0\n", ': > "$LOOPFLOW_UI_TEST_SNAPSHOT_PATH"\n'])
def test_fails_when_the_app_quits_without_a_snapshot(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], script: str
) -> None:
    app = _app(tmp_path, script)

    assert check_desktop_launch.main([str(app)]) == 1
    assert "without writing its -ui-test-mode snapshot" in capsys.readouterr().err


def test_requires_a_fresh_snapshot_when_reusing_a_capture_directory(tmp_path: Path) -> None:
    app = _app(tmp_path, 'printf png > "$LOOPFLOW_UI_TEST_SNAPSHOT_PATH"\n')
    check_desktop_launch.check_launch(app, tmp_path)
    app = _app(tmp_path, "exit 0\n")

    with pytest.raises(RuntimeError, match="without writing its -ui-test-mode snapshot"):
        check_desktop_launch.check_launch(app, tmp_path)


def test_fails_with_output_when_the_app_exits_nonzero(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    app = _app(
        tmp_path,
        'printf png > "$LOOPFLOW_UI_TEST_SNAPSHOT_PATH"\necho launch failed >&2\nexit 7\n',
    )

    assert check_desktop_launch.main([str(app)]) == 1
    assert "App exited 7 from -ui-test-mode: launch failed" in capsys.readouterr().err


def test_fails_with_output_when_the_app_hangs(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    app = _app(
        tmp_path,
        'printf png > "$LOOPFLOW_UI_TEST_SNAPSHOT_PATH"\necho snapshot written\nsleep 60 &\nwait\n',
    )
    monkeypatch.setattr(check_desktop_launch, "LAUNCH_TIMEOUT", 1)

    assert check_desktop_launch.main([str(app)]) == 1
    error = capsys.readouterr().err
    assert "did not exit within 1s" in error
    assert "snapshot written" in error
    assert "Stacks:" in error


def test_names_a_missing_build(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    assert check_desktop_launch.main([str(tmp_path / "LoopflowMac")]) == 1
    assert "run `swift build` first" in capsys.readouterr().err
