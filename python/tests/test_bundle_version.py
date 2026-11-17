from __future__ import annotations

import importlib.util
import subprocess
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "bundle_version.py"
SPEC = importlib.util.spec_from_file_location("bundle_version_test", SCRIPT)
version = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(version)


@pytest.mark.parametrize("status", ["", " M swift/Interaction.swift\n", "?? swift/Input.swift\n"])
def test_build_identity_marks_modified_or_untracked_sources(monkeypatch, tmp_path, status):
    def git(command, **kwargs):
        output = "candidate\n" if command[1] == "rev-parse" else status
        return subprocess.CompletedProcess(command, 0, stdout=output, stderr="")

    monkeypatch.setattr(version.subprocess, "run", git)
    assert version.read_build_revision(tmp_path) == ("candidate-dirty" if status else "candidate")
