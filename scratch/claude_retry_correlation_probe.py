"""Native inbox correlation and hook-wake admission with isolated local Messages responses."""

import argparse
import fcntl
import json
import os
import pty
import select
import shlex
import socket
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time
import urllib.request
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


def _hook(base: str, rewake: bool = False) -> None:
    event = json.load(sys.stdin)
    payload = {
        "event": event,
        "inbox_available": bool(os.environ.get("CLAUDE_CODE_MESSAGING_SOCKET")),
    }
    request = urllib.request.Request(base + "/hook", data=json.dumps(payload).encode())
    with urllib.request.urlopen(request, timeout=3):
        pass
    if rewake:
        if event["hook_event_name"] != "StopFailure":
            return
        # Two barriers model credential preparation and the final wake dispatch.
        # No credentials are touched: admission must work before adding handoff.
        for phase in ("prepare", "admit"):
            deadline = time.monotonic() + 45
            while time.monotonic() < deadline:
                with urllib.request.urlopen(base + "/dispatch", timeout=3) as response:
                    ready = json.load(response).get(phase)
                if ready:
                    break
                time.sleep(0.05)
            else:
                raise TimeoutError(phase)
            with urllib.request.urlopen(
                urllib.request.Request(base + "/" + phase, data=b"{}"), timeout=3
            ):
                pass
        print("REWAKE_REJECTED_TURN_FIXTURE", file=sys.stderr)
        raise SystemExit(2)
    if event["hook_event_name"] != "SessionStart":
        return
    deadline = time.monotonic() + 45
    while time.monotonic() < deadline:
        with urllib.request.urlopen(base + "/dispatch", timeout=3) as response:
            dispatch = json.load(response)
        if dispatch:
            with socket.socket(socket.AF_UNIX) as inbox:
                inbox.settimeout(3)
                inbox.connect(os.environ["CLAUDE_CODE_MESSAGING_SOCKET"])
                wire = (
                    json.dumps({"type": "auth", "token": os.environ["CLAUDE_CODE_MESSAGING_TOKEN"]})
                    + "\n"
                )
                wire += (
                    json.dumps(
                        {
                            "type": "user",
                            "session_id": event["session_id"],
                            "uuid": dispatch["uuid"],
                            "priority": "next",
                            "from": "loopflow",
                            "message": {"role": "user", "content": "RETRY_CORRELATION_FIXTURE"},
                        }
                    )
                    + "\n"
                )
                inbox.sendall(wire.encode())
            with urllib.request.urlopen(
                urllib.request.Request(base + "/sent", data=b"{}"), timeout=3
            ):
                pass
            return
        time.sleep(0.1)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--claude", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--hook")
    parser.add_argument("--quota", action="store_true")
    parser.add_argument("--async-rewake", action="store_true")
    parser.add_argument("--race", choices=("none", "preparation", "dispatch"), default="none")
    args = parser.parse_args()
    if args.hook:
        _hook(args.hook, args.async_rewake)
        return
    if os.environ.get("LOOPFLOW_NETWORK_ISOLATED") != "1":
        raise RuntimeError("run through scripts/test_network.py")
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    hooks = []
    messages = []
    dispatch = {}
    sent = threading.Event()
    prepared = threading.Event()

    class Messages(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_GET(self):
            body = json.dumps(dispatch if self.path == "/dispatch" else {}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self):
            body = json.loads(
                self.rfile.read(int(self.headers.get("Content-Length", "0"))) or b"{}"
            )
            if self.path == "/hook":
                hooks.append(body)
            elif self.path == "/sent":
                sent.set()
            elif self.path == "/prepare":
                prepared.set()
            elif self.path == "/admit":
                sent.set()
            elif self.path.startswith("/v1/messages"):
                messages.append(body)
                if (args.quota and not args.async_rewake and not sent.is_set()) or (
                    args.async_rewake and len(messages) == 1
                ):
                    rejected = json.dumps(
                        {
                            "type": "error",
                            "error": {
                                "type": "rate_limit_error",
                                "message": "You've hit your limit",
                            },
                        }
                    ).encode()
                    self.send_response(429)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Content-Length", str(len(rejected)))
                    self.send_header("anthropic-ratelimit-unified-status", "rejected")
                    self.send_header("anthropic-ratelimit-unified-reset", "4102444800")
                    self.end_headers()
                    self.wfile.write(rejected)
                    return
                message = {
                    "id": "msg_fixture_" + str(len(messages)),
                    "type": "message",
                    "role": "assistant",
                    "model": body["model"],
                    "content": [],
                    "stop_reason": None,
                    "stop_sequence": None,
                    "usage": {"input_tokens": 10, "output_tokens": 1},
                }
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.end_headers()
                for kind, event in [
                    ("message_start", {"message": message}),
                    (
                        "content_block_start",
                        {"index": 0, "content_block": {"type": "text", "text": ""}},
                    ),
                    (
                        "content_block_delta",
                        {
                            "index": 0,
                            "delta": {"type": "text_delta", "text": "CORRELATION_COMPLETED"},
                        },
                    ),
                    ("content_block_stop", {"index": 0}),
                    (
                        "message_delta",
                        {
                            "delta": {"stop_reason": "end_turn", "stop_sequence": None},
                            "usage": {"output_tokens": 5},
                        },
                    ),
                    ("message_stop", {}),
                ]:
                    self.wfile.write(
                        f"event: {kind}\ndata: {json.dumps({'type': kind, **event})}\n\n".encode()
                    )
                    self.wfile.flush()
                return
            self.do_GET()

    server = ThreadingHTTPServer(("127.0.0.1", 0), Messages)
    worker = threading.Thread(target=server.serve_forever)
    worker.start()
    base = f"http://127.0.0.1:{server.server_port}"
    child = None
    master = None
    terminal = bytearray()
    results = {}
    try:
        with tempfile.TemporaryDirectory(prefix="loo811-claude-", dir="/tmp") as temporary:
            root = Path(temporary).resolve()
            native = root / ".claude"
            native.mkdir()
            config = {
                "hasCompletedOnboarding": True,
                "theme": "dark",
                "customApiKeyResponses": {
                    "approved": ["local-fixture-not-a-credential"],
                    "rejected": [],
                },
                "projects": {str(root): {"hasTrustDialogAccepted": True}},
            }
            (native / ".claude.json").write_text(json.dumps(config))
            command = shlex.join([sys.executable, str(Path(__file__).resolve()), "--hook", base])
            if args.async_rewake:
                command += " --async-rewake"
            settings = {
                "hooks": {
                    name: [
                        {
                            "hooks": [
                                {
                                    "type": "command",
                                    "command": command,
                                    **(
                                        {"asyncRewake": True, "timeout": 90}
                                        if args.async_rewake and name == "StopFailure"
                                        else {"async": True}
                                        if name == "SessionStart"
                                        else {}
                                    ),
                                }
                            ]
                        }
                    ]
                    for name in ("SessionStart", "UserPromptSubmit", "Stop", "StopFailure")
                }
            }
            (native / "settings.json").write_text(json.dumps(settings))
            env = {
                "HOME": str(root),
                "CLAUDE_CONFIG_DIR": str(native),
                "PATH": "/usr/bin:/bin",
                "TERM": "xterm-256color",
                "ANTHROPIC_API_KEY": "local-fixture-not-a-credential",
                "ANTHROPIC_BASE_URL": base,
                "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1",
                "DISABLE_TELEMETRY": "1",
                "CLAUDE_CODE_MAX_RETRIES": "0",
            }
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))

            def own():
                os.setsid()
                fcntl.ioctl(0, termios.TIOCSCTTY, 0)

            child = subprocess.Popen(
                [
                    str(args.claude.resolve()),
                    "--model",
                    "sonnet",
                    "--permission-mode",
                    "default",
                    "--strict-mcp-config",
                    "--mcp-config",
                    '{"mcpServers":{}}',
                ],
                cwd=root,
                env=env,
                stdin=slave,
                stdout=slave,
                stderr=slave,
                preexec_fn=own,
            )
            os.close(slave)

            def read(seconds):
                deadline = time.monotonic() + seconds
                while time.monotonic() < deadline:
                    if select.select([master], [], [], 0.05)[0]:
                        try:
                            data = os.read(master, 65536)
                        except OSError:
                            return
                        terminal.extend(data)
                        if b"\x1b[6n" in data:
                            os.write(master, b"\x1b[1;1R")

            def type_text(text):
                os.write(master, text.encode())
                read(0.3)

            def wait_for(predicate, label):
                deadline = time.monotonic() + 15
                while not predicate() and time.monotonic() < deadline:
                    read(0.1)
                assert predicate(), label

            def stopped():
                return sum(h["event"].get("hook_event_name") == "Stop" for h in hooks)

            def newer_input():
                type_text("NEWER_INPUT_" + args.race.upper())
                type_text("\r")
                wait_for(lambda: stopped() >= 1, "newer typed turn did not complete")

            read(4)
            if b"ANTHROPIC_API_KEY" in terminal:
                type_text("\x1b[A\r")
                read(2)
            type_text("FIRST_CORRELATION_FIXTURE")
            type_text("\r")
            read(4)
            if args.async_rewake:
                wait_for(
                    lambda: any(h["event"].get("hook_event_name") == "StopFailure" for h in hooks),
                    "rejection did not reach native failure hook",
                )
                if args.race == "preparation":
                    newer_input()
                dispatch["prepare"] = True
                wait_for(prepared.is_set, "failure hook did not reach final dispatch barrier")
                if args.race == "dispatch":
                    newer_input()
                before_wake = len(messages)
                stops_before = stopped()
                type_text("DRAFT_BEFORE_RETRY")
                dispatch["admit"] = True
                wait_for(sent.is_set, "failure hook did not exit")
                wait_for(lambda: stopped() > stops_before, "native hook wake did not complete")
                results.update(
                    mechanism="StopFailure.asyncRewake",
                    race=args.race,
                    requests_before_wake=before_wake,
                    requests_after_wake=len(messages),
                    newer_input_completed_before_wake=stops_before == 1,
                    stale_wake_after_newer_input=args.race != "none"
                    and len(messages) > before_wake,
                    wake_reached_model=any(
                        "REWAKE_REJECTED_TURN_FIXTURE" in json.dumps(m) for m in messages
                    ),
                )
            else:
                type_text("DRAFT_BEFORE_RETRY")
                dispatch["uuid"] = str(uuid.uuid4())
                read(6)
                results["dispatch_uuid"] = dispatch["uuid"]
                results["socket_write_completed"] = sent.is_set()
            type_text("_AFTER_RETRY")
            type_text("\r")
            read(4)
            results["client_alive"] = child.poll() is None
            results["hooks"] = hooks
            results["requests"] = [
                {"model": message["model"], "messages": message.get("messages")}
                for message in messages
            ]
            transcript = []
            for path in native.glob("projects/**/*.jsonl"):
                (out / path.name).write_bytes(path.read_bytes())
                transcript.extend(json.loads(line) for line in path.read_text().splitlines())
            users = [row for row in transcript if row.get("type") == "user"]
            # The inbox UUID identifies a transcript row; Stop identifies its prompt.
            # Require the fixture response too, so an unrelated Stop cannot pass.
            completed = {
                hook["event"]["prompt_id"]
                for hook in hooks
                if hook["event"].get("hook_event_name") == "Stop"
                and hook["event"].get("prompt_id")
                and hook["event"].get("last_assistant_message") == "CORRELATION_COMPLETED"
            }
            if not args.async_rewake:
                retry = [row for row in users if row.get("uuid") == dispatch["uuid"]]
                results["retry_prompt_id"] = retry[0].get("promptId") if len(retry) == 1 else None
                results["retry_stop_correlated"] = results["retry_prompt_id"] in completed
            drafts = [
                row
                for row in users
                if row.get("message", {}).get("content") == "DRAFT_BEFORE_RETRY_AFTER_RETRY"
            ]
            results["draft_submitted_intact"] = len(drafts) == 1
            results["subsequent_turn_completed"] = (
                len(drafts) == 1 and drafts[0].get("promptId") in completed
            )
            if args.quota or args.async_rewake:
                results["limit_hook"] = [
                    h["event"] for h in hooks if h["event"].get("hook_event_name") == "StopFailure"
                ]
                assert results["limit_hook"], "quota rejection never reached StopFailure"
            else:
                assert len(messages) == 3, "expected initial, inbox and subsequent local turns"
            required = ["client_alive", "draft_submitted_intact", "subsequent_turn_completed"]
            if args.async_rewake:
                results["version"] = subprocess.check_output(
                    [str(args.claude.resolve()), "--version"], env=env, cwd=root, text=True
                ).strip()
                required.append("wake_reached_model")
                if args.race != "none":
                    required.append("stale_wake_after_newer_input")
            else:
                required.append("retry_stop_correlated")
            assert all(results[key] for key in required), results
    finally:
        (out / "terminal.bin").write_bytes(terminal)
        if master is not None:
            os.close(master)
        if child is not None and child.poll() is None:
            # Only this disposable native client's process group is signalled.
            try:
                os.killpg(child.pid, 15)
            except (ProcessLookupError, PermissionError):
                child.terminate()
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, 9)
                child.wait(timeout=5)
        results["client_reaped"] = child is None or child.poll() is not None
        server.shutdown()
        server.server_close()
        worker.join(timeout=3)
        results["http_worker_reaped"] = not worker.is_alive()
        (out / "results.json").write_text(json.dumps(results, indent=2) + "\n")
        print(
            json.dumps(
                {key: value for key, value in results.items() if key not in ("hooks", "requests")}
            )
        )


if __name__ == "__main__":
    main()
