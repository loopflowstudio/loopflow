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
        self.fail_request: int | None = None
        self.commands: dict[int, str] = {}
        self.invalid_decision_output = False
        self.decision_outputs: list[dict] = []
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
        if len(self.server.requests) == self.server.fail_request:
            self.send_response(400)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(
                json.dumps(
                    {
                        "error": {
                            "type": "invalid_request_error",
                            "code": "fixture_transient",
                            "message": "Service temporarily unavailable; retry this turn",
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
            text = "Fixture complete."
            if output_format := request.get("text", {}).get("format"):
                if output_format.get("type") == "json_schema":
                    schema = output_format["schema"]
                    assert schema["additionalProperties"] is False
                    text = json.dumps(
                        self.server.decision_outputs.pop(0)
                        if self.server.decision_outputs
                        else {"decision": "unknown"}
                        if self.server.invalid_decision_output
                        else {"decision": "advance", "summary": "Native output proof"}
                    )
            item = {
                "type": "message",
                "id": "msg_done",
                "role": "assistant",
                "content": [{"type": "output_text", "text": text}],
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
    parser.add_argument("--lf", required=True, type=Path)
    parser.add_argument("--lf-home", type=Path)
    parser.add_argument("--control", type=Path)
    parser.add_argument("--gated", action="store_true")
    parser.add_argument("--launch", action="store_true")
    parser.add_argument("--public-connect", action="store_true")
    parser.add_argument("--flow-blocked", action="store_true")
    parser.add_argument("--flow-decision-retry", choices=("missing", "replace"))
    parser.add_argument("--flow-driver-loss", choices=("running", "completed", "both"))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    server = Responses()
    if not args.launch:
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
                    if args.public_connect:
                        results["public_connect"] = _live_driver_contract(binary, work, env, server)
                        return
                    if args.flow_blocked:
                        _flow_blocked_contract(binary, work, env, server, results)
                        return
                    if args.flow_decision_retry:
                        _flow_decision_retry_contract(
                            binary,
                            work,
                            env,
                            server,
                            results,
                            replace=args.flow_decision_retry == "replace",
                        )
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
                    parser.error("--launch requires a Flow proof mode")
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
                    _boundary(args.control, "before", "handoff")
                    active_turn = first.start_turn(thread, "Run the held fixture command.")
                    assert server.held.wait(10), "engine did not request held response"
                    second = Client(endpoint)
                    clients.append(second)
                    resumed = second.call("thread/resume", {"threadId": thread})
                    assert resumed["thread"]["status"]["type"] == "active", resumed
                    first.close()
                    server.release.set()
                    second.wait_turn(active_turn)
                    _boundary(args.control, "after", "replace")
                    second.wait_turn(second.start_turn(thread))
                    results["replaced_provider_command_completed"] = True
                    results["active_turn_survived_disconnect"] = active_turn
                    results.update(thread=thread, engine_alive=engine.poll() is None)
                    return
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
        if "resume" in sys.argv and os.environ.get("LF_PROBE_CLIENT"):
            raise RuntimeError("public connect bypassed the retained engine relay")
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
    headless: subprocess.Popen,
) -> None:
    session = results["session_id"]
    processes = []
    replaced = []
    controls = []
    inspectors = []

    def wait(path: Path) -> None:
        deadline = time.monotonic() + 15
        while not path.exists():
            assert time.monotonic() < deadline, f"timed out: {path}"
            for child in processes:
                if child in replaced:
                    continue
                if child.poll() is not None:
                    _, error = child.communicate(timeout=5)
                    raise AssertionError(f"connect exited: {child.returncode}: {error.decode()}")
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
                call(0, 0, "thread/read", {"threadId": thread})
                with sqlite3.connect(env["LF_DB_PATH"]) as database:
                    active = database.execute(
                        "SELECT provider_turn FROM session_events WHERE session_id=? "
                        "AND kind='started' ORDER BY seq DESC LIMIT 1",
                        (session,),
                    ).fetchone()[0]
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
        # Explicit client replacement must use the same live-engine path as
        # ordinary connect, with the current turn and shared sibling untouched.
        with sqlite3.connect(env["LF_DB_PATH"]) as database:
            before_prepare = database.execute(
                "SELECT driver_exec_id,driver_generation,provider_endpoint,provider_thread,"
                "provider_generation FROM agent_sessions WHERE id=?",
                (session,),
            ).fetchone()
        prepared = _command(
            [str(binary), "session", "connect", session, "--replace", "--json"],
            work,
            env,
            timeout=15,
        )
        assert prepared.returncode == 0, prepared.stderr
        assert "--replace" in json.loads(prepared.stdout)["open_argv"]
        assert all(previous.poll() is None for previous in processes)
        with sqlite3.connect(env["LF_DB_PATH"]) as database:
            after_prepare = database.execute(
                "SELECT driver_exec_id,driver_generation,provider_endpoint,provider_thread,"
                "provider_generation FROM agent_sessions WHERE id=?",
                (session,),
            ).fetchone()
        assert after_prepare == before_prepare
        replaced.extend(processes)
        control = work.parent / f"{session}-replacement"
        control.mkdir()
        controls.append(control)
        replacement = subprocess.Popen(
            [str(binary), "session", "connect", session, "--replace"],
            cwd=work,
            env={**env, "LF_PROBE_CLIENT": str(control)},
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
        )
        processes.append(replacement)
        wait(control / "ready")
        for previous in replaced:
            previous.communicate(timeout=10)
        assert (
            engine.call("thread/read", {"threadId": thread})["thread"]["status"]["type"] == "active"
        )
        assert (
            engine.call("thread/read", {"threadId": sibling})["thread"]["status"]["type"]
            == "active"
        )
        with sqlite3.connect(env["LF_DB_PATH"]) as database:
            retained_connection = database.execute(
                "SELECT provider_endpoint,provider_thread,provider_generation "
                "FROM agent_sessions WHERE id=?",
                (session,),
            ).fetchone()
            driver, observed_generation, interactive = database.execute(
                "SELECT driver_exec_id,provider_generation,interactive "
                "FROM agent_sessions WHERE id=?",
                (session,),
            ).fetchone()
            before = {row[0] for row in database.execute("SELECT id FROM execs")}
        assert observed_generation == generation and interactive == 1
        assert retained_connection == (endpoint, thread, generation)
        assert driver != before_prepare[0]
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
        call(2, 0, "thread/read", {"threadId": thread, "includeTurns": True})
        history_command = [str(binary), "session", "history", session, "--json", "--limit", "0"]
        recorded = _command(history_command, work, env, timeout=15)
        assert recorded.returncode == 0, recorded.stderr
        history = json.loads(recorded.stdout)
        completions = [event for event in history if event["kind"] == "completed"]
        assert len(completions) == 1, history
        selected = [event for event in completions if event["provider_turn"] == active]
        assert len(selected) == 1 and selected[0]["payload"]["status"] == "completed", selected
        usage = [
            event
            for event in history
            if event["kind"] == "usage" and event["provider_turn"] == active
        ]
        assert usage and usage[-1]["payload"]["total"]["inputTokens"] > 0, history
        call(2, 1, "thread/read", {"threadId": thread, "includeTurns": True})
        replay = _command(history_command, work, env, timeout=15)
        assert replay.returncode == 0 and json.loads(replay.stdout) == history, replay
        results["history"] = history
        results["public_connect"] = dict(
            provider_generation=generation,
            driver=driver,
            retained_client_rejected=True,
            active_turn_and_sibling_survived=True,
            replaced_clients_exited=True,
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
    _init_repo(work, env)
    _command([str(binary), "session", "list", "--json"], work, env, timeout=15)
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


def _flow_blocked_contract(
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
    (work / ".lf/flows/native-proof.yaml").write_text(
        "- step:\n    id: work\n    name: native-proof\n"
        "- step:\n    id: decide\n    name: native-proof\n    repeat:\n      from: work\n"
    )
    # The Ask is completed through public commands below. No interactive terminal
    # is opened: this transport stub supplies no conversation or completion.
    tmux = binary.parent / "tmux"
    tmux.write_text("#!/bin/sh\nexit 0\n")
    tmux.chmod(0o700)
    server.decision_outputs = [
        {"decision": "blocked", "reason": "Which policy applies?"},
        {"decision": "blocked", "reason": "Which remaining scope is accepted?"},
        {"decision": "advance", "summary": "Both answers received"},
    ]
    log = (work / "flow.log").open("w+")
    driver = subprocess.Popen(
        [str(binary), "--model", "codex", "flow", "native-proof", "-b", "--no-loopflow"],
        cwd=work,
        env=env,
        stdout=log,
        stderr=log,
    )
    database = env["LF_DB_PATH"]

    def pending_question() -> tuple:
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            if driver.poll() is not None:
                log.seek(0)
                raise AssertionError(log.read())
            if Path(database).exists():
                with sqlite3.connect(database) as db:
                    if db.execute(
                        "SELECT 1 FROM sqlite_master WHERE name='agent_sessions'"
                    ).fetchone():
                        question = db.execute(
                            "SELECT s.id,e.receipt_key,s.request FROM agent_sessions s "
                            "JOIN session_events e ON e.seq=s.current_capture "
                            "WHERE s.kind='ask' AND s.completed_at IS NULL AND s.input_published=1"
                        ).fetchone()
                        if question:
                            return question
            time.sleep(0.05)
        raise AssertionError("Flow did not open its question")

    try:
        question = pending_question()
        with sqlite3.connect(database) as db:
            flow_id, cursor = db.execute("SELECT id,review_json FROM flow_sessions").fetchone()
            starts = db.execute(
                "SELECT COUNT(*) FROM session_events WHERE kind='started'"
            ).fetchone()[0]
        assert starts == 2, starts
        driver.terminate()
        driver.wait(timeout=10)
        driver = subprocess.Popen(
            [str(binary), "flow", "resume", flow_id],
            cwd=work,
            env=env,
            stdout=log,
            stderr=log,
        )
        assert pending_question() == question, "driver recovery replaced the keyed question"
        for index in range(2):
            if index:
                question = pending_question()
            with sqlite3.connect(database) as db:
                current = db.execute(
                    "SELECT review_json FROM flow_sessions WHERE id=?", (flow_id,)
                ).fetchone()[0]
                assert json.loads(current)["index"] == json.loads(cursor)["index"]
            answer_env = {
                **env,
                "LF_RUN_ID": question[1],
                "LF_HUMAN_SESSION": json.dumps({"kind": "ask", "id": question[0]}),
            }
            ready = _command(
                [str(binary), "session", "ready", f"Accepted answer {index + 1}"],
                work,
                answer_env,
                20,
            )
            assert ready.returncode == 0, ready.stderr
            complete = _command([str(binary), "session", "complete", question[0]], work, env, 20)
            assert complete.returncode == 0, complete.stderr
        assert driver.wait(timeout=60) == 0
        with sqlite3.connect(database) as db:
            assert db.execute("SELECT state,failure_json FROM flow_sessions").fetchone() == (
                "completed",
                None,
            )
            asks = db.execute(
                "SELECT id,request,ready_summary,completed_at FROM agent_sessions WHERE kind='ask'"
            ).fetchall()
            assert len(asks) == 2 and all(row[3] for row in asks), asks
            turns = db.execute(
                "SELECT e.session_id,e.provider_thread,e.provider_turn FROM session_events e "
                "JOIN agent_sessions s ON s.id=e.session_id "
                "WHERE s.node=1 AND e.kind='started' ORDER BY e.seq"
            ).fetchall()
            assert len(turns) == 3 and len({row[:2] for row in turns}) == 1, turns
            assert len({row[2] for row in turns}) == 3, turns
            assert db.execute(
                "SELECT COUNT(*) FROM flow_events WHERE kind='consumed'"
            ).fetchone() == (4,)
            results.update(
                blocked_asks=asks, deciding_turns=turns, blocked_driver_recovery="passed"
            )
        assert "Accepted answer 1" in json.dumps(server.requests)
        assert "Accepted answer 2" in json.dumps(server.requests)
    finally:
        if driver.poll() is None:
            driver.terminate()
            driver.wait(timeout=10)
        log.seek(0)
        results["blocked_driver_log"] = log.read()
        log.close()


def _flow_decision_retry_contract(
    binary: Path,
    work: Path,
    env: dict[str, str],
    server: Responses,
    results: dict,
    *,
    replace: bool,
) -> None:
    _init_repo(work, env)
    for directory in ("skills", "flows"):
        (work / ".lf" / directory).mkdir(parents=True, exist_ok=True)
    (work / ".lf/skills/native-proof.md").write_text("Run the fixture command.")
    (work / ".lf/flows/native-proof.yaml").write_text(
        "- step:\n    id: work\n    name: native-proof\n"
        "- step:\n    id: decide\n    name: native-proof\n    repeat:\n      from: work\n"
    )
    server.fail_request = 4
    server.invalid_decision_output = not replace
    server.commands[3] = "printf failed-turn-work"
    server.commands[5] = "printf retry-work"
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
    schemas = [request.get("text", {}).get("format") for request in server.requests]
    assert any(schema and schema.get("type") == "json_schema" for schema in schemas), schemas
    completed = [
        (seq, json.loads(payload)["status"])
        for seq, kind, _, payload in results["history"]
        if kind == "completed"
    ]
    assert [status for _, status in completed] == (
        ["completed", "failed", "completed"]
        if replace
        else ["completed", "failed", "completed", "completed", "completed"]
    )
    consumed = [seq for kind, seq in results["flow_events"] if kind == "consumed"]
    if replace:
        assert command.returncode == 0 and results["flow"][0] == "completed", results
        assert consumed == [completed[0][0], completed[2][0]], consumed
    else:
        assert command.returncode != 0 and results["flow"][0] == "current", results
        assert "structured output validation exhausted" in command.stderr, command.stderr
        assert consumed == [completed[0][0]], consumed
        assert json.loads(results["flow"][2])["progress"].get("verdict") is None
    results["failed_decision_discarded"] = "passed"


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
    (work / ".lf/flows/native-proof.yaml").write_text("- step:\n    name: native-proof\n")
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
        if not server.held.wait(15):
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM)
            _, stderr = child.communicate(timeout=10)
            raise AssertionError(f"native turn did not reach held response: {stderr}")
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
