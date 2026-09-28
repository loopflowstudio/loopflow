# /// script
# requires-python = ">=3.10"
# dependencies = ["websockets>=15,<16"]
# ///
"""Exercise a real Codex engine with credential-free local Responses and private Homes."""

import argparse
import hashlib
import json
import os
import queue
import shlex
import shutil
import signal
import sqlite3
import subprocess
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from websockets.exceptions import ConnectionClosed
from websockets.sync.client import unix_connect


class Responses(ThreadingHTTPServer):
    def __init__(self) -> None:
        super().__init__(("127.0.0.1", 0), Handler)
        self.requests: list[dict] = []
        self.held = threading.Event()
        self.release = threading.Event()
        self.command = (
            'printf "caller=%s generation=%s\\n" "$LF_PROBE_SESSION" "$LF_PROBE_GENERATION"'
        )


class Handler(BaseHTTPRequestHandler):
    def log_message(self, format: str, *args: object) -> None:
        pass

    def do_POST(self) -> None:
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        self.server.requests.append(request)
        last_user = max(i for i, item in enumerate(request["input"]) if item.get("role") == "user")
        outputs = [
            item
            for item in request["input"][last_user:]
            if item.get("type") == "function_call_output"
        ]
        if not outputs and "held" in json.dumps(request["input"][last_user]):
            self.server.held.set()
            assert self.server.release.wait(20), "held response was not released"
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        if not outputs:
            item = {
                "type": "function_call",
                "id": "fc_provenance",
                "call_id": "call_provenance",
                "name": "exec_command",
                "arguments": json.dumps(
                    {
                        "cmd": self.server.command,
                        "max_output_tokens": 100,
                    }
                ),
            }
        else:
            item = {
                "type": "message",
                "id": "msg_done",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Fixture complete."}],
            }
        response = {"id": "resp_provenance", "status": "in_progress", "output": []}
        self._event("response.created", {"response": response})
        self._event("response.output_item.added", {"output_index": 0, "item": item})
        self._event("response.output_item.done", {"output_index": 0, "item": item})
        response.update(status="completed", output=[item])
        self._event("response.completed", {"response": response})

    def _event(self, kind: str, value: dict) -> None:
        data = json.dumps({"type": kind, **value})
        self.wfile.write(f"event: {kind}\ndata: {data}\n\n".encode())
        self.wfile.flush()


class Client:
    def __init__(self, path: Path) -> None:
        self.events: list[dict] = []
        self.pending: queue.Queue = queue.Queue()
        self.socket = unix_connect(str(path), uri="ws://localhost", open_timeout=10)
        self.next_id = 0
        self.reader = threading.Thread(target=self._read, daemon=True)
        self.reader.start()
        self.call(
            "initialize",
            {
                "clientInfo": {"name": "loopflow_connect_fixture", "version": "1"},
                "capabilities": {"experimentalApi": True},
            },
        )
        self.socket.send(json.dumps({"method": "initialized"}))

    def _read(self) -> None:
        try:
            for message in self.socket:
                event = json.loads(message)
                self.events.append(event)
                self.pending.put(event)
        except ConnectionClosed:
            pass

    def call(self, method: str, params: dict, *, rejected: bool = False) -> dict:
        self.next_id += 1
        ident = self.next_id
        self.socket.send(json.dumps({"id": ident, "method": method, "params": params}))
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            event = self.pending.get(timeout=max(0.1, deadline - time.monotonic()))
            if event.get("id") == ident:
                if rejected:
                    assert event.get("error", {}).get("code") == -32001, event
                    return event["error"]
                assert "error" not in event, event
                return event["result"]
        raise TimeoutError(method)

    def start_turn(self, thread: str, text: str = "Run the fixture command.") -> str:
        turn = self.call(
            "turn/start",
            {
                "threadId": thread,
                "input": [{"type": "text", "text": text, "text_elements": []}],
            },
        )["turn"]
        return turn["id"]

    def wait_turn(self, turn_id: str) -> dict:
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            event = self.pending.get(timeout=max(0.1, deadline - time.monotonic()))
            if event.get("method") == "turn/completed" and event["params"]["turn"]["id"] == turn_id:
                assert event["params"]["turn"]["status"] == "completed", event
                return event
        raise TimeoutError("turn/completed")

    def close(self) -> None:
        self.socket.close()
        self.reader.join(timeout=2)


def main() -> None:
    signal.signal(signal.SIGTERM, _terminate)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--lf", type=Path)
    parser.add_argument("--lf-home", type=Path)
    parser.add_argument("--control", type=Path)
    parser.add_argument("--retained-client", action="store_true")
    parser.add_argument("--gated", action="store_true")
    parser.add_argument("--launch", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    server = Responses()
    if args.lf and not args.launch:
        assert args.lf_home and args.control, (
            "lf ownership proof needs a private Home and control directory"
        )
        server.command = shlex.join([str(args.lf), "session", "list", "--all", "--json"])
    serving = threading.Thread(target=server.serve_forever, daemon=True)
    serving.start()
    results = {}
    clients = []
    try:
        with tempfile.TemporaryDirectory(prefix="lf-connect-", dir="/tmp") as directory:
            root = Path(directory)
            home, work = root / "codex", root / "work"
            home.mkdir()
            work.mkdir()
            (home / "config.toml").write_text(f"""model = "gpt-5.4"
model_provider = "fixture"
cli_auth_credentials_store = "file"
allow_login_shell = false
[features]
shell_snapshot = false
[model_providers.fixture]
name = "Local synthetic fixture"
base_url = "http://127.0.0.1:{server.server_port}/v1"
wire_api = "responses"
requires_openai_auth = false
[analytics]
enabled = false
[feedback]
enabled = false
""")
            # Never inherit credentials, execution authority, or the real provider Home.
            env = {key: os.environ[key] for key in ("PATH", "TMPDIR", "LANG") if key in os.environ}
            env.update(HOME=str(root), CODEX_HOME=str(home), LF_HOME=str(root / "lf"))
            if args.launch:
                assert args.lf
                # Pin bytes: another contributor may build the source path while
                # this private-Home proof is running.
                binary = root / "bin" / "lf"
                binary.parent.mkdir()
                shutil.copy2(args.lf, binary)
                env.update(
                    LF_BIN=str(binary),
                    LF_DB_PATH=str(root / "lf" / "loopflow.db"),
                    PATH=f"{binary.parent}:{args.codex.parent}:{env['PATH']}",
                )
                results["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
                server.command = shlex.join([str(binary), "session", "list", "--all", "--json"])
                _launch_contract(binary, work, env, results)
                return
            endpoint = root / "engine.sock"
            with (args.output / "engine.log").open("w") as log:
                engine = subprocess.Popen(
                    [str(args.codex), "app-server", "--listen", f"unix://{endpoint}"],
                    cwd=work,
                    env=env,
                    stdin=subprocess.DEVNULL,
                    stdout=log,
                    stderr=log,
                    start_new_session=True,
                )
                try:
                    deadline = time.monotonic() + 10
                    while (
                        not endpoint.exists()
                        and engine.poll() is None
                        and time.monotonic() < deadline
                    ):
                        time.sleep(0.05)
                    first = Client(endpoint)
                    clients.append(first)
                    params = {
                        "cwd": str(work),
                        "model": "gpt-5.4",
                        "modelProvider": "fixture",
                        "approvalPolicy": "never",
                        "sandbox": "danger-full-access",
                        "config": {
                            "shell_environment_policy.set": {
                                "LF_PROBE_SESSION": "conversation-a",
                                "LF_PROBE_GENERATION": "1",
                            }
                        },
                    }
                    if args.lf:
                        params["config"]["shell_environment_policy.set"].update(
                            {
                                "LF_HOME": str(args.lf_home),
                                "LF_DB_PATH": str(args.lf_home / "loopflow.db"),
                                "LF_AGENT_CALLER": (args.control / "caller.json").read_text(),
                            }
                        )
                    thread = first.call("thread/start", params)["thread"]["id"]
                    first.wait_turn(first.start_turn(thread))
                    if args.gated:
                        assert args.control
                        results.update(
                            _gated(args.control, endpoint, first, thread, params, server, clients)
                        )
                        results["engine_alive"] = engine.poll() is None
                        return
                    if args.control:
                        _boundary(args.control, "before", "handoff")
                    active_turn = first.start_turn(thread, "Run the held fixture command.")
                    assert server.held.wait(10), "engine did not request held response"
                    second = Client(endpoint)
                    clients.append(second)
                    resumed = second.call("thread/resume", {"threadId": thread})
                    assert resumed["thread"]["status"]["type"] == "active", resumed
                    if not args.retained_client:
                        first.close()
                    server.release.set()
                    second.wait_turn(active_turn)
                    if args.control:
                        _boundary(args.control, "after", "replace")
                        second.wait_turn(second.start_turn(thread))
                        results["replaced_provider_command_completed"] = True
                        results["active_turn_survived_disconnect"] = active_turn
                        results.update(thread=thread, engine_alive=engine.poll() is None)
                        return
                    params["config"]["shell_environment_policy.set"] = {
                        "LF_PROBE_SESSION": "conversation-b",
                        "LF_PROBE_GENERATION": "7",
                    }
                    sibling = second.call("thread/start", params)["thread"]["id"]
                    second.wait_turn(second.start_turn(sibling))
                    second.wait_turn(second.start_turn(thread))
                    if args.retained_client:
                        # Native attachment does not transfer Loopflow authority.
                        # Keep this counterexample separate from the gated proof.
                        first.wait_turn(first.start_turn(thread))
                        results["old_native_client_can_still_start_turns"] = True
                    outputs = [
                        item["output"]
                        for request in server.requests
                        for item in request["input"][-1:]
                        if item.get("type") == "function_call_output"
                    ]
                    expected = [
                        "conversation-a generation=1",
                        "conversation-a generation=1",
                        "conversation-b generation=7",
                        "conversation-a generation=1",
                    ]
                    if args.retained_client:
                        expected.append("conversation-a generation=1")
                    assert len(outputs) == len(expected), outputs
                    for output, caller in zip(outputs, expected):
                        assert f"caller={caller}" in output, output
                    results["command_provenance"] = expected
                    results["active_turn_survived_attachment"] = active_turn
                    results.update(thread=thread, engine_alive=engine.poll() is None)
                finally:
                    for client in clients:
                        client.close()
                    if engine.poll() is None:
                        os.killpg(engine.pid, signal.SIGTERM)
                        try:
                            engine.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            os.killpg(engine.pid, signal.SIGKILL)
                            engine.wait(timeout=5)
    finally:
        server.release.set()
        server.shutdown()
        server.server_close()
        (args.output / "requests.json").write_text(json.dumps(server.requests, indent=2))
        (args.output / "events.json").write_text(json.dumps([c.events for c in clients], indent=2))
        (args.output / "results.json").write_text(json.dumps(results, indent=2))
        print(json.dumps(results), flush=True)


def _terminate(signum: int, _frame: object) -> None:
    raise SystemExit(128 + signum)


def _command(args: list[str], work: Path, env: dict[str, str], timeout: int):
    with subprocess.Popen(
        args, cwd=work, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
    ) as child:
        try:
            stdout, stderr = child.communicate(timeout=timeout)
        except BaseException:
            # Give this exact fixture CLI a chance to stop its provider before
            # force-stopping it. Never signal a process found by name or ancestry.
            child.terminate()
            try:
                child.communicate(timeout=10)
            except subprocess.TimeoutExpired:
                child.kill()
                child.communicate()
            raise
        return subprocess.CompletedProcess(args, child.returncode, stdout, stderr)


def _launch_contract(binary: Path, work: Path, env: dict[str, str], results: dict) -> None:
    subprocess.run(["git", "init", "--quiet", str(work)], env=env, check=True)
    subprocess.run(
        [
            "git",
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.test",
            "commit",
            "--allow-empty",
            "--quiet",
            "-m",
            "fixture",
        ],
        cwd=work,
        env=env,
        check=True,
    )
    launch = _command(
        [str(binary), "-b", "--model", "codex", ":", "Run the fixture command."],
        work=work,
        env=env,
        timeout=60,
    )
    results["launch_exit"] = launch.returncode
    assert launch.returncode == 0, launch.stderr
    default = _command(
        [str(binary), "session", "list", "--all", "--json"],
        work=work,
        env=env,
        timeout=30,
    )
    assert default.returncode == 0, default.stderr
    assert json.loads(default.stdout) == [], (
        "headless work must not enter the default interactive view"
    )
    headless = _command(
        [str(binary), "session", "list", "--all", "--interactive", "false", "--json"],
        work=work,
        env=env,
        timeout=30,
    )
    results["headless_list_exit"] = headless.returncode
    assert headless.returncode == 0, headless.stderr
    sessions = json.loads(headless.stdout)
    assert len(sessions) == 1, sessions
    assert sessions[0]["interactive"] is False, sessions
    results["session_id"] = sessions[0]["id"]
    with sqlite3.connect(env["LF_DB_PATH"]) as database:
        children = database.execute(
            "SELECT id,parent_exec_id,caller_session_id,caller_provider_generation "
            "FROM execs WHERE via_agent=1"
        ).fetchall()
        assert len(children) == 1, children
        child, parent, session, generation = children[0]
        assert session == results["session_id"] and generation == 1, children
        creator = database.execute(
            "SELECT provider_exec_id,driver_exec_id FROM sessions WHERE id=?", (session,)
        ).fetchone()
        assert creator == (parent, None), (creator, children)
        assert database.execute("SELECT outcome FROM execs WHERE id=?", (parent,)).fetchone() == (
            "succeeded",
        )
        results["nested_agent_exec"] = {
            "child": child,
            "parent": parent,
            "session": session,
            "provider_generation": generation,
        }
    rename = _command(
        [
            str(binary),
            "session",
            "rename",
            sessions[0]["id"],
            "Retained headless conversation",
            "--json",
        ],
        work=work,
        env=env,
        timeout=30,
    )
    assert rename.returncode == 0, rename.stderr
    renamed = json.loads(rename.stdout)
    assert renamed["id"] == sessions[0]["id"]
    assert renamed["title"] == "Retained headless conversation"
    results["renamed_without_replacing_conversation"] = True


def _boundary(control: Path, completed: str, next_action: str) -> None:
    (control / f"{completed}.done").touch()
    deadline = time.monotonic() + 60
    while not (control / f"{next_action}.go").exists():
        if time.monotonic() > deadline:
            raise TimeoutError(next_action)
        time.sleep(0.05)


def _gated(
    control: Path,
    endpoint: Path,
    fixture: Client,
    thread: str,
    params: dict,
    server: Responses,
    clients: list[Client],
) -> dict:
    sibling_params = json.loads(json.dumps(params))
    sibling_params["config"]["shell_environment_policy.set"].pop("LF_AGENT_CALLER", None)
    sibling = fixture.call("thread/start", sibling_params)["thread"]["id"]
    active = fixture.start_turn(thread, "Run the held fixture command.")
    sibling_turn = fixture.start_turn(sibling, "Run the held sibling command.")
    assert server.held.wait(10)
    (control / "engine.json").write_text(json.dumps({"endpoint": str(endpoint), "thread": thread}))
    _boundary(control, "native", "sockets")
    old = Client(control / "old.sock")
    observer = Client(control / "observer.sock")
    clients.extend([old, observer])
    for client in (old, observer):
        assert (
            client.call("thread/resume", {"threadId": thread})["thread"]["status"]["type"]
            == "active"
        )
    observer.call("turn/interrupt", {"threadId": thread, "turnId": active}, rejected=True)
    old.call(
        "turn/steer",
        {
            "threadId": thread,
            "expectedTurnId": active,
            "input": [{"type": "text", "text": "Continue original driver.", "text_elements": []}],
        },
    )
    _boundary(control, "attached", "transfer")
    current = Client(control / "current.sock")
    clients.append(current)
    current.call("thread/resume", {"threadId": thread})
    writes = [
        (
            "turn/start",
            {"threadId": thread, "input": [{"type": "text", "text": "stale", "text_elements": []}]},
        ),
        (
            "turn/steer",
            {
                "threadId": thread,
                "expectedTurnId": active,
                "input": [{"type": "text", "text": "stale", "text_elements": []}],
            },
        ),
        ("turn/interrupt", {"threadId": thread, "turnId": active}),
        ("thread/name/set", {"threadId": thread, "name": "stale name"}),
    ]
    for method, arguments in writes:
        old.call(method, arguments, rejected=True)
    current.call(
        "turn/steer",
        {
            "threadId": thread,
            "expectedTurnId": active,
            "input": [{"type": "text", "text": "Continue the fixture.", "text_elements": []}],
        },
    )
    # The transferred conversation and its sibling have not been interrupted.
    for target in (thread, sibling):
        assert (
            fixture.call("thread/read", {"threadId": target})["thread"]["status"]["type"]
            == "active"
        )
    server.release.set()
    current.wait_turn(active)
    fixture.wait_turn(sibling_turn)
    old.wait_turn(active)
    current.wait_turn(current.start_turn(thread))
    assert not any("stale" in json.dumps(request) for request in server.requests)
    return {
        "old_client_wrote_before_transfer": True,
        "rejected_old_client_writes": [method for method, _ in writes],
        "passive_client_rejected": True,
        "old_client_still_receives_completion": True,
        "current_driver_continued": True,
        "active_turn": active,
        "sibling_turn": sibling_turn,
    }


if __name__ == "__main__":
    main()
