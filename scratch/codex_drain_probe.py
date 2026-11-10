# /// script
# dependencies = ["websockets>=15,<16"]
# ///
import argparse
import fcntl
import http.server
import json
import os
import pathlib
import pty
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "tests/e2e"))
from codex_auth_reload_probe import _auth
from codex_connect import Client
from websockets.exceptions import ConnectionClosed

parser = argparse.ArgumentParser(
    description="Probe native Codex drain admission with no credentials or external inference."
)
parser.add_argument("--codex", required=True, type=pathlib.Path)
parser.add_argument("--output", required=True, type=pathlib.Path)
parser.add_argument("--approval", action="store_true")
parser.add_argument("--token-handoff", action="store_true")
parser.add_argument("--control", action="store_true")
parser.add_argument("--quota", action="store_true")
parser.add_argument("--refresh", action="store_true")
parser.add_argument("--refresh-request", type=int, default=2)
args = parser.parse_args()
if os.environ.get("LOOPFLOW_NETWORK_ISOLATED") != "1":
    raise RuntimeError("run through scripts/test_network.py")
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
codex = str(args.codex.resolve())
held = threading.Event()
release = threading.Event()
requests = []
request_accounts = []
refresh_requests = []
refreshed = threading.Event()
approval = args.approval


class RefreshClient(Client):
    def _read(self):
        try:
            for message in self.socket:
                event = json.loads(message)
                self.events.append(event)
                self.pending.put(event)
                if event.get("method") == "account/chatgptAuthTokens/refresh":
                    refresh_requests.append(event["params"])
                    # Let the native TUI answer first if this request is broadcast.
                    time.sleep(1)
                    self.socket.send(
                        json.dumps(
                            {
                                "id": event["id"],
                                "result": refresh_tokens,
                            }
                        )
                    )
                    refreshed.set()
        except ConnectionClosed:
            pass


class Model(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        payload = (
            {
                "accounts": [
                    {
                        "id": name,
                        "name": name,
                        "workspace_backend_origin": "https://fixture.example.test",
                        "account_routing_override": "NO_CONSTRAINT",
                    }
                    for name in ("first", "second")
                ]
            }
            if "/accounts/check" in self.path
            else {}
        )
        body = json.dumps(payload).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        if self.path != "/v1/responses":
            self.send_error(404)
            return
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        requests.append(request)
        request_accounts.append(self.headers.get("chatgpt-account-id"))
        if args.refresh and len(requests) == args.refresh_request:
            body = b'{"error":{"type":"authentication_error","message":"fixture expired"}}'
            self.send_response(401)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if len(requests) == 1 and args.quota:
            body = json.dumps(
                {
                    "error": {
                        "type": "usage_limit_reached",
                        "message": "Synthetic fixture usage limit",
                        "plan_type": "plus",
                        "resets_at": 4102444800,
                    }
                }
            ).encode()
            self.send_response(429)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if len(requests) == 1 and not approval:
            held.set()
            if not release.wait(30):
                return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        item = {
            "id": "msg_fixture",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "DRAIN_TURN_COMPLETED"}],
        }
        if len(requests) == 1 and approval:
            item = {
                "id": "fc_approval",
                "type": "function_call",
                "name": "exec_command",
                "call_id": "approval",
                "arguments": json.dumps(
                    {
                        "cmd": "/bin/echo APPROVAL_EXECUTED",
                        "sandbox_permissions": "require_escalated",
                        "justification": "Allow disposable recovery fixture?",
                    }
                ),
            }
        response = {"id": "resp_fixture", "status": "in_progress", "output": []}
        events = [
            ("response.created", {"response": response}),
            ("response.output_item.added", {"output_index": 0, "item": item}),
            ("response.output_item.done", {"output_index": 0, "item": item}),
            (
                "response.completed",
                {
                    "response": {
                        **response,
                        "status": "completed",
                        "output": [item],
                        "usage": {"input_tokens": 20, "output_tokens": 5, "total_tokens": 25},
                    }
                },
            ),
        ]
        for kind, data in events:
            self.wfile.write(
                f"event: {kind}\ndata: {json.dumps({'type': kind, **data})}\n\n".encode()
            )
            self.wfile.flush()


model = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Model)
worker = threading.Thread(target=model.serve_forever)
worker.start()
children = []
connections = []
master = None
transcript = bytearray()
results = {"case": "approval" if approval else "racing_input"}

with tempfile.TemporaryDirectory(prefix="loo811-drain-", dir="/tmp") as temporary:
    root = pathlib.Path(temporary)
    home = root / "codex"
    home.mkdir()
    (home / "config.toml").write_text(f"""cli_auth_credentials_store="file"
chatgpt_base_url="http://127.0.0.1:{model.server_port}/backend-api/"
model="fixture"
model_provider="fixture"
[model_providers.fixture]
name="fixture"
base_url="http://127.0.0.1:{model.server_port}/v1"
wire_api="responses"
requires_openai_auth={str(args.token_handoff).lower()}
[projects.{json.dumps(str(root))}]
trust_level="trusted"
""")
    if args.token_handoff:
        _auth(home, "first")
    refresh_tokens = (
        {
            "accessToken": json.loads(_auth(home, "second", "refreshed@example.test"))["tokens"][
                "access_token"
            ],
            "chatgptAccountId": "second",
            "chatgptPlanType": "plus",
        }
        if args.refresh
        else None
    )
    if args.refresh:
        _auth(home, "first")
    env = {
        "HOME": str(root),
        "CODEX_HOME": str(home),
        "PATH": "/usr/bin:/bin",
        "TERM": "xterm-256color",
        "RUST_LOG": "warn",
    }
    endpoint = root / "agent.sock"
    log = open(out / f"drain-{results['case']}-agent.log", "w")

    def server():
        process = subprocess.Popen(
            [codex, "app-server", "--listen", "unix://" + str(endpoint)],
            cwd=root,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=log,
            stderr=log,
            start_new_session=True,
        )
        children.append(process)
        deadline = time.monotonic() + 10
        while not endpoint.exists() and time.monotonic() < deadline and process.poll() is None:
            time.sleep(0.05)
        return process

    def stop(process):
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)

    def read(seconds):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if select.select([master], [], [], 0.05)[0]:
                try:
                    data = os.read(master, 65536)
                except OSError:
                    time.sleep(max(0, deadline - time.monotonic()))
                    return
                transcript.extend(data)
                if b"\x1b[6n" in data:
                    os.write(master, b"\x1b[1;1R")
            if client.poll() is not None:
                time.sleep(max(0, deadline - time.monotonic()))
                return

    try:
        provider = server()
        setup = RefreshClient(endpoint) if args.refresh else Client(endpoint)
        connections.append(setup)
        thread = setup.call(
            "thread/start",
            {
                "cwd": str(root),
                "model": "fixture",
                "modelProvider": "fixture",
                "approvalPolicy": "on-request" if approval else "never",
                "sandbox": "read-only" if approval else "danger-full-access",
            },
        )["thread"]["id"]
        setup.call("thread/name/set", {"threadId": thread, "name": "Drain probe"})
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))

        def own():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        client = subprocess.Popen(
            [codex, "--no-alt-screen", "--remote", "unix://" + str(endpoint), "resume", thread],
            cwd=root,
            env=env,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            preexec_fn=own,
        )
        children.append(client)
        os.close(slave)
        read(3)
        turn = setup.start_turn(thread, "Run the disposable fixture.")
        read(3)
        if approval:
            results["approval_before_drain"] = any(
                e.get("method") == "item/commandExecution/requestApproval" for e in setup.events
            )
        elif not args.quota:
            assert held.is_set(), "model never received admitted input"
        if args.token_handoff:
            if not approval:
                os.write(master, b"\x1b[200~DRAFT_BEFORE_HANDOFF\x1b[201~")
                read(0.5)
            if not args.control:
                saved = _auth(home, "second")
                results["token_handoff"] = setup.call(
                    "account/login/start",
                    {
                        "type": "chatgptAuthTokens",
                        "accessToken": json.loads(saved)["tokens"]["access_token"],
                        "chatgptAccountId": "second",
                        "chatgptPlanType": "plus",
                    },
                )
            results["account_after_handoff"] = setup.call("account/read", {})
            if args.quota:
                retry = setup.start_turn(thread, "Run the disposable fixture.")
                read(2)
                results["quota_retry_completed"] = any(
                    e.get("method") == "turn/completed"
                    and e["params"]["turn"]["id"] == retry
                    and e["params"]["turn"]["status"] == "completed"
                    for e in setup.events
                )
        else:
            provider.send_signal(signal.SIGHUP)
        read(1)
        results["provider_waits_for_admitted_turn"] = provider.poll() is None
        if provider.poll() is not None:
            raise RuntimeError("drain killed admitted turn")
        # This call observes the native rejection, never retries it.
        if not args.token_handoff:
            try:
                setup.start_turn(thread, "RPC_DURING_DRAIN")
            except AssertionError as error:
                results["new_turn_rejection"] = error.args[0].get("error")
        if approval:
            os.write(master, b"y")
            read(0.5)
            os.write(master, b"\r")
        else:
            os.write(master, b"\x1b[200~TYPED_DURING_DRAIN\x1b[201~")
            read(0.5)
            os.write(master, b"\r")
            read(1)
            release.set()
        read(5)
        results["client_survived_drain"] = client.poll() is None
        results["client_exit"] = client.poll()
        results["native_drain_error"] = b"turn/steer failed: Server is draining" in transcript
        results["old_provider_exited"] = provider.poll() is not None
        results["old_turn_completed"] = any(
            e.get("method") == "turn/completed"
            and e["params"]["turn"]["id"] == turn
            and e["params"]["turn"]["status"] == "completed"
            for e in setup.events
        )
        results["approval_executed"] = any(
            "APPROVAL_EXECUTED" in json.dumps(r.get("input", []))
            and "function_call_output" in json.dumps(r.get("input", []))
            for r in requests
        )
        if not args.token_handoff:
            if provider.poll() is None:
                raise RuntimeError("drain did not complete")
            endpoint.unlink(missing_ok=True)
            replacement = server()
        read(5)
        before = len(transcript)
        if client.poll() is None:
            os.write(master, b"\x1b[200~_AFTER_REPLACEMENT\x1b[201~")
            read(0.5)
            os.write(master, b"\r")
        read(8)
        results["client_survived"] = client.poll() is None
        results["typed_input_reached_model"] = any(
            "TYPED_DURING_DRAIN" in json.dumps(r) for r in requests
        )
        results["subsequent_input_reached_model"] = any(
            "_AFTER_REPLACEMENT" in json.dumps(r) for r in requests
        )
        results["subsequent_turn_completed"] = b"DRAIN_TURN_COMPLETED" in transcript[before:]
        results["local_fixture_requests"] = len(requests)
        results["turn_results"] = [
            {"id": e["params"]["turn"]["id"], "status": e["params"]["turn"]["status"]}
            for e in setup.events
            if e.get("method") == "turn/completed"
        ]
        results["original_turn_id"] = turn
        if args.token_handoff:
            results["request_accounts"] = request_accounts
            results["completed_commands"] = sum(
                e.get("method") == "item/completed"
                and e["params"]["item"].get("type") == "commandExecution"
                for e in setup.events
            )
            results["draft_reached_model"] = any(
                "DRAFT_BEFORE_HANDOFF" in json.dumps(r) for r in requests
            )
        results["version"] = subprocess.check_output(
            [codex, "--version"], env=env, cwd=root, text=True
        ).strip()
        if args.refresh:
            results["refresh_requests"] = refresh_requests
            results["refresh_answered"] = refreshed.is_set()
            results["account_after_refresh"] = setup.call("account/read", {})
        results["native_turns"] = setup.call(
            "thread/read", {"threadId": thread, "includeTurns": True}
        )
    finally:
        release.set()
        for connection in connections:
            connection.close()
        # Close the PTY before wait: native exit may block flushing an unread tty.
        if master is not None:
            os.close(master)
        for process in reversed(children):
            stop(process)
        log.close()
        model.shutdown()
        model.server_close()
        worker.join()
        results["all_children_reaped"] = all(p.poll() is not None for p in children)
        (out / f"drain-{results['case']}-terminal.bin").write_bytes(transcript)
        (out / f"drain-{results['case']}-results.json").write_text(json.dumps(results, indent=2))
        print(json.dumps({key: value for key, value in results.items() if key != "native_turns"}))

assert results["client_survived_drain"], "native client exited on drain rejection"
if args.quota:
    assert any(t["id"] == turn and t["status"] == "failed" for t in results["turn_results"])
    assert results["quota_retry_completed"]
else:
    assert results["old_turn_completed"], "admitted turn was not completed"
assert results["subsequent_turn_completed"], "subsequent typed turn did not complete"
if approval:
    assert results["approval_before_drain"] and results["approval_executed"]
else:
    assert results["typed_input_reached_model"], "input submitted during draining was lost"
if args.token_handoff:
    assert request_accounts[0] == "first"
    expected = "first" if args.control else "second"
    assert request_accounts[-1] == expected
    assert results["account_after_handoff"]["account"]["email"] == expected + "@example.test"
    if approval:
        assert results["completed_commands"] == 1
    else:
        assert results["draft_reached_model"]
if args.refresh:
    assert results["refresh_answered"]
    assert results["account_after_refresh"]["account"]["email"] == "refreshed@example.test"
