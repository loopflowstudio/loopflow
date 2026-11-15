# /// script
# dependencies = ["websockets>=15,<16", "pyte>=0.8,<0.9"]
# ///
"""Exercise Task-bound native recovery through the served Session projection."""

import argparse
import base64
import datetime
import faulthandler
import fcntl
import hashlib
import http.server
import json
import os
import pty
import select
import signal
import socket
import sqlite3
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time
from pathlib import Path

import pyte

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tests/e2e"))
from claude_shared_home import _forget_native_login, _login, _seed_keychain_login
from codex_connect import Client, _stop_fixture_agent_processes


def _seed_codex_login(home: Path, name: str) -> None:
    claims = {
        "email": name + "@example.test",
        "exp": 4102444800,
        "https://api.openai.com/auth": {
            "chatgpt_account_id": name,
            "chatgpt_user_id": name,
            "chatgpt_plan_type": "plus",
        },
    }
    token = (
        "e30."
        + base64.urlsafe_b64encode(json.dumps(claims).encode()).decode().rstrip("=")
        + ".fixture"
    )
    payload = json.dumps(
        {
            "auth_mode": "chatgpt",
            "tokens": {
                "id_token": token,
                "access_token": token,
                "refresh_token": "fixture-" + name,
                "account_id": name,
            },
            "last_refresh": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        }
    ).encode()
    (home / "auth.json").write_bytes(payload)


def _stop_owned_processes(root: Path) -> None:
    # Owners deliberately outlive viewers. Stop every fixture process before
    # removing its store, including detached watchers and terminal owners.
    def owned() -> dict[int, str]:
        result = {}
        output = subprocess.check_output(["ps", "-axo", "pid=,command="], text=True)
        for line in output.splitlines():
            fields = line.strip().split(None, 1)
            if len(fields) == 2 and str(root) + "/" in fields[1]:
                result[int(fields[0])] = fields[1]
        return result

    pending = owned()
    for sig in (signal.SIGTERM, signal.SIGKILL):
        current = owned()
        for pid, command in pending.items():
            if current.get(pid) == command:
                try:
                    os.kill(pid, sig)
                except ProcessLookupError:
                    pass
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            current = owned()
            pending = {pid: cmd for pid, cmd in pending.items() if current.get(pid) == cmd}
            if not pending:
                return
            time.sleep(0.1)
    raise RuntimeError("disposable native recovery processes did not exit")


# Raise on the main thread so a hung scenario still runs fixture cleanup.
def _watchdog(signum, frame):
    faulthandler.dump_traceback()
    raise TimeoutError("native recovery probe exceeded its 15-minute watchdog")


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--lf", type=Path, required=True)
parser.add_argument("--codex", type=Path)
parser.add_argument("--claude", type=Path)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--cancel", action="store_true")
parser.add_argument("--approval", action="store_true")
# Codex: after recovery, answer the recovered login's next request with 401
# so Codex asks for a token refresh; --restart replaces the Machine server first.
parser.add_argument("--unauthorized", action="store_true")
parser.add_argument("--restart", action="store_true")
args = parser.parse_args()
signal.signal(signal.SIGALRM, _watchdog)
signal.alarm(900)
provider = "claude" if args.claude else "codex"
native_binary = args.claude or args.codex
if os.environ.get("LOOPFLOW_NETWORK_ISOLATED") != "1":
    raise RuntimeError("run with scripts/test_network.py")
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
release = threading.Event()
approval_sent = threading.Event()
unauthorized = threading.Event()
unauthorized_sent = []
requests = []
served_slow = []
get_paths = []


class Backend(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        get_paths.append(self.path)
        account = self.headers.get("chatgpt-account-id", "first")
        body = {
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
        if "/usage" in self.path:
            body = {
                "plan_type": "plus",
                "rate_limit": {
                    "allowed": True,
                    "limit_reached": False,
                    "primary_window": {
                        "used_percent": 10 if account == "first" else 20,
                        "reset_at": int(time.time()) + 3600,
                        "reset_after_seconds": 3600,
                        "limit_window_seconds": 18000,
                    },
                    "secondary_window": None,
                },
            }
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        try:
            self.wfile.write(data)
        except BrokenPipeError:
            pass  # Native preflight helpers can cancel unrelated discovery reads.

    def do_POST(self):
        if not (self.path == "/v1/responses" or self.path.startswith("/v1/messages")):
            self.send_error(404)
            return
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        account = self.headers.get("chatgpt-account-id")
        if provider == "claude":
            account = next(
                (
                    name
                    for name in ("first", "second")
                    if self.headers.get("Authorization", "").endswith("fixture-" + name)
                ),
                "unknown",
            )
        requests.append(
            {
                "account": account,
                "input": request.get("input", request.get("messages")),
                "rotated": self.headers.get("Authorization", "").endswith(".rotated"),
            }
        )
        approval_turn = (
            args.approval
            and "REJECTED_PROMPT_1" in json.dumps(request)
            and not approval_sent.is_set()
        )
        if unauthorized.is_set() and account == "second":
            unauthorized.clear()
            unauthorized_sent.append(len(requests) - 1)
            data = json.dumps({"error": {"type": "invalid_token", "message": "fixture expiry"}})
            self.send_response(401)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data.encode())
            return
        if account == "first" and len(requests) <= 2 and not approval_turn:
            release.wait()
            data = json.dumps(
                {
                    "error": {
                        "type": "usage_limit_reached",
                        "message": "fixture quota",
                        "plan_type": "plus",
                        "resets_at": int(time.time()) + 3600,
                    }
                }
            ).encode()
            if provider == "claude":
                data = json.dumps(
                    {
                        "type": "error",
                        "error": {"type": "rate_limit_error", "message": "You've hit your limit"},
                    }
                ).encode()
            self.send_response(429)
            if provider == "claude":
                self.send_header("anthropic-ratelimit-unified-status", "rejected")
                self.send_header("anthropic-ratelimit-unified-reset", str(int(time.time()) + 3600))
                # Claude 2.1.325 reads a 429 without a claim as server throttling.
                self.send_header("anthropic-ratelimit-unified-representative-claim", "five_hour")
                self.send_header("anthropic-ratelimit-unified-overage-status", "rejected")
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        if provider == "claude":
            if approval_turn:
                approval_sent.set()
            block = (
                {"type": "tool_use", "id": "approval", "name": "Bash", "input": {}}
                if approval_turn
                else {"type": "text", "text": ""}
            )
            delta = (
                {
                    "type": "input_json_delta",
                    "partial_json": json.dumps(
                        {"command": f"echo EFFECT >> {root / 'approval-effect'}"}
                    ),
                }
                if approval_turn
                else {"type": "text_delta", "text": "RECOVERY_COMPLETED"}
            )
            message = {
                "id": f"msg_{len(requests)}",
                "type": "message",
                "role": "assistant",
                "model": request["model"],
                "content": [],
                "stop_reason": None,
                "stop_sequence": None,
                "usage": {"input_tokens": 10, "output_tokens": 1},
            }
            for kind, data in [
                ("message_start", {"message": message}),
                (
                    "content_block_start",
                    {"index": 0, "content_block": block},
                ),
                (
                    "content_block_delta",
                    {"index": 0, "delta": delta},
                ),
                ("content_block_stop", {"index": 0}),
                (
                    "message_delta",
                    {
                        "delta": {
                            "stop_reason": "tool_use" if approval_turn else "end_turn",
                            "stop_sequence": None,
                        },
                        "usage": {"output_tokens": 5},
                    },
                ),
                ("message_stop", {}),
            ]:
                self.wfile.write(
                    f"event: {kind}\ndata: {json.dumps({'type': kind, **data})}\n\n".encode()
                )
                self.wfile.flush()
            return
        item = {
            "id": "msg_fixture",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "RECOVERY_COMPLETED"}],
        }
        if approval_turn:
            approval_sent.set()
            item = {
                "id": "fc_approval",
                "type": "function_call",
                "name": "exec_command",
                "call_id": "approval",
                "arguments": json.dumps(
                    {
                        "cmd": f"/bin/echo EFFECT >> {root / 'approval-effect'}",
                        "sandbox_permissions": "require_escalated",
                        "justification": "Allow disposable recovery fixture?",
                    }
                ),
            }
        response = {"id": "resp_fixture", "status": "in_progress", "output": []}
        for kind, data in [
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
        ]:
            self.wfile.write(
                f"event: {kind}\ndata: {json.dumps({'type': kind, **data})}\n\n".encode()
            )
            self.wfile.flush()


backend = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Backend)
worker = threading.Thread(target=backend.serve_forever)
clients = []
server = None
credential_lock = None
tasks = []
masters = []
terminal = [bytearray(), bytearray()]
screens = [pyte.Screen(120, 40), pyte.Screen(120, 40)]
streams = [pyte.ByteStream(screen) for screen in screens]
terminal_lock = threading.Lock()
stop_reader = threading.Event()
reader = None
results = {"candidate_sha256": hashlib.sha256(args.lf.read_bytes()).hexdigest()}
with tempfile.TemporaryDirectory(prefix="loo811-product-", dir="/tmp") as temporary:
    root = Path(temporary).resolve()
    native = root / ("." + provider)
    native.mkdir()
    bindir = root / "bin"
    bindir.mkdir()
    processes = root / "processes"
    processes.mkdir()
    # Keep each limit hook's input: Claude's StopFailure shape decides recovery.
    (bindir / "lf").write_text(
        "#!/bin/sh\n"
        'case "$*" in *"--moment limit"*)\n'
        f' tee "{root}/hook-$$.json" | exec {args.lf.resolve()} "$@";;\n'
        ' *"--moment record-limit"*)\n'
        f' RUST_LOG=debug exec {args.lf.resolve()} "$@" '
        f'2>"{root}/hook-$$.err";;\n'
        f' *) exec {args.lf.resolve()} "$@";;\nesac\n'
    )
    (bindir / "lf").chmod(0o700)
    native_args = ["--permission-mode", "default"] if args.approval and provider == "claude" else []
    wrapper = bindir / provider
    wrapper.write_text(f"""#!{sys.executable}
import os,sys,subprocess,json
from pathlib import Path
if 'app-server' in sys.argv:
 pid=os.getpid()
 stamp=subprocess.check_output(['ps','-p',str(pid),'-o','lstart='],text=True).strip()
 Path({str(processes)!r},str(pid)+'.json').write_text(json.dumps([pid,stamp]))
debug=[]
if {provider!r}=='claude':
 debug=['--debug-file',{str(root)!r}+'/claude-debug-'+str(os.getpid())+'.log']
os.execv({str(native_binary.resolve())!r},[{str(native_binary.resolve())!r},*debug,*{native_args!r},*sys.argv[1:]])
""")
    wrapper.chmod(0o700)
    config = f"""cli_auth_credentials_store="file"
chatgpt_base_url="http://127.0.0.1:{backend.server_port}/backend-api/"
model="fixture"
model_provider="fixture"
[model_providers.fixture]
name="fixture"
base_url="http://127.0.0.1:{backend.server_port}/v1"
wire_api="responses"
requires_openai_auth=true
[projects.{json.dumps(str(root))}]
trust_level="trusted"
"""
    for i in range(2):
        config += f'\n[projects.{json.dumps(str(root / f"task-{i}"))}]\ntrust_level="trusted"\n'
    if args.approval:
        assert not args.cancel
        config = 'approval_policy="on-request"\nsandbox_mode="read-only"\n' + config
    if provider == "codex":
        (native / "config.toml").write_text(config)
        _seed_codex_login(native, "first")
    else:
        (native / ".claude.json").write_text(
            json.dumps(
                {
                    "hasCompletedOnboarding": True,
                    "theme": "dark",
                    "projects": {
                        str(path): {"hasTrustDialogAccepted": True}
                        for path in (root, root / "task-0", root / "task-1")
                    },
                }
            )
        )
    env = {
        "HOME": str(root),
        "LF_HOME": str(root / "machine"),
        "CODEX_HOME": str(native),
        "PATH": f"{bindir}:/usr/bin:/bin",
        "TERM": "xterm-256color",
        "LF_BIN": str(bindir / "lf"),
        "GIT_CONFIG_NOSYSTEM": "1",
    }
    if provider == "claude":
        env.pop("CODEX_HOME")
        env.update(
            HOME=str(Path.home()),
            USER="fixture-current",
            CLAUDE_CONFIG_DIR=str(native),
            ANTHROPIC_BASE_URL=f"http://127.0.0.1:{backend.server_port}",
            CLAUDE_CODE_MAX_RETRIES="0",
            CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="1",
            DISABLE_TELEMETRY="1",
        )

    def lf(*argv):
        result = subprocess.run(
            [str(bindir / "lf"), *argv],
            cwd=root,
            env=env,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=180,
        )
        if result.returncode:
            raise RuntimeError(result.stderr)
        return result.stdout

    def served(operation):
        status = json.loads(lf("server", "status"))
        assert status["ready"], status
        with socket.socket(socket.AF_UNIX) as connection:
            connection.settimeout(120)
            began = time.monotonic()
            connection.connect(status["socket"])
            stream = connection.makefile("rwb")
            hello = {
                "type": "hello",
                "revision": 5,
                "min_revision": 5,
                "client": {
                    "kind": "fixture",
                    "platform": "macos",
                    "name": "native-recovery",
                    "version": "1",
                },
            }
            stream.write(json.dumps(hello).encode() + b"\n")
            stream.flush()
            assert json.loads(stream.readline())["type"] == "welcome"
            stream.write(
                json.dumps(
                    {"type": "request", "id": "check", "operation": operation, "select": None}
                ).encode()
                + b"\n"
            )
            stream.flush()
            response = json.loads(stream.readline())
            elapsed = time.monotonic() - began
            if elapsed > 2:
                served_slow.append([operation["kind"], round(elapsed, 1), time.time()])
            assert response["type"] == "response", response
            return response["result"]

    def sessions():
        listed = served(
            {
                "kind": "session.list",
                "directory": str(root),
                "headless": False,
                "after": None,
                "limit": None,
                "task": None,
            }
        )
        return [session for session in listed if set(session["task_ids"]) & set(tasks)]

    def pump_terminal():
        # Keep draining while synchronous setup/served commands run too. A full
        # PTY must never block the client before it processes composer input.
        while not stop_reader.is_set():
            for fd in select.select(masters, [], [], 0.05)[0]:
                try:
                    data = os.read(fd, 65536)
                except OSError:
                    continue
                i = masters.index(fd)
                with terminal_lock:
                    terminal[i].extend(data)
                    streams[i].feed(data)
                if b"\x1b[6n" in data:
                    os.write(fd, b"\x1b[1;1R")

    def read(seconds):
        time.sleep(seconds)  # The reader continues through every blocking call.

    def screen_text(i):
        with terminal_lock:
            return "\n".join(screens[i].display)

    def wait_for(condition, label):
        deadline = time.monotonic() + 60
        while not condition():
            assert all(child.poll() is None for child in clients), label + ": client exited"
            if time.monotonic() >= deadline:
                raise AssertionError(label)
            read(0.05)

    def paste_draft(i):
        # Raw mode and native bracketed-paste enablement rule out kernel echo
        # or an lf startup screen masquerading as composer acceptance.
        def ready():
            flags = termios.tcgetattr(masters[i])[3]
            with terminal_lock:
                paste_enabled = b"\x1b[?2004h" in terminal[i]
            return paste_enabled and not flags & (termios.ICANON | termios.ECHO)

        wait_for(ready, f"composer {i} did not enter raw paste mode")
        os.write(masters[i], f"\x1b[200~DRAFT_{i}\x1b[201~".encode())
        wait_for(lambda: f"DRAFT_{i}" in screen_text(i), f"draft {i} never appeared")
        (out / f"draft-{i}-before.txt").write_text(screen_text(i))

    try:
        worker.start()
        subprocess.run(["git", "init", "-q", str(root)], env=env, check=True)
        subprocess.run(
            ["git", "-C", str(root), "config", "user.email", "fixture@example.test"],
            env=env,
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(root), "config", "user.name", "Fixture"], env=env, check=True
        )
        subprocess.run(
            ["git", "-C", str(root), "commit", "--allow-empty", "-qm", "Fixture"],
            env=env,
            check=True,
        )
        lf("session", "list", "--json")
        for i in range(2):
            checkout = root / f"task-{i}"
            subprocess.run(
                [
                    "git",
                    "-C",
                    str(root),
                    "worktree",
                    "add",
                    "-qb",
                    f"task-{i}",
                    str(checkout),
                    "HEAD",
                ],
                env=env,
                check=True,
            )
            task = json.loads(
                lf(
                    "task",
                    "create",
                    "--title",
                    f"Recovery fixture {i}",
                    "--worktree",
                    str(checkout),
                    "--json",
                )
            )
            tasks.append(task["task"]["id"])
        server = subprocess.Popen(
            [str(bindir / "lf"), "server", "run"],
            cwd=root,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )

        with sqlite3.connect(root / "machine/loopflow.db") as db:
            for name in ("first", "second"):
                profile = root / "machine/accounts" / provider / name
                profile.mkdir(parents=True)
                if provider == "codex":
                    _seed_codex_login(profile, name)
                    (profile / "config.toml").write_text(config)
                    db.execute(
                        "INSERT INTO provider_accounts(provider,account_id,home,credential_state,"
                        "created_at,updated_at) VALUES('codex',?,?,'connected',?,?)",
                        (name, str(profile), int(time.time()), int(time.time())),
                    )
                else:
                    credential = json.dumps(_login(name))
                    (profile / ".credentials.json").write_text(credential)
                    db.execute(
                        "INSERT INTO provider_accounts(provider,account_id,home,credential_state,"
                        "created_at,updated_at,observed_email,observed_subject,observed_credential_digest)"
                        " VALUES('claude',?,?,'connected',?,?,?,?,?)",
                        (
                            name,
                            str(profile),
                            int(time.time()),
                            int(time.time()),
                            name + "@example.test",
                            name,
                            hashlib.sha256(credential.encode()).hexdigest(),
                        ),
                    )
        for name in ("first", "second"):
            lf(
                "set",
                "--global",
                f"accounts.{provider}.{name}",
                f"{{login_email: {name}@example.test, routing_state: automatic}}",
            )
        if provider == "claude":
            _seed_keychain_login(native, "fixture-current", "first")
            _seed_keychain_login(native, "fixture-legacy", "first")
        for i in range(2):
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))

            def own():
                os.setsid()
                fcntl.ioctl(0, termios.TIOCSCTTY, 0)

            clients.append(
                subprocess.Popen(
                    [
                        str(bindir / "lf"),
                        "--task",
                        tasks[i],
                        "--account",
                        f"{provider}=first",
                        "-a",
                        "claude:sonnet" if provider == "claude" else "codex:fixture",
                        ":",
                        f"REJECTED_PROMPT_{i}",
                    ],
                    cwd=root,
                    env=env,
                    stdin=slave,
                    stdout=slave,
                    stderr=slave,
                    preexec_fn=own,
                )
            )
            os.close(slave)
            masters.append(master)
        reader = threading.Thread(target=pump_terminal)
        reader.start()
        for _ in range(45):
            read(1)
            if len(requests) >= 2 or any(child.poll() is not None for child in clients):
                break
        assert all(child.poll() is None for child in clients), "native launch failed"
        # Claude keeps the real HOME for its Keychain, so the list can include
        # native conversations outside this Machine. Only this fixture's Tasks count.
        records = [
            record
            for record in json.loads(lf("session", "list", "--all", "--json"))
            if set(record["task_ids"]) & set(tasks)
        ]
        assert len(records) == 2, records
        for record in records:
            lf("session", "bind", record["id"], "--task", record["task_ids"][0])
        before = sessions()
        results["before"] = before
        for i, fd in enumerate(masters):
            if not args.approval or i == 0:
                paste_draft(i)
        results["drafts_visible_before_recovery"] = all(
            f"DRAFT_{i}" in screen_text(i) for i in range(1 if args.approval else 2)
        )
        assert results["drafts_visible_before_recovery"]
        assert len(requests) == 2, "backend released rejection before drafts were present"
        if args.cancel:
            credential_lock = open(root / "machine/accounts" / provider / ".native.lock", "a+")
            fcntl.flock(credential_lock, fcntl.LOCK_EX)
        release.set()
        if args.cancel:
            for _ in range(30):
                read(0.5)
                with sqlite3.connect(root / "machine/loopflow.db") as db:
                    admitted = db.execute(
                        "SELECT COUNT(*) FROM session_events "
                        "WHERE receipt_key LIKE 'recovery:%:admitted'"
                    ).fetchone()[0]
                if admitted == 2:
                    break
            assert admitted == 2, "native submissions did not reach their guards"
            for fd in masters:
                os.write(fd, b"\x1b")
            read(1)
            fcntl.flock(credential_lock, fcntl.LOCK_UN)
            credential_lock.close()
            credential_lock = None
        expected_state = "uncertain" if args.cancel else "continued"
        for _ in range(100):
            read(1)
            results["sessions"] = sessions()
            if len(
                [
                    session
                    for session in results["sessions"]
                    if (session.get("recovery") or {}).get("state") == expected_state
                ]
            ) == (1 if args.approval else 2):
                break
        results["expected_recoveries_settled"] = len(results["sessions"]) == 2 and sum(
            (session.get("recovery") or {}).get("state") == expected_state
            for session in results["sessions"]
        ) == (1 if args.approval else 2)
        results["expected_state"] = expected_state
        prior = {record["id"]: record for record in before}
        results["task_identity_preserved"] = all(
            current["task_primary"]
            and all(
                current[key] == prior[current["id"]][key]
                for key in ("task_ids", "task_primary", "cwd", "agent_process_id", "model")
            )
            for current in results["sessions"]
        )
        # Desktop invokes the same served SessionRecover mutation. Checking a
        # dispatched/completed attempt again must not produce another input.
        results["served_recovery"] = served(
            {
                "kind": "session.recover",
                "directory": str(root),
                "ids": [record["id"] for record in results["sessions"]],
            }
        )
        read(2)
        assert results["expected_recoveries_settled"], results["sessions"]
        results["native_threads"] = []
        with sqlite3.connect(root / "machine/loopflow.db") as db:
            endpoints = db.execute(
                "SELECT p.endpoint,s.provider_thread FROM agent_sessions s "
                "JOIN processes p ON p.id=s.agent_process_id WHERE p.endpoint IS NOT NULL"
            ).fetchall()
        for endpoint, thread in endpoints if provider == "codex" else []:
            connection = Client(Path(endpoint.removeprefix("unix://")))
            try:
                results["native_threads"].append(
                    connection.call("thread/read", {"threadId": thread, "includeTurns": True})
                )
            finally:
                connection.close()
        if provider == "codex":
            results["one_retry_each"] = all(
                sum(
                    item["type"] == "userMessage"
                    and any(
                        part.get("text", "").startswith("<lf-recovery:")
                        for part in item.get("content", [])
                    )
                    for turn in record["thread"]["turns"]
                    for item in turn["items"]
                )
                == (
                    0
                    if args.cancel or (args.approval and "REJECTED_PROMPT_1" in json.dumps(record))
                    else 1
                )
                for record in results["native_threads"]
            )
        else:
            results["one_retry_each"] = all(
                sum(
                    row.get("type") == "user"
                    and f'"attempt":"{session["recovery"]["request"]}"'
                    in str(row.get("message", {}).get("content"))
                    for transcript in native.rglob("*.jsonl")
                    for row in map(json.loads, transcript.read_text().splitlines())
                )
                == (0 if args.cancel else 1)
                for session in results["sessions"]
                if session.get("recovery") is not None
            )
        results["drafts_visible_after_recovery"] = all(
            f"DRAFT_{i}" in screen_text(i) for i in range(1 if args.approval else 2)
        )
        for i in range(1 if args.approval else 2):
            (out / f"draft-{i}-after.txt").write_text(screen_text(i))
        assert results["drafts_visible_after_recovery"], "recovery removed a visible draft"
        results["drafts_unsent_after_recovery"] = all(
            f"DRAFT_{i}" not in json.dumps(requests) for i in range(1 if args.approval else 2)
        )
        assert results["drafts_unsent_after_recovery"], "recovery submitted an unsent draft"
        if args.approval:
            claude_content = (
                [
                    block
                    for transcript in native.rglob("*.jsonl")
                    for row in map(json.loads, transcript.read_text().splitlines())
                    if isinstance(row.get("message", {}).get("content"), list)
                    for block in row["message"]["content"]
                ]
                if provider == "claude"
                else []
            )
            results["approval_pending_after_recovery"] = (
                approval_sent.is_set()
                and not (root / "approval-effect").exists()
                and (
                    any(
                        turn["status"] == "inProgress"
                        for record in results["native_threads"]
                        for turn in record["thread"]["turns"]
                    )
                    if provider == "codex"
                    else any(
                        block.get("type") == "tool_use" and block.get("id") == "approval"
                        for block in claude_content
                    )
                    and not any(
                        block.get("type") == "tool_result"
                        and block.get("tool_use_id") == "approval"
                        for block in claude_content
                    )
                )
            )
            assert results["approval_pending_after_recovery"], "pending approval did not survive"
            os.write(masters[1], b"y" if provider == "codex" else b"1")
            read(0.3)
            os.write(masters[1], b"\r")
            for _ in range(60):
                read(1)
                if (root / "approval-effect").exists():
                    break
            read(2)
            results["completed_effect_once"] = (root / "approval-effect").read_text() == "EFFECT\n"
            assert results["completed_effect_once"]
            paste_draft(1)
        if args.restart:
            assert provider == "codex" and not args.cancel and not args.approval
            server.terminate()
            server.wait(timeout=30)
            server = subprocess.Popen(
                [str(bindir / "lf"), "server", "run"],
                cwd=root,
                env=env,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                start_new_session=True,
            )

            def replacement_ready():
                try:
                    return json.loads(lf("server", "status"))["ready"]
                except RuntimeError:
                    return False

            wait_for(replacement_ready, "replacement Machine server did not become ready")
            read(5)  # The replacement reader reconnects to each live conversation.
            results["sessions_after_restart"] = sessions()
        if args.unauthorized or args.restart:
            # Rotate the broker's copy: only a refresh answered from it can carry it.
            stored = root / "machine/accounts/codex/second/auth.json"
            login = json.loads(stored.read_text())
            login["tokens"]["access_token"] = (
                login["tokens"]["access_token"].removesuffix(".fixture") + ".rotated"
            )
            stored.write_text(json.dumps(login))
            unauthorized.set()
        for fd in masters:
            os.write(fd, b"\x1b[200~_EDITED\x1b[201~")
        wait_for(
            lambda: all(f"DRAFT_{i}_EDITED" in screen_text(i) for i in range(2)),
            "edited drafts did not appear in both composers",
        )
        for fd in masters:
            os.write(fd, b"\r")
        wait_for(
            lambda: all(
                any(f"DRAFT_{i}_EDITED" in json.dumps(request["input"]) for request in requests)
                for i in range(2)
            ),
            "edited drafts did not reach inference",
        )
        results["drafts_intact"] = all(
            any(f"DRAFT_{i}_EDITED" in json.dumps(request["input"]) for request in requests)
            for i in range(2)
        )
        results["native_clients_alive"] = all(child.poll() is None for child in clients)
        results["selected_login"] = [request["account"] for request in requests[:2]] == [
            "first",
            "first",
        ] and all(
            request["account"]
            == (
                "first"
                if args.cancel
                or (
                    provider == "codex"
                    and args.approval
                    and "REJECTED_PROMPT_1" in json.dumps(request["input"])
                )
                else "second"
            )
            for request in requests[2:]
        )

        def typed_completed():
            if provider == "codex":
                completed = []
                for endpoint, thread in endpoints:
                    connection = Client(Path(endpoint.removeprefix("unix://")))
                    try:
                        record = connection.call(
                            "thread/read", {"threadId": thread, "includeTurns": True}
                        )
                        completed.extend(
                            item
                            for turn in record["thread"]["turns"]
                            if turn["status"] == "completed"
                            for item in turn["items"]
                            if item["type"] == "userMessage"
                        )
                    finally:
                        connection.close()
            else:
                with sqlite3.connect(root / "machine/loopflow.db") as db:
                    stopped = {
                        row[0]
                        for row in db.execute(
                            "SELECT receipt_key FROM session_events "
                            "WHERE receipt_key LIKE 'quota_boundary:%:stopped'"
                        )
                    }
                completed = [
                    row["message"]
                    for transcript in native.rglob("*.jsonl")
                    # Ignore a record the native client is still appending.
                    for row in map(json.loads, transcript.read_text().split("\n")[:-1])
                    if row.get("type") == "user"
                    and f"quota_boundary:{transcript.stem}:{row.get('promptId')}:stopped" in stopped
                ]
            return all(
                any(f"DRAFT_{i}_EDITED" in json.dumps(item) for item in completed) for i in range(2)
            )

        # A previous retry's terminal marker cannot establish typed completion.
        wait_for(typed_completed, "edited drafts did not complete natively")
        results["typed_completion"] = True
        # The recovery row ends at the person's next message in that conversation.
        for _ in range(60):
            results["sessions_after_input"] = sessions()
            if all(session.get("recovery") is None for session in results["sessions_after_input"]):
                break
            read(1)
        results["rows_ended_after_input"] = all(
            session.get("recovery") is None for session in results["sessions_after_input"]
        )
        if args.unauthorized or args.restart:
            # The 401'd request is retried with the token only the broker held.
            [rejected] = unauthorized_sent
            results["refresh_from_broker"] = any(
                request["rotated"] and request["input"] == requests[rejected]["input"]
                for request in requests[rejected + 1 :]
            )
            assert results["refresh_from_broker"], "Codex's retry did not use the refreshed login"
        results["requests"] = requests
        results["get_paths"] = get_paths
        assert all(
            results[name]
            for name in (
                "expected_recoveries_settled",
                "task_identity_preserved",
                "one_retry_each",
                "selected_login",
                "typed_completion",
                "drafts_intact",
                "native_clients_alive",
                "rows_ended_after_input",
            )
        )
    finally:
        signal.alarm(0)
        if credential_lock is not None:
            fcntl.flock(credential_lock, fcntl.LOCK_UN)
            credential_lock.close()
        try:
            with sqlite3.connect(root / "machine/loopflow.db") as db:
                results["recovery_receipts"] = [
                    (session, key, json.loads(payload))
                    for session, key, payload in db.execute(
                        "SELECT session_id,receipt_key,payload FROM session_events "
                        "WHERE receipt_key LIKE 'recovery:%' ORDER BY seq"
                    )
                ]
                results["client_rows"] = [
                    list(row)
                    for row in db.execute(
                        "SELECT s.id,s.provider_thread,s.interactive,p.id,p.endpoint,p.pid,"
                        "p.os_started_at,p.completed_at,p.agent_interactive,"
                        "(SELECT count(*) FROM steer_adapters a WHERE a.agent_process_id=p.id) "
                        "FROM agent_sessions s LEFT JOIN processes p ON p.id=s.agent_process_id"
                    )
                ]
                results["event_keys"] = [
                    (session, kind, key)
                    for session, kind, key in db.execute(
                        "SELECT session_id,kind,receipt_key FROM session_events ORDER BY seq"
                    )
                ]
        except sqlite3.Error as error:
            results["receipt_error"] = str(error)
        release.set()
        results["get_paths"] = get_paths
        results["requests"] = requests
        results["served_slow"] = served_slow
        results["unauthorized_sent"] = unauthorized_sent
        if args.unauthorized or args.restart:
            # Codex's own log says whether the 401 asked Loopflow for a refresh.
            for log in native.glob("logs_*.sqlite"):
                with sqlite3.connect(log) as source, sqlite3.connect(out / log.name) as copy:
                    source.backup(copy)
        stop_reader.set()
        if reader is not None:
            reader.join()
        for i, data in enumerate(terminal):
            (out / f"terminal-{i}.bin").write_bytes(data)
        for fd in masters:
            os.close(fd)
        for child in clients:
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait(timeout=5)
        if server is not None:
            server.terminate()
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait(timeout=5)
        _stop_fixture_agent_processes(processes)
        _stop_owned_processes(root)
        results["owners_reaped"] = True
        backend.shutdown()
        backend.server_close()
        worker.join()
        if provider == "claude":
            for debug in [
                *root.glob("claude-debug-*.log"),
                *root.glob("hook-*.json"),
                *root.glob("hook-*.err"),
            ]:
                (out / debug.name).write_bytes(debug.read_bytes())
            for transcript in native.rglob("*.jsonl"):
                (out / transcript.name).write_bytes(transcript.read_bytes())
            _forget_native_login(native)
        if provider == "claude":
            subprocess.run(
                ["sqlite3", str(root / "machine/loopflow.db"), f".backup {out / 'machine.db'}"]
            )
        if (root / "machine/logs/lf.log").exists():
            (out / "lf.log").write_bytes((root / "machine/logs/lf.log").read_bytes())
        results["clients_reaped"] = all(child.poll() is not None for child in clients)
        (out / "results.json").write_text(json.dumps(results, indent=2) + "\n")
        print(
            json.dumps(
                {
                    k: v
                    for k, v in results.items()
                    if k
                    not in (
                        "sessions",
                        "before",
                        "requests",
                        "get_paths",
                        "native_threads",
                        "recovery_receipts",
                        "event_keys",
                        "client_rows",
                        "served_recovery",
                        "sessions_after_input",
                    )
                }
            )
        )
