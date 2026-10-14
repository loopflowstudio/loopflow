#!/usr/bin/env python3
"""Exercise native Codex terminal hooks without accounts or external networking."""

import argparse
import fcntl
import hashlib
import http.server
import json
import os
import pty
import select
import shlex
import signal
import sqlite3
import struct
import subprocess
import tempfile
import termios
import threading
import time
from pathlib import Path

import tomllib


class Model(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_args: object) -> None:
        pass

    def do_POST(self) -> None:
        request = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        answer = "FOLLOWUP_DONE" if b"typed reconnect follow-up" in request else "FIXTURE_DONE"
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        item = {
            "id": "msg_fixture",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": answer}],
        }
        response = {"id": "resp_fixture", "status": "in_progress", "output": []}
        events = [
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
        ]
        for kind, data in events:
            self.wfile.write(
                f"event: {kind}\ndata: {json.dumps({'type': kind, **data})}\n\n".encode()
            )
            self.wfile.flush()


def _terminal(
    args: list[str],
    env: dict[str, str],
    cwd: Path,
    transcript: Path,
    follow_up: bool = False,
) -> int:
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))

    def _own_terminal() -> None:
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)

    child = subprocess.Popen(
        args, env=env, cwd=cwd, stdin=slave, stdout=slave, stderr=slave, preexec_fn=_own_terminal
    )
    os.close(slave)
    output = bytearray()
    deadline = time.monotonic() + 30
    exiting = False
    submit_at = None
    exit_at = None
    pending_follow_up = follow_up
    expected = b"FOLLOWUP_DONE" if follow_up else b"FIXTURE_DONE"
    try:
        while time.monotonic() < deadline:
            if exit_at is not None and time.monotonic() >= exit_at:
                os.write(master, b"/exit")
                submit_at = time.monotonic() + 0.3
                exit_at = None
            if submit_at is not None and time.monotonic() >= submit_at:
                os.write(master, b"\r")
                submit_at = None
            if select.select([master], [], [], 0.1)[0]:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    break
                output.extend(chunk)
                if b"\x1b[6n" in chunk:
                    os.write(master, b"\x1b[1;1R")
                if pending_follow_up and b"FIXTURE_DONE" in output:
                    os.write(master, b"typed reconnect follow-up")
                    submit_at = time.monotonic() + 0.3
                    pending_follow_up = False
                if not exiting and expected in output:
                    exit_at = time.monotonic() + 1
                    exiting = True
            if child.poll() is not None:
                break
        assert child.poll() is not None, "terminal did not exit within 30 seconds"
        assert expected in output, "terminal never displayed the completed turn"
        return child.returncode
    finally:
        transcript.write_bytes(output)
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGKILL)
        child.wait()
        os.close(master)


def _case(root: Path, lf: Path, codex: Path, port: int, wrapped: bool) -> None:
    root.mkdir()
    home, native, bin_dir = root / "home", root / "codex", root / "bin"
    for directory in [home, native, bin_dir]:
        directory.mkdir()
    config = f"""cli_auth_credentials_store="file"
model="fixture"
model_provider="fixture"
[model_providers.fixture]
name="fixture"
base_url="http://127.0.0.1:{port}/v1"
wire_api="responses"
requires_openai_auth=false
[projects.{json.dumps(str(root))}]
trust_level="trusted"
"""
    (native / "config.toml").write_text(config)
    hooks = []
    states = []
    for event, event_name, matcher in [
        ("SessionStart", "session_start", "startup|resume"),
        ("Stop", "stop", None),
    ]:
        command = f"cat > {shlex.quote(str(root / event))}"
        matching = f"matcher={json.dumps(matcher)}," if matcher else ""
        hooks.append(
            f'{event}=[{{{matching}hooks=[{{type="command",command={json.dumps(command)},timeout=30}}]}}]'
        )
        declaration = {
            "event_name": event_name,
            "hooks": [{"type": "command", "command": command, "timeout": 30, "async": False}],
        }
        if matcher:
            declaration["matcher"] = matcher
        digest = hashlib.sha256(
            json.dumps(declaration, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest()
        key = json.dumps(f"/<session-flags>/config.toml:{event_name}:0:0")
        states.append(f'{key}={{trusted_hash="sha256:{digest}"}}')
    # Trust only this wrapper's declarations. The same IDs must survive resume.
    hooks.append("state={" + ",".join(states) + "}")
    host_args = ["-c", "hooks={" + ",".join(hooks) + "}"] if wrapped else []
    wrapper = (
        "#!/bin/sh\nexec " + shlex.join([str(codex), "--no-alt-screen", *host_args]) + ' "$@"\n'
    )
    (bin_dir / "codex").write_text(wrapper)
    (bin_dir / "codex").chmod(0o755)
    env = {
        "HOME": str(home),
        "CODEX_HOME": str(native),
        "LF_HOME": str(root / "lf"),
        "LF_BIN": str(lf),
        "PATH": f"{bin_dir}:/usr/bin:/bin",
        "TERM": "xterm-256color",
    }
    args = [str(lf), "--diff", "none", "-a", "codex", ":", "hello"]
    status = _terminal(args, env, root, root / "launch.txt")
    assert status == 0, (status, root / "launch.txt")
    with sqlite3.connect(root / "lf" / "loopflow.db") as database:
        sessions = database.execute(
            "SELECT DISTINCT json_extract(payload,'$.evidence.provider_session_id'),session_id "
            "FROM session_events WHERE kind='observed' "
            "AND json_extract(payload,'$.evidence.provider_session_id') IS NOT NULL"
        ).fetchall()
    assert len(sessions) == 1, sessions
    native_id, session_id = sessions[0]
    assert any(native.glob(f"sessions/*/*/*/rollout-*-{native_id}.jsonl")), native_id
    if wrapped:
        for event in ["SessionStart", "Stop"]:
            assert json.loads((root / event).read_text())["session_id"] == native_id
    # Native UI onboarding may add its own preferences; supplied settings survive.
    saved = tomllib.loads((native / "config.toml").read_text())
    for key, value in tomllib.loads(config).items():
        assert saved[key] == value, key
    assert not (native / "hooks.json").exists()
    status = _terminal(
        [str(lf), "session", "connect", session_id], env, root, root / "resume.txt", follow_up=True
    )
    assert status == 0, (status, root / "resume.txt")
    assert native_id.encode() in (root / "resume.txt").read_bytes()
    with sqlite3.connect(root / "lf" / "loopflow.db") as database:
        answers = database.execute(
            "SELECT json_extract(payload,'$.text') FROM session_events "
            "WHERE session_id=? AND kind='output' ORDER BY seq",
            (session_id,),
        ).fetchall()
        completed = database.execute(
            "SELECT count(*) FROM session_events WHERE session_id=? AND kind='completed'",
            (session_id,),
        ).fetchone()[0]
        live = database.execute(
            "SELECT count(*) FROM processes WHERE kind='agent' AND completed_at IS NULL"
        ).fetchone()[0]
    assert answers == [("FIXTURE_DONE",), ("FOLLOWUP_DONE",)], answers
    assert completed == 2, completed
    assert live == 0, live
    print(
        f"{'wrapped' if wrapped else 'plain'}: captured two turns in {native_id}, "
        f"reconnected {session_id}, zero live providers",
        flush=True,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lf", type=Path, required=True)
    parser.add_argument("--codex", type=Path, required=True)
    options = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix="lf-codex-terminal-")).resolve()
    print(f"Evidence: {root}", flush=True)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Model)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        for wrapped in [False, True]:
            _case(
                root / ("wrapped" if wrapped else "plain"),
                options.lf.resolve(),
                options.codex.resolve(),
                server.server_port,
                wrapped,
            )
    finally:
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    main()
