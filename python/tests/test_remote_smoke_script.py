"""Tests for scripts/test_remote_smoke.py helper logic."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parents[2] / "scripts"
SCRIPT_PATH = SCRIPTS_DIR / "test_remote_smoke.py"
MODULE_NAME = "_test_remote_smoke_script_module"
spec = importlib.util.spec_from_file_location(MODULE_NAME, SCRIPT_PATH)
if spec is None or spec.loader is None:
    raise RuntimeError(f"failed to load module spec from {SCRIPT_PATH}")
remote_smoke = importlib.util.module_from_spec(spec)
sys.path.insert(0, str(SCRIPTS_DIR))
try:
    sys.modules[MODULE_NAME] = remote_smoke
    spec.loader.exec_module(remote_smoke)
finally:
    sys.path.pop(0)


def test_resolve_tls_rejects_insecure_with_ca_cert() -> None:
    args = SimpleNamespace(insecure=True, ca_cert="/tmp/ca.pem")
    with pytest.raises(ValueError, match="either --insecure or --ca-cert"):
        remote_smoke._resolve_tls(args)
