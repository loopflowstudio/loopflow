from __future__ import annotations

import os
from pathlib import Path
from urllib.parse import urlparse
from collections.abc import Iterator
from typing import Any, Optional

import httpx

from .errors import LoopflowError, WaveAlreadyRunning
from .models import Wave, WaveRun


def _resolve_base_url() -> str:
    url = os.environ.get("LFD_URL")
    if url:
        return url.rstrip("/")

    host = os.environ.get("LFD_HOST", "127.0.0.1")
    port = os.environ.get("LFD_PORT", "2486")
    return f"http://{host}:{port}"


def _is_local_base_url(base_url: str) -> bool:
    host = urlparse(base_url).hostname
    return host in {"127.0.0.1", "localhost", "::1"}


def _resolve_token(base_url: str) -> Optional[str]:
    token = os.environ.get("LFD_TOKEN")
    if token:
        return token

    if not _is_local_base_url(base_url):
        return None

    token_path = Path.home() / ".lf" / "session-token"
    try:
        return token_path.read_text().strip() or None
    except OSError:
        return None


class Client:
    def __init__(
        self,
        base_url: Optional[str] = None,
        timeout: float = 10.0,
        token: Optional[str] = None,
    ) -> None:
        resolved = base_url.rstrip("/") if base_url else _resolve_base_url()
        self._base_url = resolved
        resolved_token = token or _resolve_token(resolved)
        headers = {}
        if resolved_token:
            headers["Authorization"] = f"Bearer {resolved_token}"
        self._client = httpx.Client(
            base_url=resolved, timeout=timeout, headers=headers,
        )

    def close(self) -> None:
        self._client.close()

    def health(self) -> dict[str, Any]:
        return self._request_json("GET", "/health")

    def status(self) -> dict[str, Any]:
        return self._request_json("GET", "/status")

    def waves(self, repo: Optional[str] = None) -> list[Wave]:
        params: dict[str, str] = {}
        if repo:
            params["repo"] = repo
        payload = self._request_json("GET", "/v0/waves", params=params)
        data = payload.get("data", [])
        return [Wave.model_validate(item) for item in data]

    def wave(self, name_or_id: str) -> Optional[Wave]:
        payload = self._request_json(
            "GET",
            f"/v0/waves/{name_or_id}",
            allow_not_found=True,
        )
        if payload is None:
            return None
        return Wave.model_validate(payload)

    def create_wave(
        self,
        name: str,
        repo: str,
        flow: Optional[str] = None,
        direction: Optional[list[str]] = None,
        area: Optional[list[str]] = None,
    ) -> Wave:
        body: dict[str, Any] = {"repo": repo, "name": name}
        if flow is not None:
            body["flow"] = flow
        if direction is not None:
            body["direction"] = direction
        if area is not None:
            body["area"] = area
        payload = self._request_json("POST", "/v0/waves", json=body)
        return Wave.model_validate(payload)

    def update_wave(
        self,
        name_or_id: str,
        flow: Optional[str] = None,
        direction: Optional[list[str]] = None,
        area: Optional[list[str]] = None,
        status: Optional[str] = None,
    ) -> Wave:
        body: dict[str, Any] = {}
        if flow is not None:
            body["flow"] = flow
        if direction is not None:
            body["direction"] = direction
        if area is not None:
            body["area"] = area
        if status is not None:
            body["status"] = status
        payload = self._request_json("PATCH", f"/v0/waves/{name_or_id}", json=body)
        return Wave.model_validate(payload)

    def delete_wave(self, name_or_id: str) -> None:
        self._request_json("DELETE", f"/v0/waves/{name_or_id}")

    def run_wave(
        self,
        name_or_id: str,
        flow: Optional[str] = None,
        direction: Optional[list[str]] = None,
        area: Optional[list[str]] = None,
    ) -> dict[str, Any]:
        body: dict[str, Any] = {}
        if flow is not None:
            body["flow"] = flow
        if direction is not None:
            body["direction"] = direction
        if area is not None:
            body["area"] = area
        return self._request_json("POST", f"/v0/waves/{name_or_id}/run", json=body)

    def add_stimulus(
        self,
        name_or_id: str,
        kind: str,
        cron: Optional[str] = None,
    ) -> dict[str, Any]:
        body: dict[str, Any] = {"kind": kind}
        if cron is not None:
            body["cron"] = cron
        return self._request_json("POST", f"/v0/waves/{name_or_id}/stimulus", json=body)

    def remove_stimulus(self, name_or_id: str, stimulus_id: str) -> dict[str, Any]:
        return self._request_json(
            "DELETE", f"/v0/waves/{name_or_id}/stimulus/{stimulus_id}"
        )

    def stop_wave(self, name_or_id: str) -> dict[str, Any]:
        return self._request_json("POST", f"/v0/waves/{name_or_id}/stop")

    def land_wave(
        self,
        name_or_id: str,
        strict: Optional[bool] = None,
        local: Optional[bool] = None,
        create_pr: Optional[bool] = None,
        worktree: Optional[str] = None,
        lint: Optional[bool] = None,
    ) -> dict[str, Any]:
        body: dict[str, Any] = {}
        if strict is not None:
            body["strict"] = strict
        if local is not None:
            body["local"] = local
        if create_pr is not None:
            body["create_pr"] = create_pr
        if worktree is not None:
            body["worktree"] = worktree
        if lint is not None:
            body["lint"] = lint
        return self._request_json("POST", f"/v0/waves/{name_or_id}/land", json=body)

    def next_wave(self, name_or_id: str) -> dict[str, Any]:
        return self._request_json("POST", f"/v0/waves/{name_or_id}/next")

    def wave_runs(
        self,
        wave_id: Optional[str] = None,
        repo: Optional[str] = None,
        limit: Optional[int] = None,
    ) -> list[WaveRun]:
        params: dict[str, str] = {}
        if wave_id:
            params["wave_id"] = wave_id
        if repo:
            params["repo"] = repo
        if limit is not None:
            params["limit"] = str(limit)
        payload = self._request_json("GET", "/v0/wave_runs", params=params)
        data = payload.get("data", [])
        return [WaveRun.model_validate(item) for item in data]

    def wave_logs(self, name_or_id: str) -> Iterator[str]:
        try:
            with self._client.stream(
                "GET",
                f"/v0/waves/{name_or_id}/logs",
            ) as response:
                if response.status_code >= 400:
                    message = _extract_error_message(response)
                    if response.status_code == 412:
                        raise WaveAlreadyRunning(message)
                    raise LoopflowError(message)
                for line in response.iter_lines():
                    if line:
                        yield line
        except httpx.RequestError as exc:
            raise ConnectionError(str(exc)) from exc

    def _request_json(
        self,
        method: str,
        path: str,
        json: Optional[dict[str, Any]] = None,
        params: Optional[dict[str, str]] = None,
        allow_not_found: bool = False,
    ) -> Any:
        try:
            response = self._client.request(method, path, json=json, params=params)
        except httpx.RequestError as exc:
            raise ConnectionError(str(exc)) from exc

        if response.status_code == 404 and allow_not_found:
            return None

        if response.status_code >= 400:
            message = _extract_error_message(response)
            if response.status_code == 412:
                raise WaveAlreadyRunning(message)
            raise LoopflowError(message)

        return response.json()


def _extract_error_message(response: httpx.Response) -> str:
    try:
        data = response.json()
    except ValueError:
        return response.text or f"HTTP {response.status_code}"

    if isinstance(data, dict):
        error = data.get("error")
        if isinstance(error, dict):
            message = error.get("message")
            if message:
                return message
        if isinstance(error, str):
            return error

    return response.text or f"HTTP {response.status_code}"
