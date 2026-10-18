"""Reproduce native attribution/recovery boundaries; passing does not prove a repair."""

import argparse
import json
import os
import shutil
import subprocess
import tempfile
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from probe_support import NativeServer, isolated_environment


class Model(ThreadingHTTPServer):
    def __init__(self, hold: int = 1, fail: int | None = None) -> None:
        super().__init__(("127.0.0.1", 0), ModelRequest)
        self.requests: list[dict] = []
        self.arrived = threading.Event()
        self.release = threading.Event()
        self.hold = hold
        self.fail = fail
        self.fail_all = False
        self.answer: str | None = None
        self.tool_commands = {1: "printf native-tool-proof"}

    def __enter__(self) -> "Model":
        threading.Thread(target=self.serve_forever, daemon=True).start()
        return self

    def __exit__(self, *args: object) -> None:
        self.release.set()
        self.shutdown()
        self.server_close()


class ModelRequest(BaseHTTPRequestHandler):
    def log_message(self, format: str, *args: object) -> None:
        pass

    def do_POST(self) -> None:
        model = self.server
        assert isinstance(model, Model)
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        model.requests.append(request)
        index = len(model.requests)
        if getattr(model, "tool_on_primary", False) and any(
            tool.get("function", {}).get("name") == "shell" for tool in request.get("tools", [])
        ):
            model.tool_commands[index] = "printf native-tool-proof"
            model.tool_on_primary = False
        if model.fail_all or index == model.fail:
            self.send_response(400)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(
                b'{"error":{"message":"deterministic model failure",'
                b'"type":"invalid_request_error"}}'
            )
            return
        delta = {"content": model.answer or f"answer-{index}"}
        finish = "stop"
        if index in model.tool_commands:
            delta = {
                "tool_calls": [
                    {
                        "index": 0,
                        "id": "call_probe" if index == 1 else f"call_probe_{index}",
                        "type": "function",
                        "function": {
                            "name": "shell",
                            "arguments": json.dumps(
                                {
                                    "command": model.tool_commands[index],
                                    "description": "Probe native tool",
                                }
                            ),
                        },
                    }
                ]
            }
            finish = "tool_calls"
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        for position, chunk in enumerate(
            [
                {
                    "choices": [
                        {
                            "index": 0,
                            "delta": {
                                "content": "probe-stream-start" if index == model.hold else ""
                            },
                            "finish_reason": None,
                        }
                    ]
                },
                {"choices": [{"index": 0, "delta": delta, "finish_reason": None}]},
                {
                    "choices": [{"index": 0, "delta": {}, "finish_reason": finish}],
                    "usage": {"prompt_tokens": 11, "completion_tokens": 7, "total_tokens": 18},
                },
            ]
        ):
            chunk.update(
                {
                    "id": f"chatcmpl-{index}",
                    "object": "chat.completion.chunk",
                    "created": 1,
                    "model": "probe",
                }
            )
            try:
                self.wfile.write(b"data: " + json.dumps(chunk).encode() + b"\n\n")
                self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                return
            if position == 0 and index == model.hold:
                model.arrived.set()
                assert model.release.wait(30), "probe did not release model"
        try:
            self.wfile.write(b"data: [DONE]\n\n")
        except (BrokenPipeError, ConnectionResetError):
            pass


class Native:
    def __init__(self, root: Path, model: Model, *, managed: bool = False) -> None:
        self.repo = root / "repo"
        self.repo.mkdir()
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True)
        environment = isolated_environment(root)
        environment["OPENCODE_CONFIG_CONTENT"] = json.dumps(
            {
                "providers": {
                    "probe": {
                        "package": "@ai-sdk/openai-compatible",
                        "settings": {"baseURL": f"http://127.0.0.1:{model.server_port}/v1"},
                        "models": {"probe": {"limit": {"context": 200000, "output": 1000}}},
                    }
                },
                "permissions": [{"action": "*", "resource": "*", "effect": "allow"}],
            }
        )
        executable = shutil.which("opencode")
        assert executable, "opencode is not installed"
        self.server = NativeServer(executable, self.repo, environment, managed=managed)

    def request(self, path: str, body: dict | None = None) -> object:
        request = urllib.request.Request(
            self.server.endpoint + path,
            data=json.dumps(body).encode() if body is not None else None,
            headers={
                "Authorization": self.server.authorization,
                "Content-Type": "application/json",
            },
        )
        with urllib.request.urlopen(request, timeout=20) as response:
            content = response.read()
            if "text/event-stream" in response.headers.get("Content-Type", ""):
                return [
                    json.loads(line[6:])
                    for line in content.splitlines()
                    if line.startswith(b"data: ")
                ]
            return json.loads(content) if content else None

    def log(self, session: str) -> list[dict]:
        return self.request(f"/api/experimental/session/{session}/log?follow=false")

    def history(self, session: str) -> dict:
        return self.request(f"/api/experimental/session/{session}/export")["data"]

    def wait_for_assistant(self, session: str, count: int) -> None:
        # Request arrival precedes persistence; interrupt only the intended native step.
        # Export omits unsettled messages, so use the live context for this barrier.
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            messages = self.request(f"/api/session/{session}/context")["data"]
            if sum(m["type"] == "assistant" for m in messages) >= count:
                return
            time.sleep(0.05)
        raise AssertionError("native assistant step was not persisted")

    def settle(self, session: str) -> dict:
        # Settlement is not completion: shutdown may lack idle; replay may emit empty success.
        self.request(f"/api/experimental/session/{session}/wait", {})
        return self.history(session)

    def create(self) -> str:
        return self.request(
            "/api/session",
            {
                "location": {"directory": str(self.repo)},
                "title": "Native attribution probe",
                "model": {"providerID": "probe", "id": "probe"},
            },
        )["data"]["id"]

    def prompt(self, session: str, text: str, **options: object) -> dict:
        return self.request(f"/api/session/{session}/prompt", {"text": text, **options})["data"]


def _scenario(name: str) -> dict:
    restarting = name in {
        "shutdown",
        "readmission",
        "synthetic_resume",
        "interrupt_resume",
        "repeated_readmission",
    }
    model = Model(hold=2 if restarting else 1, fail=2 if name == "failure" else None)
    with tempfile.TemporaryDirectory(prefix="loo504-native-") as directory, model:
        root = Path(directory).resolve()
        native = Native(root, model)
        with native.server:
            session = native.create()
            initial = native.prompt(session, "Initiate native tool turn", resume=False)
            before = native.history(session)
            assert not before["messages"], "admission alone produced execution evidence"
            # Re-submit the provider-confirmed ID to wake its existing admission.
            # The provider keeps the original payload and creates no duplicate input.
            native.prompt(session, "must not replace admitted text", id=initial["id"])
            assert model.arrived.wait(20), native.log(session)
            admissions = [initial]
            if name in {"steer", "queue"}:
                admissions.append(native.prompt(session, "MID_STEP_STEER", delivery="steer"))
            if name == "queue":
                admissions.append(native.prompt(session, "QUEUED_INPUT", delivery="queue"))
            interrupted = None
            if restarting:
                native.wait_for_assistant(session, 2)
                native.server.stop()
                native.server.start()
                interrupted = native.history(session)
                assert not any(m["type"] == "idle" for m in interrupted["messages"])
                assert len(model.requests) == 2
            if name == "synthetic_resume":
                native.request(
                    f"/api/session/{session}/synthetic",
                    {
                        "text": (
                            "The server restarted while you were working. "
                            "Continue from where you left off without repeating completed work."
                        ),
                        "description": "Continuing after restart",
                        "metadata": {"notice": "restart"},
                    },
                )
            if name == "readmission":
                native.prompt(session, "must not replace admitted text", id=initial["id"])
            model.release.set()
            recovery = []
            if name in {"interrupt_resume", "repeated_readmission"}:
                for attempt in range(3):
                    if attempt:
                        native.server.stop()
                        native.server.start()
                    if name == "interrupt_resume":
                        response = native.request(
                            f"/api/session/{session}/interrupt?resume=true", {}
                        )
                        assert response == {"interrupted": False}, response
                    else:
                        response = native.prompt(
                            session, "must not replace admitted text", id=initial["id"]
                        )
                        assert response["id"] == initial["id"]
                    snapshot = native.settle(session)
                    terminals = [m for m in snapshot["messages"] if m["type"] == "idle"]
                    assert len(model.requests) == 2, "recovery made a new model request"
                    assert len(terminals) == (0 if name == "interrupt_resume" else attempt + 1)
                    assert all(m["outcome"] == "succeeded" for m in terminals)
                    assert [m for m in snapshot["messages"] if m["type"] != "idle"] == (
                        interrupted["messages"]
                    ), "recovery changed the interrupted transcript"
                    assert snapshot["info"]["tokens"] == interrupted["info"]["tokens"]
                    recovery.append(
                        {
                            "attempt": attempt + 1,
                            "response": response,
                            "idle_markers": terminals,
                            "model_requests": len(model.requests),
                            "transcript_and_usage_unchanged": True,
                        }
                    )
            history = native.settle(session)
            replay = native.history(session)
            assert replay == history, "completed history changed when reread"
            users = [m for m in history["messages"] if m["type"] == "user"]
            assert [m["id"] for m in users] == [a["id"] for a in admissions]
            assert users[0]["text"] == initial["payload"]["text"]
            markers = [m for m in history["messages"] if m["type"] == "idle"]
            if name in {"shutdown", "interrupt_resume"}:
                assert not markers
                assert len(model.requests) == 2
            else:
                assert len(markers) == (3 if name == "repeated_readmission" else 1)
                assert markers[0]["outcome"] == ("failed" if name == "failure" else "succeeded")
            assistants = [m for m in history["messages"] if m["type"] == "assistant"]
            if name in {"readmission", "repeated_readmission", "interrupt_resume"}:
                assert len(model.requests) == 2, (
                    "readmission unexpectedly executed another model step"
                )
                assert assistants[-1].get("error", {}).get("type") == "aborted", (
                    name,
                    assistants[-1],
                )
            tools = [p for m in assistants for p in m["content"] if p["type"] == "tool"]
            assert len(tools) == 1
            assert tools[0]["state"]["content"] == [{"type": "text", "text": "native-tool-proof"}]
            measured = [m["tokens"] for m in assistants if "tokens" in m]
            for field in ("input", "output"):
                assert sum(t[field] for t in measured) == history["info"]["tokens"][field]
            if name in {"steer", "queue"}:
                assert "MID_STEP_STEER" in json.dumps(model.requests[1]["messages"])
            if name == "queue":
                assert "QUEUED_INPUT" not in json.dumps(model.requests[1]["messages"])
                assert "QUEUED_INPUT" in json.dumps(model.requests[2]["messages"])
            return {
                "admissions": admissions,
                "before_execution": before,
                "after_restart": interrupted,
                "history": history,
                "log": native.log(session),
                "model_requests": len(model.requests),
                "replay_unchanged": replay == history,
                "recovery_attempts": recovery,
            }


def main() -> None:
    assert os.environ.get("LOOPFLOW_NETWORK_ISOLATED"), "run through scripts/test_network.py"
    scenarios = [
        "steer",
        "queue",
        "failure",
        "shutdown",
        "readmission",
        "synthetic_resume",
        "interrupt_resume",
        "repeated_readmission",
    ]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scenario", action="append", choices=scenarios)
    arguments = parser.parse_args()
    report = {name: _scenario(name) for name in arguments.scenario or scenarios}
    # Drop disposable checkout locations; retain native IDs, timestamps and message order.
    for case in report.values():
        for key in ("before_execution", "after_restart", "history"):
            if case[key]:
                case[key]["info"].pop("location", None)
                case[key]["info"].pop("subpath", None)
    version = (
        subprocess.run(
            [shutil.which("opencode"), "--version"], capture_output=True, check=True, timeout=10
        )
        .stdout.decode()
        .strip()
    )
    print(json.dumps({"provider_version": version, "scenarios": report}, indent=2))


if __name__ == "__main__":
    main()
