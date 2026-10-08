"""Measure native prompt readiness without submitting a provider turn."""

import argparse
import errno
import fcntl
import json
import os
import pty
import select
import signal
import struct
import subprocess
import termios
import time
from pathlib import Path

import pyte
from fixture import require_fixture


def build_environment(home: Path, lf: Path | None) -> dict[str, str]:
    env = {k: v for k, v in os.environ.items() if not k.startswith(("LF_", "LOOPFLOW_", "CMUX_"))}
    env.pop("CLAUDE_CODE_CHILD_SESSION", None)
    env.update(LF_HOME=str(home), TERM="xterm-256color", RUST_LOG="off")
    # Bypass terminal integration wrappers equally for direct and lf launches.
    env["PATH"] = os.pathsep.join(p for p in env["PATH"].split(os.pathsep) if "cmux" not in p)
    if lf:
        env["LF_BIN"] = str(lf)
    return env


def start_sampler(pid: int, output: Path) -> subprocess.Popen:
    return subprocess.Popen(
        ["/usr/bin/sample", str(pid), "5", "1", "-mayDie", "-file", str(output / "sample.txt")],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def _terminal() -> None:
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)


def measure(
    command: list[str],
    cwd: Path,
    env: dict[str, str],
    output: Path,
    provider: str,
    timeout: float = 45,
    profile: bool = False,
    trust_fixture: bool = False,
) -> dict:
    output.mkdir(mode=0o700, parents=True, exist_ok=True)
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 160, 0, 0))
    start = time.monotonic()
    child = subprocess.Popen(
        command, cwd=cwd, env=env, stdin=slave, stdout=slave, stderr=slave, preexec_fn=_terminal
    )
    os.close(slave)
    sampler = start_sampler(child.pid, output) if profile else None
    transcript = bytearray()
    screen = pyte.Screen(160, 40)
    terminal = pyte.ByteStream(screen)
    ready = None
    marker_at = None
    trusted = False
    marker = b"zzlfprobezz"
    status = "timeout"
    try:
        while time.monotonic() - start < timeout:
            if select.select([master], [], [], 0.05)[0]:
                try:
                    data = os.read(master, 65536)
                except OSError as exc:
                    if exc.errno != errno.EIO:
                        raise
                    status = "exited"
                    break
                if not data:
                    break
                transcript.extend(data)
                terminal.feed(data)
                if b"\x1b[6n" in data:
                    os.write(master, b"\x1b[1;1R")
                if b"\x1b[c" in data or b"\x1b[>c" in data:
                    os.write(master, b"\x1b[?1;2c")
                plain = "\n".join(screen.display)
                compact = "".join(plain.split())
                if trust_fixture and not trusted and "Yes,Itrustthisfolder" in compact:
                    time.sleep(0.5)
                    os.write(master, b"\x1b[B")
                    time.sleep(0.2)
                    os.write(master, b"\r")
                    trusted = True
                if trust_fixture and not trusted and "Continuewithouttrusting" in compact:
                    os.write(master, b"\x1b")
                    trusted = True
                prompt = (
                    "❯" in plain and "mode" in plain
                    if provider == "claude"
                    else "contextleft" in compact or "?forshortcuts" in compact
                )
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
                    time.sleep(0.1)
                    if provider == "codex":
                        os.write(master, b"\x03")
                        time.sleep(0.2)
                        os.write(master, b"\x03")
                    else:
                        os.write(master, b"/exit\r")
                    deadline = time.monotonic() + 5
                    while child.poll() is None and time.monotonic() < deadline:
                        if select.select([master], [], [], 0.05)[0]:
                            try:
                                drained = os.read(master, 65536)
                                if not drained:
                                    break
                                transcript.extend(drained)
                            except OSError as exc:
                                if exc.errno != errno.EIO:
                                    raise
                                break
                    break
            if child.poll() is not None:
                status = "exited"
                break
    finally:
        # Only the process group created above is ours. Copied Machine process
        # identities are never used for cleanup.
        (output / "terminal.private").write_bytes(transcript)
        os.close(master)
        if child.poll() is None:
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except (ProcessLookupError, PermissionError):
                child.wait(timeout=1)
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait(timeout=5)
        if sampler:
            sampler.wait(timeout=15)
    result = {
        "status": status,
        "ready_ms": None if ready is None else ready * 1000,
        "editor_ms": None if marker_at is None else (marker_at - start) * 1000,
        "returncode": child.returncode,
        "load1": os.getloadavg()[0],
        "dialog_handled": trusted,
    }
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
    require_fixture(args.home)
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    print(
        json.dumps(
            measure(
                command,
                args.cwd,
                build_environment(args.home, args.lf),
                args.output,
                args.provider,
                args.timeout,
                args.profile,
                args.trust_fixture,
            )
        )
    )


if __name__ == "__main__":
    main()
