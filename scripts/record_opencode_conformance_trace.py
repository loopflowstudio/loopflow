#!/usr/bin/env python3
"""Record OpenCode conformance traces for replay tests."""

import argparse
import json
import shutil
import signal
import socket
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterator

import httpx

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT_DIR = REPO_ROOT / "rust/loopflow/src/lfd/sessions/harness/testdata"
DEFAULT_TIMEOUT_SECONDS = 60


@dataclass(frozen=True)
class Scenario:
    name: str
    fixture_name: str
    prompt: str


SCENARIOS = [
    Scenario(
        name="normal_turn",
        fixture_name="opencode_normal_turn.ndjson",
        prompt="Reply with exactly: TRACE_NORMAL_OK",
    ),
    Scenario(
        name="tool_lifecycle",
        fixture_name="opencode_tool_lifecycle.ndjson",
        prompt="Run `echo TRACE_TOOL_OK` and then explain what you ran.",
    ),
    Scenario(
        name="error_turn",
        fixture_name="opencode_error_turn.ndjson",
        prompt="Run `bash -lc 'exit 7'` and report the failure.",
    ),
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=DEFAULT_OUTPUT_DIR,
        help=f"Fixture output directory (default: {DEFAULT_OUTPUT_DIR})",
    )
    parser.add_argument(
        "--timeout-seconds",
        type=int,
        default=DEFAULT_TIMEOUT_SECONDS,
        help=f"Per-scenario timeout (default: {DEFAULT_TIMEOUT_SECONDS})",
    )
    parser.add_argument(
        "--skip-version",
        action="store_true",
        help="Skip querying `opencode --version` when writing the manifest",
    )
    return parser.parse_args()


def _allocate_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def _wait_for_server(client: httpx.Client, base_url: str, timeout_seconds: int) -> None:
    deadline = time.monotonic() + timeout_seconds
    delay = 0.1
    while time.monotonic() < deadline:
        try:
            client.get(base_url)
            return
        except httpx.HTTPError:
            time.sleep(delay)
            delay = min(delay * 2, 1.0)
    raise RuntimeError(f"Timed out waiting for OpenCode at {base_url}")


def _iter_sse_payloads(response: httpx.Response) -> Iterator[str]:
    data_lines: list[str] = []
    for line in response.iter_lines():
        if line is None:
            continue
        if line == "":
            if data_lines:
                yield "\n".join(data_lines)
                data_lines = []
            continue
        if line.startswith("data:"):
            data_lines.append(line[len("data:") :].lstrip())
    if data_lines:
        yield "\n".join(data_lines)


def _is_done(scenario: Scenario, event: dict[str, Any], state: dict[str, Any]) -> bool:
    event_type = event.get("type")
    properties = event.get("properties")
    if not isinstance(properties, dict):
        return False

    if event_type == "session.status":
        status = properties.get("status")
        if status == "active":
            state["saw_active"] = True
        if scenario.name == "normal_turn":
            return bool(state.get("saw_active")) and status == "idle"
        if scenario.name == "tool_lifecycle":
            return bool(state.get("saw_tool_completed")) and status == "idle"
        if scenario.name == "error_turn":
            return status == "error"

    if scenario.name == "tool_lifecycle" and event_type == "message.part.updated":
        part = properties.get("part")
        if isinstance(part, dict):
            part_type = str(part.get("type", "")).lower()
            if "tool" in part_type and part.get("state") == "completed":
                state["saw_tool_completed"] = True

    if scenario.name == "error_turn" and event_type == "session.error":
        return True

    return False


def _record_scenario(
    client: httpx.Client,
    base_url: str,
    scenario: Scenario,
    timeout_seconds: int,
) -> dict[str, Any]:
    session_response = client.post(f"{base_url}/session", json={})
    session_response.raise_for_status()
    session_create_body = session_response.json()

    session_id = session_create_body.get("id")
    if not isinstance(session_id, str) or not session_id:
        raise RuntimeError(
            f"OpenCode /session response missing canonical id: {session_create_body}"
        )

    stream_url = f"{base_url}/event"
    message_url = f"{base_url}/session/{session_id}/message"
    message_payload = {"parts": [{"type": "text", "text": scenario.prompt}]}

    captured_payloads: list[str] = []
    state: dict[str, Any] = {}
    timed_out = False
    deadline = time.monotonic() + timeout_seconds

    with client.stream(
        "GET",
        stream_url,
        headers={"accept": "text/event-stream"},
        timeout=timeout_seconds + 5,
    ) as event_stream:
        event_stream.raise_for_status()
        send_message = client.post(message_url, json=message_payload)
        send_message.raise_for_status()

        for payload in _iter_sse_payloads(event_stream):
            if payload.strip() in {"", "[DONE]"}:
                continue

            try:
                event = json.loads(payload)
            except json.JSONDecodeError:
                continue

            properties = event.get("properties")
            if not isinstance(properties, dict):
                continue

            if properties.get("sessionID") != session_id:
                continue

            captured_payloads.append(payload)

            if event.get("type") == "permission.asked":
                request_id = properties.get("requestID")
                if isinstance(request_id, str) and request_id:
                    approval_url = f"{base_url}/session/{session_id}/permissions/{request_id}"
                    approval = client.post(approval_url, json={"response": "always"})
                    approval.raise_for_status()

            if _is_done(scenario, event, state):
                break

            if time.monotonic() > deadline:
                timed_out = True
                break

    delete_response = client.delete(f"{base_url}/session/{session_id}")
    if delete_response.status_code >= 400:
        raise RuntimeError(
            f"Failed to delete OpenCode session {session_id}: "
            f"{delete_response.status_code} {delete_response.text}"
        )

    return {
        "name": scenario.name,
        "fixture": scenario.fixture_name,
        "prompt": scenario.prompt,
        "session_id": session_id,
        "session_create_response": session_create_body,
        "sse_payload_count": len(captured_payloads),
        "timed_out": timed_out,
        "events": captured_payloads,
    }


def _opencode_version(skip_version: bool) -> str:
    if skip_version:
        return "skipped"
    try:
        completed = subprocess.run(
            ["opencode", "--version"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (FileNotFoundError, subprocess.CalledProcessError):
        return "unknown"
    return completed.stdout.strip() or "unknown"


def main() -> None:
    args = parse_args()
    output_dir: Path = args.output_dir
    output_dir.mkdir(parents=True, exist_ok=True)

    if shutil.which("opencode") is None:
        raise RuntimeError("`opencode` was not found on PATH")

    port = _allocate_port()
    base_url = f"http://127.0.0.1:{port}"

    process = subprocess.Popen(
        ["opencode", "serve", "--port", str(port)],
        cwd=REPO_ROOT,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )

    manifest_scenarios: list[dict[str, Any]] = []
    with httpx.Client(timeout=30) as client:
        try:
            _wait_for_server(client, base_url, timeout_seconds=15)

            for scenario in SCENARIOS:
                result = _record_scenario(client, base_url, scenario, args.timeout_seconds)
                fixture_path = output_dir / scenario.fixture_name
                lines = [
                    json.dumps(result["session_create_response"], separators=(",", ":")),
                    *result["events"],
                ]
                fixture_path.write_text("\n".join(lines) + "\n")

                manifest_scenarios.append(
                    {
                        "name": result["name"],
                        "fixture": result["fixture"],
                        "prompt": result["prompt"],
                        "session_id": result["session_id"],
                        "session_create_response": result["session_create_response"],
                        "sse_payload_count": result["sse_payload_count"],
                        "timed_out": result["timed_out"],
                    }
                )
        finally:
            process.send_signal(signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)

    manifest = {
        "provider": "opencode",
        "captured_at": datetime.now(timezone.utc).isoformat(),
        "opencode_version": _opencode_version(skip_version=args.skip_version),
        "scenarios": manifest_scenarios,
    }
    manifest_path = output_dir / "opencode_trace_manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")

    print(f"Wrote {len(manifest_scenarios)} OpenCode trace fixtures to {output_dir}")


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # pragma: no cover - standalone utility script
        print(f"ERROR: {exc}", file=sys.stderr)
        sys.exit(1)
