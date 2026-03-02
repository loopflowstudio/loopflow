"""Shared trigger CRUD scenarios for API smoke coverage."""

from __future__ import annotations

from loopflow.client import Client

from .api_harness import ApiAssertions, ApiClient
from .lfd_runtime import LfdRuntime


def add_trigger_repo_happy(client: Client, state: dict[str, str]) -> None:
    wave_id = state["trigger_wave_id"]
    result = client.add_trigger(wave_id, signal="repo", flow="integrate")

    assert result.get("id"), "trigger should have an id"
    assert result["signal"] == "repo"
    assert result.get("flow") == "integrate"
    state["trigger_id"] = result["id"]


def list_triggers_happy(raw: ApiClient, state: dict[str, str]) -> None:
    wave_id = state["trigger_wave_id"]
    response = raw.request("GET", f"/v0/waves/{wave_id}/triggers")
    ApiAssertions.expect_status(response, 200)
    payload = response.json()
    data = payload.get("data", [])

    assert len(data) == 1, f"expected 1 trigger, got {len(data)}"
    assert data[0]["signal"] == "repo"
    assert data[0]["id"] == state["trigger_id"]


def remove_trigger_happy(client: Client, state: dict[str, str]) -> None:
    wave_id = state["trigger_wave_id"]
    result = client.remove_trigger(wave_id, state["trigger_id"])
    assert result.get("deleted") is True


def list_triggers_empty(raw: ApiClient, state: dict[str, str]) -> None:
    wave_id = state["trigger_wave_id"]
    response = raw.request("GET", f"/v0/waves/{wave_id}/triggers")
    ApiAssertions.expect_status(response, 200)
    data = response.json().get("data", [])
    assert len(data) == 0, f"expected 0 triggers after removal, got {len(data)}"


def add_trigger_ci_failure_happy(client: Client, state: dict[str, str]) -> None:
    wave_id = state["trigger_wave_id"]
    result = client.add_trigger(wave_id, signal="ci_failure")

    assert result.get("id"), "ci_failure trigger should have an id"
    assert result["signal"] == "ci_failure"
    state["ci_trigger_id"] = result["id"]
