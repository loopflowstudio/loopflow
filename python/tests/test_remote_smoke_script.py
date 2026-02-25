"""Tests for scripts/test_remote_smoke.py helper logic."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from types import SimpleNamespace

import httpx
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


class _FakeApi:
    def __init__(self, response: httpx.Response) -> None:
        self._response = response

    def request(self, method: str, path: str) -> httpx.Response:  # noqa: ARG002
        return self._response


def test_resolve_repo_path_prefers_explicit_override() -> None:
    path = remote_smoke._resolve_repo_path(_FakeApi(httpx.Response(200, json={})), "/tmp/explicit")
    assert path == "/tmp/explicit"


def test_resolve_repo_path_requires_repo_on_fresh_host() -> None:
    payload = {"object": "list", "data": []}
    api = _FakeApi(httpx.Response(200, json=payload))
    with pytest.raises(AssertionError, match="pass --repo"):
        remote_smoke._resolve_repo_path(api, None)


def test_resolve_repo_path_uses_first_valid_entry() -> None:
    payload = {"object": "list", "data": [{"path": ""}, {"path": "/srv/loopflow"}]}
    api = _FakeApi(httpx.Response(200, json=payload))
    assert remote_smoke._resolve_repo_path(api, None) == "/srv/loopflow"


def test_resolve_tls_rejects_insecure_with_ca_cert() -> None:
    args = SimpleNamespace(insecure=True, ca_cert="/tmp/ca.pem")
    with pytest.raises(ValueError, match="either --insecure or --ca-cert"):
        remote_smoke._resolve_tls(args)
