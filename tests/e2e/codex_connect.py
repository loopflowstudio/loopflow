# /// script
# requires-python = ">=3.10"
# dependencies = ["websockets>=15,<16"]
# ///
"""Exercise a real Codex app-server with credential-free local Responses and private Machines."""

import argparse
import base64
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


def _agent_process(database: sqlite3.Connection, session: str, columns: str) -> tuple:
    """Read the Session's current AgentProcess (p) beside the Session (s)."""
    return database.execute(
        f"SELECT {columns} FROM agent_sessions s "
        "LEFT JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?",
        (session,),
    ).fetchone()


class Responses(ThreadingHTTPServer):
    def __init__(self) -> None:
        super().__init__(("127.0.0.1", 0), Handler)
        self.requests: list[dict] = []
        self.fail_request: int | None = None
        self.commands: dict[int, str] = {}
        self.invalid_decision_output = False
        self.decision_outputs: list[dict] = []
        self.hold_seconds = 20
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
        prompt = request.get("instructions", "") + json.dumps(request["input"][last_user])
        held = (
            "held conversation",
            "held fixture",
            "held before driver death",
            "held after driver death",
        )
        if not outputs and any(marker in prompt for marker in held):
            self.server.held.set()
            assert self.server.release.wait(self.server.hold_seconds), (
                "held response was not released"
            )
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
                        "cmd": self.server.commands.get(
                            len(self.server.requests), self.server.command
                        ),
                        "max_output_tokens": 100,
                    }
                ),
            }
        else:
            text = "Fixture complete."
            # A Graph process writes the answer contract into the message.
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

    def call(self, method: str, params: dict) -> dict:
        self.next_id += 1
        ident = self.next_id
        self.socket.send(json.dumps({"id": ident, "method": method, "params": params}))
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            event = self.pending.get(timeout=max(0.1, deadline - time.monotonic()))
            if event.get("id") == ident:
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
    parser.add_argument("--launch", action="store_true")
    parser.add_argument("--graph-decision-retry", choices=("missing", "replace"))
    args = parser.parse_args()
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
            home = root / "codex"
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
                    if args.graph_decision_retry:
                        _graph_decision_retry_contract(
                            binary,
                            work,
                            env,
                            server,
                            results,
                            replace=args.graph_decision_retry == "replace",
                        )
                        return
                    parser.error("--launch requires a Graph proof mode")
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
        if "resume" in sys.argv and not os.environ.get("LF_PROBE_CLIENT"):
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
            raise RuntimeError("public resume bypassed the retained provider")
        os.execv(os.environ["LF_PROBE_CODEX"], [os.environ["LF_PROBE_CODEX"], *sys.argv[1:]])
    # A controlled native-protocol client exercises public lf resume. This is
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
            value = client.call(action["method"], action["params"])
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


def _graph_decision_retry_contract(
    binary: Path,
    work: Path,
    env: dict[str, str],
    server: Responses,
    results: dict,
    *,
    replace: bool,
) -> None:
    _init_repo(work, env)
    for directory in ("skills", "graphs"):
        (work / ".lf" / directory).mkdir(parents=True, exist_ok=True)
    (work / ".lf/skills/native-proof.md").write_text("Run the fixture command.")
    (work / ".lf/graphs/native-proof.yaml").write_text(
        "- loop:\n    - step:\n        id: work\n        name: native-proof\n    - native-proof\n"
    )
    server.fail_request = 4
    server.invalid_decision_output = not replace
    server.commands[3] = "printf failed-turn-work"
    server.commands[5] = "printf retry-work"
    command = _command(
        [str(binary), "--agent", "codex", "graph", "native-proof", "-b", "--no-loopflow"],
        work=work,
        env=env,
        timeout=90,
    )
    results.update(command_exit=command.returncode, command_stderr=command.stderr)
    with sqlite3.connect(_database(env)) as db:
        # A Graph is its Graph process and the step Processes that process recorded.
        results["graph"] = db.execute(
            "SELECT d.outcome FROM graph_processes f JOIN processes d ON d.id=f.lf_process_id"
        ).fetchone()
        results["history"] = db.execute(
            "SELECT seq,kind,provider_turn,payload FROM session_events ORDER BY seq"
        ).fetchall()
        results["steps"] = db.execute(
            "SELECT e.command FROM graph_process_steps s "
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
        assert command.returncode == 0 and results["graph"][0] == "succeeded", results
        assert len(results["steps"]) == 2 and not corrections, results["steps"]
    else:
        assert command.returncode != 0 and results["graph"][0] == "failed", results
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


if __name__ == "__main__":
    main()
