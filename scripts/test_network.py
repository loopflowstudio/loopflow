#!/usr/bin/env python3
"""Run a prepared test command with loopback available and external egress denied."""

from __future__ import annotations

import json
import os
import platform
import shutil
import socket
import subprocess
import sys
from pathlib import Path

NETWORK_RULES = """(deny network*)
(allow network-outbound (remote ip "localhost:*") (remote unix-socket))
(allow network-inbound (local ip "localhost:*") (local unix-socket))
(allow network-bind)
"""


def _installed_harnesses(arguments: list[str]) -> list[Path]:
    # Hide host provider installations from the application library test binary
    # so missing-CLI fixtures cannot fall through to them via lf's PATH repair.
    if Path(arguments[0]).name.rpartition("-")[0] != "loopflow":
        return []
    # lf restores these system tool directories even with a fixture PATH.
    paths = (
        Path(prefix, directory, name)
        for prefix in ("/opt/homebrew", "/usr/local")
        for directory in ("bin", "sbin")
        for name in ("claude", "codex", "opencode", "pi", "agy")
    )
    return [path for path in paths if os.access(path, os.X_OK) and path.is_file()]


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
    desktop = arguments[:1] == ["--desktop"]
    if desktop:
        arguments = arguments[1:]
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
        os.environ["LOOPFLOW_NETWORK_ISOLATED"] = "1"
        os.environ["GIT_ALLOW_PROTOCOL"] = "file"
        os.environ["GIT_TERMINAL_PROMPT"] = "0"
        os.execvp(arguments[0], arguments)
    if not arguments:
        raise SystemExit("usage: test_network.py COMMAND [ARGS...]")
    installed = _installed_harnesses(arguments)
    child = [sys.executable, str(Path(__file__).resolve()), "--isolated", *arguments]
    if platform.system() == "Darwin":
        if desktop:
            profile = Path(__file__).with_name("desktop-headless.sb").read_text()
        else:
            # macOS refuses its setuid ps inside a sandbox. Only this fixed
            # system reader leaves it; ps cannot launch fixture descendants.
            profile = (
                "(version 1)(allow default)"
                '(allow process-exec (literal "/bin/ps") (with no-sandbox))'
            )
        for path in installed:
            profile += f"(deny file-read* (literal {json.dumps(str(path))}))"
        command = ["sandbox-exec", "-p", profile + NETWORK_RULES, *child]
    elif platform.system() == "Linux" and shutil.which("unshare") and shutil.which("ip"):
        namespace = ["unshare", "--net"]
        if installed:
            # Mask only in the new, private mount namespace. /dev/null is
            # not a regular executable, so lf sees a genuinely absent CLI.
            namespace.extend(
                [
                    "--mount",
                    "--propagation",
                    "private",
                    "/bin/sh",
                    "-ec",
                    'while [ "$1" != -- ]; do /bin/mount --bind /dev/null "$1"; shift; done\n'
                    'shift; exec "$@"',
                    "hide-installed-harnesses",
                    *map(str, installed),
                    "--",
                ]
            )
        command = [
            "sudo",
            "-n",
            "-E",
            "env",
            f"PATH={os.environ['PATH']}",
            f"HOME={os.environ['HOME']}",
            *namespace,
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
