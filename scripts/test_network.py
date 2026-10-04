#!/usr/bin/env python3
"""Run a prepared test command with loopback available and external egress denied."""

from __future__ import annotations

import os
import platform
import shutil
import socket
import subprocess
import sys
from pathlib import Path

PROFILE = """(version 1)
(allow default)
; macOS setuid ps cannot execute sandboxed. This fixed system reader neither
; connects to services nor launches children; match desktop-headless.sb.
(allow process-exec (literal "/bin/ps") (with no-sandbox))
(deny network*)
(allow network-outbound (remote ip "localhost:*") (remote unix-socket))
(allow network-inbound (local ip "localhost:*") (local unix-socket))
(allow network-bind)
"""


def _probe() -> None:
    with socket.socket() as server:
        server.bind(("127.0.0.1", 0))
        server.listen(1)
        with socket.create_connection(server.getsockname(), timeout=1):
            connection, _ = server.accept()
            connection.close()
    with socket.socket() as client:
        client.settimeout(1)
        try:
            client.connect(("1.1.1.1", 443))
        except OSError:
            pass
        else:
            raise RuntimeError("external network is available inside test isolation")


def main() -> int:
    arguments = sys.argv[1:]
    if arguments[:1] == ["--isolated"]:
        arguments = arguments[1:]
        if platform.system() == "Linux":
            subprocess.run(["ip", "link", "set", "lo", "up"], check=True)
            if os.environ.get("SUDO_UID"):
                os.setgroups(os.getgrouplist(os.environ["SUDO_USER"], int(os.environ["SUDO_GID"])))
                os.setgid(int(os.environ["SUDO_GID"]))
                os.setuid(int(os.environ["SUDO_UID"]))
        _probe()
        for name in list(os.environ):
            if name.startswith("LF_"):
                del os.environ[name]
        os.environ["GIT_ALLOW_PROTOCOL"] = "file"
        os.environ["GIT_TERMINAL_PROMPT"] = "0"
        os.execvp(arguments[0], arguments)
    if not arguments:
        raise SystemExit("usage: test_network.py COMMAND [ARGS...]")
    child = [sys.executable, str(Path(__file__).resolve()), "--isolated", *arguments]
    if platform.system() == "Darwin":
        command = ["sandbox-exec", "-p", PROFILE, *child]
    elif platform.system() == "Linux" and shutil.which("unshare") and shutil.which("ip"):
        command = [
            "sudo",
            "-n",
            "-E",
            "env",
            f"PATH={os.environ['PATH']}",
            f"HOME={os.environ['HOME']}",
            "unshare",
            "--net",
            *child,
        ]
    else:
        raise SystemExit(
            "test network isolation unavailable: requires macOS sandbox-exec "
            "or Linux sudo/unshare/ip"
        )
    return subprocess.call(command)


if __name__ == "__main__":
    sys.exit(main())
