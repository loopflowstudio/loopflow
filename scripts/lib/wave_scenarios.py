#!/usr/bin/env python3
"""Shared wave CRUD scenarios for API smoke coverage."""

from __future__ import annotations

import uuid
from typing import Any

from loopflow.client import Client
from loopflow.models import Wave

from .api_harness import ApiAssertions, ApiClient
from .lfd_runtime import LfdRuntime


def create_wave_happy(client: Client, runtime: LfdRuntime, state: dict[str, str]) -> None:
    wave = client.create_wave(name=_wave_name("api-smoke"), repo=str(runtime.repo_dir))
    _expect_wave(wave)
    state["primary_wave_id"] = wave.id


def create_wave_duplicate_name_error(raw: ApiClient, runtime: LfdRuntime) -> None:
    name = _wave_name("api-dupe")
    _create_wave_raw(raw, runtime, name)

    second = raw.request(
        "POST",
        "/v0/waves",
        json={"repo": str(runtime.repo_dir), "name": name},
    )
    ApiAssertions.expect_error(second, 409, message_contains="already exists")


def list_waves_happy(client: Client, state: dict[str, str]) -> None:
    waves = client.waves()
    assert any(wave.id == state["primary_wave_id"] for wave in waves), (
        "created wave should appear in list"
    )


def list_waves_auth_error(raw: ApiClient) -> None:
    response = raw.request("GET", "/v0/waves", auth=False)
    ApiAssertions.expect_error(response, 401, message_contains="missing token")


def get_wave_happy(client: Client, state: dict[str, str]) -> None:
    wave = client.wave(state["primary_wave_id"])
    assert wave is not None, "created wave should be retrievable"
    _expect_wave(wave)
    assert wave.id == state["primary_wave_id"]


def get_wave_missing_error(raw: ApiClient) -> None:
    response = raw.request("GET", f"/v0/waves/{_wave_name('missing-wave')}")
    ApiAssertions.expect_error(response, 404, message_contains="wave not found")


def update_wave_happy(client: Client, state: dict[str, str]) -> None:
    wave = client.update_wave(
        state["primary_wave_id"],
        flow="grind",
        direction=["ux"],
        area=["docs/"],
        status="paused",
    )
    _expect_wave(wave)
    assert wave.flow == "grind"
    assert wave.direction == ["ux"]
    assert wave.area == ["docs/"]
    assert wave.status == "paused"


def update_wave_invalid_status_error(raw: ApiClient, state: dict[str, str]) -> None:
    response = raw.request(
        "PATCH",
        f"/v0/waves/{state['primary_wave_id']}",
        json={"status": "not-a-real-status"},
    )
    ApiAssertions.expect_error(response, 400, message_contains="invalid status")


def delete_wave_happy(client: Client, runtime: LfdRuntime, state: dict[str, str]) -> None:
    wave = client.create_wave(name=_wave_name("api-delete"), repo=str(runtime.repo_dir))
    client.delete_wave(wave.id)
    deleted = client.wave(wave.id)
    assert deleted is None, "deleted wave should no longer be retrievable"
    state["deleted_wave_id"] = wave.id


def delete_wave_missing_error(raw: ApiClient, state: dict[str, str]) -> None:
    response = raw.request("DELETE", f"/v0/waves/{state['deleted_wave_id']}")
    ApiAssertions.expect_error(response, 404, message_contains="wave not found")


def _wave_name(prefix: str) -> str:
    return f"{prefix}-{uuid.uuid4().hex[:8]}"


def _expect_wave(wave: Wave) -> None:
    assert isinstance(wave, Wave)
    assert wave.id, "wave.id should be present"
    assert wave.name, "wave.name should be present"
    assert wave.repo, "wave.repo should be present"


def _create_wave_raw(raw: ApiClient, runtime: LfdRuntime, name: str) -> dict[str, Any]:
    response = raw.request(
        "POST",
        "/v0/waves",
        json={"repo": str(runtime.repo_dir), "name": name},
    )
    ApiAssertions.expect_status(response, 200)
    payload = ApiAssertions.expect_json_object(response)
    ApiAssertions.expect_fields(
        payload,
        ["id", "object", "name", "repo", "flow", "direction", "area", "status"],
    )
    assert payload["object"] == "wave", f"unexpected object: {payload['object']}"
    assert payload["name"] == name
    assert payload["repo"] == str(runtime.repo_dir)
    return payload
