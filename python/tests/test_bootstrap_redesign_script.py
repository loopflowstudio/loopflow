"""Tests for scripts/bootstrap-redesign.py."""

from __future__ import annotations

import importlib.util
import io
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

SCRIPTS_DIR = Path(__file__).resolve().parents[2] / "scripts"
SCRIPT_PATH = SCRIPTS_DIR / "bootstrap-redesign.py"
MODULE_NAME = "_bootstrap_redesign_script_module"
spec = importlib.util.spec_from_file_location(MODULE_NAME, SCRIPT_PATH)
if spec is None or spec.loader is None:
    raise RuntimeError(f"failed to load module spec from {SCRIPT_PATH}")
bootstrap_redesign = importlib.util.module_from_spec(spec)
sys.path.insert(0, str(SCRIPTS_DIR))
try:
    sys.modules[MODULE_NAME] = bootstrap_redesign
    spec.loader.exec_module(bootstrap_redesign)
finally:
    sys.path.pop(0)


def test_resolve_repo_root_prefers_git_common_dir(monkeypatch, tmp_path) -> None:
    repo_root = tmp_path / "loopflow"
    common_dir = repo_root / ".git"
    common_dir.mkdir(parents=True)
    monkeypatch.setattr(bootstrap_redesign, "SCRIPT_ROOT", tmp_path / "loopflow.redesign")
    monkeypatch.setattr(
        bootstrap_redesign.subprocess,
        "run",
        lambda *args, **kwargs: SimpleNamespace(stdout=str(common_dir)),
    )

    assert bootstrap_redesign._resolve_repo_root() == repo_root


def test_resolve_repo_root_falls_back_to_script_root_on_git_error(monkeypatch, tmp_path) -> None:
    monkeypatch.setattr(bootstrap_redesign, "SCRIPT_ROOT", tmp_path / "loopflow.redesign")

    def raise_called_process_error(*args, **kwargs):
        raise subprocess.CalledProcessError(1, args[0])

    monkeypatch.setattr(bootstrap_redesign.subprocess, "run", raise_called_process_error)

    assert bootstrap_redesign._resolve_repo_root() == bootstrap_redesign.SCRIPT_ROOT


def test_main_creates_missing_waves_and_prints_redesign_summary(monkeypatch) -> None:
    created: list[str] = []
    expected_repo = str(bootstrap_redesign.REPO_ROOT)
    redesign_area = bootstrap_redesign.REDESIGN_AREA
    waves: dict[str, SimpleNamespace] = {
        "agent-embedding": SimpleNamespace(
            id="wave-agent-embedding",
            primary_flow="build",
            area=[expected_repo],
            status="idle",
        )
    }
    updated: list[tuple[str, str, list[str]]] = []

    def fake_wave(name: str):
        return waves.get(name)

    def fake_create_wave(name: str, repo: str):
        assert repo == expected_repo
        wave = SimpleNamespace(
            id=f"wave-{name}",
            primary_flow=bootstrap_redesign.REDESIGN_FLOW if name == "redesign" else "build",
            area=redesign_area if name == "redesign" else [repo],
            status="idle",
        )
        created.append(name)
        waves[name] = wave
        return wave

    def fake_update_wave(name: str, flow: str, area: list[str]):
        assert name == "redesign"
        wave = waves[name]
        wave.primary_flow = flow
        wave.area = area
        updated.append((name, flow, area))
        return wave

    monkeypatch.setattr(bootstrap_redesign.loopflow, "wave", fake_wave)
    monkeypatch.setattr(bootstrap_redesign.loopflow, "create_wave", fake_create_wave)
    monkeypatch.setattr(bootstrap_redesign.loopflow, "update_wave", fake_update_wave)

    stdout = io.StringIO()
    monkeypatch.setattr(sys, "stdout", stdout)

    exit_code = bootstrap_redesign.main()

    assert exit_code == 0
    assert created == ["chord-model", "redesign"]
    assert updated == [("redesign", bootstrap_redesign.REDESIGN_FLOW, redesign_area)]
    output = stdout.getvalue()
    assert "agent-embedding: exists (wave-agent-embedding)" in output
    assert "redesign: created (wave-redesign)" in output
    assert f"flow: {bootstrap_redesign.REDESIGN_FLOW}" in output
    assert f"area: {', '.join(redesign_area)}" in output
