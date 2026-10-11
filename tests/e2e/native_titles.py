# /// script
# requires-python = ">=3.10"
# dependencies = ["pyte>=0.8,<0.9"]
# ///
"""Probe titles of new native sessions with private homes and a local fake API."""

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
        titles = re.findall(rb"\x1b\][02];([^\x07]*)\x07", self.raw)
        return bool(titles) and title.encode() in titles[-1]

    def close(self) -> None:
        if self.child.poll() is None:
            os.killpg(self.child.pid, signal.SIGKILL)
        self.child.wait(timeout=5)
        os.close(self.master)


def _probe(
    lf: Path,
    executable: Path,
    provider: str,
    headless: bool,
    output: Path,
) -> None:
    # Short paths are required by AF_UNIX. Retain only synthetic evidence on failure.
    root = Path(tempfile.mkdtemp(prefix="lf-title-", dir="/tmp"))
    home, work = root / "home", root / "work"
    home.mkdir()
    work.mkdir()
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
[projects."{work}"]
trust_level="trusted"
''')
        command = [str(executable), "--no-alt-screen"]
    tui = None
    passed = False
    try:
        if headless:
            if provider == "claude":
                command += ["-p", "--output-format", "json"]
            else:
                command = [
                    str(executable),
                    "exec",
                    "--skip-git-repo-check",
                    "--dangerously-bypass-hook-trust",
                    "--json",
                ]
            result = subprocess.run(
                [*command, "Plan store migration for archived tasks"],
                cwd=work,
                env=env,
                stdin=subprocess.DEVNULL,
                capture_output=True,
                timeout=45,
            )
            output.mkdir(parents=True, exist_ok=True)
            (output / "stdout.jsonl").write_bytes(result.stdout)
            (output / "stderr.txt").write_bytes(result.stderr)
            assert result.returncode == 0, result.stderr.decode(errors="replace")
            assert b"fixture response" in result.stdout, result.stdout
            records = [
                json.loads(line)
                for path in native.rglob("*.jsonl")
                for line in path.read_text().splitlines()
            ]
            names = [entry.get("customTitle") or entry.get("thread_name") for entry in records]
            names = [name for name in names if name]
            evidence = {
                "provider": provider,
                "headless": True,
                "titled": "Plan store migration" in names,
                "names": names,
                "shared_socket": (native / "app-server-control/app-server-control.sock").exists(),
            }
            (output / "naming.json").write_text(json.dumps(evidence, indent=2))
            if provider == "claude":
                assert evidence["titled"], "headless Claude did not save the request title"
            print(json.dumps(evidence), flush=True)
            passed = True
            return
        tui = _Tui(command, work, env)
        tui.ready()
        tui.enter("Plan store migration for archived tasks")
        tui.drain(5)
        assert tui.has_title("Plan store migration"), "first request did not reach OSC"
        tui.enter("/rename Manual release notes")
        tui.drain(2)
        tui.enter("Other work entirely")
        tui.drain(3)
        assert tui.has_title("Manual release notes"), "native rename did not reach OSC"
        output.mkdir(parents=True, exist_ok=True)
        (output / "initial.ansi").write_bytes(tui.raw)
        print(
            json.dumps(
                {"provider": provider, "headless": False, "passed": True, "evidence": str(output)}
            ),
            flush=True,
        )
        passed = True
    finally:
        if tui:
            (root / "last.ansi").write_bytes(tui.raw)
            tui.close()
        if provider == "codex" and not headless:
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
    parser.add_argument("--headless", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    _probe(
        args.lf.resolve(),
        args.native.resolve(),
        args.provider,
        args.headless,
        args.output,
    )


if __name__ == "__main__":
    main()
