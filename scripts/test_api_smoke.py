#!/usr/bin/env python3
"""Live API smoke suite for wave CRUD endpoints."""

from __future__ import annotations

import uuid
from typing import Any

from lib.api_harness import ApiHarness
from lib.lfd_runtime import LfdRuntime


def _wave_name(prefix: str) -> str:
    return f"{prefix}-{uuid.uuid4().hex[:8]}"


def _expect_wave_shape(payload: dict[str, Any]) -> None:
    ApiHarness.expect_fields(
        payload,
        ["id", "object", "name", "repo", "flow", "direction", "area", "status"],
    )
    assert payload["object"] == "wave", f"unexpected object: {payload['object']}"


def main() -> int:
    with LfdRuntime() as runtime:
        harness = ApiHarness(base_url=runtime.base_url, token=runtime.token)
        state: dict[str, str] = {}

        try:
            harness.run_scenario(
                "create_wave_happy",
                lambda: _create_wave_happy(harness, runtime, state),
            )
            harness.run_scenario(
                "create_wave_duplicate_name_error",
                lambda: _create_wave_duplicate_error(harness, runtime),
            )
            harness.run_scenario("list_waves_happy", lambda: _list_waves_happy(harness, state))
            harness.run_scenario("list_waves_auth_error", lambda: _list_waves_auth_error(harness))
            harness.run_scenario("get_wave_happy", lambda: _get_wave_happy(harness, state))
            harness.run_scenario(
                "get_wave_missing_error",
                lambda: _get_wave_missing_error(harness),
            )
            harness.run_scenario("update_wave_happy", lambda: _update_wave_happy(harness, state))
            harness.run_scenario(
                "update_wave_invalid_status_error",
                lambda: _update_wave_error(harness, state),
            )
            harness.run_scenario(
                "delete_wave_happy",
                lambda: _delete_wave_happy(harness, runtime, state),
            )
            harness.run_scenario(
                "delete_wave_missing_error",
                lambda: _delete_wave_missing_error(harness, state),
            )
        finally:
            harness.print_summary()
            harness.close()

        if harness.has_failures():
            print("lfd logs:\n" + runtime.logs())
            return 1

    return 0


def _create_wave_happy(harness: ApiHarness, runtime: LfdRuntime, state: dict[str, str]) -> None:
    name = _wave_name("api-smoke")
    response = harness.request(
        "POST",
        "/v0/waves",
        json={"repo": str(runtime.repo_dir), "name": name},
    )
    ApiHarness.expect_status(response, 200)
    payload = response.json()
    assert isinstance(payload, dict)
    _expect_wave_shape(payload)
    assert payload["name"] == name
    assert payload["repo"] == str(runtime.repo_dir)
    state["primary_wave_id"] = str(payload["id"])


def _create_wave_duplicate_error(harness: ApiHarness, runtime: LfdRuntime) -> None:
    name = _wave_name("api-dupe")
    first = harness.request(
        "POST",
        "/v0/waves",
        json={"repo": str(runtime.repo_dir), "name": name},
    )
    ApiHarness.expect_status(first, 200)

    second = harness.request(
        "POST",
        "/v0/waves",
        json={"repo": str(runtime.repo_dir), "name": name},
    )
    ApiHarness.expect_error(second, 409, message_contains="already exists")


def _list_waves_happy(harness: ApiHarness, state: dict[str, str]) -> None:
    response = harness.request("GET", "/v0/waves")
    ApiHarness.expect_status(response, 200)
    payload = response.json()
    assert isinstance(payload, dict)
    ApiHarness.expect_fields(payload, ["object", "data", "has_more"])
    assert payload["object"] == "list"

    data = payload["data"]
    assert isinstance(data, list), f"expected list payload, got {type(data)!r}"
    assert any(item.get("id") == state["primary_wave_id"] for item in data), (
        "created wave should appear in list"
    )


def _list_waves_auth_error(harness: ApiHarness) -> None:
    response = harness.request("GET", "/v0/waves", auth=False)
    ApiHarness.expect_error(response, 401, message_contains="missing token")


def _get_wave_happy(harness: ApiHarness, state: dict[str, str]) -> None:
    response = harness.request("GET", f"/v0/waves/{state['primary_wave_id']}")
    ApiHarness.expect_status(response, 200)
    payload = response.json()
    assert isinstance(payload, dict)
    _expect_wave_shape(payload)
    assert payload["id"] == state["primary_wave_id"]


def _get_wave_missing_error(harness: ApiHarness) -> None:
    response = harness.request("GET", f"/v0/waves/{_wave_name('missing-wave')}")
    ApiHarness.expect_error(response, 404, message_contains="wave not found")


def _update_wave_happy(harness: ApiHarness, state: dict[str, str]) -> None:
    payload = {
        "flow": "grind",
        "direction": ["designer"],
        "area": ["docs/"],
        "status": "paused",
    }
    response = harness.request("PATCH", f"/v0/waves/{state['primary_wave_id']}", json=payload)
    ApiHarness.expect_status(response, 200)
    body = response.json()
    assert isinstance(body, dict)
    _expect_wave_shape(body)
    assert body["flow"] == payload["flow"]
    assert body["direction"] == payload["direction"]
    assert body["area"] == payload["area"]
    assert body["status"] == payload["status"]


def _update_wave_error(harness: ApiHarness, state: dict[str, str]) -> None:
    response = harness.request(
        "PATCH",
        f"/v0/waves/{state['primary_wave_id']}",
        json={"status": "not-a-real-status"},
    )
    ApiHarness.expect_error(response, 400, message_contains="invalid status")


def _delete_wave_happy(harness: ApiHarness, runtime: LfdRuntime, state: dict[str, str]) -> None:
    name = _wave_name("api-delete")
    create_response = harness.request(
        "POST",
        "/v0/waves",
        json={"repo": str(runtime.repo_dir), "name": name},
    )
    ApiHarness.expect_status(create_response, 200)
    created = create_response.json()
    assert isinstance(created, dict)
    wave_id = str(created["id"])

    delete_response = harness.request("DELETE", f"/v0/waves/{wave_id}")
    ApiHarness.expect_status(delete_response, 200)
    deleted = delete_response.json()
    assert isinstance(deleted, dict)
    ApiHarness.expect_fields(deleted, ["id", "object", "deleted"])
    assert deleted["id"] == wave_id
    assert deleted["object"] == "wave"
    assert deleted["deleted"] is True
    state["deleted_wave_id"] = wave_id


def _delete_wave_missing_error(harness: ApiHarness, state: dict[str, str]) -> None:
    response = harness.request("DELETE", f"/v0/waves/{state['deleted_wave_id']}")
    ApiHarness.expect_error(response, 404, message_contains="wave not found")


if __name__ == "__main__":
    raise SystemExit(main())
