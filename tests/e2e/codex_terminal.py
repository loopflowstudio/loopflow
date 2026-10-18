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
    requests: list[bytes] = []

    def log_message(self, *_args: object) -> None:
        pass

    def do_POST(self) -> None:
        request = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        self.requests.append(request)
        answer = "FOLLOWUP_DONE" if b"typed reconnect follow-up" in request else "FIXTURE_DONE"
        payload = json.loads(request)
        call_id = "tool-followup" if answer == "FOLLOWUP_DONE" else "tool-initial"
        tool_finished = any(
            item.get("type") == "function_call_output" and item.get("call_id") == call_id
            for item in payload.get("input", [])
        )
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        item = {
            "id": "msg_fixture",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": answer}],
        }
        if not tool_finished:
            item = {
                "id": f"fc_{call_id}",
                "type": "function_call",
                "name": "exec_command",
                "call_id": call_id,
                "arguments": json.dumps({"cmd": "/bin/echo NATIVE_TOOL_DONE"}),
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
    waiting_for_diagnostic = False
    expected = b"FOLLOWUP_DONE" if follow_up else b"FIXTURE_DONE"
    try:
        while time.monotonic() < deadline:
            if waiting_for_diagnostic:
                assert b"cmux title update failed" not in output, (
                    "Loopflow warning entered the native draft display"
                )
                logs = Path(env["LF_HOME"]) / "logs"
                if any(
                    b"cmux title update failed" in path.read_bytes()
                    for path in logs.glob("terminal-*.log")
                ):
                    assert b"cmux title update failed" not in output
                    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 100, 0, 0))
                    os.write(master, b"\x7f ready")
                    submit_at = time.monotonic() + 0.3
                    waiting_for_diagnostic = False
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
                    os.write(master, "typed reconnect follow-up café 雪 🦀x".encode())
                    (cwd / "fail-title").touch()
                    with sqlite3.connect(Path(env["LF_HOME"]) / "loopflow.db") as database:
                        session = database.execute(
                            "SELECT id FROM agent_sessions LIMIT 1"
                        ).fetchone()[0]
                        # Completed Sessions reject CLI rename. Change only this
                        # fixture's title to exercise the active reconnect reader.
                        database.execute(
                            "UPDATE agent_sessions SET title='Warning during draft' WHERE id=?",
                            (session,),
                        )
                    waiting_for_diagnostic = True
                    pending_follow_up = False
                if not exiting and expected in output:
                    exit_at = time.monotonic() + 1
                    exiting = True
            if child.poll() is not None:
                break
        assert child.poll() is not None, "terminal did not exit within 30 seconds"
        assert expected in output, "terminal never displayed the completed turn"
        if follow_up:
            assert not waiting_for_diagnostic, "background failure never reached diagnostics"
            assert b"cmux title update failed" not in output
            assert b"Loopflow recorded diagnostics:" in output
        return child.returncode
    finally:
        transcript.write_bytes(output)
        if child.poll() is None:
            # Cleanly exit even with a draft, reaping the private app-server too.
            os.write(master, b"\x15/exit")
            time.sleep(0.3)
            os.write(master, b"\r")
            cleanup_deadline = time.monotonic() + 5
            while child.poll() is None and time.monotonic() < cleanup_deadline:
                if select.select([master], [], [], 0.1)[0]:
                    try:
                        os.read(master, 65536)
                    except OSError:
                        break
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGKILL)
        child.wait(timeout=5)
        os.close(master)


def _case(root: Path, lf: Path, codex: Path, port: int, wrapped: bool) -> None:
    root.mkdir()
    Model.requests.clear()
    (root / "scratch").mkdir()
    (root / "scratch/context.md").write_text("NATIVE_CONTEXT_BEFORE_RESUME")
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
        if wrapped:
            states.append(f'{key}={{trusted_hash="sha256:{digest}"}}')
    # Trust Loopflow's stable declarations as well: an untrusted hook never runs,
    # so checking only the host callbacks would conceal broken context delivery.
    command = f"env LF_HOME='{root / 'lf'}' '{lf}'"
    for event, index, matcher, suffix, timeout in [
        ("session_start", int(wrapped), "startup|resume", "__context-block --moment start", 30),
        ("session_start", int(wrapped) + 1, "compact", "__context-block --moment compact", 30),
        ("user_prompt_submit", 0, None, "__context-block --moment tool", 30),
        ("post_tool_use", 0, None, "__context-block --moment tool", 30),
        ("stop", int(wrapped), None, "__planning-check", 10),
    ]:
        declaration = {
            "event_name": event,
            "hooks": [
                {
                    "type": "command",
                    "command": f"{command} {suffix}",
                    "timeout": timeout,
                    "async": False,
                }
            ],
        }
        if matcher:
            declaration["matcher"] = matcher
        digest = hashlib.sha256(
            json.dumps(declaration, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest()
        key = json.dumps(f"/<session-flags>/config.toml:{event}:{index}:0")
        states.append(f'{key}={{trusted_hash="sha256:{digest}"}}')
    # Keep the same hook identities across resume.
    hooks.append("state={" + ",".join(states) + "}")
    host_args = ["-c", "hooks={" + ",".join(hooks if wrapped else [hooks[-1]]) + "}"]
    wrapper = (
        "#!/bin/sh\nexec " + shlex.join([str(codex), "--no-alt-screen", *host_args]) + ' "$@"\n'
    )
    (bin_dir / "codex").write_text(wrapper)
    (bin_dir / "codex").chmod(0o755)
    (bin_dir / "cmux").write_text('#!/bin/sh\n[ ! -f "$PWD/fail-title" ]\n')
    (bin_dir / "cmux").chmod(0o755)
    env = {
        "HOME": str(home),
        "CODEX_HOME": str(native),
        "LF_HOME": str(root / "lf"),
        "LF_BIN": str(lf),
        "PATH": f"{bin_dir}:/usr/bin:/bin",
        "TERM": "xterm-256color",
        "RUST_LOG": "warn",
        "CMUX_WORKSPACE_ID": "fixture",
        "CMUX_SURFACE_ID": "fixture",
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
    binding = (
        root
        / "lf"
        / "context-deliveries"
        / (hashlib.sha256(native_id.encode()).hexdigest() + ".json")
    )
    assert binding.exists(), f"Native context binding missing: {binding}"
    assert b"Hook failed" not in (root / "launch.txt").read_bytes()
    # Native UI onboarding may add its own preferences; supplied settings survive.
    saved = tomllib.loads((native / "config.toml").read_text())
    for key, value in tomllib.loads(config).items():
        assert saved[key] == value, key
    assert not (native / "hooks.json").exists()
    assert any(b"NATIVE_CONTEXT_BEFORE_RESUME" in request for request in Model.requests)
    binding.unlink()  # Reconnect must restore this Session's context, not another's.
    (root / "scratch/context.md").write_text("NATIVE_CONTEXT_AFTER_RESUME")
    status = _terminal(
        [str(lf), "session", "connect", session_id], env, root, root / "resume.txt", follow_up=True
    )
    assert status == 0, (status, root / "resume.txt")
    assert binding.exists(), "resume did not restore its context binding"
    assert any(b"NATIVE_CONTEXT_AFTER_RESUME" in request for request in Model.requests)
    assert any(
        "typed reconnect follow-up café 雪 🦀 ready"
        in json.dumps(json.loads(request), ensure_ascii=False)
        for request in Model.requests
    ), "Unicode draft did not survive warning, resize and editing"
    (root / "model-requests.json").write_text(
        json.dumps([json.loads(request) for request in Model.requests])
    )
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
        callbacks = database.execute(
            "SELECT command, exit_code, error FROM processes "
            "WHERE command LIKE '%__context-block%' OR command LIKE '%__planning-check%'"
        ).fetchall()
    assert len(callbacks) >= 8, callbacks
    assert all(code == 0 and error is None for _, code, error in callbacks), callbacks
    assert sum("__planning-check" in command for command, _, _ in callbacks) == 2, callbacks
    assert any(b"NATIVE_TOOL_DONE" in request for request in Model.requests)
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
