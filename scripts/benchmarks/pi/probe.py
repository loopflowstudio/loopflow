"""Exercise an installed Pi against a local model with disposable state."""

import argparse
import fcntl
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


class Model(http.server.ThreadingHTTPServer):
    def __init__(self) -> None:
        super().__init__(("127.0.0.1", 0), Handler)
        self.requests: list[dict] = []
        self.reply = "Pi protocol proof"
        self.error: str | None = None
        self.error_status = 400
        self.input_tokens = 21
        self.tool_command: str | None = None
        self.tool_timeout: int | None = None
        self.context_file: Path | None = None
        self.hold = False
        self.arrived = threading.Event()
        self.release = threading.Event()


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_args: object) -> None:
        pass

    def do_POST(self) -> None:
        model = self.server
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        model.requests.append(request)
        if model.hold and len(model.requests) == 1:
            model.arrived.set()
            assert model.release.wait(35), "steer probe did not release the model"
        if model.error:
            error, model.error = model.error, None
            if model.context_file:
                model.context_file.write_text("AFTER_COMPACT 雪")
            self.send_response(model.error_status)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(
                json.dumps({"error": {"message": error, "type": "invalid_request_error"}}).encode()
            )
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        input_tokens, model.input_tokens = model.input_tokens, 21
        delta = {"role": "assistant", "content": model.reply}
        finish = "stop"
        if model.tool_command and len(model.requests) == 1:
            delta = {
                "role": "assistant",
                "tool_calls": [
                    {
                        "index": 0,
                        "id": "proof-tool",
                        "type": "function",
                        "function": {
                            "name": "bash",
                            "arguments": json.dumps(
                                {
                                    "command": model.tool_command,
                                    **(
                                        {"timeout": model.tool_timeout}
                                        if model.tool_timeout
                                        else {}
                                    ),
                                }
                            ),
                        },
                    }
                ],
            }
            finish = "tool_calls"
        for chunk in [
            {
                "choices": [
                    {
                        "index": 0,
                        "delta": delta,
                        "finish_reason": None,
                    }
                ]
            },
            {
                "choices": [{"index": 0, "delta": {}, "finish_reason": finish}],
                "usage": {
                    "prompt_tokens": input_tokens,
                    "completion_tokens": 4,
                    "total_tokens": input_tokens + 4,
                },
            },
        ]:
            self.wfile.write(("data: " + json.dumps(chunk) + "\n\n").encode())
        self.wfile.write(b"data: [DONE]\n\n")
        self.wfile.flush()


class Fixture:
    def __init__(self, root: Path, binary: Path, pi: Path, model: Model) -> None:
        self.root, self.binary, self.model = root, binary, model
        self.repo = root / "repo"
        self.repo.mkdir()
        subprocess.run(["git", "init", "-q", "-b", "main", str(self.repo)], check=True)
        (self.repo / ".lf").mkdir()
        (self.repo / "scratch").mkdir()
        (self.repo / ".lf/config.yaml").write_text(
            "diff: false\ndiff_files: false\npaste: false\nagent: pi:fixture/proof\n"
        )
        self.context = self.repo / "scratch/proof.md"
        self.context.write_text("BEFORE_COMPACT 雪")
        subprocess.run(["git", "-C", str(self.repo), "add", "."], check=True)
        subprocess.run(
            [
                "git",
                "-C",
                str(self.repo),
                "-c",
                "user.name=Pi Fixture",
                "-c",
                "user.email=pi@example.test",
                "commit",
                "-q",
                "-m",
                "fixture",
            ],
            check=True,
        )
        agent = root / "pi-agent"
        agent.mkdir()
        (agent / "models.json").write_text(
            json.dumps(
                {
                    "providers": {
                        "fixture": {
                            "baseUrl": f"http://127.0.0.1:{model.server_port}/v1",
                            "api": "openai-completions",
                            "apiKey": "fixture-not-a-secret",
                            "models": [{"id": "proof", "contextWindow": 200000}],
                        }
                    }
                }
            )
        )
        # Failures must escape Pi's own retry policy before testing Loopflow's policy.
        (agent / "settings.json").write_text(
            json.dumps(
                {
                    "retry": {"enabled": False},
                    "compaction": {"reserveTokens": 1000, "keepRecentTokens": 1},
                }
            )
        )
        bin_dir = root / "bin"
        bin_dir.mkdir()
        (bin_dir / "pi").symlink_to(pi)
        self.env = {
            "PATH": str(bin_dir) + os.pathsep + os.environ["PATH"],
            "HOME": str(root),
            "LF_BIN": str(binary),
            "LF_HOME": str(root / "machine"),
            "PI_CODING_AGENT_DIR": str(agent),
            "PI_OFFLINE": "1",
            "LF_USER_NAME": "Pi Fixture",
        }

    def run(
        self, args: list[str], *, success: bool = True, input_text: str | None = None
    ) -> subprocess.CompletedProcess:
        result = subprocess.run(
            [str(self.binary), *args],
            cwd=self.repo,
            env=self.env,
            input=input_text or "",
            text=True,
            capture_output=True,
            timeout=90,
        )
        if (result.returncode == 0) != success:
            evidence = []
            for path in (self.root / "machine/runs").rglob("*"):
                if path.is_file() and path.suffix in (".jsonl", ".log"):
                    lines = path.read_text(errors="replace").splitlines()
                    evidence.extend(
                        line[:1500]
                        for line in lines
                        if any(
                            key in line
                            for key in ("errorMessage", "compaction_end", "extension_error")
                        )
                    )
            raise AssertionError(
                (args, result.returncode, result.stdout[-2000:], result.stderr[-2000:], evidence)
            )
        return result

    def identity(self) -> tuple[str, str]:
        with sqlite3.connect(self.root / "machine/loopflow.db") as db:
            row = db.execute(
                "SELECT s.id,json_extract(e.payload,'$.evidence.provider_session_id') "
                "FROM agent_sessions s JOIN session_events e ON e.session_id=s.id "
                "WHERE s.provider='pi' "
                "AND json_extract(e.payload,'$.evidence.provider_session_id') IS NOT NULL "
                "ORDER BY e.seq DESC LIMIT 1"
            ).fetchone()
            assert row, db.execute("SELECT kind,payload FROM session_events").fetchall()
            return row


def _scenario(fixture: Fixture, mode: str) -> dict:
    model = fixture.model
    failure = mode in ("empty", "retry", "rate-limit", "server-error", "invalid-model")
    if mode == "empty":
        model.reply = ""
    if mode == "retry":
        model.error = "connection reset"
    if mode in ("rate-limit", "server-error", "invalid-model"):
        model.error_status, model.error = {
            "rate-limit": (429, "rate limit exceeded"),
            "server-error": (503, "service unavailable"),
            "invalid-model": (404, "model not found"),
        }[mode]
    native = mode == "native"
    first = (
        ["-i", ":", "FIRST_INPUT 雪"]
        if native
        else ["-b", "resume", "--message", "FIRST_INPUT 雪"]
    )
    if mode in ("overflow", "threshold"):
        first[-1] += " prior context 雪" * 2000
    fixture.run(first, success=not failure)
    session, token = fixture.identity()
    with sqlite3.connect(fixture.root / "machine/loopflow.db") as db:
        fixture.repo = Path(
            db.execute("SELECT cwd FROM agent_sessions WHERE id=?", (session,)).fetchone()[0]
        )
    fixture.context = fixture.repo / "scratch/proof.md"
    assert Path(token).is_file()
    assert "BEFORE_COMPACT" in json.dumps(model.requests[0])
    assert len(model.requests) == 1, "Loopflow must not synthesize retries"
    if mode == "overflow":
        model.error = "maximum context length is 200000 tokens"
        model.context_file = fixture.context
    if mode == "threshold":
        model.input_tokens = 199500
        fixture.context.write_text("AFTER_COMPACT 雪")
    if mode == "retry":
        model.reply = "Explicit recovery"
    second = (
        ["-i", "resume", session]
        if native
        else ["-b", "resume", session, "--message", "EXPLICIT_SECOND_INPUT 雪"]
    )
    fixture.run(
        second,
        success=mode != "empty",
        input_text="EXPLICIT_SECOND_INPUT 雪\n" if native else None,
    )
    assert fixture.identity() == (session, token)
    if mode in ("overflow", "threshold"):
        entries = [json.loads(line) for line in Path(token).read_text().splitlines()]
        assert any(e["type"] == "compaction" for e in entries), (
            f"{mode} must compact real Pi history"
        )
        if mode == "threshold":
            fixture.run(["-b", "resume", session, "--message", "AFTER_THRESHOLD"])
            assert fixture.identity() == (session, token)
        assert "AFTER_COMPACT" in json.dumps(model.requests[-1]), (
            "first post-compaction request must refresh context"
        )
    else:
        assert len(model.requests) == 2
        assert "FIRST_INPUT" in json.dumps(model.requests[-1])
    with sqlite3.connect(fixture.root / "machine/loopflow.db") as db:
        usage = db.execute(
            "SELECT count(*) FROM session_events WHERE kind='usage' "
            "AND json_extract(payload,'$.provider')='pi'"
        ).fetchone()[0]
        assert usage > 0
    return {
        "scenario": mode,
        "model_requests": len(model.requests),
        "exact_resume": True,
        "usage_receipts": usage,
    }


def _steer_scenario(fixture: Fixture) -> dict:
    model = fixture.model
    binary = fixture.binary
    model.hold = True
    process = subprocess.Popen(
        [str(binary), "-b", "resume", "--message", "FIRST_INPUT"],
        cwd=fixture.repo,
        env=fixture.env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )
    try:
        assert model.arrived.wait(20), "model request never arrived"
        session, token = fixture.identity()
        fixture.run(["session", "steer", session, "LIVE_STEER 雪"])
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            receipts = json.loads(fixture.run(["session", "steers", session, "--json"]).stdout)
            if receipts["items"] and receipts["items"][0]["evidence"].startswith("emitted"):
                break
            time.sleep(0.1)
        else:
            raise AssertionError(receipts)
        assert len(model.requests) == 1, "delivery receipt is independent of model incorporation"
        model.release.set()
        stdout, stderr = process.communicate(timeout=30)
        assert process.returncode == 0, stderr
        assert len(model.requests) == 2
        assert "LIVE_STEER" in json.dumps(model.requests[-1]), (
            "steer must reach the same run's next model call"
        )
        history = json.loads(fixture.run(["session", "history", session, "--json"]).stdout)

        def ownership(event: dict) -> tuple:
            return event["provider_turn"], event["lf_process_id"], event["task_id"]

        starts = [ownership(e) for e in history if e["kind"] == "started"]
        ends = [ownership(e) for e in history if e["kind"] == "completed" and e["provider_turn"]]
        assert len(starts) == 1 and ends == starts, (starts, ends)
        assert fixture.identity() == (session, token)
        return {
            "scenario": "steer",
            "model_requests": 2,
            "same_turn_and_process": True,
            "receipt": receipts["items"][0]["evidence"],
        }
    finally:
        model.release.set()
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()


def _tools_scenario(fixture: Fixture) -> dict:
    settings_path = fixture.root / "pi-agent/settings.json"
    settings = json.loads(settings_path.read_text())
    settings["shellCommandPrefix"] = "export PI_PROBE_PREFIX=preserved"
    settings_path.write_text(json.dumps(settings))
    session = None
    for command, timeout, expected in [
        ('printf "prefix=%s\\n" "$PI_PROBE_PREFIX"', None, "prefix=preserved"),
        ("echo failed-output >&2; exit 7", None, "code 7"),
        ("echo before-timeout; sleep 60", 1, "timed out"),
    ]:
        fixture.model.requests.clear()
        fixture.model.tool_command = command
        fixture.model.tool_timeout = timeout
        args = (
            ["-b", "resume", session, "--message", "RUN_TOOL"]
            if session
            else ["-b", "resume", "--message", "RUN_TOOL"]
        )
        fixture.run(args)
        session, _ = fixture.identity()
        assert len(fixture.model.requests) == 2
        results = [m for m in fixture.model.requests[-1]["messages"] if m["role"] == "tool"]
        assert expected in json.dumps(results), results
    return {"scenario": "tools", "prefix_output_exit_timeout": True}


def _handoff_scenario(fixture: Fixture) -> dict:
    model = fixture.model
    model.hold = True
    fixture.env.update(TERM="xterm-256color", COLORTERM="truecolor")
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 28, 100, 0, 0))
    viewers = []

    def attach(args: list[str]) -> subprocess.Popen:
        process = subprocess.Popen(
            [str(fixture.binary), *args],
            cwd=fixture.repo,
            env=fixture.env,
            stdin=slave,
            stdout=slave,
            stderr=slave,
        )
        viewers.append(process)
        return process

    def provider() -> tuple:
        with sqlite3.connect(fixture.root / "machine/loopflow.db") as db:
            return db.execute(
                "SELECT p.id,p.pid,p.attachment_token,s.transport_lf_process_id "
                "FROM processes p JOIN agent_sessions s ON p.id=s.agent_process_id "
                "WHERE s.id=?",
                (session,),
            ).fetchone()

    try:
        first = attach(["-i", ":", "HANDOFF_INPUT"])
        assert model.arrived.wait(20), "model request never arrived"
        session, token = fixture.identity()
        original = provider()
        first.kill()
        first.wait(timeout=5)
        # Exercise the public viewer path while the original model call is pending.
        replacement = attach(["session", "attach", session])
        fixture.run(["session", "steer", session, "HANDOFF_STEER 雪"])
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            receipts = json.loads(fixture.run(["session", "steers", session, "--json"]).stdout)
            if receipts["items"] and receipts["items"][0]["evidence"].startswith("emitted"):
                break
            time.sleep(0.1)
        else:
            raise AssertionError(receipts)
        assert provider() == original, "viewer changed provider ownership"
        assert len(model.requests) == 1, "viewer replayed the initiating input"
        model.release.set()
        output = bytearray()
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            assert replacement.poll() is None, bytes(output[-3000:])
            if select.select([master], [], [], 0.1)[0]:
                output.extend(os.read(master, 65536))
            history = json.loads(fixture.run(["session", "history", session, "--json"]).stdout)
            ends = [e for e in history if e["kind"] == "completed" and e["provider_turn"]]
            if ends:
                break
        else:
            raise AssertionError(bytes(output[-3000:]))
        starts = [e for e in history if e["kind"] == "started"]
        assert len(starts) == len(ends) == 1, (starts, ends)
        for key in ("provider_turn", "lf_process_id", "task_id"):
            assert starts[0][key] == ends[0][key], (key, starts, ends)
        assert ends[0]["payload"]["status"] == "completed", ends
        assert len(model.requests) == 2 and "HANDOFF_STEER" in json.dumps(model.requests[-1])
        assert fixture.identity() == (session, token) and provider() == original
        os.write(master, b"\x04")
        deadline = time.monotonic() + 10
        while replacement.poll() is None and time.monotonic() < deadline:
            if select.select([master], [], [], 0.1)[0]:
                output.extend(os.read(master, 65536))
        assert replacement.wait(timeout=1) == 0
        return {"scenario": "handoff", "model_requests": 2, "same_provider_turn_and_process": True}
    finally:
        model.release.set()
        for viewer in viewers:
            if viewer.poll() is None:
                viewer.kill()
                viewer.wait()
        os.close(slave)
        os.close(master)


def _crash_scenario(fixture: Fixture, *, native: bool = False) -> dict:
    ready = fixture.root / "tool-pids"
    fixture.model.tool_command = (
        f"sleep 60 & child=$!; echo $$ $child > {shlex.quote(str(ready))}.tmp; "
        f"mv {shlex.quote(str(ready))}.tmp {shlex.quote(str(ready))}; wait"
    )
    args = (
        ["-i", ":", "RUN_TOOL"] if native else ["-b", "resume", "--message", "RUN_TOOL"]
    )
    process = subprocess.Popen(
        [str(fixture.binary), *args],
        cwd=fixture.repo,
        env=fixture.env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )
    children = []
    try:
        deadline = time.monotonic() + 25
        while not ready.exists() and time.monotonic() < deadline and process.poll() is None:
            time.sleep(0.1)
        if not ready.exists():
            if process.poll() is None:
                process.terminate()
            stdout, stderr = process.communicate(timeout=10)
            raise AssertionError(
                ("Pi never started the real bash tool", stdout[-3000:], stderr[-3000:])
            )
        children = [int(pid) for pid in ready.read_text().split()]
        session, token = fixture.identity()
        with sqlite3.connect(fixture.root / "machine/loopflow.db") as db:
            provider_pid = db.execute(
                "SELECT p.pid FROM processes p JOIN agent_sessions s "
                "ON p.id=s.agent_process_id WHERE s.id=?",
                (session,),
            ).fetchone()[0]
        os.kill(provider_pid, signal.SIGKILL)
        _, stderr = process.communicate(timeout=20)
        assert process.returncode != 0, stderr
        assert fixture.identity() == (session, token)
        history = json.loads(fixture.run(["session", "history", session, "--json"]).stdout)
        usage = [e for e in history if e["kind"] == "usage"]
        assert usage, "paid tool step must survive the crash"
        assert not any(
            e["kind"] == "completed" and e["payload"].get("status") == "completed" for e in history
        )
        live = subprocess.run(
            ["ps", "-o", "pid=,stat=", "-p", ",".join(map(str, children))],
            capture_output=True,
            text=True,
        )
        survivors = [
            line.strip() for line in live.stdout.splitlines() if "Z" not in line.split()[-1]
        ]
        assert not survivors, f"Pi crash left tool processes alive: {survivors}"
        for _ in range(2):
            if native:
                fixture.run(["-i", "resume", session], input_text="EXPLICIT_RECOVERY\n")
            else:
                fixture.run(["-b", "resume", session, "--message", "EXPLICIT_RECOVERY"])
            assert fixture.identity() == (session, token)
            later = json.loads(fixture.run(["session", "history", session, "--json"]).stdout)
            assert all(event in later for event in usage), "recovery rewrote prior paid usage"
            assert sum(e["kind"] == "usage" for e in later) > len(usage)
        return {
            "scenario": "crash-native" if native else "crash",
            "failed_turn": True,
            "children_cleaned": True,
            "exact_recovery": True,
        }
    finally:
        for pid in children:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()


def _pty_scenario(fixture: Fixture, *, compact: bool = False) -> dict:
    model = fixture.model
    binary = fixture.binary
    fixture.env.update(TERM="xterm-256color", COLORTERM="truecolor")
    identities = []
    for args, draft, expected, request_count in [
        (["-i", ":", "FIRST_INPUT 雪"], "SECOND_DRAFT 雪x", "SECOND_DRAFT 雪 edited", 2),
        (None, "RESUMED 雪x", "RESUMED 雪 edited", 3),
    ]:
        if args is None:
            args = ["-i", "resume", identities[0][0]]
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 28, 100, 0, 0))
        process = subprocess.Popen(
            [str(binary), *args],
            cwd=fixture.repo,
            env=fixture.env,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            start_new_session=True,
        )
        os.close(slave)
        output = bytearray()

        def drain_until(predicate) -> None:
            deadline = time.monotonic() + 35
            while time.monotonic() < deadline:
                if predicate():
                    return
                assert process.poll() is None, bytes(output[-3000:])
                if select.select([master], [], [], 0.1)[0]:
                    output.extend(os.read(master, 65536))
            raise AssertionError(bytes(output[-3000:]))

        try:
            drain_until(
                lambda: b"Pi protocol proof" in output if request_count == 2 else b"Pi" in output
            )
            if compact and request_count == 2:
                session, token = fixture.identity()
                with sqlite3.connect(fixture.root / "machine/loopflow.db") as db:
                    checkout = Path(
                        db.execute(
                            "SELECT cwd FROM agent_sessions WHERE id=?", (session,)
                        ).fetchone()[0]
                    )
                (checkout / "scratch/proof.md").write_text("MANUAL_COMPACT 雪")
                os.write(master, b"/compact\r")
                drain_until(
                    lambda: any(
                        json.loads(line)["type"] == "compaction"
                        for line in Path(token).read_text().splitlines()
                    )
                )
                output.clear()
            expected_count = request_count + (1 if compact else 0)
            os.write(master, draft.encode())
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 36, 80, 0, 0))
            os.kill(process.pid, signal.SIGWINCH)
            # Backspace removes the ASCII suffix while preserving the Unicode draft.
            os.write(master, b"\x7f edited\r")
            drain_until(
                lambda: (
                    len(model.requests) == expected_count
                    and expected in json.dumps(model.requests[-1], ensure_ascii=False)
                )
            )
            drain_until(lambda: b"Pi protocol proof" in output)
            if compact:
                assert "MANUAL_COMPACT" in json.dumps(model.requests[-1])
            identities.append(fixture.identity())
            os.write(master, b"\x04")
            deadline = time.monotonic() + 10
            while process.poll() is None and time.monotonic() < deadline:
                if select.select([master], [], [], 0.1)[0]:
                    try:
                        output.extend(os.read(master, 65536))
                    except OSError:
                        break
            assert process.wait(timeout=5) == 0
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            os.close(master)
    assert identities[0] == identities[1]
    return {
        "scenario": "manual" if compact else "pty",
        "model_requests": len(model.requests),
        "unicode_edit_resize": True,
        "exact_resume": True,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--lf", type=Path, default=Path("target/debug/lf"))
    parser.add_argument("--pi", type=Path, required=True)
    parser.add_argument(
        "--scenario",
        choices=[
            "all",
            "retry",
            "empty",
            "overflow",
            "threshold",
            "native",
            "pty",
            "manual",
            "steer",
            "handoff",
            "rate-limit",
            "server-error",
            "invalid-model",
            "crash",
            "crash-native",
            "tools",
        ],
        default="all",
    )
    args = parser.parse_args()
    modes = (
        [
            "retry",
            "empty",
            "overflow",
            "threshold",
            "native",
            "pty",
            "manual",
            "steer",
            "handoff",
            "rate-limit",
            "server-error",
            "invalid-model",
            "crash",
            "crash-native",
            "tools",
        ]
        if args.scenario == "all"
        else [args.scenario]
    )
    for mode in modes:
        model = Model()
        threading.Thread(target=model.serve_forever, daemon=True).start()
        try:
            with tempfile.TemporaryDirectory(prefix=f"pi-{mode}-") as directory:
                fixture = Fixture(
                    Path(directory).resolve(), args.lf.resolve(), args.pi.resolve(), model
                )
                if mode in ("pty", "manual"):
                    result = _pty_scenario(fixture, compact=mode == "manual")
                elif mode == "steer":
                    result = _steer_scenario(fixture)
                elif mode == "handoff":
                    result = _handoff_scenario(fixture)
                elif mode == "tools":
                    result = _tools_scenario(fixture)
                elif mode in ("crash", "crash-native"):
                    result = _crash_scenario(fixture, native=mode == "crash-native")
                else:
                    result = _scenario(fixture, mode)
                print(json.dumps(result), flush=True)
        finally:
            model.shutdown()
            model.server_close()


if __name__ == "__main__":
    main()
