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
    EVENTS.put({"type": kind, "properties": properties})


def _launch(session, request, prompt):
    with (HOME / "launched").open("a") as output:
        output.write(os.environ["LF_RUN_ID"] + "\n")
    assistant = "msg_" + uuid.uuid4().hex
    info = {"id": assistant, "sessionID": session, "role": "assistant", "parentID": request,
            "time": {"created": int(time.time() * 1000)}, "modelID": "fixture", "providerID": "opencode"}
    message = {"info": info, "parts": []}
    with LOCK:
        sessions[session].append(message)
        _save(sessions)
    _event("message.updated", info=info)
    fail = HOME / "fail-once"
    transient = HOME / "transient-once"
    if transient.exists() and "Decide the fixture." in str(prompt):
        transient.unlink()
        (HOME / "original-caller").write_text(os.environ["LF_AGENT_CALLER"])
        info["error"] = {"name": "APIError", "data": {"message": "fixture status 502"}}
    elif fail.exists():
        fail.unlink()
        info["error"] = {"name": "APIError", "data": {"message": "fixture failure"}}
    elif (HOME / "decide-enabled").exists():
        permission = "per_" + uuid.uuid4().hex
        reply = threading.Event()
        REPLIES[permission] = reply
        _event("permission.asked", sessionID=session, id=permission,
               tool={"messageID": assistant, "callID": "call_fixture"})
        if not reply.wait(15):
            info["error"] = {"name": "APIError", "data": {"message": "fixture permission unanswered"}}
        else:
            env = os.environ.copy()
            env.update(LF_HOME=env["LF_CONTROL_HOME"], LF_DB_PATH=env["LF_CONTROL_DB_PATH"])
            original = HOME / "original-caller"
            if original.exists():
                stale = env | {"LF_AGENT_CALLER": original.read_text()}
                rejected = subprocess.run([env["LF_BIN"], "flow", "decide", "advance", "Earlier request"], env=stale, capture_output=True, text=True, timeout=15)
                (HOME / "old-decision.log").write_text(rejected.stdout + rejected.stderr)
                original.unlink()
            output = subprocess.run([env["LF_BIN"], "flow", "decide", "advance", "Proof observed"],
                                    env=env, capture_output=True, text=True, timeout=15)
            with (HOME / "decide.log").open("a") as log:
                log.write(output.stdout + output.stderr)
            if (HOME / "disconnect-after-tool").exists():
                (HOME / "tool-effect").write_text("completed")
                EVENTS.put(None)
                return
    info.update(finish="stop", tokens={"input": 20, "output": 5, "reasoning": 0,
                                      "cache": {"read": 0, "write": 0}}, cost=0)
    info["time"]["completed"] = int(time.time() * 1000)
    message["parts"] = [{"id": "part_" + assistant, "sessionID": session,
                         "messageID": assistant, "type": "text", "text": "Fixture completed."}]
    with LOCK:
        _save(sessions)
    _event("message.updated", info=info)
    _event("session.status", sessionID=session, status={"type": "idle"})


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
        if self.path == "/event":
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
        elif self.path.startswith("/session/"):
            session = self.path.split("/")[2]
            with LOCK:
                if session not in sessions:
                    self._json({}, 404)
                else:
                    self._json(sessions[session] if self.path.endswith("/message") else {"id": session})
        else:
            self._json({})

    def do_PATCH(self):
        self.rfile.read(int(self.headers.get("Content-Length", 0)))
        self._json({})

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
        if self.path == "/session":
            session = "ses_" + uuid.uuid4().hex
            with LOCK:
                sessions[session] = []
                _save(sessions)
            self._json({"id": session})
        elif self.path.endswith("/prompt_async"):
            if evidence := os.environ.get("LF_TEST_REPLAY_EVIDENCE"):
                Path(evidence).write_text(json.dumps({
                    "input": os.environ["LF_RUN_ID"],
                    "parent": os.environ.get("LF_PARENT_RUN_ID"),
                    "request": body,
                }))
            self._json({})
            threading.Thread(target=_launch, args=(self.path.split("/")[2], body["messageID"], body["parts"]), daemon=True).start()
        elif self.path.startswith("/permission/"):
            assert body == {"reply": "once"}
            REPLIES[self.path.split("/")[2]].set()
            self._json({})
        else:
            self._json({})


if "--version" in sys.argv:
    raise SystemExit(0)
if "serve" not in sys.argv:
    print("message=created id=ses_" + os.environ["LF_RUN_ID"], file=sys.stderr)
    with (HOME / "launched").open("a") as output:
        output.write(os.environ["LF_RUN_ID"] + "\n")
    if __WAIT__:
        sys.stdin.readline()
    raise SystemExit(0)
sessions = json.loads(SESSIONS.read_text()) if SESSIONS.exists() else {}
ThreadingHTTPServer(("127.0.0.1", int(sys.argv[sys.argv.index("--port") + 1])), Server).serve_forever()
