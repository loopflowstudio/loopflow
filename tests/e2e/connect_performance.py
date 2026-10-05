"""Retained native terminal endpoints using private Homes and synthetic Responses."""

import fcntl
import hashlib
import json
import os
import platform
import pty
import select
import shlex
import sqlite3
import struct
import subprocess
import termios
import time
import uuid
from contextlib import closing
from pathlib import Path


def _terminal() -> None:
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)


def _spawn(argv: list[str], work: Path, env: dict[str, str]) -> tuple[subprocess.Popen, int]:
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 160, 0, 0))
    try:
        child = subprocess.Popen(
            argv, cwd=work, env=env, stdin=slave, stdout=slave, stderr=slave, preexec_fn=_terminal
        )
    finally:
        os.close(slave)
    return child, master


def _read(master: int, marker: bytes, timeout: float = 20) -> bytes:
    output = bytearray()
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if not select.select([master], [], [], 0.01)[0]:
            continue
        try:
            value = os.read(master, 65536)
        except OSError:
            break
        if not value:
            break
        output.extend(value)
        if b"\x1b[6n" in value:
            os.write(master, b"\x1b[1;1R")
        if b"\x1b[c" in value:
            os.write(master, b"\x1b[?1;2c")
        if marker and marker in output:
            return bytes(output)
    if marker:
        raise AssertionError(f"terminal did not display {marker!r}: {bytes(output)[-3000:]!r}")
    return bytes(output)


def _stop(child: subprocess.Popen, master: int) -> None:
    try:
        if child.poll() is None:
            child.terminate()
            try:
                _wait(child, master)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=5)
    finally:
        os.close(master)


def _wait(child: subprocess.Popen, master: int) -> None:
    deadline = time.monotonic() + 5
    while child.poll() is None and time.monotonic() < deadline:
        _read(master, b"", 0.05)
    child.wait(timeout=1)


def _state(env: dict[str, str], session: str) -> tuple:
    with closing(sqlite3.connect(Path(env["LF_HOME"]) / "loopflow.db")) as db:
        return db.execute(
            "SELECT id,current_capture,driver_exec_id,driver_generation,"
            "provider_generation,provider_thread "
            "FROM agent_sessions WHERE id=?",
            (session,),
        ).fetchone()


def _native_identity(env: dict[str, str]) -> list[dict]:
    clients = []
    for path in (Path(env["LF_HOME"]) / "runs").glob("*/*/provider-clients/*.json"):
        client = json.loads(path.read_text())
        client["os_birth"] = subprocess.check_output(
            ["ps", "-p", str(client["pid"]), "-o", "lstart="], text=True
        ).strip()
        clients.append(client)
    assert len(clients) == 1, "expected exactly one retained native UI"
    return clients


def measure(
    binary: Path,
    work: Path,
    env: dict[str, str],
    server: object,
    results: dict,
    output: Path,
    samples: int,
    review_fixture: Path | None = None,
    history_turns: int = 1,
) -> None:
    results.update(
        endpoint="native Codex screen bytes and composer response; no compositor",
        measured_at=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        host=platform.platform(),
        tmux_version=subprocess.check_output(["tmux", "-V"], text=True).strip(),
        samples=[],
        history_turns=history_turns,
        targets=None,
        provider_paths={
            "codex_conversation": "attempted",
            "codex_flow_review": "unmeasured",
            "claude": "unmeasured",
            "opencode": "unmeasured",
        },
        observer_limits=[
            "synthetic Responses, no authenticated provider",
            "composer response is a readiness upper bound, final submission checks draft contents",
            "fixed live UI/history; no provider restart between samples",
        ],
    )
    config = work / ".lf"
    (config / "skills/repo").mkdir(parents=True)
    (config / "config.yaml").write_text("agent: codex\n")
    (config / "skills/repo/session.md").write_text(
        "Run the fixture command and report its response.\n"
    )
    subprocess.run(["git", "init", "-q", str(work)], env=env, check=True)
    subprocess.run(["git", "add", ".lf"], cwd=work, env=env, check=True)
    subprocess.run(
        [
            "git",
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.test",
            "commit",
            "-qm",
            "fixture",
        ],
        cwd=work,
        env=env,
        check=True,
    )
    env = {**env, "TERM": "xterm-256color", "COLORTERM": "truecolor", "LF_PROBE_NATIVE_UI": "1"}
    review = None
    if review_fixture:
        setup = subprocess.run(
            [str(review_fixture), "write_native_review_fixture", "--ignored", "--test-threads=1"],
            env={**env, "LF_REVIEW_FIXTURE": str(work.parent), "TMPDIR": str(work.parent)},
            cwd=work,
            capture_output=True,
            timeout=30,
        )
        (output / "review-setup.log").write_bytes(setup.stdout + setup.stderr)
        setup.check_returncode()
        review = json.loads((work.parent / "review.json").read_text())
        work = Path(review["cwd"])
        results["provider_paths"]["codex_flow_review"] = "attempted"
        results["provider_paths"]["codex_conversation"] = "not selected"
    with (Path(env["CODEX_HOME"]) / "config.toml").open("a") as trusted:
        trusted.write(f'\n[projects.{json.dumps(str(work.resolve()))}]\ntrust_level = "trusted"\n')
    marker = f"retained-{uuid.uuid4().hex}"
    server.response_text = marker
    server.release.set()
    if review:
        session = review["id"]
    else:
        opened = subprocess.run(
            [str(binary), "session", "ensure", "--json"],
            cwd=work,
            env=env,
            capture_output=True,
            timeout=40,
        )
        (output / "ensure.log").write_bytes(opened.stderr)
        opened.check_returncode()
        session = json.loads(opened.stdout)["id"]
    digest = hashlib.sha256(f"{Path(env['LF_HOME']).resolve()}\0{session}".encode()).hexdigest()[
        :32
    ]
    socket = Path(f"/tmp/lf-term-{os.geteuid()}") / digest / "socket"
    tmux = ["tmux", "-S", str(socket)]
    clients = []
    results["session"] = session
    try:
        child, master = _spawn([str(binary), "session", "connect", session], work, env)
        clients.append((child, master))
        initial = _read(master, marker.encode(), 40)
        (output / "initial.bin").write_bytes(initial)
        for index in range(1, history_turns):
            marker = f"history-{index}-{uuid.uuid4().hex}"
            server.response_text = marker
            os.write(master, f"Continue fixture history turn {index}.".encode())
            # Enter is separate from typing: Codex distinguishes pasted lines.
            time.sleep(0.05)
            os.write(master, b"\r")
            _read(master, marker.encode())
        draft = f"retain-draft-{uuid.uuid4().hex}"
        os.write(master, b"\x1b[200~" + draft.encode() + b"\x1b[201~")
        (output / "draft.bin").write_bytes(_read(master, draft.encode()))
        before = _state(env, session)
        results["before"] = before
        results["native_before"] = _native_identity(env)
        rejected = subprocess.run(
            [str(binary), "session", "connect", session, "--take-control"],
            cwd=work,
            env=env,
            capture_output=True,
            timeout=10,
        )
        (output / "rejected-attachment.log").write_bytes(rejected.stdout + rejected.stderr)
        assert rejected.returncode != 0
        assert _native_identity(env) == results["native_before"]
        assert _state(env, session) == before
        results["failed_attachment_preserved"] = True
        histories = list((Path(env["CODEX_HOME"]) / "sessions").rglob("*.jsonl"))
        assert len(histories) == 1, "expected one native conversation history"
        history = histories[0].read_bytes()
        results["history_bytes_before"] = len(history)
        results["history_prefix_sha256"] = hashlib.sha256(history).hexdigest()
        if review:
            rejected = subprocess.run(
                [str(binary), "session", "complete", session],
                cwd=work,
                env=env,
                capture_output=True,
                timeout=10,
            )
            (output / "not-ready.log").write_bytes(rejected.stdout + rejected.stderr)
            assert rejected.returncode != 0, "unready review completed"
            assert before == _state(env, session)
        provider_pid = subprocess.check_output(
            [*tmux, "display", "-p", "-t", "session:0.0", "#{pane_pid}"], text=True
        ).strip()
        results["service_pid"] = provider_pid
        os.write(master, b"\x02d")
        _wait(child, master)
        assert child.returncode == 0, "initial attachment failed while detaching"
        for index in range(samples):
            sample = {
                "index": index,
                "output_ms": None,
                "input_response_ms": None,
                "status": "failed",
            }
            results["samples"].append(sample)
            started = time.monotonic()
            child, master = _spawn([str(binary), "session", "connect", session], work, env)
            clients.append((child, master))
            transcript = _read(master, draft.encode())
            sample["output_ms"] = (time.monotonic() - started) * 1000
            assert marker.encode() in transcript, "attachment lost visible retained history"
            suffix = f"-input-{index}"
            os.write(master, b"\x1b[200~" + suffix.encode() + b"\x1b[201~")
            transcript += _read(master, suffix.encode())
            sample["input_response_ms"] = (time.monotonic() - started) * 1000
            (output / f"terminal-{index}.bin").write_bytes(transcript)
            os.write(master, b"\x7f" * len(suffix))
            _read(master, b"", 0.15)
            sample["same_identity"] = before == _state(env, session)
            assert sample["same_identity"], "attachment changed Session/driver identity"
            os.write(master, b"\x02d")
            _wait(child, master)
            assert child.returncode == 0, "attachment failed while detaching"
            sample["status"] = "passed"
        simultaneous = [
            _spawn([str(binary), "session", "connect", session], work, env) for _ in range(2)
        ]
        clients.extend(simultaneous)
        for _, terminal in simultaneous:
            _read(terminal, draft.encode())
        attached = subprocess.check_output([*tmux, "list-clients", "-F", "#{client_pid}"])
        assert len(attached.splitlines()) == 1, "concurrent connects acquired two controllers"
        for presenter, terminal in simultaneous:
            presenter.terminate()
            _wait(presenter, terminal)
        assert before == _state(env, session)
        assert results["native_before"] == _native_identity(env)
        results["concurrent_connects_one_controller"] = True
        child, master = _spawn([str(binary), "session", "connect", session], work, env)
        clients.append((child, master))
        _read(master, draft.encode())
        stalled, unread = _spawn([str(binary), "session", "connect", session], work, env)
        clients.append((stalled, unread))
        time.sleep(0.5)
        stalled.terminate()
        stalled.wait(timeout=3)
        assert before == _state(env, session)
        results["unread_passive_termination_preserves_session"] = True
        # A second connection is passive. Its typing cannot alter the draft.
        passive, second = _spawn([str(binary), "session", "connect", session], work, env)
        clients.append((passive, second))
        (output / "passive.bin").write_bytes(_read(second, draft.encode()))
        os.write(second, b"FORBIDDEN INPUT\r")
        _read(second, b"", 0.2)
        replacement, third = _spawn(
            [str(binary), "session", "connect", session, "--take-control"], work, env
        )
        clients.append((replacement, third))
        (output / "takeover.bin").write_bytes(_read(third, draft.encode()))
        _wait(child, master)
        assert child.returncode == 0, "previous controller failed while detaching"
        if review:
            server.command = shlex.join([str(binary), "session", "ready", "retained review proof"])
        os.write(third, b"\r")
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            requests = [
                request for request in server.requests if draft in json.dumps(request.get("input"))
            ]
            if requests:
                submitted = json.dumps(requests[-1]["input"])
                assert "FORBIDDEN INPUT" not in submitted
                assert "-input-" not in submitted
                results["submitted_retained_draft"] = True
                (output / "submitted.json").write_text(json.dumps(requests[-1]["input"], indent=2))
                break
            _read(third, b"", 0.1)
        else:
            raise AssertionError("retained draft did not reach the provider")
        results["after"] = _state(env, session)
        assert results["after"] == before
        results["takeover_keeps_driver"] = True
        results["native_after"] = _native_identity(env)
        assert results["native_after"] == results["native_before"]
        assert histories == list((Path(env["CODEX_HOME"]) / "sessions").rglob("*.jsonl"))
        assert histories[0].read_bytes().startswith(history)
        results["native_history_preserved"] = True
        if review:
            deadline = time.monotonic() + 15
            while True:
                with closing(sqlite3.connect(Path(env["LF_HOME"]) / "loopflow.db")) as db:
                    ready = db.execute(
                        "SELECT ready_summary FROM agent_sessions WHERE id=?", (session,)
                    ).fetchone()[0]
                if ready == "retained review proof":
                    break
                if time.monotonic() >= deadline:
                    raise AssertionError("native review did not become ready")
                _read(third, b"", 0.1)
            results["native_review_ready"] = True
        timings = Path(env["LF_HOME"]) / "runtime/session-connect"
        for path in timings.glob("*.jsonl"):
            diagnostic = subprocess.run(
                [str(binary), "session", "timings", path.stem],
                cwd=work,
                env=env,
                capture_output=True,
                timeout=10,
            )
            diagnostic.check_returncode()
            (output / path.name).write_bytes(diagnostic.stdout)
            records = [json.loads(line) for line in diagnostic.stdout.splitlines()]
            assert all(record["selector"] == session for record in records)
            if not any(record["phase"] == "attached" for record in records):
                assert any(record["phase"] == "failed" for record in records), records
            assert any(
                record["phase"] == "input_ready" and record["unavailable"] for record in records
            )
    except BaseException as error:
        results["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        cleanup_errors = []
        screen = subprocess.run(
            [*tmux, "capture-pane", "-p", "-e", "-t", "session:0.0"], capture_output=True, timeout=5
        )
        (output / "last-screen.bin").write_bytes(screen.stdout + screen.stderr)
        try:
            completed = subprocess.run(
                [str(binary), "session", "complete", session],
                cwd=work,
                env=env,
                capture_output=True,
                timeout=20,
            )
            (output / "complete.log").write_bytes(completed.stdout + completed.stderr)
            completed.check_returncode()
            if review:
                with closing(sqlite3.connect(Path(env["LF_HOME"]) / "loopflow.db")) as db:
                    settled = db.execute(
                        "SELECT s.completed_at,s.ready_summary,f.state "
                        "FROM agent_sessions s JOIN flow_sessions f ON f.id=s.flow_session_id "
                        "WHERE s.id=? AND f.id=? AND s.current_capture=?",
                        (session, review["flow"], before[1]),
                    ).fetchone()
                assert settled is not None and settled[0] is not None
                assert settled[1:] == ("retained review proof", "completed"), settled
                results["exact_review_completion"] = True
            deadline = time.monotonic() + 5
            while subprocess.run([*tmux, "has-session"], capture_output=True).returncode == 0:
                if time.monotonic() >= deadline:
                    raise TimeoutError("retained service survived Session completion")
                time.sleep(0.05)
        except Exception as error:
            cleanup_errors.append(str(error))
            subprocess.run([*tmux, "kill-server"], capture_output=True, timeout=5)
        for child, master in clients:
            try:
                _stop(child, master)
            except Exception as error:
                cleanup_errors.append(str(error))
        results["cleanup_errors"] = cleanup_errors
        results["server_absent"] = (
            subprocess.run([*tmux, "has-session"], capture_output=True, timeout=5).returncode != 0
        )
        if "native_before" in results:
            native = results["native_before"][0]
            process = subprocess.run(
                ["ps", "-p", str(native["pid"]), "-o", "lstart="],
                capture_output=True,
                text=True,
                timeout=5,
            )
            results["native_absent"] = (
                process.returncode != 0 or process.stdout.strip() != native["os_birth"]
            )
            if not results["native_absent"]:
                cleanup_errors.append("the owned native UI survived completion")
    if results["cleanup_errors"] or not results["server_absent"]:
        raise AssertionError("retained Session cleanup failed; see cleanup_errors")
