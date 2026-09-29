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
import sys
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
        self.fail = False
        self.transient = False
        self.fail_request: int | None = None
        self.commands: dict[int, str] = {}
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
        if self.server.fail or len(self.server.requests) == self.server.fail_request:
            transient = self.server.transient
            if transient:
                self.server.fail = False
            self.send_response(400)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(
                json.dumps(
                    {
                        "error": {
                            "type": "invalid_request_error",
                            "code": "fixture_transient" if transient else "context_length_exceeded",
                            "message": "Service temporarily unavailable; retry this turn"
                            if transient
                            else "Controlled Flow retry failure",
                        }
                    }
                ).encode()
            )
            return
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
                        "cmd": "printf sibling-only"
                        if "held sibling" in json.dumps(request["input"][last_user])
                        else self.server.commands.get(
                            len(self.server.requests), self.server.command
                        ),
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
        response.update(
            status="completed",
            output=[item],
            usage={
                "input_tokens": 20,
                "output_tokens": 5,
                "total_tokens": 25,
                "input_tokens_details": {"cached_tokens": 0},
                "output_tokens_details": {"reasoning_tokens": 0},
            },
        )
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
    parser.add_argument("--public-connect", action="store_true")
    parser.add_argument("--live-handoff", action="store_true")
    parser.add_argument("--flow-retry", action="store_true")
    parser.add_argument("--flow-engine-loss", action="store_true")
    parser.add_argument("--flow-automatic-retry", action="store_true")
    parser.add_argument("--flow-decision-retry", choices=("missing", "replace", "late"))
    parser.add_argument("--flow-driver-loss", choices=("running", "completed", "both"))
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
                engines = root / "engines"
                engines.mkdir()
                shim = binary.parent / "codex"
                shim.write_text(
                    f"#!{sys.executable}\nimport runpy\n"
                    f"runpy.run_path({str(Path(__file__).resolve())!r}, "
                    "run_name='fixture')['_provider_entry']()\n"
                )
                shim.chmod(0o700)
                env.update(
                    LF_BIN=str(binary),
                    LF_DB_PATH=str(root / "lf" / "loopflow.db"),
                    PATH=f"{binary.parent}:{args.codex.parent}:{env['PATH']}",
                    LF_PROBE_CODEX=str(args.codex),
                    LF_PROBE_ENGINES=str(engines),
                )
                results["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
                server.command = (
                    'printf "caller-present=%s home=%s database=%s\\n" '
                    '"${LF_AGENT_CALLER:+yes}" "$LF_HOME" "$LF_DB_PATH"\n'
                    "ps -p $$ -o etime= 2>&1\n"
                    + "RUST_LOG=loopflow::journal=debug "
                    + shlex.join([str(binary), "session", "list", "--all", "--json"])
                )
                try:
                    if args.flow_decision_retry:
                        _flow_decision_retry_contract(
                            binary,
                            work,
                            env,
                            server,
                            results,
                            replace=args.flow_decision_retry == "replace",
                            late=args.flow_decision_retry == "late",
                        )
                        return
                    if args.flow_automatic_retry:
                        _flow_automatic_retry_contract(binary, work, env, server, results)
                        return
                    if args.flow_driver_loss:
                        _flow_driver_loss_contract(
                            binary,
                            work,
                            env,
                            server,
                            results,
                            completed_first=args.flow_driver_loss == "completed",
                            replace_engine=args.flow_driver_loss == "both",
                        )
                        return
                    if args.flow_retry or args.flow_engine_loss:
                        _flow_retry_contract(
                            binary,
                            work,
                            env,
                            server,
                            results,
                            replace_engine=args.flow_engine_loss,
                        )
                        return
                    _launch_contract(binary, work, env, results)
                    if args.public_connect:
                        _public_connection_contract(binary, work, env, server, results)
                    if args.live_handoff:
                        results["live_handoff"] = _live_driver_contract(binary, work, env, server)
                finally:
                    _stop_fixture_engines(engines)
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


def _provider_entry() -> None:
    if "app-server" in sys.argv:
        pid = os.getpid()
        stamp = subprocess.check_output(["ps", "-p", str(pid), "-o", "lstart="], text=True).strip()
        (Path(os.environ["LF_PROBE_ENGINES"]) / f"{pid}.json").write_text(json.dumps([pid, stamp]))
        os.execv(os.environ["LF_PROBE_CODEX"], [os.environ["LF_PROBE_CODEX"], *sys.argv[1:]])
    if "--remote" not in sys.argv:
        os.execv(os.environ["LF_PROBE_CODEX"], [os.environ["LF_PROBE_CODEX"], *sys.argv[1:]])
    # A controlled native-protocol client exercises public lf connect. This is
    # deliberately not evidence of the rendered Codex TUI or Desktop.
    remote = sys.argv[sys.argv.index("--remote") + 1].removeprefix("unix://")
    thread = sys.argv[-1]
    control = Path(os.environ["LF_PROBE_CLIENT"])
    client = Client(Path(remote))
    try:
        client.call("thread/resume", {"threadId": thread})
        (control / "ready").touch()
        index = 0
        while True:
            if not client.reader.is_alive():
                return
            request = control / f"{index}.request"
            if not request.exists():
                time.sleep(0.02)
                continue
            action = json.loads(request.read_text())
            if action["method"] == "exit":
                return
            value = client.call(
                action["method"], action["params"], rejected=action.get("rejected", False)
            )
            (control / f"{index}.response").write_text(json.dumps(value))
            index += 1
    finally:
        client.close()


def _stop_fixture_engines(directory: Path) -> None:
    for record in directory.glob("*.json"):
        pid, stamp = json.loads(record.read_text())
        observed = subprocess.run(
            ["ps", "-p", str(pid), "-o", "lstart="], capture_output=True, text=True
        )
        if observed.returncode == 0 and observed.stdout.strip() == stamp:
            assert os.getpgid(pid) == pid, "fixture engine must own its process group"
            os.killpg(pid, signal.SIGTERM)
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                current = subprocess.run(
                    ["ps", "-p", str(pid), "-o", "lstart=", "-o", "stat="],
                    capture_output=True,
                    text=True,
                )
                if (
                    current.returncode != 0
                    or not current.stdout.strip().startswith(stamp)
                    or "Z" in current.stdout.split()[-1]
                ):
                    break
                time.sleep(0.02)
            else:
                assert os.getpgid(pid) == pid
                os.killpg(pid, signal.SIGKILL)
                raise AssertionError(f"fixture engine {pid} required force termination")


def _public_connection_contract(
    binary: Path,
    work: Path,
    env: dict[str, str],
    server: Responses,
    results: dict,
    headless: subprocess.Popen | None = None,
) -> None:
    session = results["session_id"]
    processes = []
    controls = []
    inspectors = []

    def wait(path: Path) -> None:
        deadline = time.monotonic() + 15
        while not path.exists():
            assert time.monotonic() < deadline, f"timed out: {path}"
            for child in processes:
                assert child.poll() is None, f"connect exited: {child.returncode}"
            time.sleep(0.02)

    def call(index: int, sequence: int, method: str, params: dict, rejected: bool = False):
        root = controls[index]
        (root / f"{sequence}.request").write_text(
            json.dumps(dict(method=method, params=params, rejected=rejected))
        )
        output = root / f"{sequence}.response"
        wait(output)
        return json.loads(output.read_text())

    try:
        with sqlite3.connect(env["LF_DB_PATH"]) as database:
            endpoint, thread, generation = database.execute(
                "SELECT provider_endpoint,provider_thread,provider_generation "
                "FROM agent_sessions WHERE id=?",
                (session,),
            ).fetchone()
        engine = Client(Path(endpoint))
        inspectors.append(engine)
        engine.call("thread/resume", {"threadId": thread})
        sibling = engine.call("thread/start", {"cwd": str(work), "approvalPolicy": "never"})[
            "thread"
        ]["id"]
        engine.start_turn(sibling, "held sibling")
        assert server.held.wait(10)
        for label in ["first", "second"]:
            control = work.parent / f"{session}-{label}"
            control.mkdir()
            controls.append(control)
            child = subprocess.Popen(
                [str(binary), "session", "connect", session],
                cwd=work,
                env={**env, "LF_PROBE_CLIENT": str(control)},
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
            )
            processes.append(child)
            wait(control / "ready")
            if label == "first":
                if headless is None:
                    active = call(
                        0,
                        0,
                        "turn/start",
                        {
                            "threadId": thread,
                            "input": [{"type": "text", "text": "held conversation"}],
                        },
                    )["turn"]["id"]
                else:
                    call(0, 0, "thread/read", {"threadId": thread})
                    with sqlite3.connect(env["LF_DB_PATH"]) as database:
                        active = database.execute(
                            "SELECT provider_turn FROM session_events WHERE session_id=? "
                            "AND kind='started' ORDER BY seq DESC LIMIT 1",
                            (session,),
                        ).fetchone()[0]
        if headless is not None:
            headless.send_signal(signal.SIGINT)
            headless.communicate(timeout=10)
            assert headless.returncode == 130, headless.returncode
            with sqlite3.connect(env["LF_DB_PATH"]) as database:
                outcome = database.execute(
                    "SELECT outcome FROM execs "
                    "WHERE id=(SELECT provider_exec_id FROM agent_sessions WHERE id=?)",
                    (session,),
                ).fetchone()[0]
            assert outcome == "interrupted", outcome
            results["old_headless_driver"] = dict(exit_code=130, outcome=outcome)
        assert (
            engine.call("thread/read", {"threadId": thread})["thread"]["status"]["type"] == "active"
        )
        call(0, 1, "turn/interrupt", {"threadId": thread, "turnId": active}, rejected=True)
        call(
            0,
            2,
            "turn/start",
            {"threadId": thread, "input": [{"type": "text", "text": "stale turn"}]},
            rejected=True,
        )
        call(
            0,
            3,
            "turn/steer",
            {
                "threadId": thread,
                "expectedTurnId": active,
                "input": [{"type": "text", "text": "stale steer"}],
            },
            rejected=True,
        )
        assert (
            engine.call("thread/read", {"threadId": thread})["thread"]["status"]["type"] == "active"
        )
        assert (
            engine.call("thread/read", {"threadId": sibling})["thread"]["status"]["type"]
            == "active"
        )
        with sqlite3.connect(env["LF_DB_PATH"]) as database:
            driver, observed_generation, interactive = database.execute(
                "SELECT driver_exec_id,provider_generation,interactive "
                "FROM agent_sessions WHERE id=?",
                (session,),
            ).fetchone()
            before = {row[0] for row in database.execute("SELECT id FROM execs")}
        assert observed_generation == generation and interactive == 1
        server.release.set()
        engine.wait_turn(active)
        with sqlite3.connect(env["LF_DB_PATH"]) as database:
            after = database.execute(
                "SELECT id,parent_exec_id,caller_session_id,caller_provider_generation "
                "FROM execs WHERE via_agent=1"
            ).fetchall()
        children = [row for row in after if row[0] not in before]
        assert len(children) == 1, children
        child_id, child_parent, child_session, child_generation = children[0]
        assert (child_parent, child_session, child_generation) == (driver, session, generation), (
            children
        )
        call(1, 0, "thread/read", {"threadId": thread, "includeTurns": True})
        history_command = [str(binary), "session", "history", session, "--json", "--limit", "0"]
        recorded = _command(history_command, work, env, timeout=15)
        assert recorded.returncode == 0, recorded.stderr
        history = json.loads(recorded.stdout)
        completions = [event for event in history if event["kind"] == "completed"]
        assert len(completions) == (2 if headless is None else 1), history
        selected = [event for event in completions if event["provider_turn"] == active]
        assert len(selected) == 1 and selected[0]["payload"]["status"] == "completed", selected
        usage = [
            event
            for event in history
            if event["kind"] == "usage" and event["provider_turn"] == active
        ]
        assert usage and usage[-1]["payload"]["total"]["inputTokens"] > 0, history
        call(1, 1, "thread/read", {"threadId": thread, "includeTurns": True})
        replay = _command(history_command, work, env, timeout=15)
        assert replay.returncode == 0 and json.loads(replay.stdout) == history, replay
        results["history"] = history
        if headless is None:
            # Every lf receiver exits before this turn completes.
            server.held.clear()
            server.release.clear()
            orphan_turn = call(
                1,
                2,
                "turn/start",
                {
                    "threadId": thread,
                    "input": [{"type": "text", "text": "held receiver departure"}],
                },
            )["turn"]["id"]
            assert server.held.wait(10), "provider did not reach held third turn"
            with sqlite3.connect(env["LF_DB_PATH"]) as database:
                original_driver = database.execute(
                    "SELECT driver_exec_id FROM agent_sessions WHERE id=?", (session,)
                ).fetchone()[0]
            departed = []
            for child in processes:
                child.terminate()
                stdout, stderr = child.communicate(timeout=10)
                departed.append(
                    {
                        "pid": child.pid,
                        "exit": child.returncode,
                        "stderr": stderr.decode() if isinstance(stderr, bytes) else stderr,
                    }
                )
            processes.clear()
            server.release.set()
            native_completion = engine.wait_turn(orphan_turn)
            with sqlite3.connect(env["LF_DB_PATH"]) as database:
                absent = database.execute(
                    "SELECT count(*) FROM session_events "
                    "WHERE session_id=? AND provider_turn=? AND kind='completed'",
                    (session, orphan_turn),
                ).fetchone()[0]
                absent_usage = database.execute(
                    "SELECT count(*) FROM session_events "
                    "WHERE session_id=? AND provider_turn=? AND kind='usage'",
                    (session, orphan_turn),
                ).fetchone()[0]
                driver_outcome = database.execute(
                    "SELECT outcome,exit_code FROM execs WHERE id=?", (original_driver,)
                ).fetchone()
            results["driverless_before_reconnect"] = {
                "turn": orphan_turn,
                "driver": original_driver,
                "driver_outcome": driver_outcome,
                "departed": departed,
                "recorded_completions": absent,
                "recorded_usage": absent_usage,
                "native_status": native_completion["params"]["turn"]["status"],
            }
            assert absent == 0 and absent_usage == 0, "a Loopflow receiver unexpectedly remained"
            control = work.parent / f"{session}-recovery"
            control.mkdir()
            controls.append(control)
            resumed = subprocess.Popen(
                [str(binary), "session", "connect", session],
                cwd=work,
                env={**env, "LF_PROBE_CLIENT": str(control)},
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
            )
            processes.append(resumed)
            wait(control / "ready")
            recovered_command = _command(history_command, work, env, timeout=15)
            assert recovered_command.returncode == 0, recovered_command.stderr
            recovered = json.loads(recovered_command.stdout)
            recovered_completion = [
                e
                for e in recovered
                if e["kind"] == "completed" and e["provider_turn"] == orphan_turn
            ]
            assert (
                len(recovered_completion) == 1
                and recovered_completion[0]["payload"]["status"] == "completed"
            ), recovered
            assert [e for e in recovered if e["seq"] <= history[-1]["seq"]] == history, (
                "earlier history changed"
            )
            call(2, 0, "thread/read", {"threadId": thread, "includeTurns": True})
            replayed = _command(history_command, work, env, timeout=15)
            assert replayed.returncode == 0 and json.loads(replayed.stdout) == recovered, replayed
            with sqlite3.connect(env["LF_DB_PATH"]) as database:
                retained_outcome = database.execute(
                    "SELECT outcome,exit_code FROM execs WHERE id=?", (original_driver,)
                ).fetchone()
            assert retained_outcome == driver_outcome
            missing_usage = [
                e for e in recovered if e["kind"] == "usage" and e["provider_turn"] == orphan_turn
            ]
            results["driverless_recovery"] = {
                "passed": True,
                "completion": recovered_completion[0],
                "usage_events": missing_usage,
                "driver_outcome_unchanged": retained_outcome,
                "earlier_history_unchanged": True,
                "replay_unchanged": True,
            }
            assert missing_usage, "native resume did not recover known usage"
            assert missing_usage[-1]["payload"]["total"]["inputTokens"] == 120, missing_usage
            assert missing_usage[-1]["payload"]["total"]["outputTokens"] == 30, missing_usage
            starts = [
                e for e in recovered if e["kind"] == "started" and e["provider_turn"] == orphan_turn
            ]
            assert len(starts) == 1 and starts[0]["exec_id"] == original_driver, starts
            results["driverless_recovery"]["origin"] = starts[0]
            results["driverless_recovery"]["whole_history"] = recovered
            usage_command = _command(
                [str(binary), "usage", "--json", "--days", "0"], work, env, timeout=15
            )
            assert usage_command.returncode == 0, usage_command.stderr
            with sqlite3.connect(env["LF_DB_PATH"]) as database:
                input_id = database.execute(
                    "SELECT input_id FROM agent_sessions WHERE id=?", (session,)
                ).fetchone()[0]
            summaries = [row for row in json.loads(usage_command.stdout) if row["id"] == input_id]
            assert len(summaries) == 1, summaries
            summary = summaries[0]["usage"]
            results["driverless_recovery"]["public_usage"] = summary
            assert summary["input_tokens"] == 120 and summary["output_tokens"] == 30, summary
            assert summary["streams"] == 3 and summary["final_streams"] == 1, summary
            assert summary["cost_usd"] is None, summary
        results["public_connect"] = dict(
            provider_generation=generation,
            driver=driver,
            retained_client_rejected=True,
            active_turn_and_sibling_survived=True,
            nested_child_id=child_id,
            nested_child_parent=child_parent,
            nested_child_session=child_session,
            native_client="controlled protocol fixture",
        )
    finally:
        server.release.set()
        for client in inspectors:
            client.close()
        for child in processes:
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait(timeout=5)


def _live_driver_contract(binary: Path, work: Path, env: dict[str, str], server: Responses) -> dict:
    with sqlite3.connect(env["LF_DB_PATH"]) as database:
        existing = {row[0] for row in database.execute("SELECT id FROM agent_sessions")}
    server.held.clear()
    server.release.clear()
    child = subprocess.Popen(
        [str(binary), "-b", "--model", "codex", ":", "held conversation"],
        cwd=work,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        assert server.held.wait(15), "headless provider never reached held upstream"
        with sqlite3.connect(env["LF_DB_PATH"]) as database:
            sessions = [
                row[0]
                for row in database.execute("SELECT id FROM agent_sessions")
                if row[0] not in existing
            ]
        assert len(sessions) == 1, sessions
        result = {"session_id": sessions[0]}
        _public_connection_contract(binary, work, env, server, result, headless=child)
        return result
    finally:
        server.release.set()
        if child.poll() is None:
            child.terminate()
            try:
                child.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.communicate(timeout=5)


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


def _init_repo(work: Path, env: dict[str, str]) -> None:
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


def _flow_decision_retry_contract(
    binary: Path,
    work: Path,
    env: dict[str, str],
    server: Responses,
    results: dict,
    *,
    replace: bool,
    late: bool,
) -> None:
    _init_repo(work, env)
    for directory in ("skills", "flows"):
        (work / ".lf" / directory).mkdir(parents=True, exist_ok=True)
    (work / ".lf/skills/native-proof.md").write_text("Run the fixture command.")
    (work / ".lf/flows/native-proof.yaml").write_text(
        "- step:\n    id: work\n    name: native-proof\n"
        "- step:\n    id: decide\n    name: native-proof\n    repeat:\n      from: work\n"
    )
    server.transient = True
    server.fail_request = 4
    decision = [
        "env",
        "LF_HOME=" + env["LF_HOME"],
        "LF_DB_PATH=" + env["LF_DB_PATH"],
        str(binary),
        "flow",
        "decide",
    ]
    server.commands[3] = shlex.join(
        decision + ["iterate" if replace else "advance", "decision from failed native turn"]
    )
    server.commands[5] = (
        shlex.join(decision + ["advance", "retry decision"]) if replace else "printf no-decision"
    )
    if late:
        child = work / "delayed-decision.py"
        child.write_text(
            "import json, os, subprocess, sys, time\nfrom pathlib import Path\n"
            "root = Path(__file__).parent\n"
            "def wait(name, seconds):\n"
            "    deadline = time.monotonic() + seconds\n"
            "    while not (root / name).exists():\n"
            "        assert time.monotonic() < deadline, name\n"
            "        time.sleep(0.05)\n"
            "if sys.argv[1] == 'launch':\n"
            "    with (root / 'child.log').open('w') as log:\n"
            "        subprocess.Popen([sys.executable, __file__, 'child'], "
            "stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True)\n"
            "    wait('child-ready', 5)\n"
            "elif sys.argv[1] == 'child':\n"
            "    (root / 'child-ready').touch()\n"
            "    wait('retry-started', 25)\n"
            f"    result = subprocess.run({decision + ['advance', 'late failed-turn decision']!r}, "
            "capture_output=True, text=True, timeout=25)\n"
            "    (root / 'late-result.json').write_text(json.dumps({"
            "'exit': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr}))\n"
            "else:\n"
            "    (root / 'retry-started').touch()\n"
            "    wait('late-result.json', 30)\n"
            "    print('retry completed without a decision')\n"
        )
        server.commands[3] = shlex.join([sys.executable, str(child), "launch"])
        server.commands[5] = shlex.join([sys.executable, str(child), "release"])
    command = _command(
        [str(binary), "--model", "codex", "flow", "native-proof", "-b", "--no-loopflow"],
        work=work,
        env=env,
        timeout=90,
    )
    results.update(command_exit=command.returncode, command_stderr=command.stderr)
    with sqlite3.connect(env["LF_DB_PATH"]) as db:
        results["flow"] = db.execute(
            "SELECT state,failure_json,review_json FROM flow_sessions"
        ).fetchone()
        results["history"] = db.execute(
            "SELECT seq,kind,provider_turn,payload FROM session_events ORDER BY seq"
        ).fetchall()
        results["flow_events"] = db.execute(
            "SELECT kind,session_event FROM flow_events ORDER BY seq"
        ).fetchall()
    outputs = [
        item["output"]
        for request in server.requests
        for item in request["input"]
        if item.get("type") == "function_call_output"
    ]
    results["tool_outputs"] = outputs
    if late:
        results["late_result"] = json.loads((work / "late-result.json").read_text())
        assert results["late_result"]["exit"] != 0, (
            "Flow accepted a failed turn descendant decision after its successor started",
            results["late_result"],
        )
    else:
        assert any("Decision recorded" in value for value in outputs), outputs
    completed = [
        (seq, json.loads(payload)["status"])
        for seq, kind, _, payload in results["history"]
        if kind == "completed"
    ]
    assert [status for _, status in completed] == ["completed", "failed", "completed"]
    consumed = [seq for kind, seq in results["flow_events"] if kind == "consumed"]
    if replace:
        assert command.returncode == 0 and results["flow"][0] == "completed", results
        assert consumed == [completed[0][0], completed[2][0]], consumed
    else:
        assert command.returncode != 0 and results["flow"][0] == "current", results
        assert "requires a decision" in command.stderr, command.stderr
        assert consumed == [completed[0][0]], consumed
        assert json.loads(results["flow"][2])["progress"].get("verdict") is None
    results["failed_decision_discarded"] = "passed"


def _flow_automatic_retry_contract(
    binary: Path,
    work: Path,
    env: dict[str, str],
    server: Responses,
    results: dict,
) -> None:
    _init_repo(work, env)
    for directory in ("skills", "flows"):
        (work / ".lf" / directory).mkdir(parents=True, exist_ok=True)
    (work / ".lf/skills/native-proof.md").write_text("Run the fixture command.")
    (work / ".lf/flows/native-proof.yaml").write_text("- native-proof\n")
    server.fail = server.transient = True
    command = _command(
        [str(binary), "--model", "codex", "flow", "native-proof", "-b", "--no-loopflow"],
        work=work,
        env=env,
        timeout=90,
    )
    results.update(command_exit=command.returncode, command_stderr=command.stderr)
    with sqlite3.connect(env["LF_DB_PATH"]) as db:
        results["run_tables"] = db.execute(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='runs'"
        ).fetchone()[0]
        results["flow_state"] = db.execute(
            "SELECT state,failure_json FROM flow_sessions"
        ).fetchone()
        results["history"] = db.execute(
            "SELECT seq,kind,provider_turn,payload FROM session_events ORDER BY seq"
        ).fetchall()
        origins = db.execute(
            "SELECT session_id,provider_thread,provider_generation,exec_id "
            "FROM session_events WHERE kind='started' ORDER BY seq"
        ).fetchall()
        results["origins"] = origins
        results["flow_events"] = db.execute(
            "SELECT kind,session_event FROM flow_events ORDER BY seq"
        ).fetchall()
    assert command.returncode == 0, command.stderr
    assert len(origins) == 2 and origins[0] == origins[1], origins
    assert all(value is not None for value in origins[0]), origins
    history = results["history"]
    completed = [
        (seq, turn, json.loads(payload))
        for seq, kind, turn, payload in history
        if kind == "completed"
    ]
    assert [payload["status"] for _, _, payload in completed] == ["failed", "completed"]
    assert completed[0][1] != completed[1][1]
    starts = [seq for seq, kind, _, _ in history if kind == "started"]
    assert results["flow_events"] == [
        ("selected", starts[0]),
        ("selected", starts[1]),
        ("consumed", completed[1][0]),
    ], results["flow_events"]
    assert results["flow_state"] == ("completed", None), results["flow_state"]
    usage = [json.loads(payload) for _, kind, _, payload in history if kind == "usage"]
    assert usage[-1]["total"]["inputTokens"] == 40
    assert usage[-1]["total"]["outputTokens"] == 10
    assert len(list(Path(env["LF_PROBE_ENGINES"]).glob("*.json"))) == 1
    results["automatic_retry"] = "passed"
    assert results["run_tables"] == 0, (
        "Flow agent history must use AgentSession and Exec without a separate Run owner",
        results["run_tables"],
    )
    results["session_owners"] = "passed"
    report = _command(
        [str(binary), "usage", "--days", "0", "--json"], work=work, env=env, timeout=30
    )
    assert report.returncode == 0, report.stderr
    rows = json.loads(report.stdout)
    results["public_usage"] = rows
    assert len(rows) == 1, rows
    assert rows[0]["outcome"] == "completed", rows
    assert rows[0]["usage"]["input_tokens"] == 40, rows
    assert rows[0]["usage"]["output_tokens"] == 10, rows
    assert rows[0]["usage"]["cost_usd"] is None, rows
    results["usage_owners"] = "passed"


def _flow_retry_contract(
    binary: Path,
    work: Path,
    env: dict[str, str],
    server: Responses,
    results: dict,
    *,
    replace_engine: bool,
) -> None:
    _init_repo(work, env)
    for directory in ("skills", "flows"):
        (work / ".lf" / directory).mkdir(parents=True, exist_ok=True)
    (work / ".lf/skills/native-proof.md").write_text("Run the fixture command.")
    (work / ".lf/flows/native-proof.yaml").write_text("- native-proof\n")
    server.fail = True
    failed = _command(
        [str(binary), "--model", "codex", "flow", "native-proof", "-b", "--no-loopflow"],
        work=work,
        env=env,
        timeout=60,
    )
    results["first_exit"] = failed.returncode
    results["first_stderr"] = failed.stderr
    assert failed.returncode != 0, "controlled native failure must block the Flow"
    with sqlite3.connect(env["LF_DB_PATH"]) as db:
        flow, failure = db.execute("SELECT id,failure_json FROM flow_sessions").fetchone()
        assert failure, failed.stderr
        before = db.execute(
            "SELECT id,provider_endpoint,provider_thread,provider_generation,provider_exec_id "
            "FROM agent_sessions WHERE flow_session_id=?",
            (flow,),
        ).fetchone()
        assert before and before[1] and before[2], before
        prior = db.execute("SELECT * FROM session_events ORDER BY seq").fetchall()
        results["before"] = before
        results["prior_history"] = prior
        assert any(
            row["status"] == "failed"
            for (payload,) in db.execute(
                "SELECT payload FROM session_events WHERE kind='completed'"
            )
            for row in [json.loads(payload)]
        ), prior
    if replace_engine:
        engines = Path(env["LF_PROBE_ENGINES"])
        results["stopped_engine_receipts"] = [
            json.loads(path.read_text()) for path in engines.glob("*.json")
        ]
        _stop_fixture_engines(engines)
        results["fixture_engine_stop_completed"] = True
    server.fail = False
    retry = _command(
        [str(binary), "flow", "resume", flow, "--retry"], work=work, env=env, timeout=60
    )
    results["retry_exit"] = retry.returncode
    results["retry_stderr"] = retry.stderr
    assert retry.returncode == 0, retry.stderr
    with sqlite3.connect(env["LF_DB_PATH"]) as db:
        after = db.execute(
            "SELECT id,provider_endpoint,provider_thread,provider_generation,provider_exec_id "
            "FROM agent_sessions WHERE flow_session_id=?",
            (flow,),
        ).fetchone()
        results["after"] = after
        if replace_engine:
            assert after[0] == before[0] and after[2] == before[2], (
                "engine recovery must retain the Session and native thread"
            )
            assert after[1] and after[1] != before[1]
            assert after[3] == before[3] + 1
            assert after[4] != before[4]
        else:
            assert after == before, "retry replaced the native conversation or provider"
        assert (
            db.execute(
                "SELECT * FROM session_events WHERE seq<=? ORDER BY seq", (prior[-1][0],)
            ).fetchall()
            == prior
        )
        completed = db.execute(
            "SELECT provider_turn,payload FROM session_events WHERE kind='completed' ORDER BY seq"
        ).fetchall()
        results["completions"] = completed
        assert [json.loads(payload)["status"] for _, payload in completed] == [
            "failed",
            "completed",
        ], completed
        assert completed[0][0] != completed[1][0]
        assert db.execute("SELECT state FROM flow_sessions WHERE id=?", (flow,)).fetchone() == (
            "completed",
        )
        children = db.execute(
            "SELECT caller_session_id,caller_provider_generation,parent_exec_id "
            "FROM execs WHERE via_agent=1"
        ).fetchall()
        results["children"] = children
        assert len(children) == 1 and children[0][:2] == (after[0], after[3]), children
        assert children[0][2] != before[4], "retry child must follow its new driver"
        if replace_engine:
            assert children[0][2] == after[4]
    assert len(list(Path(env["LF_PROBE_ENGINES"]).glob("*.json"))) == 1 + replace_engine, (
        "retry must replace only the terminated engine"
    )
    results["flow_retry"] = "passed"


def _flow_driver_loss_contract(
    binary: Path,
    work: Path,
    env: dict[str, str],
    server: Responses,
    results: dict,
    *,
    completed_first: bool,
    replace_engine: bool,
) -> None:
    _init_repo(work, env)
    for directory in ("skills", "flows"):
        (work / ".lf" / directory).mkdir(parents=True, exist_ok=True)
    (work / ".lf/skills/native-proof.md").write_text("Run the held fixture command.")
    (work / ".lf/flows/native-proof.yaml").write_text("- native-proof\n")
    child = subprocess.Popen(
        [str(binary), "--model", "codex", "flow", "native-proof", "-b", "--no-loopflow"],
        cwd=work,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )
    try:
        assert server.held.wait(15), "native turn did not reach held response"
        assert child.poll() is None
        receipts = [json.loads(p.read_text()) for p in Path(env["LF_PROBE_ENGINES"]).glob("*.json")]
        assert len(receipts) == 1, receipts
        engine_pid, engine_stamp = receipts[0]
        assert os.getpgid(child.pid) == child.pid
        assert os.getpgid(engine_pid) == engine_pid and engine_pid != child.pid
        with sqlite3.connect(env["LF_DB_PATH"]) as db:
            flow = db.execute("SELECT id FROM flow_sessions").fetchone()[0]
            before = db.execute(
                "SELECT id,provider_thread,provider_generation,driver_exec_id "
                "FROM agent_sessions WHERE flow_session_id=?",
                (flow,),
            ).fetchone()
            results["before"] = before
            results["prior_history"] = db.execute(
                "SELECT * FROM session_events ORDER BY seq"
            ).fetchall()
        os.killpg(child.pid, signal.SIGKILL)
        stdout, stderr = child.communicate(timeout=10)
        results.update(first_exit=child.returncode, first_stderr=stderr, driver_pid=child.pid)
        observed = subprocess.run(
            ["ps", "-p", str(engine_pid), "-o", "lstart="], capture_output=True, text=True
        )
        assert observed.returncode == 0 and observed.stdout.strip() == engine_stamp
        results["engine_survived_driver"] = receipts[0]
        with sqlite3.connect(env["LF_DB_PATH"]) as db:
            endpoint = db.execute(
                "SELECT provider_endpoint FROM agent_sessions WHERE id=?", (before[0],)
            ).fetchone()[0]
            results["driver_outcome_before"] = db.execute(
                "SELECT outcome,exit_code FROM execs WHERE id=?", (before[3],)
            ).fetchone()
        retry = None
        if replace_engine:
            observed = subprocess.run(
                ["ps", "-p", str(engine_pid), "-o", "lstart="], capture_output=True, text=True
            )
            assert observed.returncode == 0 and observed.stdout.strip() == engine_stamp
            assert os.getpgid(engine_pid) == engine_pid
            os.killpg(engine_pid, signal.SIGKILL)
            deadline = time.monotonic() + 10
            while True:
                observed = subprocess.run(
                    ["ps", "-p", str(engine_pid), "-o", "lstart="], capture_output=True, text=True
                )
                if observed.returncode != 0:
                    break
                assert observed.stdout.strip() == engine_stamp, "fixture PID replaced"
                assert time.monotonic() < deadline, "fixture engine did not exit"
                time.sleep(0.05)
            results["engine_exited_before_retry"] = engine_pid
            server.release.set()
            retry = _command(
                [str(binary), "flow", "resume", flow, "--retry"], work=work, env=env, timeout=45
            )
            retry_exit, retry_stderr = retry.returncode, retry.stderr
        elif completed_first:
            server.release.set()
            observer = Client(Path(endpoint))
            try:
                deadline = time.monotonic() + 20
                while True:
                    native = observer.call(
                        "thread/read", {"threadId": before[1], "includeTurns": True}
                    )
                    turns = native["thread"]["turns"]
                    if turns and turns[-1]["status"] == "completed":
                        break
                    assert time.monotonic() < deadline, native
                    time.sleep(0.05)
                results["native_completed_before_resume"] = native
            finally:
                observer.close()
            retry = _command([str(binary), "flow", "resume", flow], work=work, env=env, timeout=45)
            retry_exit, retry_stderr = retry.returncode, retry.stderr
        else:
            # Recover while work is still native-owned. No second turn/start
            # should be sent; releasing the response completes the selected turn.
            retry = subprocess.Popen(
                [str(binary), "flow", "resume", flow, "--retry"],
                cwd=work,
                env=env,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                start_new_session=True,
            )
            try:
                time.sleep(1)
                assert retry.poll() is None, "resume refused the surviving turn"
                server.release.set()
                _, retry_stderr = retry.communicate(timeout=45)
                retry_exit = retry.returncode
            finally:
                if retry.poll() is None:
                    os.killpg(retry.pid, signal.SIGTERM)
                    retry.communicate(timeout=10)
        with sqlite3.connect(env["LF_DB_PATH"]) as db:
            results["driver_outcome_after"] = db.execute(
                "SELECT outcome,exit_code FROM execs WHERE id=?", (before[3],)
            ).fetchone()
        assert results["driver_outcome_after"] == results["driver_outcome_before"], (
            "native completion rewrote the dead command outcome"
        )
        results.update(retry_exit=retry_exit, retry_stderr=retry_stderr)
        with sqlite3.connect(env["LF_DB_PATH"]) as db:
            results["after"] = db.execute(
                "SELECT id,provider_thread,provider_generation,driver_exec_id "
                "FROM agent_sessions WHERE flow_session_id=?",
                (flow,),
            ).fetchone()
            results["history_after"] = db.execute(
                "SELECT * FROM session_events ORDER BY seq"
            ).fetchall()
            results["flow_state"] = db.execute(
                "SELECT state,failure_json FROM flow_sessions WHERE id=?", (flow,)
            ).fetchone()
        assert retry_exit == 0, retry_stderr
        assert results["flow_state"][0] == "completed", results["flow_state"]
        if replace_engine:
            assert results["before"][:2] == results["after"][:2], "conversation identity changed"
            assert results["after"][2] == before[2] + 1
        else:
            assert results["before"][:3] == results["after"][:3], "surviving conversation changed"
        with sqlite3.connect(env["LF_DB_PATH"]) as db:
            consumed = db.execute(
                "SELECT e.session_event,s.provider_turn,s.exec_id FROM flow_events e "
                "JOIN session_events s ON s.seq=e.session_event "
                "WHERE e.flow_id=? AND e.kind='consumed'",
                (flow,),
            ).fetchall()
            selected = db.execute(
                "SELECT s.provider_turn FROM flow_events e "
                "JOIN session_events s ON s.seq=e.session_event "
                "WHERE e.flow_id=? AND e.kind='selected'",
                (flow,),
            ).fetchall()
            assert len(consumed) == 1 and selected[-1] == (consumed[0][1],), (selected, consumed)
            assert len(selected) == 1 + replace_engine
            if not replace_engine:
                assert db.execute(
                    "SELECT COUNT(*) FROM session_events WHERE kind='completed'"
                ).fetchone() == (1,)
            results["consumed"] = consumed
        history = _command(
            [str(binary), "session", "history", before[0], "--json"],
            work=work,
            env=env,
            timeout=30,
        )
        assert history.returncode == 0, history.stderr
        events = json.loads(history.stdout)
        completed = [event for event in events if event["seq"] == consumed[0][0]]
        assert len(completed) == 1 and completed[0]["seq"] == consumed[0][0], events
        if replace_engine:
            assert completed[0]["exec_id"] != before[3], completed
        else:
            assert completed[0]["exec_id"] == before[3], completed
        assert completed[0]["provider_generation"] == before[2] + replace_engine, completed
        results["public_history"] = events
        repeated = _command([str(binary), "flow", "resume", flow], work=work, env=env, timeout=30)
        assert repeated.returncode == 0, repeated.stderr
        with sqlite3.connect(env["LF_DB_PATH"]) as db:
            assert db.execute(
                "SELECT COUNT(*) FROM flow_events WHERE kind='consumed'"
            ).fetchone() == (1,)
        results["driver_loss_recovery"] = "passed"
    finally:
        server.release.set()
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            child.communicate(timeout=10)


def _launch_contract(binary: Path, work: Path, env: dict[str, str], results: dict) -> None:
    _init_repo(work, env)
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
        results["observed_execs"] = database.execute(
            "SELECT id,command,via_agent,parent_exec_id FROM execs ORDER BY started_at,id"
        ).fetchall()
        children = database.execute(
            "SELECT id,parent_exec_id,caller_session_id,caller_provider_generation "
            "FROM execs WHERE via_agent=1"
        ).fetchall()
        results["observed_agent_children"] = children
        results["observed_session_driver"] = database.execute(
            "SELECT provider_exec_id,driver_exec_id,provider_generation "
            "FROM agent_sessions WHERE id=?",
            (results["session_id"],),
        ).fetchone()
        assert len(children) == 1, children
        child, parent, session, generation = children[0]
        assert session == results["session_id"] and generation == 1, children
        creator = database.execute(
            "SELECT provider_exec_id,driver_exec_id FROM agent_sessions WHERE id=?", (session,)
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
