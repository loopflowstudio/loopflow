# /// script
# dependencies = ["websockets>=15,<16"]
# ///
"""Exercise recovery admission waits and native input with two isolated TUIs."""

import argparse
import fcntl
import hashlib
import http.server
import json
import os
import pty
import select
import signal
import sqlite3
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time
from pathlib import Path

from codex_auth_reload_probe import _auth

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tests/e2e"))
from claude_shared_home import _forget_native_login, _login, _seed_keychain_login
from codex_connect import Client, _stop_fixture_agent_processes


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


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--lf", type=Path, required=True)
parser.add_argument("--codex", type=Path)
parser.add_argument("--claude", type=Path)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
provider = "claude" if args.claude else "codex"
native_binary = args.claude or args.codex
if os.environ.get("LOOPFLOW_NETWORK_ISOLATED") != "1":
    raise RuntimeError("run with scripts/test_network.py")
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
release = threading.Event()
requests = []
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
            {"account": account, "input": request.get("input", request.get("messages"))}
        )
        if account == "first" and len(requests) <= 2:
            release.wait(30)
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
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        if provider == "claude":
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
                    {"index": 0, "content_block": {"type": "text", "text": ""}},
                ),
                (
                    "content_block_delta",
                    {"index": 0, "delta": {"type": "text_delta", "text": "RECOVERY_COMPLETED"}},
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
worker.start()
clients = []
masters = []
terminal = [bytearray(), bytearray()]
results = {}
with tempfile.TemporaryDirectory(prefix="loo811-product-", dir="/tmp") as temporary:
    root = Path(temporary).resolve()
    native = root / ("." + provider)
    native.mkdir()
    bindir = root / "bin"
    bindir.mkdir()
    processes = root / "processes"
    processes.mkdir()
    (bindir / "lf").symlink_to(args.lf.resolve())
    wrapper = bindir / provider
    wrapper.write_text(f"""#!{sys.executable}
import os,sys,subprocess,json
from pathlib import Path
if 'app-server' in sys.argv:
 pid=os.getpid()
 stamp=subprocess.check_output(['ps','-p',str(pid),'-o','lstart='],text=True).strip()
 Path({str(processes)!r},str(pid)+'.json').write_text(json.dumps([pid,stamp]))
os.execv({str(native_binary.resolve())!r},[{str(native_binary.resolve())!r},*sys.argv[1:]])
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
    if provider == "codex":
        (native / "config.toml").write_text(config)
        _auth(native, "first")
    else:
        (native / ".claude.json").write_text(
            json.dumps(
                {
                    "hasCompletedOnboarding": True,
                    "theme": "dark",
                    "projects": {str(root): {"hasTrustDialogAccepted": True}},
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
        "RUST_LOG": "warn",
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
            timeout=60,
        )
        if result.returncode:
            raise RuntimeError(result.stderr)
        return result.stdout

    def read(seconds):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            ready = select.select(masters, [], [], 0.05)[0]
            for fd in ready:
                try:
                    data = os.read(fd, 65536)
                except OSError:
                    continue
                terminal[masters.index(fd)].extend(data)
                if b"\x1b[6n" in data:
                    os.write(fd, b"\x1b[1;1R")

    try:
        subprocess.run(["git", "init", "-q", str(root)], env=env, check=True)
        subprocess.run(
            ["git", "-C", str(root), "config", "user.email", "fixture@example.test"],
            env=env,
            check=True,
        )
        lf("session", "list", "--json")
        with sqlite3.connect(root / "machine/loopflow.db") as db:
            for name in ("first", "second"):
                profile = root / "machine/accounts" / provider / name
                profile.mkdir(parents=True)
                if provider == "codex":
                    _auth(profile, name)
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
        for _ in range(45):
            read(1)
            if len(requests) >= 2 or any(child.poll() is not None for child in clients):
                break
        assert all(child.poll() is None for child in clients), "native launch failed"
        for i, fd in enumerate(masters):
            os.write(fd, f"\x1b[200~DRAFT_{i}\x1b[201~".encode())
        read(1)
        release.set()
        for _ in range(45):
            read(1)
            results["sessions"] = json.loads(lf("session", "list", "--all", "--json"))
            if len(results["sessions"]) == 2 and all(
                (session.get("recovery") or {}).get("state") == "waiting_native"
                and "cannot condition" in session["recovery"]["reason"]
                for session in results["sessions"]
            ):
                break
        results["admission_wait_both"] = len(results["sessions"]) == 2 and all(
            (session.get("recovery") or {}).get("state") == "waiting_native"
            and "cannot condition" in session["recovery"]["reason"]
            and session["recovery"]["request"] is None
            for session in results["sessions"]
        )
        assert results["admission_wait_both"], results["sessions"]
        results["no_recovery_input"] = len(requests) == 2
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
        for fd in masters:
            os.write(fd, b"\x1b[200~_EDITED\x1b[201~")
        read(0.5)
        for fd in masters:
            os.write(fd, b"\r")
        read(6)
        results["drafts_intact"] = all(
            any(f"DRAFT_{i}_EDITED" in json.dumps(request["input"]) for request in requests)
            for i in range(2)
        )
        results["native_clients_alive"] = all(child.poll() is None for child in clients)
        results["no_credential_handoff"] = all(request["account"] == "first" for request in requests)
        results["typed_completion"] = all(b"RECOVERY_COMPLETED" in data for data in terminal)
        results["only_typed_turns"] = len(requests) == 4
        results["requests"] = requests
        results["get_paths"] = get_paths
        assert (
            results["admission_wait_both"]
            and results["no_recovery_input"]
            and results["no_credential_handoff"]
            and results["only_typed_turns"]
            and results["typed_completion"]
            and results["drafts_intact"]
            and results["native_clients_alive"]
        )
    finally:
        release.set()
        results["get_paths"] = get_paths
        results["requests"] = requests
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
        _stop_fixture_agent_processes(processes)
        _stop_owned_processes(root)
        results["owners_reaped"] = True
        if provider == "claude":
            for transcript in native.rglob("*.jsonl"):
                (out / transcript.name).write_bytes(transcript.read_bytes())
            _forget_native_login(native)
        if (root / "machine/logs/lf.log").exists():
            (out / "lf.log").write_bytes((root / "machine/logs/lf.log").read_bytes())
        results["clients_reaped"] = all(child.poll() is not None for child in clients)
        backend.shutdown()
        backend.server_close()
        worker.join()
        (out / "results.json").write_text(json.dumps(results, indent=2) + "\n")
        print(
            json.dumps(
                {
                    k: v
                    for k, v in results.items()
                    if k not in ("sessions", "requests", "get_paths", "native_threads")
                }
            )
        )
