#!/usr/bin/env python3
"""Overlap the app's exact control CLI preparation with its ordinary CI build."""

from __future__ import annotations

import json
import os
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
XCODE_ARGS = [
    "-project",
    "LoopflowSwift.xcodeproj",
    "-scheme",
    "LoopflowMac",
    "-destination",
    "platform=macOS",
    "-derivedDataPath",
    ".build/xcode-derived-data",
    "-disableAutomaticPackageResolution",
    "CODE_SIGNING_ALLOWED=YES",
    "CODE_SIGNING_REQUIRED=YES",
    "CODE_SIGN_STYLE=Manual",
    "CODE_SIGN_IDENTITY=-",
    "DEVELOPMENT_TEAM=",
]


def _stamp(message: str, started: float) -> None:
    print(
        f"{datetime.now(timezone.utc).isoformat()} +{time.monotonic() - started:.3f}s {message}",
        flush=True,
    )


def _signal_group(pid: int, signum: int) -> bool:
    try:
        os.killpg(pid, signum)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        if signum == 0:
            return True
        # Darwin can report EPERM while an orphaned group contains only zombies.
        rows = subprocess.check_output(["ps", "-axo", "pgid=,stat="], text=True)
        if any(
            int(group) == pid and not state.startswith("Z")
            for group, state in (row.split() for row in rows.splitlines())
        ):
            raise
        return False


def _xcode_compilers() -> set[int]:
    output = str(REPO_ROOT / "swift/.build/xcode-derived-data") + "/"
    rows = subprocess.check_output(["ps", "-axo", "pid=,stat=,comm="], text=True)
    pids = {
        int(pid)
        for pid, state, command in (row.split(maxsplit=2) for row in rows.splitlines())
        if not state.startswith("Z")
        and Path(command).name in {"swift-frontend", "swiftc", "clang", "ld"}
    }
    if not pids:
        return set()
    result = subprocess.run(
        ["ps", "-wwp", ",".join(map(str, pids)), "-o", "pid=,args="],
        capture_output=True,
        text=True,
    )
    if result.returncode not in (0, 1):
        raise RuntimeError(f"Cannot inspect Xcode compilers: {result.stderr}")
    return {
        int(pid)
        for pid, args in (row.split(maxsplit=1) for row in result.stdout.splitlines())
        if any(
            f"{flag} {output}" in args
            for flag in ("-o", "-emit-module-path", "-output-file-map", "-emit-dependencies-path")
        )
    }


def _stop_xcode_compilers(preexisting: set[int]) -> None:
    # Xcode's shared build service can outlive xcodebuild. Output paths delimit
    # this build's compilers; the service itself is never a cleanup target.
    for signum in (None, signal.SIGTERM, signal.SIGKILL):
        remaining = _xcode_compilers() - preexisting
        for pid in remaining:
            if signum is not None:
                try:
                    os.kill(pid, signum)
                except ProcessLookupError:
                    pass
        deadline = time.monotonic() + 2
        while remaining and time.monotonic() < deadline:
            time.sleep(0.05)
            remaining = _xcode_compilers() - preexisting
        if not remaining:
            return
    raise RuntimeError(f"Xcode compilers survived cleanup: {sorted(remaining)}")


def _cleanup(children: list[subprocess.Popen]) -> None:
    # The session leader may have exited while a compiler still owns its group.
    groups = [child.pid for child in children if _signal_group(child.pid, signal.SIGTERM)]
    deadline = time.monotonic() + 2
    while groups and time.monotonic() < deadline:
        for child in children:
            child.poll()
        groups = [pid for pid in groups if _signal_group(pid, 0)]
        if groups:
            time.sleep(0.05)
    for pid in groups:
        _signal_group(pid, signal.SIGKILL)
    for child in children:
        child.wait()


def _run_builds(prepare: list[str], xcode: list[str], cwd: Path, env: dict[str, str]) -> int:
    started = time.monotonic()
    children: dict[str, subprocess.Popen] = {}
    preexisting = _xcode_compilers()
    succeeded = False
    cancelled = 0

    def _cancel(signum: int, frame: object) -> None:
        nonlocal cancelled
        cancelled = signum

    handlers = {s: signal.signal(s, _cancel) for s in (signal.SIGINT, signal.SIGTERM)}
    try:
        for label, command, environment in (("control", prepare, env), ("xcode", xcode, None)):
            if cancelled:
                return 128 + cancelled
            children[label] = subprocess.Popen(
                command, cwd=cwd, env=environment, start_new_session=True
            )
            _stamp(f"{label} started pid={children[label].pid}", started)
        pending = children.copy()
        while pending:
            if cancelled:
                return 128 + cancelled
            for label, child in list(pending.items()):
                status = child.poll()
                if status is None:
                    continue
                _stamp(f"{label} finished status={status}", started)
                del pending[label]
                if status:
                    return status if status > 0 else 128 - status
            if pending:
                time.sleep(0.05)
        succeeded = True
        return 0
    except OSError as error:
        print(f"Cannot launch CI build: {error}", file=sys.stderr, flush=True)
        return 1
    finally:
        for signum in handlers:
            signal.signal(signum, signal.SIG_IGN)
        try:
            _cleanup(list(children.values()))
            if not succeeded:
                _stop_xcode_compilers(preexisting)
            _stamp("builds and cleanup finished", started)
        finally:
            for signum, handler in handlers.items():
                signal.signal(signum, handler)


def main() -> int:
    started = time.monotonic()
    swift = REPO_ROOT / "swift"
    xcode = ["/usr/bin/time", "-l", "xcodebuild", "build-for-testing", *XCODE_ARGS]
    if os.environ.get("XCODE_CACHE_HIT") == "true":
        os.chdir(swift)
        os.execvp(xcode[0], xcode)
    # Resolve effective Xcode inputs rather than maintaining a second SDK/target.
    settings = subprocess.check_output(
        ["xcodebuild", "-showBuildSettings", "-json", *XCODE_ARGS], cwd=swift, text=True
    )
    app = next(
        row["buildSettings"] for row in json.loads(settings) if row["target"] == "LoopflowMac"
    )
    env = dict(
        os.environ, SDKROOT=app["SDKROOT"], MACOSX_DEPLOYMENT_TARGET=app["MACOSX_DEPLOYMENT_TARGET"]
    )
    # Match the toolchain search path in the app's control-tools shell phase.
    env["PATH"] = f"{Path.home() / '.cargo/bin'}:/opt/homebrew/bin:/usr/local/bin:{env['PATH']}"
    _stamp(f"SDKROOT={env['SDKROOT']} deployment={env['MACOSX_DEPLOYMENT_TARGET']}", started)
    result = _run_builds(
        [
            "/usr/bin/time",
            "-l",
            sys.executable,
            str(REPO_ROOT / "scripts/loopflow-dev.py"),
            "prepare-control-tools",
        ],
        xcode,
        swift,
        env,
    )
    _stamp(f"total status={result}", started)
    return result


if __name__ == "__main__":
    sys.exit(main())
