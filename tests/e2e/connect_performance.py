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
from pathlib import Path


def _terminal() -> None:
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)


def _state(env: dict[str, str]) -> tuple | None:
    database = Path(env["LF_HOME"]) / "loopflow.db"
    if not database.exists():
        return None
    with sqlite3.connect(database) as db:
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
            "No draft retention, pagination, authenticated response, or Flow-review proof",
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
                response = f"input-{uuid.uuid4().hex}"
                sent = False
                submit_at = None
                released = False
                with (output / f"terminal-{index}.bin").open("wb") as terminal:
                    while time.monotonic() - started < 45:
                        if submit_at is not None and time.monotonic() >= submit_at:
                            os.write(master, b"\r")
                            sample["input_sent_ms"] = (time.monotonic() - started) * 1000
                            submit_at = None
                        state = _state(env)
                        if state and state[5] != before[5] and not released:
                            sample["driver_claim_ms"] = (time.monotonic() - started) * 1000
                            server.release.set()
                            released = True
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
                            if marker.encode() in transcript and not sent:
                                sample["output_ms"] = (time.monotonic() - started) * 1000
                                server.response_text = response
                                os.write(master, b"Report the input probe result.")
                                submit_at = time.monotonic() + 0.3
                                sent = True
                            if sent and response.encode() in transcript:
                                sample["input_response_ms"] = (time.monotonic() - started) * 1000
                                break
                        if client.poll() is not None:
                            break
                sample["after"] = _state(env)
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
                (output / "results.json").write_text(json.dumps(results, indent=2))
        if sample["status"] != "passed":
            break
