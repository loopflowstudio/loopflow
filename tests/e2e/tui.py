"""Drive a provider TUI in a pseudo-terminal and render its screen."""

import fcntl
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


def _terminal() -> None:
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)


class _Tui:
    def __init__(self, command: list[str], work: Path, env: dict[str, str]) -> None:
        self.master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 160, 0, 0))
        self.child = subprocess.Popen(
            command,
            cwd=work,
            env=env,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            preexec_fn=_terminal,
        )
        os.close(slave)
        self.raw = bytearray()
        self.screen = pyte.Screen(160, 40)
        self.stream = pyte.ByteStream(self.screen)

    def drain(self, seconds: float) -> None:
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if not select.select([self.master], [], [], 0.05)[0]:
                continue
            try:
                data = os.read(self.master, 65536)
            except OSError:
                break
            if not data:
                break
            self.raw.extend(data)
            self.stream.feed(data)
            if b"\x1b[6n" in data:
                os.write(self.master, b"\x1b[1;1R")
            if b"\x1b[c" in data or b"\x1b[>c" in data:
                os.write(self.master, b"\x1b[?1;2c")

    def enter(self, text: str) -> None:
        os.write(self.master, text.encode())
        self.drain(0.3)
        os.write(self.master, b"\r")

    def close(self) -> None:
        if self.child.poll() is None:
            os.killpg(self.child.pid, signal.SIGKILL)
        self.child.wait(timeout=5)
        os.close(self.master)
