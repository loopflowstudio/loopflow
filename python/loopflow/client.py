from __future__ import annotations

import json
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
    Chord,
    ProviderInfo,
    Repo,
    Session,
    SessionConfig,
    SessionEventEnvelope,
    UsageSummary,
    Wave,
    WaveRun,
)

ModelT = TypeVar("ModelT", bound=BaseModel)


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

    def revoke_connection_tokens(
        self,
        prefix: Optional[str] = None,
        revoke_all: bool = False,
    ) -> int:
        body: dict[str, Any] = {"all": revoke_all}
        if prefix is not None:
            body["prefix"] = prefix
        payload = self._request_json("POST", "/v0/tokens/revoke", json=body)
        revoked = payload.get("revoked")
        if not isinstance(revoked, int):
            raise LoopflowError("invalid token revoke response payload")
        return revoked

    def usage_summary(
        self,
        group_by: str = "wave",
        wave: Optional[str] = None,
        flow: Optional[str] = None,
        step: Optional[str] = None,
        model: Optional[str] = None,
        source: Optional[str] = None,
        from_: Optional[str] = None,
        to_: Optional[str] = None,
    ) -> UsageSummary:
        optional = {
            "wave": wave,
            "flow": flow,
            "step": step,
            "model": model,
            "source": source,
            "from": from_,
            "to": to_,
        }
        params = {"group_by": group_by, **{k: v for k, v in optional.items() if v is not None}}
        payload = self._request_json("GET", "/v0/usage/summary", params=params)
        return UsageSummary.model_validate(payload)

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

    def create_chord(self, name: str) -> Chord:
        payload = self._request_json("POST", "/v0/chords", json={"name": name})
        return Chord.model_validate(payload)

    def list_chords(self) -> list[Chord]:
        payload = self._request_json("GET", "/v0/chords")
        return self._parse_model_list(payload, Chord)

    def get_chord(self, chord_id: str) -> Optional[Chord]:
        return self._request_optional_model(f"/v0/chords/{chord_id}", Chord)

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

    def delete_chord(self, chord_id: str) -> None:
        self._request_json("DELETE", f"/v0/chords/{chord_id}")

    def add_chord_member(self, chord_id: str, wave_id: str) -> None:
        self._request_json(
            "POST",
            f"/v0/chords/{chord_id}/members",
            json={"wave_id": wave_id},
        )

    def remove_chord_member(self, chord_id: str, wave_id: str) -> None:
        self._request_json(
            "DELETE",
            f"/v0/chords/{chord_id}/members/{wave_id}",
        )

    def list_chord_members(self, chord_id: str) -> list[Wave]:
        payload = self._request_json("GET", f"/v0/chords/{chord_id}/members")
        return self._parse_model_list(payload, Wave)

    def list_wave_chords(self, wave_id: str) -> list[Chord]:
        payload = self._request_json("GET", f"/v0/waves/{wave_id}/chords")
        return self._parse_model_list(payload, Chord)

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
        source_wave_id: Optional[str] = None,
        max_iterations: Optional[int] = None,
    ) -> dict[str, Any]:
        body: dict[str, Any] = {"kind": kind}
        if cron is not None:
            body["cron"] = cron
        if source_wave_id is not None:
            body["source_wave_id"] = source_wave_id
        if max_iterations is not None:
            body["max_iterations"] = max_iterations
        return self._request_json("POST", f"/v0/waves/{name_or_id}/stimulus", json=body)

    def remove_stimulus(self, name_or_id: str, stimulus_id: str) -> dict[str, Any]:
        return self._request_json("DELETE", f"/v0/waves/{name_or_id}/stimulus/{stimulus_id}")

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
        return self._parse_model_list(payload, WaveRun)

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

    def create_session(
        self,
        harness: str,
        wave_run_id: Optional[str] = None,
        config: Optional[SessionConfig] = None,
    ) -> Session:
        body: dict[str, Any] = {"harness": harness, "config": {}}
        if wave_run_id is not None:
            body["wave_run_id"] = wave_run_id
        if config is not None:
            body["config"] = config.model_dump(exclude_none=True)
        payload = self._request_json("POST", "/v0/sessions", json=body)
        return Session.model_validate(payload)

    def session(self, session_id: str) -> Optional[Session]:
        return self._request_optional_model(f"/v0/sessions/{session_id}", Session)

    def send_session_input(self, session_id: str, content: str) -> Session:
        payload = self._request_json(
            "POST",
            f"/v0/sessions/{session_id}/input",
            json={"content": content},
        )
        return Session.model_validate(payload)

    def stop_session(self, session_id: str) -> Session:
        payload = self._request_json("DELETE", f"/v0/sessions/{session_id}")
        return Session.model_validate(payload)

    def stream_session_events(
        self,
        session_id: str,
        after_seq: Optional[int] = None,
        timeout: float = 60.0,
    ) -> Iterator[SessionEventEnvelope]:
        params: dict[str, str] = {}
        if after_seq is not None:
            params["after_seq"] = str(after_seq)

        try:
            with self._client.stream(
                "GET",
                f"/v0/sessions/{session_id}/events",
                params=params or None,
                timeout=timeout,
            ) as response:
                if response.status_code >= 400:
                    self._raise_for_error(response)

                pending_seq: Optional[int] = None
                for line in response.iter_lines():
                    if not line:
                        continue
                    if line.startswith("id:"):
                        value = line[3:].strip()
                        try:
                            pending_seq = int(value)
                        except ValueError:
                            pending_seq = None
                        continue

                    if not line.startswith("data:"):
                        continue

                    payload = line[5:].strip()
                    if not payload:
                        continue

                    try:
                        event = json.loads(payload)
                    except json.JSONDecodeError:
                        continue

                    if not isinstance(event, dict):
                        continue

                    yield SessionEventEnvelope(seq=pending_seq, event=event)
                    pending_seq = None
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
