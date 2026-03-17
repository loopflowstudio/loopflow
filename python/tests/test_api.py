"""Tests for public exports in loopflow.api."""

from __future__ import annotations

import loopflow.api as api


def test_api_exports_revoke_connection_tokens() -> None:
    assert "revoke_connection_tokens" in api.__all__
    assert api.revoke_connection_tokens is not None

