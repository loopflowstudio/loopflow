"""Deterministic native editing boundary; provider protocols remain stand-ins."""

import fcntl
import os
import pty
import select
import signal
import sqlite3
import struct
import subprocess
import sys
import termios
import time
import tty
from pathlib import Path


def _editor() -> None:
    with open("/dev/tty", "rb+", buffering=0) as terminal:
        original = termios.tcgetattr(terminal)
        tty.setraw(terminal)
        draft = bytearray()

        def _redraw(*_args: object) -> None:
            if _args:
                size = os.get_terminal_size(terminal.fileno())
                os.write(1, f"\r\nRESIZED {size.columns}x{size.lines}\r\n".encode())
            os.write(1, b"\r\x1b[2KDRAFT: " + draft)

        signal.signal(signal.SIGWINCH, _redraw)
        try:
            os.write(1, b"EDITOR_READY\r\n")
            _redraw()
            while True:
                byte = terminal.read(1)
                if byte == b"\r":
                    break
                if byte == b"\x7f":
                    draft = bytearray(draft.decode()[:-1].encode())
                else:
                    draft.extend(byte)
                _redraw()
            Path(os.environ["HOME"], "submitted").write_bytes(draft)
            os.write(1, b"\r\nEDITOR_EXIT\r\n")
            os.write(2, b"PROVIDER_STDERR\n")
        finally:
            termios.tcsetattr(terminal, termios.TCSANOW, original)


def _exercise(lf: str, provider: str, home: Path, repo: str) -> None:
    env = {
        "HOME": str(home),
        "LF_HOME": str(home / "machine"),
        "LF_BIN": lf,
        "PATH": f"{home / 'bin'}:/usr/bin:/bin",
        "CODEX_HOME": str(home / ".codex"),
        "CLAUDE_CONFIG_DIR": str(home / ".claude"),
        "TERM": "xterm-256color",
        "RUST_LOG": "warn",
        "CMUX_WORKSPACE_ID": "fixture",
        "CMUX_SURFACE_ID": "fixture",
    }
    cmux = home / "bin/cmux"
    cmux.write_text('#!/bin/sh\n[ ! -f "$HOME/warn" ]\n')
    cmux.chmod(0o755)
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))

    def _own_terminal() -> None:
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)

    child = subprocess.Popen(
        [lf, "-i", ":", "launch"],
        env=env,
        cwd=repo,
        stdin=slave,
        stdout=slave,
        stderr=slave,
        preexec_fn=_own_terminal,
    )
    os.close(slave)
    output = bytearray()
    deadline = time.monotonic() + 40

    def _read_until(predicate) -> None:
        while not predicate():
            assert time.monotonic() < deadline, output.decode(errors="replace")
            if select.select([master], [], [], 0.05)[0]:
                output.extend(os.read(master, 65536))

    try:
        _read_until(lambda: b"EDITOR_READY" in output)
        os.write(master, "café 雪 🦀x".encode())
        _read_until(lambda: "DRAFT: café 雪 🦀x".encode() in output)
        (home / "warn").touch()
        with sqlite3.connect(home / "machine/loopflow.db") as database:
            session = database.execute("SELECT id FROM agent_sessions LIMIT 1").fetchone()[0]
        renamed = subprocess.run(
            [lf, "session", "rename", session, "Edited draft"],
            env=env,
            cwd=repo,
            capture_output=True,
            timeout=10,
        )
        assert renamed.returncode == 0, renamed.stderr

        def _diagnostics() -> bytes:
            return b"".join(
                path.read_bytes() for path in (home / "machine/logs").glob("terminal-*.log")
            )

        _read_until(lambda: b"cmux title update failed" in _diagnostics())
        assert b"cmux title update failed" not in output, output
        fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 100, 0, 0))
        _read_until(lambda: b"RESIZED 100x32" in output)
        os.write(master, b"\x7f ready")
        _read_until(lambda: "DRAFT: café 雪 🦀 ready".encode() in output)
        os.write(master, b"\r")
        # lf replaces the provider's exit text with its own way back.
        _read_until(lambda: b"Continue with: lf resume" in output)
        while child.poll() is None:
            if select.select([master], [], [], 0.05)[0]:
                try:
                    output.extend(os.read(master, 65536))
                except OSError:
                    break
            assert time.monotonic() < deadline, output
        assert child.wait(timeout=5) == 0, output
        assert (home / "submitted").read_text() == "café 雪 🦀 ready"
        assert b"cmux title update failed" not in output
        assert b"Loopflow recorded diagnostics:" in output
        if provider != "opencode":
            assert b"EDITOR_EXIT" not in output, output
            # Replaced exit text survives in the execution owner's log.
            owner_log = home / "machine/logs/transport.log"
            assert b"PROVIDER_STDERR" in output + owner_log.read_bytes(), output
        else:
            assert b"PROVIDER_STDERR" not in output
            assert any(
                b"PROVIDER_STDERR" in path.read_bytes()
                for path in (home / "machine/runs").glob("*/*/opencode.stderr.log")
            )
        print(f"{provider}: draft, warning, resize, Unicode submission and exit passed")
    finally:
        (home / "terminal.txt").write_bytes(output)
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGKILL)
        child.wait()
        os.close(master)


if __name__ == "__main__":
    _exercise(sys.argv[1], sys.argv[2], Path(sys.argv[3]), sys.argv[4])
