from __future__ import annotations

import os
from collections.abc import Iterator
from pathlib import Path
from typing import Any, Optional, TypeVar
from urllib.parse import urlparse

import httpx
from pydantic import BaseModel

from .errors import LoopflowError, WaveAlreadyRunning
from .models import (
    AuthFlow,
    AuthProviderStatus,
    ProviderInfo,
    Repo,
    Run,
    Session,
    SessionConnectionInfo,
    Wave,
    WaveAgentTree,
)

ModelT = TypeVar("ModelT", bound=BaseModel)


def _compact_dict(**values: Any) -> dict[str, Any]:
    return {key: value for key, value in values.items() if value is not None}


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
            base_url=resolved,
            timeout=timeout,
            headers=headers,
        )

    def close(self) -> None:
        self._client.close()

    def health(self) -> dict[str, Any]:
        return self._request_json("GET", "/health")

    def status(self) -> dict[str, Any]:
        return self._request_json("GET", "/status")

    def auth_status(
        self,
        provider: Optional[str] = None,
    ) -> list[AuthProviderStatus] | AuthProviderStatus:
        if provider is None:
            payload = self._request_json("GET", "/v0/auth")
            providers = payload.get("providers", [])
            return [AuthProviderStatus.model_validate(item) for item in providers]

        payload = self._request_json("GET", f"/v0/auth/{provider}")
        return AuthProviderStatus.model_validate(payload)

    def start_auth(self, provider: str) -> AuthFlow:
        payload = self._request_json("POST", f"/v0/auth/{provider}")
        return AuthFlow.model_validate(payload)

    def complete_auth(self, provider: str, code: str) -> None:
        body = {"code": code}
        self._request_json("POST", f"/v0/auth/{provider}/complete", json=body)

    def disconnect_auth(self, provider: str) -> AuthProviderStatus:
        payload = self._request_json("DELETE", f"/v0/auth/{provider}")
        return AuthProviderStatus.model_validate(payload)

    def configure_api_key(
        self,
        provider: str,
        api_key: str,
    ) -> AuthProviderStatus:
        body = {"api_key": api_key}
        payload = self._request_json("PUT", f"/v0/auth/{provider}/credential", json=body)
        return AuthProviderStatus.model_validate(payload)

    def providers(self) -> list[ProviderInfo]:
        payload = self._request_json("GET", "/v0/providers")
        if not isinstance(payload, list):
            raise LoopflowError("invalid providers response payload")
        return [ProviderInfo.model_validate(item) for item in payload]

    def waves(self, repo: Optional[str] = None) -> list[Wave]:
        params: dict[str, str] = {}
        if repo:
            params["repo"] = repo
        payload = self._request_json("GET", "/v0/waves", params=params)
        return self._parse_model_list(payload, Wave)

    def wave(self, name_or_id: str) -> Optional[Wave]:
        return self._request_optional_model(f"/v0/waves/{name_or_id}", Wave)

    def create_wave(
        self,
        name: str,
        repo: str,
        flow: Optional[str] = None,
        crons: Optional[list[dict[str, str]]] = None,
        direction: Optional[list[str]] = None,
        area: Optional[list[str]] = None,
        status: Optional[str] = None,
        goal: Optional[str] = None,
    ) -> Wave:
        body = {
            "repo": repo,
            "name": name,
            **_compact_dict(
                flow=flow,
                goal=goal,
                crons=crons,
                direction=direction,
                area=area,
                status=status,
            ),
        }
        payload = self._request_json("POST", "/v0/waves", json=body)
        return Wave.model_validate(payload)

    def update_wave(
        self,
        name_or_id: str,
        flow: Optional[str] = None,
        crons: Optional[list[dict[str, str]]] = None,
        direction: Optional[list[str]] = None,
        area: Optional[list[str]] = None,
        status: Optional[str] = None,
        goal: Optional[str] = None,
    ) -> Wave:
        body = _compact_dict(
            flow=flow,
            goal=goal,
            crons=crons,
            direction=direction,
            area=area,
            status=status,
        )
        payload = self._request_json("PATCH", f"/v0/waves/{name_or_id}", json=body)
        return Wave.model_validate(payload)

    def delete_wave(self, name_or_id: str) -> None:
        self._request_json("DELETE", f"/v0/waves/{name_or_id}")

    def list_repos(self) -> list[Repo]:
        payload = self._request_json("GET", "/v0/repos")
        return self._parse_model_list(payload, Repo)

    def add_repo(self, path: str) -> Repo:
        payload = self._request_json("POST", "/v0/repos", json={"path": path})
        return Repo.model_validate(payload)

    def remove_repo(self, path: str) -> None:
        self._request_json("DELETE", "/v0/repos", json={"path": path})

    def add_child(self, owner: str, repo: str, child_owner: str, child_repo: str) -> None:
        self._request_json(
            "POST",
            f"/v0/repos/{owner}/{repo}/children/{child_owner}/{child_repo}",
        )

    def remove_child(self, owner: str, repo: str, child_owner: str, child_repo: str) -> None:
        self._request_json(
            "DELETE",
            f"/v0/repos/{owner}/{repo}/children/{child_owner}/{child_repo}",
        )

    def list_children(self, owner: str, repo: str) -> list[Repo]:
        payload = self._request_json("GET", f"/v0/repos/{owner}/{repo}/children")
        return self._parse_model_list(payload, Repo)

    def list_parents(self, owner: str, repo: str) -> list[Repo]:
        payload = self._request_json("GET", f"/v0/repos/{owner}/{repo}/parents")
        return self._parse_model_list(payload, Repo)

    def run_wave(
        self,
        name_or_id: str,
        flow: Optional[str] = None,
        direction: Optional[list[str]] = None,
        area: Optional[list[str]] = None,
        goal: Optional[str] = None,
    ) -> Session:
        body = _compact_dict(flow=flow, goal=goal, direction=direction, area=area)
        payload = self._request_json("POST", f"/v0/waves/{name_or_id}/run", json=body)
        return Session.model_validate(payload)

    def ensure_wave_agent(self, name_or_id: str) -> Session:
        return self.run_wave(name_or_id)

    def get_wave_agent_tree(
        self,
        name_or_id: str,
        active_only: bool = True,
    ) -> WaveAgentTree:
        payload = self._request_json(
            "GET",
            f"/v0/waves/{name_or_id}/agent-tree",
            params={"active_only": "true" if active_only else "false"},
        )
        return WaveAgentTree.model_validate(payload)

    def add_trigger(
        self,
        name_or_id: str,
        signal: str,
        flow: Optional[str] = None,
        source_wave_id: Optional[str] = None,
        max_iterations: Optional[int] = None,
    ) -> dict[str, Any]:
        body = {
            "signal": signal,
            **_compact_dict(
                flow=flow,
                source_wave_id=source_wave_id,
                max_iterations=max_iterations,
            ),
        }
        return self._request_json("POST", f"/v0/waves/{name_or_id}/triggers", json=body)

    def remove_trigger(self, name_or_id: str, trigger_id: str) -> dict[str, Any]:
        return self._request_json("DELETE", f"/v0/waves/{name_or_id}/triggers/{trigger_id}")

    def stop_wave(self, name_or_id: str) -> dict[str, Any]:
        return self._request_json("POST", f"/v0/waves/{name_or_id}/stop")

    def land_wave(
        self,
        name_or_id: str,
        strict: Optional[bool] = None,
        local: Optional[bool] = None,
        create_pr: Optional[bool] = None,
        worktree: Optional[str] = None,
    ) -> dict[str, Any]:
        body = _compact_dict(
            strict=strict,
            local=local,
            create_pr=create_pr,
            worktree=worktree,
        )
        return self._request_json("POST", f"/v0/waves/{name_or_id}/land", json=body)

    def next_wave(self, name_or_id: str) -> dict[str, Any]:
        return self._request_json("POST", f"/v0/waves/{name_or_id}/next")

    def runs(
        self,
        wave_id: Optional[str] = None,
        repo: Optional[str] = None,
        limit: Optional[int] = None,
    ) -> list[Run]:
        params = _compact_dict(
            wave_id=wave_id,
            repo=repo,
            limit=str(limit) if limit is not None else None,
        )
        payload = self._request_json("GET", "/v0/runs", params=params)
        return self._parse_model_list(payload, Run)

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

    def run_worker(
        self,
        name_or_id: str,
        flow: str,
        task: str,
        parent_session_id: Optional[str] = None,
    ) -> Session:
        body = _compact_dict(flow=flow, task=task, parent_session_id=parent_session_id)
        payload = self._request_json("POST", f"/v0/waves/{name_or_id}/workers", json=body)
        return Session.model_validate(payload)

    def list_sessions(
        self,
        wave_id: Optional[str] = None,
        parent_session_id: Optional[str] = None,
        use: Optional[str] = None,
        active_only: bool = True,
    ) -> list[Session]:
        params = _compact_dict(
            wave_id=wave_id,
            parent_session_id=parent_session_id,
            use=use,
            active_only="true" if active_only else "false",
        )
        payload = self._request_json("GET", "/v0/sessions", params=params)
        return [Session.model_validate(item) for item in self._parse_dict_list(payload)]

    def get_session(self, session_id: str) -> Session:
        payload = self._request_json("GET", f"/v0/sessions/{session_id}")
        return Session.model_validate(payload)

    def current_session(self, cwd: str) -> Optional[Session]:
        payload = self._request_json(
            "GET",
            "/v0/sessions/current",
            params={"cwd": cwd},
            allow_not_found=True,
        )
        if payload is None:
            return None
        return Session.model_validate(payload)

    def attach_session(self, session_id: str) -> SessionConnectionInfo:
        payload = self._request_json("POST", f"/v0/sessions/{session_id}/attach")
        return SessionConnectionInfo.model_validate(payload)

    def list_attention(self, status: Optional[str] = None) -> list[dict[str, Any]]:
        params = _compact_dict(status=status)
        payload = self._request_json("GET", "/v0/attention", params=params)
        return self._parse_dict_list(payload)

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
            self._raise_for_error(response)

        if response.status_code == 204 or not response.content:
            return None

        return response.json()

    @staticmethod
    def _parse_model_list(payload: Any, model_type: type[ModelT]) -> list[ModelT]:
        if not isinstance(payload, dict):
            raise LoopflowError("invalid list response payload")

        data = payload.get("data", [])
        if not isinstance(data, list):
            raise LoopflowError("invalid list response payload")

        return [model_type.model_validate(item) for item in data]

    def _request_optional_model(self, path: str, model_type: type[ModelT]) -> Optional[ModelT]:
        payload = self._request_json("GET", path, allow_not_found=True)
        if payload is None:
            return None
        return model_type.model_validate(payload)

    @staticmethod
    def _parse_dict_list(payload: Any) -> list[dict[str, Any]]:
        if not isinstance(payload, dict):
            raise LoopflowError("invalid list response payload")

        data = payload.get("data", [])
        if not isinstance(data, list):
            raise LoopflowError("invalid list response payload")
        if not all(isinstance(item, dict) for item in data):
            raise LoopflowError("invalid list response payload")

        return data

    @staticmethod
    def _raise_for_error(response: httpx.Response) -> None:
        message = _extract_error_message(response)
        if response.status_code == 412:
            raise WaveAlreadyRunning(message)
        raise LoopflowError(message)


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
