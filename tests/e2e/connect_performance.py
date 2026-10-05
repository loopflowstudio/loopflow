"""PTY endpoints for the isolated Codex fixture; responses are synthetic."""

import fcntl
import json
import os
import platform
import pty
import select
import signal
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


def _state(env: dict[str, str]) -> tuple | None:
    database = Path(env["LF_HOME"]) / "loopflow.db"
    if not database.exists():
        return None
    with closing(sqlite3.connect(database)) as db:
        return db.execute(
            "SELECT id,provider_thread,provider_generation,provider_pid,"
            "provider_started_at,driver_exec_id FROM agent_sessions "
            "WHERE provider_endpoint IS NOT NULL ORDER BY created_at DESC LIMIT 1"
        ).fetchone()


def _process_stamp(pid: int) -> str | None:
    result = subprocess.run(
        ["ps", "-p", str(pid), "-o", "lstart="], capture_output=True, text=True, check=False
    )
    return result.stdout.strip() if result.returncode == 0 else None


def _stop(child: subprocess.Popen, *, group: bool = False) -> None:
    if child.poll() is None:
        if group:
            os.killpg(child.pid, signal.SIGTERM)
        else:
            child.terminate()
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            if group:
                os.killpg(child.pid, signal.SIGKILL)
            else:
                child.kill()
            child.wait(timeout=5)


def _reject_attachment(
    binary: Path, work: Path, env: dict[str, str], before: tuple, log: Path
) -> None:
    stamp = _process_stamp(before[3])
    rejected = subprocess.run(
        [str(binary), "session", "connect", before[0], "--replace"],
        cwd=work,
        env={**env, "LF_PROBE_NATIVE_UI": "1", "LF_PROBE_REJECT_UI": "1"},
        capture_output=True,
        timeout=30,
    )
    log.write_bytes(rejected.stderr)
    if rejected.returncode == 0:
        raise AssertionError("invalid native launch unexpectedly succeeded")
    if _state(env) != before or _process_stamp(before[3]) != stamp:
        raise AssertionError("rejected attachment changed driver or engine")


def measure(
    binary: Path,
    work: Path,
    env: dict[str, str],
    server: object,
    results: dict,
    output: Path,
    samples: int,
) -> None:
    if samples < 1:
        raise ValueError("samples must be positive")
    with (Path(env["CODEX_HOME"]) / "config.toml").open("a") as config:
        config.write(f'\n[projects.{json.dumps(str(work.resolve()))}]\ntrust_level = "trusted"\n')
    results.update(
        endpoint="native Codex PTY bytes, synthetic local Responses server",
        host=platform.platform(),
        provider_paths={
            "codex_conversation": "attempted",
            "codex_flow_review": "not measured; source route replaces native clients",
            "claude": "no existing Codex relay path; not measured",
            "opencode": "no existing Codex relay path; not measured",
        },
        samples=[],
        targets=None,
        measured_at=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        observer_limits=[
            "PTY byte markers, not terminal screen reconstruction or compositor presentation",
            "Input response includes a deliberate 300ms submission delay and synthetic tool work",
            "Database polling and concurrent host work affect timings",
            "No pagination, authenticated response, or Flow-review proof",
            "Failed replacement preserves the original UI draft; "
            "successful takeover does not copy drafts",
        ],
    )
    for index in range(samples):
        sample = {"index": index, "output_ms": None, "input_response_ms": None}
        results["samples"].append(sample)
        server.held.clear()
        server.release.clear()
        marker = f"retained-{uuid.uuid4().hex}"
        server.response_text = marker
        log_path = output / f"seed-{index}.log"
        with log_path.open("wb") as log:
            seed = subprocess.Popen(
                [str(binary), "--mode", "batch", "--model", "codex", ":", "held conversation"],
                cwd=work,
                env=env,
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=log,
            )
            client = None
            master = slave = None
            try:
                if not server.held.wait(30):
                    raise TimeoutError("seed did not reach held Responses request")
                before = _state(env)
                if before is None:
                    raise RuntimeError("seed has no recorded live engine")
                sample["before"] = before
                sample["engine_before"] = _process_stamp(before[3])
                if sample["engine_before"] is None:
                    raise RuntimeError("recorded engine is no longer alive")
                _reject_attachment(binary, work, env, before, output / f"rejected-{index}.log")
                sample["failed_start_preserved"] = True
                master, slave = pty.openpty()
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 160, 0, 0))
                started = time.monotonic()
                client = subprocess.Popen(
                    [str(binary), "session", "connect", before[0]],
                    cwd=work,
                    env={**env, "TERM": "xterm-256color", "LF_PROBE_NATIVE_UI": "1"},
                    stdin=slave,
                    stdout=slave,
                    stderr=slave,
                    preexec_fn=_terminal,
                )
                os.close(slave)
                slave = None
                transcript = bytearray()
                marker_bytes = marker.encode()
                response = f"input-{uuid.uuid4().hex}"
                response_bytes = response.encode()
                submit_at = None
                with (output / f"terminal-{index}.bin").open("wb") as terminal:
                    while time.monotonic() - started < 45:
                        if submit_at is not None and time.monotonic() >= submit_at:
                            os.write(master, b"\r")
                            sample["input_sent_ms"] = (time.monotonic() - started) * 1000
                            submit_at = None
                        if "driver_claim_ms" not in sample:
                            state = _state(env)
                            if state and state[5] != before[5]:
                                sample["driver_claim_ms"] = (time.monotonic() - started) * 1000
                                server.release.set()
                        if select.select([master], [], [], 0.05)[0]:
                            try:
                                chunk = os.read(master, 65536)
                            except OSError:
                                break
                            if not chunk:
                                break
                            terminal.write(chunk)
                            terminal.flush()
                            transcript.extend(chunk)
                            # Answer terminal capability queries, never infer readiness from them.
                            if b"\x1b[6n" in chunk:
                                os.write(master, b"\x1b[1;1R")
                            if b"\x1b[c" in chunk:
                                os.write(master, b"\x1b[?1;2c")
                            if sample["output_ms"] is None and marker_bytes in transcript:
                                sample["output_ms"] = (time.monotonic() - started) * 1000
                                server.response_text = response
                                os.write(master, b"Report the input probe result.")
                                submit_at = time.monotonic() + 0.3
                            if sample["output_ms"] is not None and response_bytes in transcript:
                                sample["input_response_ms"] = (time.monotonic() - started) * 1000
                                break
                        if client.poll() is not None:
                            break
                if sample["input_response_ms"] is not None:
                    draft = f"Retain draft {uuid.uuid4().hex}"
                    os.write(master, b"\x1b[200~" + draft.encode() + b"\x1b[201~")
                    draft_output = bytearray()
                    deadline = time.monotonic() + 5
                    while draft.encode() not in draft_output and time.monotonic() < deadline:
                        if select.select([master], [], [], 0.05)[0]:
                            draft_output.extend(os.read(master, 65536))
                    (output / f"draft-{index}.bin").write_bytes(draft_output)
                    if draft.encode() not in draft_output:
                        raise AssertionError("native UI did not display the draft")
                    current = _state(env)
                    _reject_attachment(
                        binary, work, env, current, output / f"rejected-draft-{index}.log"
                    )
                    if client.poll() is not None:
                        raise AssertionError("failed replacement stopped the controlling UI")
                    os.write(master, b"\r")
                    deadline = time.monotonic() + 10
                    while time.monotonic() < deadline:
                        if any(
                            draft in json.dumps(request["input"]) for request in server.requests
                        ):
                            sample["failed_replacement_preserved_draft"] = True
                            break
                        if select.select([master], [], [], 0.05)[0]:
                            with (output / f"draft-{index}.bin").open("ab") as log:
                                log.write(os.read(master, 65536))
                    else:
                        raise AssertionError("retained UI did not submit its exact draft")
                sample["after"] = _state(env)
                timing_path = output / f"timings-{index}.jsonl"
                with closing(sqlite3.connect(Path(env["LF_HOME"]) / "loopflow.db")) as db:
                    db.execute("BEGIN EXCLUSIVE")
                    with timing_path.open("w") as timing_log:
                        diagnostic = subprocess.Popen(
                            [str(binary), "session", "timings", sample["after"][5]],
                            cwd=work, env=env, stdout=timing_log, stderr=subprocess.DEVNULL,
                        )
                        try:
                            deadline = time.monotonic() + 5
                            while "input_accepted" not in timing_path.read_text():
                                if time.monotonic() >= deadline or diagnostic.poll() is not None:
                                    raise AssertionError("timing output waits for SQLite admission")
                                time.sleep(0.02)
                        finally:
                            db.rollback()
                            diagnostic.wait(timeout=15)
                        if diagnostic.returncode != 0:
                            raise AssertionError("timing reader failed")
                phases = [json.loads(line) for line in timing_path.read_text().splitlines()]
                if not {"lookup_complete", "attached", "input_accepted"}.issubset(
                    {phase["phase"] for phase in phases}
                ):
                    raise AssertionError("connection diagnostic is missing an observed phase")
                if any(phase["phase"] == "exited" for phase in phases):
                    raise AssertionError("attached lifetime was recorded before client exit")
                sample["diagnostic_while_attached"] = True
                sample["engine_after"] = _process_stamp(before[3])
                sample["continuity"] = (
                    sample["after"] is not None
                    and before[:5] == sample["after"][:5]
                    and sample["engine_before"] == sample["engine_after"]
                )
                sample["status"] = (
                    "passed"
                    if sample["input_response_ms"] is not None and sample["continuity"]
                    else "failed"
                )
                sample["elapsed_ms"] = (time.monotonic() - started) * 1000
                sample["exit_before_cleanup"] = client.poll()
                # This sample started with a held turn. It does not prove retained-history replay.
                sample["history_population"] = "one in-flight seed turn"
            except Exception as error:
                sample.update(status="failed", error=str(error))
            finally:
                server.release.set()
                for fd in (master, slave):
                    if fd is not None:
                        os.close(fd)
                if client is not None:
                    _stop(client, group=True)
                _stop(seed)
                if sample.get("diagnostic_while_attached"):
                    diagnostic = subprocess.run(
                        [str(binary), "session", "timings", sample["after"][5]],
                        cwd=work, env=env, capture_output=True, text=True, check=True, timeout=5,
                    )
                    (output / f"timings-after-{index}.jsonl").write_text(diagnostic.stdout)
                    phases = [json.loads(line) for line in diagnostic.stdout.splitlines()]
                    if not any(phase["attached_lifetime_ms"] is not None for phase in phases):
                        sample.update(status="failed", error="attached lifetime was not recorded")
                (output / "results.json").write_text(json.dumps(results, indent=2))
        if sample["status"] != "passed":
            break
