# /// script
# requires-python = ">=3.10"
# dependencies = ["websockets>=15,<16"]
# ///
"""Exercise a real Codex app-server with credential-free local Responses and private Machines."""

import argparse
import base64
import contextlib
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

ANSWER_CONTRACT = "Return the final answer as the declared JSON value"


def _database(env: dict[str, str]) -> str:
    return str(Path(env["LF_HOME"]) / "loopflow.db")


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
            # A Flow process writes the answer contract into the message.
            if ANSWER_CONTRACT in json.dumps(request["input"]):
                text = json.dumps(
                    self.server.decision_outputs.pop(0)
                    if self.server.decision_outputs
                    else {"decision": "unknown"}
                    if self.server.invalid_decision_output
                    else {"decision": "advance", "summary": "Native output proof", "reason": None}
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
    parser.add_argument("--flow-decision-retry", choices=("missing", "replace"))
    parser.add_argument("--shared-provider-home", action="store_true")
    args = parser.parse_args()
    args.launch = args.launch or args.shared_provider_home
    args.output.mkdir(parents=True, exist_ok=True)
    server = Responses()
    if not args.launch:
        assert args.lf_home and args.control, (
            "lf ownership proof needs a private Machine and control directory"
        )
        server.command = shlex.join([str(args.lf), "session", "list", "--all", "--json"])
    serving = threading.Thread(target=server.serve_forever, daemon=True)
    serving.start()
    results = {}
    clients = []
    try:
        with tempfile.TemporaryDirectory(prefix="lf-connect-", dir="/tmp") as directory:
            root = Path(directory)
            # The shared-home proof uses the provider's default home: nothing
            # may need a home override to find a Loopflow conversation.
            home = root / (".codex" if args.shared_provider_home else "codex")
            work = root / "work"
            home.mkdir()
            work.mkdir()
            (home / "config.toml").write_text(f"""model = "gpt-5.4"
model_provider = "fixture"
cli_auth_credentials_store = "file"
allow_login_shell = false
sandbox_mode = "danger-full-access"
approval_policy = "never"
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
            # Never inherit credentials, execution authority, or the real provider Machine.
            env = {key: os.environ[key] for key in ("PATH", "TMPDIR", "LANG") if key in os.environ}
            env.update(HOME=str(root), CODEX_HOME=str(home), LF_HOME=str(root / "lf"))
            if args.shared_provider_home:
                del env["CODEX_HOME"]
            if args.launch:
                # Pin bytes: another contributor may build the source path while
                # this private-Machine proof is running.
                binary = root / "bin" / "lf"
                binary.parent.mkdir()
                shutil.copy2(args.lf, binary)
                agent_processes = root / "agent-processes"
                agent_processes.mkdir()
                shim = binary.parent / "codex"
                shim.write_text(
                    f"#!{sys.executable}\nimport runpy\n"
                    f"runpy.run_path({str(Path(__file__).resolve())!r}, "
                    "run_name='fixture')['_provider_entry']()\n"
                )
                shim.chmod(0o700)
                env.update(
                    LF_BIN=str(binary),
                    PATH=f"{binary.parent}:{args.codex.parent}:{env['PATH']}",
                    LF_PROBE_CODEX=str(args.codex),
                    LF_PROBE_AGENT_PROCESSES=str(agent_processes),
                )
                results["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
                server.command = (
                    'printf "caller-present=%s home=%s\\n" '
                    '"${LF_AGENT_CALLER:+yes}" "$LF_HOME"\n'
                    "ps -p $$ -o etime= 2>&1\n"
                    + "RUST_LOG=loopflow::journal=debug "
                    + shlex.join([str(binary), "session", "list", "--all", "--json"])
                )
                try:
                    if args.shared_provider_home:
                        _shared_provider_home_contract(
                            binary, args.codex, root, work, env, server, results
                        )
                        return
                    if args.public_connect:
                        results["public_connect"] = _live_attachment_contract(
                            binary, work, env, server, shared_agent_process=True
                        )
                        results["owner_exit"] = _live_attachment_contract(
                            binary, work, env, server, shared_agent_process=False
                        )
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
                    parser.error("--launch requires a Flow proof mode")
                finally:
                    _stop_fixture_agent_processes(agent_processes)
                return
            endpoint = root / "agent.sock"
            with (args.output / "agent.log").open("w") as log:
                agent_process = subprocess.Popen(
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
                        and agent_process.poll() is None
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
                        results["agent_process_alive"] = agent_process.poll() is None
                        return
                    _boundary(args.control, "before", "handoff")
                    active_turn = first.start_turn(thread, "Run the held fixture command.")
                    assert server.held.wait(10), "provider did not request held response"
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
                    results.update(thread=thread, agent_process_alive=agent_process.poll() is None)
                    return
                finally:
                    for client in clients:
                        client.close()
                    if agent_process.poll() is None:
                        os.killpg(agent_process.pid, signal.SIGTERM)
                        try:
                            agent_process.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            os.killpg(agent_process.pid, signal.SIGKILL)
                            agent_process.wait(timeout=5)
    finally:
        server.release.set()
        server.shutdown()
        server.server_close()
        (args.output / "requests.json").write_text(json.dumps(server.requests, indent=2))
        (args.output / "events.json").write_text(json.dumps([c.events for c in clients], indent=2))
        (args.output / "results.json").write_text(json.dumps(results, indent=2))
        print(json.dumps(results), flush=True)


PROVIDER_ENV = ("CODEX_HOME", "CODEX_ACCESS_TOKEN", "OPENAI_API_KEY")


def _terminate(signum: int, _frame: object) -> None:
    raise SystemExit(128 + signum)


def _provider_entry() -> None:
    if record := os.environ.get("LF_PROBE_LAUNCHES"):
        # What a launch handed the provider. A terminal resume is recorded and
        # not run: this proof is headless.
        launch = {
            "argv": sys.argv[1:],
            **{name: os.environ.get(name) for name in PROVIDER_ENV},
        }
        with open(record, "a") as log:
            log.write(json.dumps(launch) + "\n")
        if "resume" in sys.argv and "--remote" not in sys.argv:
            return
    if "app-server" in sys.argv:
        pid = os.getpid()
        stamp = subprocess.check_output(["ps", "-p", str(pid), "-o", "lstart="], text=True).strip()
        (Path(os.environ["LF_PROBE_AGENT_PROCESSES"]) / f"{pid}.json").write_text(
            json.dumps([pid, stamp])
        )
        os.execv(os.environ["LF_PROBE_CODEX"], [os.environ["LF_PROBE_CODEX"], *sys.argv[1:]])
    if "--remote" not in sys.argv:
        if "resume" in sys.argv and os.environ.get("LF_PROBE_CLIENT"):
            raise RuntimeError("public connect bypassed the retained provider relay")
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


def _stop_fixture_agent_processes(directory: Path) -> None:
    for record in directory.glob("*.json"):
        pid, stamp = json.loads(record.read_text())
        observed = subprocess.run(
            ["ps", "-p", str(pid), "-o", "lstart="], capture_output=True, text=True
        )
        if observed.returncode == 0 and observed.stdout.strip() == stamp:
            assert os.getpgid(pid) == pid, "fixture agent process must own its process group"
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
                raise AssertionError(f"fixture agent process {pid} required force termination")


def _public_connection_contract(
    binary: Path,
    work: Path,
    env: dict[str, str],
    server: Responses,
    results: dict,
    headless: subprocess.Popen,
    shared_agent_process: bool,
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

    def _connect(label: str, *, replace: bool = False) -> None:
        control = work.parent / f"{session}-{label}"
        control.mkdir()
        controls.append(control)
        argv = [str(binary), "session", "connect", session]
        if replace:
            argv.append("--replace")
        processes.append(
            subprocess.Popen(
                argv,
                cwd=work,
                env={**env, "LF_PROBE_CLIENT": str(control)},
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
            )
        )
        wait(control / "ready")

    try:
        with sqlite3.connect(_database(env)) as database:
            endpoint, thread, generation = database.execute(
                "SELECT p.endpoint,s.provider_thread,p.provider_generation "
                "FROM agent_sessions s LEFT JOIN processes p ON p.id=s.agent_process_id "
                "WHERE s.id=?",
                (session,),
            ).fetchone()
        provider = Client(Path(endpoint))
        inspectors.append(provider)
        provider.call("thread/resume", {"threadId": thread})
        sibling = None
        if shared_agent_process:
            sibling = provider.call("thread/start", {"cwd": str(work), "approvalPolicy": "never"})[
                "thread"
            ]["id"]
            provider.start_turn(sibling, "held sibling")
        assert server.held.wait(10)
        for label in ["first", "second"]:
            _connect(label)
            if label == "first":
                call(0, 0, "thread/read", {"threadId": thread})
                with sqlite3.connect(_database(env)) as database:
                    active = database.execute(
                        "SELECT provider_turn FROM session_events WHERE session_id=? "
                        "AND kind='started' ORDER BY seq DESC LIMIT 1",
                        (session,),
                    ).fetchone()[0]
        headless.send_signal(signal.SIGINT)
        headless.communicate(timeout=10)
        assert headless.returncode == 130, headless.returncode
        with sqlite3.connect(_database(env)) as database:
            outcome = database.execute(
                "SELECT outcome FROM processes "
                "WHERE id=(SELECT p.parent_lf_process_id FROM agent_sessions s "
                "JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?)",
                (session,),
            ).fetchone()[0]
        assert outcome == "interrupted", outcome
        results["old_headless_attachment"] = dict(exit_code=130, outcome=outcome)
        assert (
            provider.call("thread/read", {"threadId": thread})["thread"]["status"]["type"]
            == "active"
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
            provider.call("thread/read", {"threadId": thread})["thread"]["status"]["type"]
            == "active"
        )
        if sibling:
            assert (
                provider.call("thread/read", {"threadId": sibling})["thread"]["status"]["type"]
                == "active"
            )
        # Explicit client replacement must use the same live-provider path as
        # ordinary connect, with the current turn and shared sibling untouched.
        with sqlite3.connect(_database(env)) as database:
            before_prepare = database.execute(
                "SELECT p.attached_lf_process_id,p.attachment_token,p.endpoint,s.provider_thread,"
                "p.provider_generation FROM agent_sessions s "
                "LEFT JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?",
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
        with sqlite3.connect(_database(env)) as database:
            after_prepare = database.execute(
                "SELECT p.attached_lf_process_id,p.attachment_token,p.endpoint,s.provider_thread,"
                "p.provider_generation FROM agent_sessions s "
                "LEFT JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?",
                (session,),
            ).fetchone()
        assert after_prepare == before_prepare
        replaced.extend(processes)
        _connect("replacement", replace=True)
        for previous in replaced:
            previous.communicate(timeout=10)
        assert (
            provider.call("thread/read", {"threadId": thread})["thread"]["status"]["type"]
            == "active"
        )
        if sibling:
            assert (
                provider.call("thread/read", {"threadId": sibling})["thread"]["status"]["type"]
                == "active"
            )
        with sqlite3.connect(_database(env)) as database:
            retained_connection = database.execute(
                "SELECT p.endpoint,s.provider_thread,p.provider_generation "
                "FROM agent_sessions s LEFT JOIN processes p ON p.id=s.agent_process_id "
                "WHERE s.id=?",
                (session,),
            ).fetchone()
            attached, observed_generation, interactive = database.execute(
                "SELECT p.attached_lf_process_id,p.provider_generation,s.interactive "
                "FROM agent_sessions s LEFT JOIN processes p ON p.id=s.agent_process_id "
                "WHERE s.id=?",
                (session,),
            ).fetchone()
            before = {row[0] for row in database.execute("SELECT id FROM processes")}
        assert observed_generation == generation and interactive == 1
        assert retained_connection == (endpoint, thread, generation)
        assert attached != before_prepare[0]
        server.release.set()
        provider.wait_turn(active)
        with sqlite3.connect(_database(env)) as database:
            after = database.execute(
                "SELECT id,parent_lf_process_id,caller_session_id,caller_provider_generation "
                "FROM processes WHERE via_agent=1"
            ).fetchall()
        children = [row for row in after if row[0] not in before]
        assert len(children) == 1, children
        child_id, child_parent, child_session, child_generation = children[0]
        assert (child_parent, child_session, child_generation) == (attached, session, generation), (
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
        if not shared_agent_process:
            # Closing the current native UI releases the provider process; obsolete UIs
            # and the original headless attachment's exit could not release it.
            (controls[2] / "2.request").write_text(json.dumps({"method": "exit"}))
            _, error = processes[2].communicate(timeout=15)
            assert processes[2].returncode == 0, error.decode()
            with sqlite3.connect(_database(env)) as database:
                closed = database.execute(
                    "SELECT p.endpoint,s.provider_thread,p.attached_lf_process_id "
                    "FROM agent_sessions s LEFT JOIN processes p ON p.id=s.agent_process_id "
                    "WHERE s.id=?",
                    (session,),
                ).fetchone()
            assert closed == (None, thread, None), closed
            assert not Path(endpoint).exists() or not provider.reader.is_alive()
            results["owner_exit_closed_agent_process"] = True
        results["history"] = history
        results["public_connect"] = dict(
            provider_generation=generation,
            attached=attached,
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


def _live_attachment_contract(
    binary: Path, work: Path, env: dict[str, str], server: Responses, shared_agent_process: bool
) -> dict:
    _init_repo(work, env)
    _command([str(binary), "session", "list", "--json"], work, env, timeout=15)
    with sqlite3.connect(_database(env)) as database:
        existing = {row[0] for row in database.execute("SELECT id FROM agent_sessions")}
    server.held.clear()
    server.release.clear()
    log = tempfile.TemporaryFile(mode="w+t")
    child = subprocess.Popen(
        [str(binary), "--batch", "--agent", "codex", ":", "held conversation"],
        stdin=subprocess.DEVNULL,
        cwd=work,
        env=env,
        stdout=subprocess.PIPE,
        stderr=log,
        text=True,
    )
    try:
        if not server.held.wait(15):
            child.terminate()
            stdout, stderr = child.communicate(timeout=10)
            log.seek(0)
            raise AssertionError(
                f"headless provider never reached held upstream: {stdout}\n{log.read()}"
            )
        with sqlite3.connect(_database(env)) as database:
            sessions = [
                row[0]
                for row in database.execute("SELECT id FROM agent_sessions")
                if row[0] not in existing
            ]
        assert len(sessions) == 1, sessions
        result = {"session_id": sessions[0]}
        _public_connection_contract(
            binary,
            work,
            env,
            server,
            result,
            headless=child,
            shared_agent_process=shared_agent_process,
        )
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
        log.close()


def _login(name: str, refresh: str = "issued") -> str:
    claims = json.dumps({"email": f"{name}@example.com", "sub": name}).encode()
    claims = base64.urlsafe_b64encode(claims).rstrip(b"=").decode()
    tokens = {
        "access_token": f"fixture-{name}",
        "refresh_token": f"{refresh}-{name}",
        "id_token": f"h.{claims}.s",
    }
    last_refresh = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    return json.dumps({"tokens": tokens, "last_refresh": last_refresh})


def _shared_provider_home_contract(
    binary: Path,
    codex: Path,
    root: Path,
    work: Path,
    env: dict[str, str],
    server: Responses,
    results: dict,
) -> None:
    """Loopflow and plain Codex share one home, signed in as one stored account."""
    _init_repo(work, env)
    native = root / ".codex"
    profiles = Path(env["LF_HOME"]) / "accounts" / "codex"
    record = root / "launches.jsonl"
    record.touch()
    env["LF_PROBE_LAUNCHES"] = str(record)
    server.command = "true"

    def lf(*args: str, timeout: int = 90) -> str:
        done = _command([str(binary), *args], work, env, timeout)
        assert done.returncode == 0, (args, done.stdout, done.stderr)
        return done.stdout

    def use(name: str) -> None:
        lf("account", "codex", "use", f"{name}@example.com")

    def rows(query: str, *values: object) -> list[tuple]:
        with sqlite3.connect(_database(env)) as database:
            return database.execute(query, values).fetchall()

    def login(home: Path) -> str:
        token = json.loads((home / "auth.json").read_text())["tokens"]["id_token"]
        claims = token.split(".")[1]
        return json.loads(base64.urlsafe_b64decode(claims + "=" * (-len(claims) % 4)))["email"]

    def launches() -> list[dict]:
        return [json.loads(line) for line in record.read_text().splitlines()]

    def conversations() -> dict[str, str]:
        """Provider conversation id → Session id, as Loopflow recorded them."""
        return dict(
            rows(
                "SELECT json_extract(payload,'$.evidence.provider_session_id'),session_id "
                "FROM session_events WHERE kind='observed' "
                "AND json_extract(payload,'$.evidence.provider_session_id') IS NOT NULL"
            )
        )

    def rollout(home: Path, conversation: str) -> bool:
        return any(home.glob(f"sessions/*/*/*/rollout-*-{conversation}.jsonl"))

    def converse(*flags: str, prompt: str = "say hi") -> tuple[str, list[dict]]:
        """Run one headless conversation; its provider id and provider launches."""
        known, before = conversations(), len(launches())
        lf(*flags, "--batch", "--agent", "codex", ":", prompt)
        started = [id for id in conversations() if id not in known]
        assert len(started) == 1, started
        agent_processes = [
            launch for launch in launches()[before:] if "app-server" in launch["argv"]
        ]
        assert agent_processes, "the conversation launched no provider process"
        return started[0], agent_processes

    def shared(agent_processes: list[dict]) -> bool:
        return all(launch[name] is None for launch in agent_processes for name in PROVIDER_ENV)

    def switches() -> int:
        return rows("SELECT COUNT(*) FROM provider_account_switches")[0][0]

    lf("session", "list", "--json")
    now = int(time.time())
    with sqlite3.connect(_database(env)) as database:
        for name in ("first", "second"):
            profile = profiles / name
            profile.mkdir(parents=True)
            (profile / "auth.json").write_text(_login(name))
            database.execute(
                "INSERT INTO provider_accounts(provider,account_id,home,login_email,"
                "credential_state,routing_state,created_at,updated_at) "
                "VALUES('codex',?,?,?,'connected','automatic',?,?)",
                (name, str(profile), f"{name}@example.com", now, now),
            )

    # A native login Loopflow has never seen is kept, not overwritten.
    stranger = _login("stranger")
    (native / "auth.json").write_text(stranger)
    use("first")
    assert login(native) == "first@example.com"
    kept = rows(
        "SELECT home,routing_state FROM provider_accounts WHERE login_email=?",
        "stranger@example.com",
    )
    assert len(kept) == 1 and kept[0][1] == "explicit_only", kept
    assert (Path(kept[0][0]) / "auth.json").read_text() == stranger
    results["unknown_native_login_kept"] = kept[0][0]

    # A launch naming no account, or the active one, changes nothing; neither
    # hands the provider a home or a credential.
    credential, switched = (native / "auth.json").read_bytes(), switches()
    ours, agent_processes = converse()
    assert shared(agent_processes), agent_processes
    _, agent_processes = converse("--account", "codex=first@example.com")
    assert shared(agent_processes), agent_processes
    assert (native / "auth.json").read_bytes() == credential and switches() == switched
    assert rollout(native, ours), "the conversation is not in the provider's own home"
    results["shared_conversation"] = ours

    # A provider's id names the same Session as Loopflow's own.
    def session(id: str) -> dict:
        return json.loads(lf("session", "connect", id, "--json"))

    def history(id: str) -> list:
        return json.loads(lf("session", "history", id, "--json"))

    assert history(ours) and history(ours) == history(conversations()[ours])

    @contextlib.contextmanager
    def plain_codex(home: Path | None = None):
        """Codex as a person runs it: no Loopflow, and no home unless given."""
        plain = {key: value for key, value in env.items() if not key.startswith("LF_")}
        if home:
            plain["CODEX_HOME"] = str(home)
        endpoint = root / f"plain-{len(list(root.glob('plain-*.log')))}.sock"
        with endpoint.with_suffix(".log").open("w") as log:
            agent_process = subprocess.Popen(
                [str(codex), "app-server", "--listen", f"unix://{endpoint}"],
                cwd=work,
                env=plain,
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=log,
                start_new_session=True,
            )
            client = None
            try:
                deadline = time.monotonic() + 10
                while (
                    not endpoint.exists()
                    and agent_process.poll() is None
                    and time.monotonic() < deadline
                ):
                    time.sleep(0.05)
                client = Client(endpoint)
                yield client
            finally:
                if client:
                    client.close()
                os.killpg(agent_process.pid, signal.SIGTERM)
                agent_process.wait(timeout=10)

    def start(client: Client) -> str:
        thread = client.call(
            "thread/start",
            {
                "cwd": str(work),
                "model": "gpt-5.4",
                "modelProvider": "fixture",
                "approvalPolicy": "never",
                "sandbox": "danger-full-access",
            },
        )["thread"]["id"]
        client.wait_turn(client.start_turn(thread))
        return thread

    # Batch exit closes its provider process: plain Codex can immediately list and
    # resume the saved conversation without fixture cleanup.
    with plain_codex() as client:
        assert ours in json.dumps(client.call("thread/list", {}))
        assert client.call("thread/resume", {"threadId": ours})["thread"]["id"] == ours
        theirs = start(client)
    assert rollout(native, theirs) and theirs not in conversations()
    results["plain_codex_conversation"] = theirs

    # The id plain Codex gave its own conversation admits a Session for it,
    # attributed to no Task, and opens it in the native home.
    def resumed(id: str) -> dict:
        before = len(launches())
        lf("session", "connect", id)
        resumes = [launch for launch in launches()[before:] if "resume" in launch["argv"]]
        assert len(resumes) == 1 and resumes[0]["argv"][-1] == id, resumes
        return resumes[0]

    admitted = session(theirs)
    assert admitted["task_ids"] == [] and admitted["work"] is None, admitted
    assert session(theirs)["id"] == admitted["id"] == session(admitted["id"])["id"]
    assert history(theirs) == history(admitted["id"])
    lf("session", "rename", theirs, "Plain", "conversation")
    assert session(admitted["id"])["title"] == "Plain conversation"
    assert shared([resumed(theirs)]) and login(native) == "first@example.com"
    unknown = _command(
        [str(binary), "session", "connect", ours[:-4] + "0000", "--json"], work, env, 30
    )
    assert unknown.returncode != 0 and "was not found" in unknown.stderr, unknown
    results["provider_ids_name_sessions"] = [conversations()[ours], admitted["id"]]

    # A → B → A: the token the provider rotated while A was active survives.
    rotated = _login("first", refresh="rotated")
    (native / "auth.json").write_text(rotated)
    use("second")
    assert login(native) == "second@example.com"
    assert (profiles / "first" / "auth.json").read_text() == rotated
    moved, agent_processes = converse()
    assert shared(agent_processes) and rollout(native, moved)
    use("first")
    assert (native / "auth.json").read_text() == rotated
    results["rotated_login_survived"] = True

    # A switch leaves a running shared agent's process alone, and Codex holds a
    # login for the life of the process. Codex may still fail the turn in
    # flight; the headless run resumes it.
    server.held.clear()
    server.release.clear()
    known, before = conversations(), len(launches())
    log = tempfile.TemporaryFile(mode="w+t")
    running = subprocess.Popen(
        [str(binary), "--batch", "--agent", "codex", ":", "held conversation"],
        stdin=subprocess.DEVNULL,
        cwd=work,
        env=env,
        stdout=subprocess.PIPE,
        stderr=log,
        text=True,
    )
    try:
        if not server.held.wait(30):
            running.terminate()
            stdout, stderr = running.communicate(timeout=10)
            log.seek(0)
            raise AssertionError(
                f"the running agent never reached the provider: {stdout}\n{log.read()}"
            )

        def alive() -> set[int]:
            pids = {
                json.loads(record.read_text())[0]
                for record in Path(env["LF_PROBE_AGENT_PROCESSES"]).glob("*.json")
            }
            return {
                pid
                for pid in pids
                if subprocess.run(["ps", "-p", str(pid)], capture_output=True).returncode == 0
            }

        agent_processes = alive()
        assert agent_processes, "the running agent has no provider process"
        live = [id for id in conversations() if id not in known]
        assert len(live) == 1 and session(live[0])["id"] == conversations()[live[0]], live
        use("second")
        assert login(native) == "second@example.com"
        assert alive() == agent_processes, "a provider process did not survive the switch"
        server.release.set()
        stdout, stderr = running.communicate(timeout=60)
        log.seek(0)
        assert running.returncode == 0, (stdout, log.read())
    finally:
        server.release.set()
        if running.poll() is None:
            running.kill()
            running.communicate()
        log.close()
    assert shared([launch for launch in launches()[before:] if "app-server" in launch["argv"]])
    results["running_agent_survived_switch"] = True

    # An isolated conversation runs in its account's own home and stays
    # there across a switch, whether isolation came from the flag or config.
    use("first")
    home = str(profiles / "second")
    pinned, agent_processes = converse("--account", "codex=second@example.com", "--isolate")
    assert {launch["CODEX_HOME"] for launch in agent_processes} == {home}, agent_processes
    assert rollout(profiles / "second", pinned) and not rollout(native, pinned)
    assert login(native) == "first@example.com"
    config = work / ".lf" / "config.yaml"
    config.parent.mkdir(exist_ok=True)
    config.write_text("isolate: true\n")
    standing, agent_processes = converse("--account", "codex=second@example.com")
    assert {launch["CODEX_HOME"] for launch in agent_processes} == {home}, agent_processes
    assert rollout(profiles / "second", standing) and not rollout(native, standing)
    # `--shared` overrides the standing default for one launch.
    override, agent_processes = converse("--shared")
    assert shared(agent_processes) and rollout(native, override)
    config.unlink()

    def isolated(conversation: str) -> bool:
        recorded = rows(
            "SELECT isolated FROM provider_session_accounts WHERE provider_session_id=?",
            conversation,
        )
        return recorded == [(1,)]

    assert isolated(pinned) and isolated(standing)
    assert not isolated(override) and not isolated(moved)
    # A conversation that lives in an account's home is opened there, under
    # that account, whichever account the native home has moved to.
    with plain_codex(profiles / "second") as client:
        apart = start(client)
    use("second")
    use("first")
    assert resumed(apart)["CODEX_HOME"] == home
    assert login(native) == "first@example.com"
    results["isolated_conversations"] = [pinned, standing, apart]


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
) -> None:
    _init_repo(work, env)
    for directory in ("skills", "flows"):
        (work / ".lf" / directory).mkdir(parents=True, exist_ok=True)
    (work / ".lf/skills/native-proof.md").write_text("Run the fixture command.")
    (work / ".lf/flows/native-proof.yaml").write_text(
        "- step:\n    id: work\n    name: native-proof\n- loop: work\n  step: native-proof\n"
    )
    server.fail_request = 4
    server.invalid_decision_output = not replace
    server.commands[3] = "printf failed-turn-work"
    server.commands[5] = "printf retry-work"
    command = _command(
        [str(binary), "--agent", "codex", "flow", "native-proof", "-b", "--no-loopflow"],
        work=work,
        env=env,
        timeout=90,
    )
    results.update(command_exit=command.returncode, command_stderr=command.stderr)
    with sqlite3.connect(_database(env)) as db:
        # A Flow is its Flow process and the step Processes that process recorded.
        results["flow"] = db.execute(
            "SELECT d.outcome FROM flow_processes f JOIN processes d ON d.id=f.lf_process_id"
        ).fetchone()
        results["history"] = db.execute(
            "SELECT seq,kind,provider_turn,payload FROM session_events ORDER BY seq"
        ).fetchall()
        results["steps"] = db.execute(
            "SELECT e.command FROM flow_process_steps s "
            "JOIN processes e ON e.id=s.lf_process_id ORDER BY s.seq"
        ).fetchall()
    outputs = [
        item["output"]
        for request in server.requests
        for item in request["input"]
        if item.get("type") == "function_call_output"
    ]
    results["tool_outputs"] = outputs
    assert any(ANSWER_CONTRACT in json.dumps(request["input"]) for request in server.requests)
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
    # The failed turn decided nothing. Each correction is another step Process
    # resuming the conversation that gave the invalid answer.
    corrections = [command for (command,) in results["steps"] if '"session","resume"' in command]
    if replace:
        assert command.returncode == 0 and results["flow"][0] == "succeeded", results
        assert len(results["steps"]) == 2 and not corrections, results["steps"]
    else:
        assert command.returncode != 0 and results["flow"][0] == "failed", results
        assert "structured output validation exhausted" in command.stderr, command.stderr
        assert len(results["steps"]) == 4 and len(corrections) == 2, results["steps"]
    results["failed_decision_discarded"] = "passed"


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
    (control / "agent_process.json").write_text(
        json.dumps({"endpoint": str(endpoint), "thread": thread})
    )
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
            "input": [
                {"type": "text", "text": "Continue original attachment.", "text_elements": []}
            ],
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
        "current_attachment_continued": True,
        "active_turn": active,
        "sibling_turn": sibling_turn,
    }


if __name__ == "__main__":
    main()
