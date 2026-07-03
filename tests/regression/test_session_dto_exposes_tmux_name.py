"""Guards against three-mirror DTO drift that previously broke Concerto's
session auto-attach: the Rust `Session` struct had `tmux_name` but
`SessionDto` dropped it on the HTTP wire, so Concerto's Swift parser
(which requires the field) silently discarded every session it fetched over
HTTP and the terminal pane stayed pointed at an empty default session.
"""

from __future__ import annotations

import time

import pytest

from scripts.lib.api_harness import ApiClient
from scripts.lib.lfd_runtime import LfdRuntime

pytestmark = pytest.mark.regression


def test_list_sessions_exposes_tmux_name(
    lfd_runtime: LfdRuntime, api_client: ApiClient
) -> None:
    wave_id = _create_wave_with_roadmap_item(lfd_runtime, api_client)
    _trigger_run(api_client, wave_id)

    session = _wait_for_session(api_client, wave_id)
    assert "tmux_name" in session, (
        f"SessionDto missing tmux_name — wire keys: {sorted(session.keys())}"
    )
    assert session["tmux_name"], "tmux_name must not be empty"
    assert session["tmux_name"].startswith("lf-"), (
        f"unexpected tmux session name: {session['tmux_name']}"
    )
    assert session["use"] in {"wave_agent", "worker", "palette"}


def test_get_session_exposes_tmux_name(
    lfd_runtime: LfdRuntime, api_client: ApiClient
) -> None:
    wave_id = _create_wave_with_roadmap_item(lfd_runtime, api_client)
    _trigger_run(api_client, wave_id)

    listed = _wait_for_session(api_client, wave_id)
    fetched = api_client.request("GET", f"/v0/sessions/{listed['id']}").json()

    assert "tmux_name" in fetched
    assert "use" in fetched
    assert fetched["tmux_name"] == listed["tmux_name"]


def _create_wave_with_roadmap_item(runtime: LfdRuntime, client: ApiClient) -> str:
    repo = str(runtime.repo_dir)
    wave_dir = runtime.repo_dir / "wave" / "designer"
    wave_dir.mkdir(parents=True, exist_ok=True)
    (wave_dir / "1-target-item.md").write_text("# Target\n")

    response = client.request(
        "POST",
        "/v0/waves",
        json={
            "repo": repo,
            "name": "designer",
            "flow": "ship-roadmap",
            "run": False,
            "status": "paused",
        },
    )
    response.raise_for_status()
    return response.json()["id"]


def _trigger_run(client: ApiClient, wave_id: str) -> None:
    response = client.request(
        "POST",
        f"/v0/waves/{wave_id}/run",
        json={"flow": "build", "roadmap_item": "1-target-item.md"},
    )
    # Either accept or precondition-failed is fine — we only need the
    # terminal session to get created as a side effect.
    assert response.status_code in {200, 412}, (
        f"unexpected status {response.status_code}: {response.text}"
    )


def _wait_for_session(
    client: ApiClient, wave_id: str, timeout_seconds: float = 10.0
) -> dict[str, object]:
    deadline = time.time() + timeout_seconds
    last_body = ""
    while time.time() < deadline:
        response = client.request("GET", "/v0/sessions")
        response.raise_for_status()
        body = response.json()
        last_body = response.text
        matches = [s for s in body.get("data", []) if s.get("wave_id") == wave_id]
        if matches:
            return matches[0]
        time.sleep(0.25)
    raise AssertionError(
        f"no session appeared for wave {wave_id} within {timeout_seconds}s; "
        f"last body: {last_body}"
    )
