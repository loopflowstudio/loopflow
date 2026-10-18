#!/usr/bin/env python3
"""Disposable native OpenCode messages and permission ordering for CLI proofs."""
import json
import os
import queue
import subprocess
import sys
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HOME = Path(__file__).resolve().parent.parent
SESSIONS = HOME / "native-sessions.json"
EVENTS = queue.Queue()
REPLIES = {}
LOCK = threading.Lock()


def _save(sessions):
    SESSIONS.write_text(json.dumps(sessions))


def _event(kind, **properties):
    EVENTS.put({"type": kind, "data": properties})


def _record_launch():
    if evidence := os.environ.get("LF_TEST_SESSION_ENV"):
        Path(evidence).write_text(os.environ.get("OPENCODE_API_KEY", ""))
    if evidence := os.environ.get("LF_RESUME_PROOF"):
        Path(evidence).write_text(os.environ["LF_CAPTURE_KEY"])
        Path(evidence + ".caller").write_text(os.environ["LF_AGENT_CALLER"])
        Path(evidence + ".args").write_text("\n".join(sys.argv[1:]))
    config = json.loads(os.environ.get("OPENCODE_CONFIG_CONTENT", "{}"))
    if "serve" not in sys.argv:
        (HOME / "received.json").write_text(json.dumps({"args":sys.argv[1:], "cwd":os.getcwd(),
        "participant":os.environ.get("LF_USER_NAME"), "opencode":json.dumps(config),
        "model":json.loads((HOME / "native-model.json").read_text()) if (HOME / "native-model.json").exists() else None}))
    with (HOME / "launched").open("a") as output:
        output.write(os.environ["LF_CAPTURE_KEY"] + "\n")
    with (HOME / "declarations").open("a") as output:
        declaration = {"capture": os.environ["LF_CAPTURE_KEY"], "as": os.environ.get("LF_AS")}
        output.write(json.dumps(declaration) + "\n")
    tool = HOME / "tool-command.json"
    if tool.exists():
        command = json.loads(tool.read_text())
        tool.unlink()
        result = subprocess.run(
            command["argv"], cwd=command["cwd"], capture_output=True, timeout=60
        )
        (HOME / "tool-result.json").write_text(
            json.dumps({"code": result.returncode, "stderr": result.stderr.decode()})
        )


CONTRACT = "Return the final answer as the declared JSON value: "


def _contract(parts):
    """The answer schema a Flow process supplied in context, if any."""
    text = "".join(part.get("text", "") for part in parts)
    start = text.rfind(CONTRACT)
    if start < 0:
        return None
    return json.JSONDecoder().raw_decode(text[start + len(CONTRACT):])[0]


def _launch(session, prompt):
    _record_launch()
    deadline = time.monotonic() + 30
    while (HOME / "hold-turn").exists():
        if time.monotonic() >= deadline:
            raise TimeoutError("fixture turn was not released")
        time.sleep(0.01)
    assistant = "msg_" + uuid.uuid4().hex
    message = {"id": assistant, "type": "assistant",
            "time": {"created": int(time.time() * 1000)},
            "modelID": "fixture", "providerID": "opencode"}
    message["content"] = []
    with LOCK:
        sessions[session].append(message)
        _save(sessions)
    _event("session.step.started", sessionID=session)
    answer = "Fixture completed."
    # A retried turn carries no contract of its own; the conversation does.
    saved = HOME / f"contract-{session}.json"
    if schema := _contract(prompt):
        saved.write_text(json.dumps(schema))
    fail = HOME / "fail-once"
    transient = HOME / "transient-once"
    if transient.exists() and "Decide the fixture." in str(prompt):
        transient.unlink()
        answer = json.dumps({"decision": "iterate", "summary": "Failed turn cannot navigate"})
        message["error"] = {"name": "APIError", "data": {"message": "fixture status 502"}}
    elif fail.exists():
        fail.unlink()
        message["error"] = {"name": "APIError", "data": {"message": "fixture failure"}}
    elif (HOME / "decide-enabled").exists():
        permission = "per_" + uuid.uuid4().hex
        reply = threading.Event()
        REPLIES[permission] = reply
        _event("permission.asked", sessionID=session, id=permission,
               source={"messageID": assistant, "id": "call_fixture", "type": "tool"})
        if not reply.wait(15):
            message["error"] = {
                "name": "APIError", "data": {"message": "fixture permission unanswered"}
            }
        else:
            schema = json.loads(saved.read_text()) if saved.exists() else None
            if schema:
                assert schema["additionalProperties"] is False
                repeats = HOME / "remaining-passes"
                remaining = int(repeats.read_text()) if repeats.exists() else 0
                if "decision" in schema["properties"]:
                    decision = "iterate" if remaining else "advance"
                    value = {"decision": decision, "summary": "Proof observed", "reason": None}
                    blocked = HOME / "blocked-once"
                    if blocked.exists():
                        blocked.unlink()
                        value = {"decision": "blocked", "summary": None,
                                 "reason": "Release target is missing"}
                    invalid = HOME / "invalid-output-once"
                    if invalid.exists() or (HOME / "invalid-output-always").exists():
                        invalid.unlink(missing_ok=True)
                        value = {"decision": "unknown"}
                    elif remaining:
                        repeats.write_text(str(remaining - 1))
                else:
                    value = {"path": schema["properties"]["path"]["enum"][0]}
                with (HOME / "decide.log").open("a") as log:
                    log.write(json.dumps(value) + "\n")
                # A provider answers in prose; the Flow process finds the value in it.
                answer = "Decision:\n```json\n" + json.dumps(value) + "\n```"
            if (HOME / "disconnect-after-tool").exists():
                (HOME / "tool-effect").write_text("completed")
                EVENTS.put(None)
                return
    message.update(finish="stop", tokens={"input": 20, "output": 5, "reasoning": 0,
                                      "cache": {"read": 0, "write": 0}}, cost=0)
    message["time"]["completed"] = int(time.time() * 1000)
    message["content"] = [{"id": "part_" + assistant, "sessionID": session,
                         "messageID": assistant, "type": "text", "text": answer}]
    with LOCK:
        sessions[session].append({"id": "msg_" + uuid.uuid4().hex, "type": "idle",
            "outcome": "failed" if "error" in message else "succeeded", "error": message.get("error"),
            "time": {"created": int(time.time() * 1000)}})
        _save(sessions)
    _event("session.execution.failed" if "error" in message else "session.execution.succeeded", sessionID=session)


class Server(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def _json(self, value, status=200):
        data = json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path == "/api/event":
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            self.wfile.write(b'data: {"type":"server.connected"}\n\n')
            self.wfile.flush()
            while True:
                event = EVENTS.get()
                if event is None:
                    return
                self.wfile.write(("data: " + json.dumps(event) + "\n\n").encode())
                self.wfile.flush()
        elif self.path.startswith("/api/session/") or self.path.startswith("/api/experimental/session/"):
            session = self.path.split("/")[4 if "/experimental/" in self.path else 3]
            with LOCK:
                if session not in sessions:
                    self._json({}, 404)
                elif self.path.endswith("/export"):
                    self._json({"data": {"messages": sessions[session]}})
                else:
                    self._json({"data": sessions[session] if self.path.endswith("/context") else {"id": session}})
        else:
            self._json({})

    def do_PATCH(self):
        self.rfile.read(int(self.headers.get("Content-Length", 0)))
        self._json({})

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
        if self.path.endswith("/model"):
            (HOME / "native-model.json").write_text(json.dumps(body["model"]))
            self._json({})
        elif self.path == "/api/session":
            if "model" in body:
                (HOME / "native-model.json").write_text(json.dumps(body["model"]))
            session = "ses_" + uuid.uuid4().hex
            with LOCK:
                sessions[session] = []
                _save(sessions)
            self._json({"data": {"id": session}})
        elif self.path.endswith("/prompt"):
            if evidence := os.environ.get("LF_TEST_REPLAY_EVIDENCE"):
                Path(evidence).write_text(json.dumps({
                    "input": os.environ["LF_CAPTURE_KEY"],
                    "request": body, "config": json.loads(os.environ.get("OPENCODE_CONFIG_CONTENT", "{}")),
                    "instructions": [Path(p).read_text() for p in json.loads(os.environ.get("OPENCODE_CONFIG_CONTENT", "{}")).get("instructions", [])],
                }))
            session = self.path.split("/")[3]
            request = body["id"]
            with LOCK:
                sessions[session].append({"id": request, "type": "user", "text": body["text"]})
                _save(sessions)
            self._json({"data": {"id": request}})
            # The native context hook carries structured response instructions.
            config = json.loads(os.environ.get("OPENCODE_CONFIG_CONTENT", "{}"))
            instructions = ""
            for plugin in config.get("plugins", []):
                source = (Path(plugin.removeprefix("file://")) / "index.js").read_text()
                spec = json.JSONDecoder().raw_decode(source.split("const spec = ", 1)[1])[0]
                instructions += spec.get("instructions", "")
            threading.Thread(target=_launch, args=(session,
                [{"text": instructions}, {"text": body["text"]}]), daemon=True).start()
        elif "/permission/" in self.path:
            assert body == {"reply": "once"}
            REPLIES[self.path.split("/")[5]].set()
            self._json({})
        else:
            self._json({})


if "--version" in sys.argv:
    raise SystemExit(0)
if "serve" not in sys.argv:
    _record_launch()
    if __WAIT__:  # noqa: F821 — substituted by the Rust fixture before execution
        sys.stdin.readline()
    raise SystemExit(0)
sessions = json.loads(SESSIONS.read_text()) if SESSIONS.exists() else {}
server = ThreadingHTTPServer(("127.0.0.1", 0), Server)
print(json.dumps({"url": f"http://127.0.0.1:{server.server_port}"}), flush=True)
server.serve_forever()
