#!/usr/bin/env python3
"""Live provider-auth contract validation with evidence capture."""

from __future__ import annotations

import argparse
import asyncio
import hashlib
import inspect
import json
import os
import subprocess
import tempfile
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import urlparse, urlunparse

import httpx

from lib.lfd_runtime import LfdRuntime

try:
    import websockets
except ImportError:  # pragma: no cover - runtime guard for script users
    websockets = None


PROVIDERS = ("github", "claude", "codex")
PROVIDER_CREDENTIAL_ROOTS = {
    "github": [".config/gh"],
    "claude": [".claude"],
    "codex": [".codex"],
}
PROVIDER_AUTH_COMMANDS = {
    "github": (
        ["gh", "auth", "login", "--web", "--hostname", "github.com", "--git-protocol", "https", "--skip-ssh-key"],
        {"GH_BROWSER": "echo"},
    ),
    "claude": (
        ["claude", "auth", "login"],
        {"BROWSER": "echo", "CLAUDE_BROWSER": "echo"},
    ),
    "codex": (
        ["codex", "login", "--device-auth"],
        {},
    ),
}


@dataclass
class ProviderResult:
    provider: str
    passed: bool
    reason: str
    evidence_dir: Path


def main() -> int:
    args = _parse_args()
    providers = _parse_providers(args.providers)
    if websockets is None:
        raise RuntimeError("Missing dependency: websockets. Run `uv sync --dev` and retry.")

    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run_dir = Path(args.reports_dir) / timestamp
    run_dir.mkdir(parents=True, exist_ok=True)

    print(f"Live auth contract run: {run_dir}")
    results: list[ProviderResult] = []

    with LfdRuntime(build_binary=not args.skip_build) as runtime:
        _write_json(
            run_dir / "run-metadata.json",
            {
                "started_at": timestamp,
                "base_url": runtime.base_url,
                "providers": providers,
                "home_dir": str(runtime.home_dir),
            },
        )

        for provider in providers:
            provider_dir = run_dir / provider
            provider_dir.mkdir(parents=True, exist_ok=True)
            result = validate_provider(
                provider=provider,
                base_url=runtime.base_url,
                token=runtime.token,
                runtime_home=runtime.home_dir,
                evidence_dir=provider_dir,
                event_timeout_seconds=args.event_timeout,
                pending_timeout_seconds=args.pending_timeout,
                transcript_timeout_seconds=args.transcript_timeout,
                http_timeout_seconds=args.http_timeout,
            )
            results.append(result)

        if "claude" in providers:
            disconnect_dir = run_dir / "claude-disconnect"
            disconnect_dir.mkdir(parents=True, exist_ok=True)
            results.append(
                validate_claude_disconnect(
                    base_url=runtime.base_url,
                    token=runtime.token,
                    runtime_home=runtime.home_dir,
                    evidence_dir=disconnect_dir,
                    http_timeout_seconds=args.http_timeout,
                )
            )

        (run_dir / "lfd.log").write_text(runtime.logs(), encoding="utf-8")

    _write_json(
        run_dir / "matrix.json",
        [
            {
                "provider": result.provider,
                "passed": result.passed,
                "reason": result.reason,
                "evidence_dir": str(result.evidence_dir),
            }
            for result in results
        ],
    )
    _print_matrix(results)
    failed = [result for result in results if not result.passed]
    if failed:
        print(f"❌ auth-live contract failed ({len(failed)}/{len(results)}). Evidence: {run_dir}")
        return 1

    print(f"✅ auth-live contract passed ({len(results)}/{len(results)}). Evidence: {run_dir}")
    return 0


def validate_provider(
    provider: str,
    base_url: str,
    token: str,
    runtime_home: Path,
    evidence_dir: Path,
    event_timeout_seconds: float,
    pending_timeout_seconds: float,
    transcript_timeout_seconds: float,
    http_timeout_seconds: float,
) -> ProviderResult:
    transcript = capture_cli_transcript(provider, transcript_timeout_seconds)
    _write_json(evidence_dir / "cli-transcript.json", transcript)
    snapshot_provider_tree(runtime_home, provider, evidence_dir / "credentials-before.txt")

    outcome = asyncio.run(
        run_provider_contract(
            provider=provider,
            base_url=base_url,
            token=token,
            event_timeout_seconds=event_timeout_seconds,
            pending_timeout_seconds=pending_timeout_seconds,
            http_timeout_seconds=http_timeout_seconds,
            evidence_dir=evidence_dir,
        )
    )

    snapshot_provider_tree(runtime_home, provider, evidence_dir / "credentials-after.txt")
    _write_json(evidence_dir / "contract-summary.json", outcome)

    return ProviderResult(
        provider=provider,
        passed=bool(outcome.get("passed")),
        reason=str(outcome.get("reason", "")),
        evidence_dir=evidence_dir,
    )


async def run_provider_contract(
    provider: str,
    base_url: str,
    token: str,
    event_timeout_seconds: float,
    pending_timeout_seconds: float,
    http_timeout_seconds: float,
    evidence_dir: Path,
) -> dict[str, Any]:
    result: dict[str, Any] = {
        "provider": provider,
        "passed": False,
        "reason": "",
    }
    headers = {"Authorization": f"Bearer {token}"}
    ws_url = ws_url_for(base_url, "/ws")
    connect_kwargs = websocket_connect_kwargs(token, event_timeout_seconds)

    async with httpx.AsyncClient(base_url=base_url, timeout=http_timeout_seconds, headers=headers) as client:
        result["status_before"] = await fetch_auth_status(client, provider)
        _write_json(evidence_dir / "status-before.json", result["status_before"])

        try:
            async with websockets.connect(ws_url, **connect_kwargs) as socket:
                connected_message = await recv_json(socket, event_timeout_seconds)
                result["ws_connected"] = connected_message

                start_response = await client.post(f"/v0/auth/{provider}")
                result["start_http_status"] = start_response.status_code
                result["start_payload"] = parse_json_object(start_response)
                _write_json(evidence_dir / "start-auth.json", result["start_payload"])

                if start_response.status_code != 200:
                    result["reason"] = (
                        f"POST /v0/auth/{provider} returned {start_response.status_code}: "
                        f"{start_response.text.strip()}"
                    )
                    return result

                start_error = validate_start_payload(provider, result["start_payload"])
                if start_error is not None:
                    result["reason"] = start_error
                    return result

                pending_samples = await poll_provider_status(client, provider, pending_timeout_seconds)
                result["status_samples"] = pending_samples
                _write_json(evidence_dir / "status-samples.json", pending_samples)
                saw_pending = any(sample.get("status") == "pending" for sample in pending_samples)
                if not saw_pending:
                    result["reason"] = "never observed pending status after start_auth"
                    return result

                auth_events, terminal_event, event_error = await collect_auth_events(
                    socket=socket,
                    provider=provider,
                    event_timeout_seconds=event_timeout_seconds,
                )
                write_jsonl(evidence_dir / "auth-events.jsonl", auth_events)
                result["auth_events"] = auth_events
                result["terminal_event"] = terminal_event

                if event_error is not None:
                    result["reason"] = event_error
                    return result

                flow_started = next(
                    (event for event in auth_events if event.get("type") == "auth.flow_started"),
                    None,
                )
                if flow_started is None:
                    result["reason"] = "missing auth.flow_started event"
                    return result

                if flow_started.get("verification_uri") != result["start_payload"].get("verification_uri"):
                    result["reason"] = "auth.flow_started verification_uri mismatch"
                    return result

                final_status = await fetch_auth_status(client, provider)
                result["status_final"] = final_status
                _write_json(evidence_dir / "status-final.json", final_status)
        except Exception as exc:  # pragma: no cover - defensive script path
            result["reason"] = f"websocket contract failure: {type(exc).__name__}: {exc}"
            return result
        finally:
            cleanup_response = await client.delete(f"/v0/auth/{provider}")
            try:
                cleanup_payload = parse_json_object(cleanup_response)
            except AssertionError:
                cleanup_payload = {"raw": cleanup_response.text}
            result["cleanup_http_status"] = cleanup_response.status_code
            result["cleanup_payload"] = cleanup_payload
            _write_json(evidence_dir / "cleanup.json", cleanup_payload)

    terminal_type = result.get("terminal_event")
    final_status_name = result.get("status_final", {}).get("status")
    if terminal_type == "auth.connected":
        if final_status_name != "active":
            result["reason"] = (
                f"auth.connected event requires active final status, got {final_status_name!r}"
            )
            return result
    elif terminal_type == "auth.failed":
        if final_status_name not in {"none", "expired"}:
            result["reason"] = (
                f"auth.failed event requires final status none/expired, got {final_status_name!r}"
            )
            return result
    else:
        result["reason"] = f"unexpected terminal auth event: {terminal_type!r}"
        return result

    result["passed"] = True
    result["reason"] = f"ok ({terminal_type}, final={final_status_name})"
    return result


async def collect_auth_events(
    socket: Any,
    provider: str,
    event_timeout_seconds: float,
) -> tuple[list[dict[str, Any]], str | None, str | None]:
    deadline = time.monotonic() + event_timeout_seconds
    events: list[dict[str, Any]] = []
    saw_flow_started = False

    while time.monotonic() < deadline:
        remaining = max(0.1, deadline - time.monotonic())
        try:
            payload = await recv_json(socket, remaining)
        except TimeoutError:
            break
        event_type = payload.get("type")
        if not isinstance(event_type, str):
            continue
        if event_type in {"connected", "ping"}:
            continue
        if payload.get("provider") != provider:
            continue
        if not event_type.startswith("auth."):
            continue

        events.append(payload)
        if event_type == "auth.flow_started":
            saw_flow_started = True
            continue
        if event_type in {"auth.connected", "auth.failed"}:
            if not saw_flow_started:
                return events, event_type, "terminal auth event arrived before auth.flow_started"
            return events, event_type, None

    return events, None, f"timed out after {event_timeout_seconds}s waiting for terminal auth event"


async def poll_provider_status(
    client: httpx.AsyncClient,
    provider: str,
    timeout_seconds: float,
) -> list[dict[str, Any]]:
    samples: list[dict[str, Any]] = []
    deadline = time.monotonic() + timeout_seconds

    while time.monotonic() < deadline:
        payload = await fetch_auth_status(client, provider)
        payload["sampled_at"] = datetime.now(timezone.utc).isoformat()
        samples.append(payload)
        if payload.get("status") == "pending":
            return samples
        await asyncio.sleep(0.4)

    return samples


async def fetch_auth_status(client: httpx.AsyncClient, provider: str) -> dict[str, Any]:
    response = await client.get(f"/v0/auth/{provider}")
    payload = parse_json_object(response)
    payload["http_status"] = response.status_code
    return payload


def validate_start_payload(provider: str, payload: dict[str, Any]) -> str | None:
    if payload.get("provider") != provider:
        return f"start payload provider mismatch: expected {provider!r}, got {payload.get('provider')!r}"

    verification_uri = payload.get("verification_uri")
    if not isinstance(verification_uri, str) or not verification_uri.startswith(("http://", "https://")):
        return f"invalid verification_uri: {verification_uri!r}"

    verification_uri_complete = payload.get("verification_uri_complete")
    if verification_uri_complete is not None and not isinstance(verification_uri_complete, str):
        return f"verification_uri_complete must be string when present, got {verification_uri_complete!r}"

    if verification_uri_complete is not None and not verification_uri_complete.startswith(
        ("http://", "https://")
    ):
        return f"invalid verification_uri_complete: {verification_uri_complete!r}"

    user_code = payload.get("user_code")
    if user_code is not None and not isinstance(user_code, str):
        return f"user_code must be string when present, got {user_code!r}"

    return None


def validate_claude_disconnect(
    base_url: str,
    token: str,
    runtime_home: Path,
    evidence_dir: Path,
    http_timeout_seconds: float,
) -> ProviderResult:
    claude_dir = runtime_home / ".claude"
    claude_dir.mkdir(parents=True, exist_ok=True)
    (claude_dir / "settings.json").write_text("{\"theme\":\"dark\"}\n", encoding="utf-8")
    (claude_dir / "projects.json").write_text("{\"recent\":[]}\n", encoding="utf-8")
    (claude_dir / "auth.json").write_text("{\"token\":\"secret\"}\n", encoding="utf-8")
    (claude_dir / "oauth-tokens.json").write_text("{\"token\":\"secret\"}\n", encoding="utf-8")
    session_cache = claude_dir / "session-cache"
    session_cache.mkdir(parents=True, exist_ok=True)
    (session_cache / "entry").write_text("session", encoding="utf-8")

    snapshot_provider_tree(runtime_home, "claude", evidence_dir / "credentials-before.txt")

    with httpx.Client(
        base_url=base_url,
        timeout=http_timeout_seconds,
        headers={"Authorization": f"Bearer {token}"},
    ) as client:
        response = client.delete("/v0/auth/claude")
        try:
            payload = parse_json_object(response)
        except AssertionError as exc:
            _write_json(
                evidence_dir / "disconnect-response.json",
                {
                    "http_status": response.status_code,
                    "raw": response.text,
                },
            )
            reason = f"DELETE /v0/auth/claude returned non-JSON response: {exc}"
            return _claude_disconnect_result(False, reason, evidence_dir)
        payload["http_status"] = response.status_code
        _write_json(evidence_dir / "disconnect-response.json", payload)

    snapshot_provider_tree(runtime_home, "claude", evidence_dir / "credentials-after.txt")

    settings_exists = (claude_dir / "settings.json").exists()
    projects_exists = (claude_dir / "projects.json").exists()
    auth_exists = (claude_dir / "auth.json").exists()
    oauth_exists = (claude_dir / "oauth-tokens.json").exists()
    session_exists = session_cache.exists()

    if response.status_code != 200:
        reason = f"DELETE /v0/auth/claude returned {response.status_code}: {response.text.strip()}"
        return _claude_disconnect_result(False, reason, evidence_dir)

    if payload.get("status") != "none":
        reason = f"expected disconnect status none, got {payload.get('status')!r}"
        return _claude_disconnect_result(False, reason, evidence_dir)

    if not settings_exists or not projects_exists:
        return _claude_disconnect_result(
            False,
            "disconnect removed non-auth Claude settings",
            evidence_dir,
        )

    if auth_exists or oauth_exists or session_exists:
        return _claude_disconnect_result(
            False,
            "disconnect did not remove Claude auth artifacts",
            evidence_dir,
        )

    return _claude_disconnect_result(
        True,
        "ok (auth artifacts removed, settings preserved)",
        evidence_dir,
    )


def _claude_disconnect_result(passed: bool, reason: str, evidence_dir: Path) -> ProviderResult:
    return ProviderResult(
        provider="claude-disconnect",
        passed=passed,
        reason=reason,
        evidence_dir=evidence_dir,
    )


def capture_cli_transcript(provider: str, timeout_seconds: float) -> dict[str, Any]:
    command, env_updates = PROVIDER_AUTH_COMMANDS[provider]
    version = detect_cli_version(command[0])

    with tempfile.TemporaryDirectory(prefix=f"auth-live-{provider}-") as temp_home:
        env = os.environ.copy()
        env.update(env_updates)
        env["HOME"] = temp_home

        command_result: dict[str, Any] = {
            "provider": provider,
            "cli_version": version,
            "command": command,
            "timeout_seconds": timeout_seconds,
        }

        try:
            completed = subprocess.run(
                command,
                capture_output=True,
                text=True,
                timeout=timeout_seconds,
                env=env,
            )
            command_result["timed_out"] = False
            command_result["return_code"] = completed.returncode
            command_result["stdout"] = completed.stdout
            command_result["stderr"] = completed.stderr
        except subprocess.TimeoutExpired as exc:
            command_result["timed_out"] = True
            command_result["return_code"] = None
            command_result["stdout"] = _decode_process_output(exc.stdout)
            command_result["stderr"] = _decode_process_output(exc.stderr)
        except FileNotFoundError as exc:
            command_result["timed_out"] = False
            command_result["return_code"] = None
            command_result["stdout"] = ""
            command_result["stderr"] = str(exc)

        return command_result


def detect_cli_version(binary: str) -> str:
    try:
        completed = subprocess.run(
            [binary, "--version"],
            capture_output=True,
            text=True,
            timeout=5,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return "unknown"
    output = (completed.stdout + "\n" + completed.stderr).strip()
    return output.splitlines()[0] if output else "unknown"


def parse_json_object(response: httpx.Response) -> dict[str, Any]:
    try:
        payload = response.json()
    except ValueError as exc:
        raise AssertionError(f"expected JSON object, got {response.text!r}") from exc
    if not isinstance(payload, dict):
        raise AssertionError(f"expected JSON object body, got {payload!r}")
    return payload


def write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    lines = [json.dumps(row, sort_keys=True) for row in rows]
    path.write_text("\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")


def snapshot_provider_tree(home_dir: Path, provider: str, output_path: Path) -> None:
    lines: list[str] = []
    lines.append(f"# provider={provider}")
    lines.append(f"# captured_at={datetime.now(timezone.utc).isoformat()}")

    for relative_root in PROVIDER_CREDENTIAL_ROOTS[provider]:
        root = home_dir / relative_root
        lines.append(f"[root] {relative_root}")
        if not root.exists():
            lines.append("MISSING")
            continue

        paths = sorted(root.rglob("*"), key=lambda path: str(path.relative_to(home_dir)))
        if not paths:
            lines.append("EMPTY")
            continue

        for path in paths:
            rel = path.relative_to(home_dir)
            if path.is_dir():
                lines.append(f"D {rel}/")
                continue

            if path.is_file():
                digest = hash_file(path)
                size = path.stat().st_size
                lines.append(f"F {rel} size={size} sha256={digest}")
                continue

            lines.append(f"O {rel}")

    output_path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(64 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


async def recv_json(socket: Any, timeout_seconds: float) -> dict[str, Any]:
    raw = await asyncio.wait_for(socket.recv(), timeout=timeout_seconds)
    if not isinstance(raw, str):
        raise AssertionError(f"expected text websocket message, got {type(raw).__name__}")
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise AssertionError(f"invalid websocket JSON: {raw!r}") from exc
    if not isinstance(payload, dict):
        raise AssertionError(f"expected websocket object payload, got {payload!r}")
    return payload


def websocket_connect_kwargs(token: str, timeout_seconds: float) -> dict[str, Any]:
    headers = {"Authorization": f"Bearer {token}"}
    timeouts = {"open_timeout": timeout_seconds, "close_timeout": timeout_seconds}
    # websockets >=14 uses `additional_headers`; older versions use `extra_headers`.
    for header_field in ("additional_headers", "extra_headers"):
        params = inspect.signature(websockets.connect).parameters
        if header_field in params:
            return {header_field: headers, **timeouts}
    # Fall back to the current API name if introspection fails entirely.
    return {"additional_headers": headers, **timeouts}


def ws_url_for(base_url: str, path: str) -> str:
    parsed = urlparse(base_url)
    if parsed.scheme == "https":
        scheme = "wss"
    elif parsed.scheme == "http":
        scheme = "ws"
    else:
        raise ValueError(f"unsupported URL scheme for websocket conversion: {parsed.scheme!r}")
    return urlunparse(parsed._replace(scheme=scheme, path=path, params="", query="", fragment=""))


def _decode_process_output(content: str | bytes | None) -> str:
    if content is None:
        return ""
    if isinstance(content, bytes):
        return content.decode("utf-8", errors="replace")
    return content


def _write_json(path: Path, payload: Any) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _parse_providers(raw: str) -> list[str]:
    providers = [provider.strip().lower() for provider in raw.split(",") if provider.strip()]
    invalid = [provider for provider in providers if provider not in PROVIDERS]
    if invalid:
        allowed = ", ".join(PROVIDERS)
        raise ValueError(f"unsupported provider(s): {', '.join(invalid)}; allowed: {allowed}")
    if not providers:
        raise ValueError("at least one provider is required")
    return providers


def _print_matrix(results: list[ProviderResult]) -> None:
    if not results:
        print("No provider checks were executed.")
        return

    width = max(len(result.provider) for result in results)
    print("\nProvider matrix:")
    print(f"{'provider'.ljust(width)} | result | reason | evidence")
    print(f"{'-' * width}-+--------+--------+---------")
    for result in results:
        status = "PASS" if result.passed else "FAIL"
        print(
            f"{result.provider.ljust(width)} | {status} | {result.reason} | {result.evidence_dir}"
        )


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate /v0/auth live contract for github/claude/codex and capture evidence."
    )
    parser.add_argument(
        "--providers",
        default="github,claude,codex",
        help="Comma-separated providers to validate (default: github,claude,codex)",
    )
    parser.add_argument(
        "--event-timeout",
        type=float,
        default=180.0,
        help="Seconds to wait for terminal auth websocket events per provider",
    )
    parser.add_argument(
        "--pending-timeout",
        type=float,
        default=20.0,
        help="Seconds to wait for pending status after starting auth",
    )
    parser.add_argument(
        "--transcript-timeout",
        type=float,
        default=12.0,
        help="Seconds to capture raw CLI auth transcript per provider",
    )
    parser.add_argument(
        "--http-timeout",
        type=float,
        default=15.0,
        help="HTTP timeout in seconds",
    )
    parser.add_argument(
        "--reports-dir",
        default="reports/auth-live",
        help="Directory for evidence output (default: reports/auth-live)",
    )
    parser.add_argument(
        "--skip-build",
        action="store_true",
        help="Skip cargo build for lfd if target/debug/lfd is already built",
    )
    return parser.parse_args()


if __name__ == "__main__":
    raise SystemExit(main())
