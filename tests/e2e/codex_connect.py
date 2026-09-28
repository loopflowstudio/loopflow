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
                        "cmd": "printf sibling-only"
                        if "held sibling" in json.dumps(request["input"][last_user])
                        else self.server.command,
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
                "FROM sessions WHERE id=?",
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
                    "WHERE id=(SELECT provider_exec_id FROM sessions WHERE id=?)",
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
                "SELECT driver_exec_id,provider_generation,interactive FROM sessions WHERE id=?",
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
                    "SELECT driver_exec_id FROM sessions WHERE id=?", (session,)
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
                driver_outcome = database.execute(
                    "SELECT outcome,exit_code FROM execs WHERE id=?", (original_driver,)
                ).fetchone()
            results["driverless_before_reconnect"] = {
                "turn": orphan_turn,
                "driver": original_driver,
                "driver_outcome": driver_outcome,
                "departed": departed,
                "recorded_completions": absent,
                "native_status": native_completion["params"]["turn"]["status"],
            }
            assert absent == 0, "a Loopflow receiver unexpectedly remained"
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
        existing = {row[0] for row in database.execute("SELECT id FROM sessions")}
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
                for row in database.execute("SELECT id FROM sessions")
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
        results["observed_execs"] = database.execute(
            "SELECT id,command,via_agent,parent_exec_id FROM execs ORDER BY started_at,id"
        ).fetchall()
        children = database.execute(
            "SELECT id,parent_exec_id,caller_session_id,caller_provider_generation "
            "FROM execs WHERE via_agent=1"
        ).fetchall()
        results["observed_agent_children"] = children
        results["observed_session_driver"] = database.execute(
            "SELECT provider_exec_id,driver_exec_id,provider_generation FROM sessions WHERE id=?",
            (results["session_id"],),
        ).fetchone()
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
