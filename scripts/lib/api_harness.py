#!/usr/bin/env python3
"""Shared HTTP assertion harness for API contract suites."""

from __future__ import annotations

import traceback
from dataclasses import dataclass
from typing import Any, Callable, Iterable

import httpx


@dataclass
class ScenarioResult:
    name: str
    passed: bool
    detail: str = ""


class ApiHarness:
    """Small scenario runner with route-level JSON assertions."""

    def __init__(self, base_url: str, token: str, timeout_seconds: float = 10.0) -> None:
        self._authed_client = httpx.Client(
            base_url=base_url,
            timeout=timeout_seconds,
            headers={"Authorization": f"Bearer {token}"},
        )
        self._anonymous_client = httpx.Client(base_url=base_url, timeout=timeout_seconds)
        self._results: list[ScenarioResult] = []

    def close(self) -> None:
        self._authed_client.close()
        self._anonymous_client.close()

    def run_scenario(self, name: str, check: Callable[[], None]) -> None:
        try:
            check()
        except AssertionError as exc:
            detail = str(exc) or "assertion failed"
            self._record(ScenarioResult(name=name, passed=False, detail=detail))
        except Exception as exc:  # pragma: no cover - defensive in script harness
            detail = f"{type(exc).__name__}: {exc}"
            stack = traceback.format_exc(limit=3)
            self._record(ScenarioResult(name=name, passed=False, detail=f"{detail}\n{stack}"))
        else:
            self._record(ScenarioResult(name=name, passed=True))

    def request(self, method: str, path: str, auth: bool = True, **kwargs: Any) -> httpx.Response:
        client = self._authed_client if auth else self._anonymous_client
        return client.request(method, path, **kwargs)

    @staticmethod
    def expect_status(response: httpx.Response, status_code: int) -> None:
        if response.status_code == status_code:
            return
        raise AssertionError(
            f"expected HTTP {status_code}, got {response.status_code}; body={response.text}"
        )

    @staticmethod
    def expect_error(
        response: httpx.Response,
        status_code: int,
        message_contains: str | None = None,
        error_type: str = "invalid_request_error",
    ) -> dict[str, Any]:
        ApiHarness.expect_status(response, status_code)
        payload = _json_dict(response)
        error = payload.get("error")
        if not isinstance(error, dict):
            raise AssertionError(f"expected error envelope, got: {payload}")

        error_value = error.get("type")
        if error_value != error_type:
            raise AssertionError(f"expected error.type={error_type}, got {error_value}")

        message = error.get("message")
        if not isinstance(message, str):
            raise AssertionError(f"expected string error.message, got {message!r}")

        if message_contains is not None and message_contains not in message:
            raise AssertionError(
                f"expected error.message to contain {message_contains!r}, got {message!r}"
            )

        return payload

    @staticmethod
    def expect_fields(payload: dict[str, Any], required_fields: Iterable[str]) -> None:
        missing = [field for field in required_fields if field not in payload]
        if missing:
            raise AssertionError(f"missing fields: {missing}; payload={payload}")

    def has_failures(self) -> bool:
        return any(not result.passed for result in self._results)

    def print_summary(self) -> None:
        total = len(self._results)
        passed = len([result for result in self._results if result.passed])
        failed = total - passed
        print(f"SUMMARY total={total} passed={passed} failed={failed}")

    def _record(self, result: ScenarioResult) -> None:
        self._results.append(result)
        if result.passed:
            print(f"PASS {result.name}")
            return
        print(f"FAIL {result.name} :: {result.detail}")


def _json_dict(response: httpx.Response) -> dict[str, Any]:
    try:
        payload = response.json()
    except ValueError as exc:
        raise AssertionError(f"expected JSON body, got: {response.text}") from exc

    if not isinstance(payload, dict):
        raise AssertionError(f"expected JSON object, got: {payload!r}")

    return payload
