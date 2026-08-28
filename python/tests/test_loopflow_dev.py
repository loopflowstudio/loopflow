from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/loopflow-dev.py"
MODULE_NAME = "_loopflow_dev_script"
SPEC = importlib.util.spec_from_file_location(MODULE_NAME, SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"failed to load module spec from {SCRIPT}")
loopflow_dev = importlib.util.module_from_spec(SPEC)
sys.modules[MODULE_NAME] = loopflow_dev
SPEC.loader.exec_module(loopflow_dev)


def test_app_environment_does_not_override_machine_install_authority(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("LF_CONTROL_HOME", "/tmp/selected-home")
    monkeypatch.setenv("LF_CONTROL_DB_PATH", "/tmp/selected-home/loopflow.db")
    monkeypatch.setenv("LF_HOME", "/tmp/ambient-home")
    monkeypatch.setenv("LF_DB_PATH", "/tmp/ambient-home/loopflow.db")

    environment = loopflow_dev._app_environment(Path("/tmp/repo"))

    assert environment == {"LOOPFLOW_DEV_WAVE_REPO": "/tmp/repo"}
