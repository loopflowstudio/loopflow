#!/usr/bin/env python3
"""Live API smoke suite for wave CRUD endpoints."""

from __future__ import annotations

import uuid
from functools import partial
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
        with ApiHarness(base_url=runtime.base_url, token=runtime.token) as harness:
            state: dict[str, str] = {}
            scenarios = [
                ("create_wave_happy", partial(_create_wave_happy, harness, runtime, state)),
                (
                    "create_wave_duplicate_name_error",
                    partial(_create_wave_duplicate_error, harness, runtime),
                ),
                ("list_waves_happy", partial(_list_waves_happy, harness, state)),
                ("list_waves_auth_error", partial(_list_waves_auth_error, harness)),
                ("get_wave_happy", partial(_get_wave_happy, harness, state)),
                ("get_wave_missing_error", partial(_get_wave_missing_error, harness)),
                ("update_wave_happy", partial(_update_wave_happy, harness, state)),
                ("update_wave_invalid_status_error", partial(_update_wave_error, harness, state)),
                ("delete_wave_happy", partial(_delete_wave_happy, harness, runtime, state)),
                ("delete_wave_missing_error", partial(_delete_wave_missing_error, harness, state)),
            ]

            for name, check in scenarios:
                harness.run_scenario(name, check)
            harness.print_summary()

        if harness.has_failures():
            print("lfd logs:\n" + runtime.logs())
            return 1

    return 0


def _create_wave_happy(harness: ApiHarness, runtime: LfdRuntime, state: dict[str, str]) -> None:
    payload = _create_wave(harness, runtime, _wave_name("api-smoke"))
    state["primary_wave_id"] = str(payload["id"])


def _create_wave_duplicate_error(harness: ApiHarness, runtime: LfdRuntime) -> None:
    name = _wave_name("api-dupe")
    _create_wave(harness, runtime, name)

    second = harness.request(
        "POST",
        "/v0/waves",
        json={"repo": str(runtime.repo_dir), "name": name},
    )
    ApiHarness.expect_error(second, 409, message_contains="already exists")


def _list_waves_happy(harness: ApiHarness, state: dict[str, str]) -> None:
    response = harness.request("GET", "/v0/waves")
    ApiHarness.expect_status(response, 200)
    payload = ApiHarness.expect_json_object(response)
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
    payload = ApiHarness.expect_json_object(response)
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
    body = ApiHarness.expect_json_object(response)
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
    created = _create_wave(harness, runtime, _wave_name("api-delete"))
    wave_id = str(created["id"])

    delete_response = harness.request("DELETE", f"/v0/waves/{wave_id}")
    ApiHarness.expect_status(delete_response, 200)
    deleted = ApiHarness.expect_json_object(delete_response)
    ApiHarness.expect_fields(deleted, ["id", "object", "deleted"])
    assert deleted["id"] == wave_id
    assert deleted["object"] == "wave"
    assert deleted["deleted"] is True
    state["deleted_wave_id"] = wave_id


def _delete_wave_missing_error(harness: ApiHarness, state: dict[str, str]) -> None:
    response = harness.request("DELETE", f"/v0/waves/{state['deleted_wave_id']}")
    ApiHarness.expect_error(response, 404, message_contains="wave not found")


def _create_wave(harness: ApiHarness, runtime: LfdRuntime, name: str) -> dict[str, Any]:
    response = harness.request(
        "POST",
        "/v0/waves",
        json={"repo": str(runtime.repo_dir), "name": name},
    )
    ApiHarness.expect_status(response, 200)
    payload = ApiHarness.expect_json_object(response)
    _expect_wave_shape(payload)
    assert payload["name"] == name
    assert payload["repo"] == str(runtime.repo_dir)
    return payload


if __name__ == "__main__":
    raise SystemExit(main())
