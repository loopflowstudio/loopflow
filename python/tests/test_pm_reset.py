"""Tests for scripts/pm_reset.py."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
from pathlib import Path

import httpx
import pytest
import yaml

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "pm_reset.py"


def _load_script():
    spec = importlib.util.spec_from_file_location("pm_reset", SCRIPT)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


pm_reset = _load_script()


def _write_wave(path: Path, name: str, **pm_fields: str) -> Path:
    wave_dir = path / "wave" / name
    wave_dir.mkdir(parents=True)
    yaml_path = wave_dir / f"{name}.yaml"
    body: dict[str, object] = {"flow": "build"}
    if pm_fields:
        body["pm"] = dict(pm_fields)
    yaml_path.write_text(yaml.safe_dump(body, sort_keys=False))
    return yaml_path


def test_find_wave_projects_returns_configured_waves(tmp_path):
    _write_wave(tmp_path, "alpha", provider="asana", asana_project="111")
    _write_wave(tmp_path, "beta", provider="asana", asana_project="222")
    _write_wave(tmp_path, "gamma")  # no pm block

    found = pm_reset.find_wave_projects(tmp_path, "asana")
    waves = {wave: pid for _, wave, pid in found}
    assert waves == {"alpha": "111", "beta": "222"}


def test_find_wave_projects_ignores_malformed_yaml(tmp_path):
    _write_wave(tmp_path, "alpha", provider="asana", asana_project="111")
    bad = tmp_path / "wave" / "bad"
    bad.mkdir(parents=True)
    (bad / "bad.yaml").write_text(":: not valid yaml ::\n  - : -")

    found = pm_reset.find_wave_projects(tmp_path, "asana")
    assert [wave for _, wave, _ in found] == ["alpha"]


def test_clear_project_id_removes_field_preserves_other_pm(tmp_path):
    yaml_path = _write_wave(
        tmp_path, "alpha", provider="asana", asana_project="111", workspace="ws1"
    )

    pm_reset.clear_project_id(yaml_path, "asana")

    data = yaml.safe_load(yaml_path.read_text())
    assert "asana_project" not in data["pm"]
    assert data["pm"]["workspace"] == "ws1"


def test_clear_project_id_drops_pm_block_when_empty(tmp_path):
    yaml_path = _write_wave(tmp_path, "alpha", asana_project="111")

    pm_reset.clear_project_id(yaml_path, "asana")

    data = yaml.safe_load(yaml_path.read_text())
    assert "pm" not in data
    assert data["flow"] == "build"


def test_delete_asana_project_uses_bearer_auth(tmp_path, monkeypatch):
    captured: dict[str, object] = {}

    def fake_delete(url, headers=None, timeout=None):  # noqa: ARG001
        captured["url"] = url
        captured["headers"] = headers
        return httpx.Response(200, json={"data": {}})

    monkeypatch.setattr(pm_reset.httpx, "delete", fake_delete)

    pm_reset.delete_asana_project("tok-xyz", "111", api_base="https://api.example.com")

    assert captured["url"] == "https://api.example.com/projects/111"
    assert captured["headers"] == {"Authorization": "Bearer tok-xyz"}


def test_delete_asana_project_swallows_404(monkeypatch):
    def fake_delete(url, headers=None, timeout=None):  # noqa: ARG001
        return httpx.Response(404)

    monkeypatch.setattr(pm_reset.httpx, "delete", fake_delete)

    # Should not raise.
    pm_reset.delete_asana_project("tok", "missing", api_base="https://api.example.com")


def test_delete_asana_project_raises_on_other_error(monkeypatch):
    def fake_delete(url, headers=None, timeout=None):  # noqa: ARG001
        return httpx.Response(500, text="boom")

    monkeypatch.setattr(pm_reset.httpx, "delete", fake_delete)

    with pytest.raises(RuntimeError, match="Asana delete failed"):
        pm_reset.delete_asana_project("tok", "111", api_base="https://api.example.com")


def test_dry_run_does_not_delete_or_mutate_yaml(tmp_path, monkeypatch):
    _write_wave(tmp_path, "alpha", provider="asana", asana_project="111")

    def explode(*args, **kwargs):  # noqa: ARG001
        raise AssertionError("HTTP call in dry-run")

    monkeypatch.setattr(pm_reset.httpx, "delete", explode)

    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--provider",
            "asana",
            "--dry-run",
            "--repo",
            str(tmp_path),
            "--skip-init",
            "--skip-push-diff",
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr

    data = yaml.safe_load((tmp_path / "wave/alpha/alpha.yaml").read_text())
    assert data["pm"]["asana_project"] == "111"
