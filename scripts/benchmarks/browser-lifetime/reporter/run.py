"""Fault only a disposable, locally linked Sentry-disabled Ghostty process."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import re
import shutil
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]


def _sha(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def _run(command: list[str], output: Path, env: dict[str, str]) -> subprocess.CompletedProcess:
    with (
        output.with_suffix(".stdout").open("wb") as stdout,
        output.with_suffix(".stderr").open("wb") as stderr,
    ):
        with subprocess.Popen(command, stdout=stdout, stderr=stderr, env=env) as child:
            try:
                code = child.wait(timeout=30)
            except BaseException:
                child.kill()
                child.wait()
                raise
    return subprocess.CompletedProcess(command, code)


def _ips(
    path: Path, pid: int, executable: Path, uuid: str, start: dt.datetime, end: dt.datetime
) -> bool:
    try:
        # The first JSON line is a summary, followed by the full report object.
        _, body = path.read_text().split("\n", 1)
        report = json.loads(body)
        if not isinstance(report, dict):
            return False
        captured = dt.datetime.strptime(report["captureTime"], "%Y-%m-%d %H:%M:%S.%f %z")
        # macOS redacts procPath under a Home to /Users/USER/.../*/. Use the
        # newly linked Mach-O UUID, PID and fault interval, never a basename alone.
        image = any(
            isinstance(item, dict)
            and isinstance(item.get("uuid"), str)
            and item["uuid"].lower() == uuid.lower()
            and item.get("name") == executable.name
            for item in report.get("usedImages", [])
        )
        return (
            report.get("pid") == pid
            and report.get("procName") == executable.name
            and start <= captured <= end
            and image
        )
    except (OSError, ValueError, KeyError, TypeError):
        return False


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--framework",
        required=True,
        type=Path,
        help="Isolated -Dsentry=false native xcframework slice",
    )
    args = parser.parse_args()
    framework = args.framework.resolve()
    library = framework / "libghostty-internal.a"
    symbols = subprocess.check_output(["nm", "-gU", str(library)], stderr=subprocess.DEVNULL)
    if b"_sentry_init" in symbols or b"google_breakpad" in symbols:
        parser.error("framework still contains Sentry/Breakpad; refusing the diagnostic fault")
    stamp = dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%S.%fZ")
    output = (
        Path.home()
        / "Library/Logs/Loopflow/desktop-exit-minidumps-2026-11-02/reporter-disabled"
        / stamp
    )
    output.mkdir(parents=True)
    for source in (
        Path(__file__),
        Path(__file__).with_name("main.swift"),
        Path(__file__).with_name("Fault.c"),
        ROOT / "swift/LoopflowMac/Services/ExitSignals.swift",
    ):
        shutil.copyfile(source, output / source.name)
    subprocess.run(
        ["xcrun", "clang", "-c", str(output / "Fault.c"), "-o", str(output / "Fault.o")], check=True
    )
    executable = output / "LoopflowReporterDiagnostic"
    subprocess.run(
        [
            "xcrun",
            "swiftc",
            "-swift-version",
            "6",
            "-I",
            str(framework / "Headers"),
            str(output / "main.swift"),
            str(output / "ExitSignals.swift"),
            str(output / "Fault.o"),
            str(library),
            "-framework",
            "AppKit",
            "-framework",
            "QuartzCore",
            "-framework",
            "Metal",
            "-framework",
            "Carbon",
            "-framework",
            "IOKit",
            "-lc++",
            "-o",
            str(executable),
        ],
        check=True,
    )
    home = output / "home"
    home.mkdir()
    env = {
        key: value for key, value in os.environ.items() if not key.startswith(("LF_", "GHOSTTY_"))
    }
    env.update(
        HOME=str(home),
        CFFIXED_USER_HOME=str(home),
        XDG_CACHE_HOME=str(home / "cache"),
        XDG_CONFIG_HOME=str(home / "config"),
        XDG_STATE_HOME=str(home / "state"),
        GHOSTTY_RESOURCES_DIR=str(ROOT / "swift/LoopflowMac/GhosttyResources"),
    )
    config = output / "config"
    config.write_text("window-vsync = false\nshell-integration = none\n")
    journal = output / "launches.ndjson"
    command = [
        "/usr/bin/sandbox-exec",
        "-p",
        "(version 1)(allow default)(deny network*)",
        str(executable),
        str(journal),
        str(config),
    ]
    clean = _run(command, output / "clean", env)
    if clean.returncode or "terminal-clean-exit" not in (output / "clean.stdout").read_text():
        print(f"Terminal initialization failed; no intentional fault run. Evidence: {output}")
        return 1
    reports = Path.home() / "Library/Logs/DiagnosticReports"
    before = set(reports.glob("LoopflowReporterDiagnostic*.ips"))
    uuid = re.search(
        r"UUID: ([0-9A-Fa-f-]+)",
        subprocess.check_output(["xcrun", "dwarfdump", "--uuid", str(executable)], text=True),
    ).group(1)
    fault_start = dt.datetime.now(dt.UTC)
    fault = _run([*command, "--fault"], output / "fault", env)
    fault_end = dt.datetime.now(dt.UTC)
    signal_lines = (
        [json.loads(line) for line in journal.read_text().splitlines()] if journal.exists() else []
    )
    stdout = (output / "fault.stdout").read_text()
    ready = [line for line in stdout.splitlines() if line.startswith("terminal-ready pid=")]
    pid = int(ready[0].split("=")[1]) if len(ready) == 1 else None
    matched = []
    deadline = time.monotonic() + 30
    while pid and time.monotonic() < deadline:
        matched = [
            path
            for path in set(reports.glob("LoopflowReporterDiagnostic*.ips")) - before
            if _ips(path, pid, executable, uuid, fault_start, fault_end)
        ]
        if matched:
            break
        time.sleep(0.25)
    for report in matched:
        shutil.copyfile(report, output / report.name)
    passed = (
        fault.returncode in (-10, -11)
        and len(signal_lines) == 1
        and signal_lines[0].get("signal") in ("SIGBUS", "SIGSEGV")
        and signal_lines[0].get("pid") == str(pid)
        and len(matched) == 1
    )
    receipt = {
        "passed": passed,
        "clean_exit": clean.returncode,
        "fault_exit": fault.returncode,
        "signal_records": signal_lines,
        "ips": [str(path) for path in matched],
        "executable_sha256": _sha(executable),
        "library_sha256": _sha(library),
        "production_signal_source_sha256": _sha(output / "ExitSignals.swift"),
        "executable_uuid": uuid,
        "fault_start": fault_start.isoformat(),
        "fault_end": fault_end.isoformat(),
        "sentry_symbols": False,
        "network": "denied in both children",
        "os": subprocess.check_output(["sw_vers"], text=True),
        "swift": subprocess.check_output(["swift", "--version"], text=True),
    }
    (output / "result.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"Sentry-disabled diagnostic: {'pass' if passed else 'incomplete'}; evidence: {output}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
