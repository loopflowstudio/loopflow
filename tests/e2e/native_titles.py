# /// script
# requires-python = ">=3.10"
# dependencies = ["websockets>=15,<16", "pyte>=0.8,<0.9"]
# ///
"""Probe native title hooks in real TUIs, private homes, and a local fake API."""

import argparse
import fcntl
import json
import os
import pty
import re
import select
import shlex
import shutil
import signal
import struct
import subprocess
import tempfile
import termios
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import pyte
from websockets.sync.client import unix_connect


class _Handler(BaseHTTPRequestHandler):
    def log_message(self, *args: object) -> None:
        pass

    def do_POST(self) -> None:
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        if "messages" in body:
            message = {
                "id": "msg_fixture",
                "type": "message",
                "role": "assistant",
                "model": body["model"],
                "content": [],
                "stop_reason": None,
                "usage": {"input_tokens": 20, "output_tokens": 5},
            }
            events = [
                ("message_start", {"message": message}),
                (
                    "content_block_start",
                    {"index": 0, "content_block": {"type": "text", "text": ""}},
                ),
                (
                    "content_block_delta",
                    {"index": 0, "delta": {"type": "text_delta", "text": "fixture response"}},
                ),
                ("content_block_stop", {"index": 0}),
                (
                    "message_delta",
                    {"delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 5}},
                ),
                ("message_stop", {}),
            ]
        else:
            item = {
                "type": "message",
                "id": "msg_done",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "fixture response"}],
            }
            response = {
                "id": "resp_fixture",
                "status": "completed",
                "output": [item],
                "usage": {"input_tokens": 20, "output_tokens": 5, "total_tokens": 25},
            }
            events = [
                ("response.created", {"response": dict(response, status="in_progress", output=[])}),
                ("response.output_item.added", {"output_index": 0, "item": item}),
                ("response.output_item.done", {"output_index": 0, "item": item}),
                ("response.completed", {"response": response}),
            ]
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        self.wfile.write(
            "".join(
                f"event: {kind}\ndata: {json.dumps(dict(type=kind, **data))}\n\n"
                for kind, data in events
            ).encode()
        )


def _terminal() -> None:
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)


class _Tui:
    def __init__(self, command: list[str], work: Path, env: dict[str, str]) -> None:
        self.master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 160, 0, 0))
        self.child = subprocess.Popen(
            command,
            cwd=work,
            env=env,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            preexec_fn=_terminal,
        )
        os.close(slave)
        self.raw = bytearray()
        self.screen = pyte.Screen(160, 40)
        self.stream = pyte.ByteStream(self.screen)

    def drain(self, seconds: float) -> None:
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if not select.select([self.master], [], [], 0.05)[0]:
                continue
            try:
                data = os.read(self.master, 65536)
            except OSError:
                break
            if not data:
                break
            self.raw.extend(data)
            self.stream.feed(data)
            if b"\x1b[6n" in data:
                os.write(self.master, b"\x1b[1;1R")
            if b"\x1b[c" in data or b"\x1b[>c" in data:
                os.write(self.master, b"\x1b[?1;2c")

    def enter(self, text: str) -> None:
        os.write(self.master, text.encode())
        self.drain(0.3)
        os.write(self.master, b"\r")

    def ready(self) -> None:
        self.drain(3)
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            self.drain(0.2)
            text = "".join("".join(self.screen.display).split())
            if "Trustallandcontinue" in text:
                self.enter("2")
                self.drain(1)
            elif "Yes,Itrustthisfolder" in text:
                self.enter("\x1b[B")
                self.drain(1)
            elif "DetectedacustomAPIkey" in text:
                self.enter("\x1b[A")
                self.drain(1)
            elif "escback" in text or "escskip" in text:
                os.write(self.master, b"\x1b")
                self.drain(0.5)
            elif (
                "AskCodextodoanything" in text and "foragents" in text
            ) or "shift+tabtocycle" in text:
                return
        raise AssertionError("TUI did not become ready: " + "\n".join(self.screen.display))

    def has_title(self, title: str) -> bool:
        return any(
            title.encode() in value for value in re.findall(rb"\x1b\][02];([^\x07]*)\x07", self.raw)
        )

    def close(self) -> None:
        if self.child.poll() is None:
            os.killpg(self.child.pid, signal.SIGKILL)
        self.child.wait(timeout=5)
        os.close(self.master)


def _rpc(connection: object, method: str, params: dict) -> dict:
    connection.send(json.dumps({"id": method, "method": method, "params": params}))
    while True:
        reply = json.loads(connection.recv(timeout=5))
        if reply.get("id") == method:
            assert "error" not in reply, reply
            return reply["result"]


def _probe(lf: Path, executable: Path, provider: str, unnamed: bool, output: Path) -> None:
    # Short paths are required by AF_UNIX. Retain only synthetic evidence on failure.
    root = Path(tempfile.mkdtemp(prefix="lf-title-", dir="/tmp"))
    home, work = root / "home", root / "work"
    home.mkdir()
    work.mkdir()
    if not unnamed:
        (work / ".lf").mkdir()
    (work / "AGENTS.md").write_text(
        "# Loopflow operating guide\nName conversations for the requested work.\n"
    )
    (work / "CLAUDE.md").write_text(
        "# Wave session operator\nName conversations for the requested work.\n"
    )
    native = home / (".claude" if provider == "claude" else ".codex")
    native.mkdir()
    handler = {
        "type": "command",
        "command": f"{shlex.quote(str(lf))} __session-title {provider}",
        "timeout": 5,
    }
    hooks = {
        "hooks": {
            "UserPromptSubmit": [{"hooks": [handler]}],
            "SessionStart": [{"matcher": "resume", "hooks": [handler]}],
        }
    }
    (native / ("settings.json" if provider == "claude" else "hooks.json")).write_text(
        json.dumps(hooks)
    )
    server = ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    env = {
        "HOME": str(home),
        "LF_HOME": str(root / "lf"),
        "TERM": "xterm-256color",
        "LANG": "en_US.UTF-8",
        "PATH": f"{executable.parent}:/usr/bin:/bin",
    }
    if provider == "claude":
        env.update(
            CLAUDE_CONFIG_DIR=str(native),
            ANTHROPIC_API_KEY="local-fixture-not-a-credential",
            ANTHROPIC_BASE_URL=f"http://127.0.0.1:{server.server_port}",
            CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="1",
            DISABLE_AUTOUPDATER="1",
        )
        (native / ".claude.json").write_text(json.dumps({"hasCompletedOnboarding": True}))
        command = [str(executable), "--model", "sonnet"]
    else:
        env["CODEX_HOME"] = str(native)
        (native / "config.toml").write_text(f'''model="gpt-5.4"
model_provider="fixture"
cli_auth_credentials_store="file"
allow_login_shell=false
sandbox_mode="read-only"
approval_policy="never"
[features]
shell_snapshot=false
[model_providers.fixture]
name="Local fixture"
base_url="http://127.0.0.1:{server.server_port}/v1"
wire_api="responses"
requires_openai_auth=false
[analytics]
enabled=false
[feedback]
enabled=false
[tui]
terminal_title=["thread-name"]
[projects."{work}"]
trust_level="trusted"
''')
        command = [str(executable), "--no-alt-screen"]
    tui = _Tui(command, work, env)
    connection = None
    passed = False
    try:
        tui.ready()
        tui.enter("Plan store migration for archived tasks")
        tui.drain(5)
        if provider == "codex":
            connection = unix_connect(str(native / "app-server-control/app-server-control.sock"))
            _rpc(
                connection, "initialize", {"clientInfo": {"name": "lf_title_probe", "version": "1"}}
            )
            threads = _rpc(
                connection,
                "thread/list",
                {"cwd": str(work.resolve()), "modelProviders": ["fixture"]},
            )["data"]
            thread = next(
                item
                for item in threads
                if item["preview"] == "Plan store migration for archived tasks"
            )
            session = thread["id"]
            assert thread["name"] == (None if unnamed else "Plan store migration"), thread["name"]
        else:
            entries = [
                json.loads(line)
                for path in (native / "projects").rglob("*.jsonl")
                for line in path.read_text().splitlines()
            ]
            session = next(entry["sessionId"] for entry in entries if entry.get("type") == "user")
        if not unnamed:
            assert tui.has_title("Plan store migration"), "first request did not reach OSC"
            tui.enter("/rename Manual release notes")
            tui.drain(2)
            tui.enter("Other work entirely")
            tui.drain(3)
            assert tui.has_title("Manual release notes"), "native rename did not reach OSC"
        output.mkdir(parents=True, exist_ok=True)
        (output / "initial.ansi").write_bytes(tui.raw)
        tui.close()
        tui = None
        if unnamed:
            (work / ".lf").mkdir()
        resume = (
            [str(executable), "resume", "--no-alt-screen", session]
            if provider == "codex"
            else [str(executable), "--model", "sonnet", "--resume", session]
        )
        tui = _Tui(resume, work, env)
        tui.ready()
        tui.drain(5)
        expected = "Plan store migration" if unnamed else "Manual release notes"
        (output / "resume.ansi").write_bytes(tui.raw)
        assert tui.has_title(expected), f"resume did not preserve {expected}"
        if connection:
            assert (
                _rpc(connection, "thread/read", {"threadId": session})["thread"]["name"] == expected
            )
        print(
            json.dumps(
                {
                    "provider": provider,
                    "unnamed_resume": unnamed,
                    "passed": True,
                    "evidence": str(output),
                }
            ),
            flush=True,
        )
        passed = True
    finally:
        if tui:
            (root / "last.ansi").write_bytes(tui.raw)
            tui.close()
        if connection:
            connection.close()
        if provider == "codex":
            subprocess.run(
                [str(executable), "app-server", "daemon", "stop"],
                cwd=work,
                env=env,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=10,
                check=True,
            )
        server.shutdown()
        server.server_close()
        if passed:
            shutil.rmtree(root)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lf", type=Path, required=True)
    parser.add_argument(
        "--native", type=Path, required=True, help="Provider binary, without a host shim"
    )
    parser.add_argument("--provider", choices=["claude", "codex"], required=True)
    parser.add_argument("--unnamed-resume", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    _probe(
        args.lf.resolve(), args.native.resolve(), args.provider, args.unnamed_resume, args.output
    )


if __name__ == "__main__":
    main()
