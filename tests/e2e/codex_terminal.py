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
import shutil
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


def _rpc_frames(buffer: bytearray) -> list[dict]:
    """Inspect only the unmasked provider-to-renderer WebSocket frames."""
    messages = []
    if buffer.startswith(b"HTTP/"):
        end = buffer.find(b"\r\n\r\n")
        if end < 0:
            return messages
        del buffer[: end + 4]
    while len(buffer) >= 2:
        opcode, size = buffer[0] & 15, buffer[1] & 127
        offset = 2
        if size == 126:
            if len(buffer) < 4:
                break
            size, offset = struct.unpack("!H", buffer[2:4])[0], 4
        elif size == 127:
            if len(buffer) < 10:
                break
            size, offset = struct.unpack("!Q", buffer[2:10])[0], 10
        if len(buffer) < offset + size:
            break
        payload = bytes(buffer[offset : offset + size])
        del buffer[: offset + size]
        if opcode == 1:
            messages.append(json.loads(payload))
    return messages


def _startup_phase(phase: str, rpc: dict) -> bool:
    result = rpc.get("result")
    return (
        phase == "before-thread"
        and "result" in rpc
        or phase == "after-thread"
        and isinstance(result, dict)
        and "thread" in result
        or phase == "first-turn"
        and rpc.get("method") == "turn/started"
    )


def _transport_proxy(arguments: list[str]) -> int:
    """Drop only the disposable renderer's socket, retaining its live PTY."""
    if "--remote" not in arguments:
        os.execv(arguments[0], arguments)
    index = arguments.index("--remote") + 1
    endpoint = arguments[index].removeprefix("unix://")
    directory = tempfile.TemporaryDirectory(prefix="lf-pty-", dir="/tmp")
    proxy_path = Path(directory.name) / "ui.sock"
    listener = socket.socket(socket.AF_UNIX)
    listener.bind(str(proxy_path))
    listener.listen()
    listener.settimeout(0.1)
    arguments[index] = f"unix://{proxy_path}"
    stop = threading.Event()

    def _forward() -> None:
        while not stop.is_set():
            try:
                client, _ = listener.accept()
            except TimeoutError:
                continue
            with client, socket.socket(socket.AF_UNIX) as upstream:
                upstream.connect(endpoint)
                if Path("transport-dropped").exists():
                    Path("transport-reconnected").touch()
                frames = bytearray()
                while not stop.is_set():
                    if Path("drop-transport").exists():
                        Path("drop-transport").unlink()
                        # No WebSocket close handshake: exercise actual EOF/reset.
                        client.shutdown(socket.SHUT_RDWR)
                        upstream.shutdown(socket.SHUT_RDWR)
                        Path("transport-dropped").touch()
                        break
                    readable, _, _ = select.select([client, upstream], [], [], 0.05)
                    closed = False
                    for source in readable:
                        data = source.recv(65536)
                        if not data:
                            closed = True
                            break
                        phase = os.environ.get("FIXTURE_RELAY_FAILURE")
                        if phase and source is upstream and not Path("transport-dropped").exists():
                            frames.extend(data)
                            if any(_startup_phase(phase, rpc) for rpc in _rpc_frames(frames)):
                                # Withhold the matched frame: after-thread leaves a
                                # created thread whose ID the renderer never receives.
                                Path("drop-transport").touch()
                                break
                        (upstream if source is client else client).sendall(data)
                    if closed:
                        break

    worker = threading.Thread(target=_forward)
    worker.start()
    try:
        return subprocess.call(arguments)
    finally:
        stop.set()
        worker.join(timeout=2)
        listener.close()
        directory.cleanup()


def _cleanup_startup_provider(env: dict[str, str]) -> None:
    database_path = Path(env["LF_HOME"]) / "loopflow.db"
    if not database_path.exists():
        return
    with sqlite3.connect(database_path) as database:
        providers = database.execute(
            "SELECT pid,command FROM processes WHERE kind='agent' AND completed_at IS NULL"
        ).fetchall()
    for pid, command in providers:
        arguments = json.loads(command)
        endpoint = arguments[arguments.index("--listen") + 1]
        actual = subprocess.run(
            ["/bin/ps", "-p", str(pid), "-o", "command="], capture_output=True, timeout=5
        )
        if actual.returncode == 0 and endpoint.encode() in actual.stdout:
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass


def _verify_startup_diagnostic(lf: str, env: dict[str, str], cwd: Path) -> bool:
    with sqlite3.connect(Path(env["LF_HOME"]) / "loopflow.db") as database:
        diagnostics = database.execute(
            "SELECT payload,agent_process_id,session_id FROM session_events "
            "WHERE json_extract(payload,'$.source')='codex_relay' ORDER BY seq"
        ).fetchall()
    if not diagnostics:
        return False
    assert all(agent for _, agent, _ in diagnostics), diagnostics
    payload_json, agent, session = diagnostics[0]
    payload = json.loads(payload_json)
    assert payload["operation"] in {"read", "write"}, payload
    assert payload["direction"] in {"client_to_relay", "relay_to_client"}, payload
    assert len(payload["error"].encode()) <= 2048
    for events in [True, False]:
        history = subprocess.run(
            [lf, "history", "show", session, *(["--events"] if events else [])],
            env=env,
            cwd=cwd,
            capture_output=True,
            timeout=10,
        )
        assert history.returncode == 0, history.stderr
        if events:
            retained = [json.loads(line) for line in history.stdout.splitlines()]
            retained = [
                event
                for event in retained
                if event.get("payload", {}).get("source") == "codex_relay"
            ]
            assert retained and retained[0]["agent_process_id"] == agent
            (cwd / "relay-history.jsonl").write_bytes(history.stdout)
        else:
            assert agent.encode() in history.stdout
            assert payload["direction"].encode() in history.stdout
    return True


def _terminal(
    args: list[str],
    env: dict[str, str],
    cwd: Path,
    transcript: Path,
    follow_up: bool = False,
    transport_failure: bool = False,
    startup_failure: str | None = None,
    outcomes: tuple[bytes, ...] = (),
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
    waiting_for_transport = False
    transport_edit_at = None
    outcomes = outcomes or (b"FOLLOWUP_DONE" if follow_up else b"FIXTURE_DONE",)
    startup_verified = False
    try:
        while time.monotonic() < deadline:
            if startup_failure and not startup_verified and (cwd / "transport-dropped").exists():
                startup_verified = _verify_startup_diagnostic(args[0], env, cwd)
                if startup_verified and startup_failure != "after-thread":
                    return 0
            if waiting_for_diagnostic:
                assert b"title update failed" not in output, (
                    "Loopflow warning entered the native draft display"
                )
                logs = Path(env["LF_HOME"]) / "logs"
                if any(
                    b"title update failed" in path.read_bytes()
                    for path in logs.glob("terminal-*.log")
                ):
                    assert b"title update failed" not in output
                    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 100, 0, 0))
                    os.write(master, b"\x7f\x1b[200~ ready\x1b[201~")
                    submit_at = time.monotonic() + 0.3
                    waiting_for_diagnostic = False
            if (
                transport_edit_at is not None
                and time.monotonic() >= transport_edit_at
                and (cwd / "transport-dropped").exists()
                and (cwd / "transport-reconnected").exists()
            ):
                os.write(master, b"\x1b[200~ after failure\x1b[201~")
                submit_at = time.monotonic() + 0.3
                transport_edit_at = None
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
                if waiting_for_transport and b"ready" in chunk:
                    (cwd / "drop-transport").touch()
                    transport_edit_at = time.monotonic() + 1
                    waiting_for_transport = False
                if pending_follow_up and b"FIXTURE_DONE" in output:
                    # Explicit paste boundaries avoid native burst detection
                    # treating the subsequent Enter as more pasted draft text.
                    os.write(
                        master, "\x1b[200~typed reconnect follow-up café 雪 🦀x\x1b[201~".encode()
                    )
                    if transport_failure:
                        os.write(master, b"\x7f\x1b[200~ ready\x1b[201~")
                        waiting_for_transport = True
                    else:
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
                if (
                    not exiting
                    and startup_failure == "after-thread"
                    and b"Reconnect failed" in output
                ):
                    # Codex refuses to retry a thread whose ID it never received and
                    # ignores /exit from that state; Ctrl+C leaves for lf resume.
                    os.write(master, b"\x03")
                    exiting = True
                if not exiting and any(outcome in output for outcome in outcomes):
                    exit_at = time.monotonic() + 1
                    exiting = True
            if child.poll() is not None:
                break
        assert child.poll() is not None, "terminal did not exit within 30 seconds"
        if startup_failure != "after-thread":
            assert any(outcome in output for outcome in outcomes), (
                "terminal never displayed the completed turn"
            )
        if startup_failure:
            assert startup_verified, "startup reset never reached retained diagnostics"
        if follow_up and transport_failure:
            assert (cwd / "transport-dropped").exists(), "transport was never interrupted"
            assert (cwd / "transport-reconnected").exists(), "renderer never reconnected"
        elif follow_up:
            assert not waiting_for_diagnostic, "background failure never reached diagnostics"
            assert b"title update failed" not in output
            assert b"Loopflow recorded diagnostics:" in output
        return child.returncode
    finally:
        if child.poll() is None:
            # Cleanly exit even with a draft, reaping the private app-server too.
            os.write(master, b"\x15/exit")
            time.sleep(0.3)
            os.write(master, b"\r")
            cleanup_deadline = time.monotonic() + 5
            while child.poll() is None and time.monotonic() < cleanup_deadline:
                if select.select([master], [], [], 0.1)[0]:
                    try:
                        output.extend(os.read(master, 65536))
                    except OSError:
                        break
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGKILL)
        child.wait(timeout=5)
        os.close(master)
        transcript.write_bytes(output)
        if startup_failure:
            _cleanup_startup_provider(env)
            assert b"client_to_relay" not in output
            assert b"codex_relay" not in output


def _resume_unacknowledged_thread(root: Path, lf: Path, env: dict[str, str], native: Path) -> None:
    """The printed lf resume continues the created thread without repeating its input.

    Leaving stops Codex, so a turn already running when the renderer lost the thread
    ID ends interrupted, and its input is lost if Codex had not yet persisted it. Input
    not yet sent is submitted on resume.
    """
    with sqlite3.connect(root / "lf" / "loopflow.db") as database:
        (session,) = database.execute("SELECT id FROM agent_sessions").fetchone()
    assert f"lf resume {session}".encode() in (root / "launch.txt").read_bytes()
    env = {key: value for key, value in env.items() if key != "FIXTURE_RELAY_FAILURE"}
    resumed = _terminal(
        [str(lf), "resume", session],
        env,
        root,
        root / "resume.txt",
        outcomes=(b"FIXTURE_DONE", b"Conversation interrupted"),
    )
    assert resumed == 0
    with sqlite3.connect(root / "lf" / "loopflow.db") as database:
        assert database.execute("SELECT count(*) FROM agent_sessions").fetchone() == (1,)
    (rollout,) = native.glob("sessions/*/*/*/rollout-*.jsonl")
    inputs = [
        line
        for line in rollout.read_text().splitlines()
        if '"role":"user"' in line.replace(" ", "") and "NATIVE_SKILL_STARTUP" in line
    ]
    assert len(inputs) <= 1, inputs
    print("after-thread: lf resume continued the created thread without replay", flush=True)


def _case(
    root: Path,
    lf: Path,
    codex: Path,
    port: int,
    wrapped: bool,
    context_repo: Path | None = None,
    cmux_wrapper: Path | None = None,
    transport_failure: bool = False,
    startup_failure: str | None = None,
) -> None:
    root.mkdir()
    Model.requests.clear()
    scratch = root / "scratch"
    scratch.mkdir()
    if context_repo:
        if (context_repo / "AGENTS.md").is_file():
            shutil.copyfile(context_repo / "AGENTS.md", root / "AGENTS.md")
        if (context_repo / "scratch").is_dir():
            shutil.copytree(context_repo / "scratch", scratch, dirs_exist_ok=True)
        wave = context_repo / ".lf/wave/infrastructure"
        for name in ["GOAL.md", "MEMORY.md"]:
            source = wave / name
            if source.exists():
                shutil.copyfile(source, scratch / f"native-{name}")
    else:
        skill = root / ".lf/skills/native-probe.md"
        skill.parent.mkdir(parents=True)
        skill.write_text("NATIVE_SKILL_STARTUP: inspect the workspace with a tool, then answer.\n")
    (scratch / "context.md").write_text("NATIVE_CONTEXT_BEFORE_RESUME")
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
    # Earlier releases' persisted decisions at Loopflow's hook IDs are stale,
    # as on a long-lived Machine; they must neither block nor be rewritten.
    for event in ["user_prompt_submit", "post_tool_use"]:
        key = json.dumps(f"/<session-flags>/config.toml:{event}:0:0")
        config += f'[hooks.state.{key}]\ntrusted_hash="sha256:{"0" * 64}"\n'
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
    # Trust only the wrapper's declarations; Loopflow must trust its own hooks
    # on the thread. The same IDs must survive resume.
    hooks.append("state={" + ",".join(states) + "}")
    host_args = ["-c", "hooks={" + ",".join(hooks) + "}"] if wrapped else []
    executable = [str(cmux_wrapper or codex)]
    if transport_failure or startup_failure:
        executable = [
            sys.executable,
            str(Path(__file__).resolve()),
            "--transport-proxy",
            *executable,
        ]
    wrapper = (
        "#!/bin/sh\nexec " + shlex.join([*executable, "--no-alt-screen", *host_args]) + ' "$@"\n'
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
    if startup_failure:
        env["FIXTURE_RELAY_FAILURE"] = startup_failure
    if cmux_wrapper:
        env.update(
            {
                "CMUX_CUSTOM_CODEX_PATH": str(codex),
                "CMUX_SURFACE_ID": "00000000-0000-0000-0000-000000000512",
                "CMUX_SOCKET_PATH": str(root / "unconnected-cmux.sock"),
                "CMUX_COMPUTER_USE_MCP_DISABLED": "1",
            }
        )
    skill_name = "demo" if context_repo else "native-probe"
    args = [str(lf), "--diff", "none", "-a", "codex", skill_name]
    status = _terminal(args, env, root, root / "launch.txt", startup_failure=startup_failure)
    if startup_failure:
        assert status == 0
        assert (root / "relay-history.jsonl").is_file()
        print(
            f"{startup_failure}: retained private operation diagnostic in Session history",
            flush=True,
        )
        if startup_failure == "after-thread":
            _resume_unacknowledged_thread(root, lf, env, native)
        return
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
    skill_marker = b"Demonstrate the changed behavior" if context_repo else b"NATIVE_SKILL_STARTUP"
    assert any(skill_marker in request for request in Model.requests)
    binding.unlink()  # Reconnect must restore this Session's context, not another's.
    (root / "scratch/context.md").write_text("NATIVE_CONTEXT_AFTER_RESUME")
    # Prompt and tool hooks deliver a steer saved between turns.
    subprocess.run(
        [str(lf), "session", "steer", session_id, "NATIVE_STEER_BETWEEN_TURNS"],
        env=env,
        cwd=root,
        stdin=subprocess.DEVNULL,
        check=True,
    )
    status = _terminal(
        [str(lf), "session", "resume", session_id],
        env,
        root,
        root / "resume.txt",
        follow_up=True,
        transport_failure=transport_failure,
    )
    assert status == 0, (status, root / "resume.txt")
    assert binding.exists(), "resume did not restore its context binding"
    assert any(b"NATIVE_CONTEXT_AFTER_RESUME" in request for request in Model.requests)
    expected_draft = "typed reconnect follow-up café 雪 🦀 ready"
    if transport_failure:
        expected_draft += " after failure"
    (root / "model-requests.json").write_text(
        json.dumps([json.loads(request) for request in Model.requests])
    )
    # Leaving prints lf's way back, never Codex's private socket commands.
    left = (root / "resume.txt").read_bytes()
    assert f"lf resume {session_id}".encode() in left
    assert b"codex --remote" not in left
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
    assert any(b"NATIVE_STEER_BETWEEN_TURNS" in request for request in Model.requests)
    steers = subprocess.run(
        [str(lf), "session", "steers", session_id, "--json"],
        env=env,
        cwd=root,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        check=True,
    ).stdout
    assert json.loads(steers)["pending"] == 0, steers
    assert any(b"NATIVE_TOOL_DONE" in request for request in Model.requests)
    assert answers == [("FIXTURE_DONE",), ("FOLLOWUP_DONE",)], answers
    assert completed == 2, completed
    assert live == 0, live
    rollouts = list(native.glob("sessions/*/*/*/rollout-*.jsonl"))
    assert len(rollouts) == 1, rollouts
    events = [json.loads(line) for line in rollouts[0].read_text().splitlines()]
    prompts = [
        item["text"]
        for event in events
        if event["type"] == "response_item" and event["payload"].get("role") == "user"
        for item in event["payload"]["content"]
        if item.get("type") == "input_text" and "typed reconnect follow-up" in item["text"]
    ]
    assert prompts == [expected_draft], f"Edited draft changed or replayed: {prompts}"
    tools = [
        event["payload"]["call_id"]
        for event in events
        if event["type"] == "response_item"
        and event["payload"].get("type") == "function_call"
        and event["payload"].get("name") == "exec_command"
    ]
    assert tools == ["tool-initial", "tool-followup"], tools
    assert (
        sum(
            event["type"] == "event_msg" and event["payload"].get("type") == "task_started"
            for event in events
        )
        == 2
    )
    print(
        f"{'wrapped' if wrapped else 'plain'}: captured two turns in {native_id}, "
        f"reconnected {session_id}, zero live providers",
        flush=True,
    )


def main() -> None:
    if sys.argv[1:2] == ["--transport-proxy"]:
        raise SystemExit(_transport_proxy(sys.argv[2:]))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lf", type=Path, required=True)
    parser.add_argument("--codex", type=Path, required=True)
    parser.add_argument("--context-repo", type=Path)
    parser.add_argument("--cmux-wrapper", type=Path)
    parser.add_argument(
        "--transport-failure",
        action="store_true",
        help="Drop the renderer socket during draft entry instead of failing a title refresh",
    )
    parser.add_argument(
        "--startup-failure", choices=["before-thread", "after-thread", "first-turn"]
    )
    options = parser.parse_args()
    if options.context_repo and not options.context_repo.is_dir():
        parser.error("--context-repo must name an existing checkout")
    root = Path(tempfile.mkdtemp(prefix="lf-codex-terminal-")).resolve()
    print(f"Evidence: {root}", flush=True)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Model)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        for wrapped in [False] if options.startup_failure else [False, True]:
            _case(
                root / ("wrapped" if wrapped else "plain"),
                options.lf.resolve(),
                options.codex.resolve(),
                server.server_port,
                wrapped,
                options.context_repo,
                options.cmux_wrapper,
                options.transport_failure,
                options.startup_failure,
            )
    finally:
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    main()
