"""Compare disposable visible-window WebKit lifetime trials; never launches Desktop."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

OFFSETS = (0, 10, 25, 50, 100, 200)
SCENARIOS = (
    "single",
    "two-close",
    "shell-replace",
    "pane-replace",
    "reparent",
    "workspace",
    "swiftui-close",
    "swiftui-reparent",
    "window-transfer",
    "occluded",
)
ANCHORS = ("load-request", "navigation-commit", "navigation-finish", "animation-frame")
VARIANTS = ("immediate", "detach-stop", "deferred", "blank")


def _events(output: str) -> list[dict]:
    events = []
    for line in output.splitlines():
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if isinstance(value, dict) and isinstance(value.get("event"), str):
            events.append(value)
    return events


def _result(returncode: int, events: list[dict], timed_out: bool = False) -> dict:
    complete = [event for event in events if event["event"] == "complete"]
    environment = next((event for event in events if event["event"] == "environment"), None)
    released = sum(event["event"] == "final-release" for event in events)
    if timed_out:
        outcome = "timeout"
    elif returncode < 0:
        outcome = "crash"
    elif any(event["event"] == "display-unavailable" for event in events):
        outcome = "unavailable"
    elif returncode != 0:
        outcome = "failed"
    elif (
        len(complete) != 1
        or not environment
        or not environment.get("display_session")
        or not any(event["event"] == "display-ready" for event in events)
        or not any(
            event["event"] == "presentation"
            and event.get("visible") is True
            and event.get("width", 0) > 0
            and event.get("height", 0) > 0
            for event in events
        )
        or complete[0].get("retained") != 0
        or complete[0].get("retiring") != 0
        or not complete[0].get("created")
        or complete[0]["created"] != released
        or any(
            event["event"] in ("incomplete", "navigation-failed", "web-content-terminated")
            for event in events
        )
        or any(event.get("same_page") is False for event in events)
    ):
        outcome = "incomplete"
    else:
        outcome = "pass"
    return {
        "outcome": outcome,
        "returncode": returncode,
        "final_releases": released,
        "retained": complete[0].get("retained") if len(complete) == 1 else None,
        "environment": environment,
    }


def _run(command: list[str], directory: Path, timeout: int) -> dict:
    directory.mkdir(parents=True)
    home = directory / "home"
    home.mkdir()
    env = {key: value for key, value in os.environ.items() if not key.startswith("LF_")}
    env.update(HOME=str(home), CFFIXED_USER_HOME=str(home), XDG_CACHE_HOME=str(home / "cache"))
    timed_out = False
    with (
        (directory / "events.ndjson").open("w") as output,
        (directory / "stderr.log").open("w") as errors,
    ):
        with subprocess.Popen(command, stdout=output, stderr=errors, env=env) as process:
            try:
                returncode = process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                process.kill()
                returncode = process.wait()
            except BaseException:
                process.kill()
                process.wait()
                raise
    return _result(returncode, _events((directory / "events.ndjson").read_text()), timed_out)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--compare", action="store_true", help="Run baseline and all experimental variants"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Compile and report environment without opening a window",
    )
    parser.add_argument("--variant", choices=VARIANTS, default="immediate")
    parser.add_argument("--scenario", choices=SCENARIOS, action="append")
    parser.add_argument("--offset-ms", type=int, choices=OFFSETS, action="append")
    parser.add_argument("--anchor", choices=ANCHORS, default="navigation-finish")
    parser.add_argument("--attempts", type=int, default=3)
    parser.add_argument(
        "--hold-ms",
        type=int,
        default=500,
        help="Experimental retention bound; not a production policy",
    )
    parser.add_argument(
        "--settle-ms",
        type=int,
        default=2000,
        help="Observation time both before and after final cleanup",
    )
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.attempts < 1 or args.hold_ms < 1 or args.settle_ms <= args.hold_ms:
        parser.error("attempts and hold must be positive; settle must exceed hold")
    if sys.platform != "darwin":
        parser.error("requires macOS AppKit/WebKit")
    stamp = dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%S.%fZ")
    output = (
        args.output
        or Path.home()
        / "Library/Logs/Loopflow/desktop-exit-minidumps-2026-11-02/browser-lifetime"
        / stamp
    ).resolve()
    if any((directory / ".git").exists() for directory in (output, *output.parents)):
        parser.error("diagnostic evidence must be outside Git checkouts")
    output.mkdir(parents=True, exist_ok=True)
    source = output / "Trial.swift"
    source.write_bytes(Path(__file__).with_name("Trial.swift").read_bytes())
    (output / "run.py").write_bytes(Path(__file__).read_bytes())
    executable = output / "BrowserLifetimeTrial"
    subprocess.run(
        ["xcrun", "swiftc", "-swift-version", "6", str(source), "-o", str(executable)], check=True
    )
    (output / "provenance.json").write_text(
        json.dumps(
            {
                "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                "executable_sha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
                "anchor": args.anchor,
                "hold_ms": args.hold_ms,
                "settle_ms": args.settle_ms,
                "swift": subprocess.check_output(
                    ["swift", "--version"], text=True, stderr=subprocess.STDOUT
                ),
            },
            indent=2,
        )
        + "\n"
    )
    check = subprocess.run([str(executable), "--check"], capture_output=True, text=True)
    (output / "environment.ndjson").write_text(check.stdout)
    (output / "environment-stderr.log").write_text(check.stderr)
    (output / "os-build.txt").write_text(subprocess.check_output(["sw_vers"], text=True))
    if args.check:
        print(check.stdout.strip())
        print(
            f"Environment saved to {output}; no window opened. "
            "Session availability does not prove visible rendering."
        )
        return check.returncode
    if check.returncode:
        print(f"Display session unavailable; no trials run. Environment: {output}", file=sys.stderr)
        return 2
    results = []
    variants = VARIANTS if args.compare else (args.variant,)
    for scenario in args.scenario or SCENARIOS:
        for offset in args.offset_ms or OFFSETS:
            for attempt in range(args.attempts):
                for variant in variants:
                    name = f"{args.anchor}-{scenario}-{offset}-{attempt}-{variant}"
                    result = _run(
                        [
                            str(executable),
                            variant,
                            scenario,
                            str(offset),
                            str(args.hold_ms),
                            str(args.settle_ms),
                            args.anchor,
                        ],
                        output / name,
                        timeout=30 + 2 * args.settle_ms // 1000,
                    )
                    result.update(
                        scenario=scenario,
                        anchor=args.anchor,
                        offset_ms=offset,
                        attempt=attempt,
                        variant=variant,
                    )
                    results.append(result)
                    (output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
                    print(
                        f"{name}: {result['outcome']}, releases={result['final_releases']}, "
                        f"retained={result['retained']}",
                        flush=True,
                    )
                    if result["outcome"] == "unavailable":
                        print(f"Visible display unavailable; sweep stopped. Evidence: {output}")
                        return 2
    for variant in variants:
        selected = [result for result in results if result["variant"] == variant]
        failures = sum(result["outcome"] != "pass" for result in selected)
        crashes = sum(result["outcome"] == "crash" for result in selected)
        print(f"{variant}: {len(selected)} attempts, {failures} failures, {crashes} native crashes")
    print(f"Evidence: {output}")
    print(
        "Experimental comparison only. No demonstrated baseline crash means "
        "the lifetime fix remains unresolved."
    )
    return int(any(result["outcome"] != "pass" for result in results))


if __name__ == "__main__":
    raise SystemExit(main())
