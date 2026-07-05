"""The gatekeeper's read surface stays up on a fresh host.

The collapse removed lfd's mutation routes — wave creation is authoring
`wave/<name>/GOAL.md`, not a POST. This pins the surviving read surface
against a live daemon, and pins that the dead create route is really gone
(405, not a silent recreation).
"""

from __future__ import annotations

import pytest

pytestmark = pytest.mark.regression


def test_reads_serve_on_fresh_daemon(api_client) -> None:
    health = api_client.request("GET", "/health")
    assert health.status_code == 200

    waves = api_client.request("GET", "/v0/waves")
    assert waves.status_code == 200
    assert waves.json()["data"] == []

    # The mutation door is closed: POST /v0/waves no longer exists.
    created = api_client.request("POST", "/v0/waves", json={"repo": "/tmp/x", "name": "ghost"})
    assert created.status_code == 405
