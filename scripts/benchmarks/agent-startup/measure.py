"""Measure native prompt readiness without submitting a provider turn."""

import argparse
import errno
import fcntl
import json
import os
import pty
import re
import select
import signal
import struct
import subprocess
import termios
import time
from pathlib import Path


def _environment(home: Path, lf: Path | None) -> dict[str, str]:
    env = {k: v for k, v in os.environ.items() if not k.startswith(("LF_", "LOOPFLOW_", "CMUX_"))}
    env.pop("CLAUDE_CODE_CHILD_SESSION", None)
    env.update(LF_HOME=str(home), TERM="xterm-256color", RUST_LOG="off")
    # Bypass terminal integration wrappers equally for direct and lf launches.
    env["PATH"] = os.pathsep.join(p for p in env["PATH"].split(os.pathsep) if "cmux" not in p)
    if lf:
        env["LF_BIN"] = str(lf)
    return env


def _plain(data: bytes) -> str:
    text = data.decode("utf-8", errors="replace")
    text = re.sub(r"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)", "", text)
    return re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", text)


def measure(command: list[str], cwd: Path, env: dict[str, str], output: Path,
            provider: str, timeout: float = 45, profile: bool = False, trust_fixture: bool = False) -> dict:
    output.mkdir(parents=True, exist_ok=True)
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 160, 0, 0))
    start = time.monotonic()
    child = subprocess.Popen(command, cwd=cwd, env=env, stdin=slave, stdout=slave,
                             stderr=slave, start_new_session=True)
    os.close(slave)
    sampler = None
    if profile:
        sampler = subprocess.Popen(["/usr/bin/sample", str(child.pid), "5", "1", "-mayDie",
                                    "-file", str(output / "sample.txt")],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    transcript = bytearray()
    ready = None
    marker_at = None
    trusted = False
    marker = b"lf_startup_probe_unsubmitted"
    status = "timeout"
    try:
        while time.monotonic() - start < timeout:
            if select.select([master], [], [], 0.05)[0]:
                try:
                    data = os.read(master, 65536)
                except OSError as exc:
                    if exc.errno != errno.EIO:
                        raise
                    break
                if not data:
                    break
                transcript.extend(data)
                if b"\x1b[6n" in data:
                    os.write(master, b"\x1b[1;1R")
                if b"\x1b[c" in data or b"\x1b[>c" in data:
                    os.write(master, b"\x1b[?1;2c")
                plain = _plain(transcript)
                compact = "".join(plain.split())
                if trust_fixture and not trusted and "Yes,Itrustthisfolder" in compact:
                    os.write(master, b"\x1b[B\r")
                    trusted = True
                prompt = ("❯" in plain and "mode" in plain if provider == "claude"
                          else "contextleft" in compact or "?forshortcuts" in compact)
                # A rendered footer alone can precede a usable editor. Require
                # a marker echoed by the raw-mode application, without Enter.
                if prompt and marker_at is None:
                    attrs = termios.tcgetattr(master)
                    if not attrs[3] & (termios.ECHO | termios.ICANON):
                        marker_at = time.monotonic()
                        os.write(master, marker)
                elif marker_at and marker.decode() in plain:
                    ready = time.monotonic() - start
                    status = "ready"
                    os.write(master, b"\x15")
                    break
            if child.poll() is not None:
                status = "exited"
                break
    finally:
        # Only the process group created above is ours. Copied Machine process
        # identities are never used for cleanup.
        (output / "terminal.private").write_bytes(transcript)
        if child.poll() is None:
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except (ProcessLookupError, PermissionError):
                child.wait(timeout=1)
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait()
        os.close(master)
        if sampler:
            sampler.wait(timeout=15)
        (output / "terminal.private").write_bytes(transcript)
    result = {"status": status, "ready_ms": None if ready is None else ready * 1000,
              "editor_ms": None if marker_at is None else (marker_at - start) * 1000,
              "returncode": child.returncode, "load1": os.getloadavg()[0]}
    (output / "result.json").write_text(json.dumps(result) + "\n")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--home", type=Path, required=True)
    parser.add_argument("--cwd", type=Path, required=True)
    parser.add_argument("--lf", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--provider", choices=["claude", "codex"], required=True)
    parser.add_argument("--profile", action="store_true")
    parser.add_argument("--trust-fixture", action="store_true")
    parser.add_argument("--timeout", type=float, default=45)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    print(json.dumps(measure(command, args.cwd, _environment(args.home, args.lf),
                             args.output, args.provider, args.timeout, args.profile, args.trust_fixture)))


if __name__ == "__main__":
    main()
