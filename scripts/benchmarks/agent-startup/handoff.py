"""Alternate release binaries at the credential-free provider-start boundary."""

import argparse
import json
import os
import select
import sqlite3
import subprocess
import time
from pathlib import Path

from measure import _environment

STAND_IN = """#!/bin/sh
if [ "$1" = --version ]; then exit 0; fi
printf 'LF_BENCH_READY\\n'
read -r finish
"""


def measure(lf: Path, home: Path, repo: Path, output: Path, session: str | None,
            profile: bool = False) -> dict:
    output.mkdir(parents=True, exist_ok=False)
    shim = output / "bin"
    shim.mkdir()
    (shim / "lf").symlink_to(lf)
    for provider in ("claude", "codex"):
        stub = shim / provider
        stub.write_text(STAND_IN)
        stub.chmod(0o755)
    env = _environment(home, shim / "lf")
    env["PATH"] = str(shim) + os.pathsep + env["PATH"]
    env["LF_PERF_OUTPUT"] = str(output)
    env["GIT_TRACE2_EVENT"] = str(output / "git.private")
    command = [str(shim / "lf")]
    if session:
        command.extend(["session", "connect", session])
    start = time.monotonic()
    with (output / "stderr.private").open("wb") as error:
        child = subprocess.Popen(command, cwd=repo, env=env, stdin=subprocess.PIPE,
                                 stdout=subprocess.PIPE, stderr=error)
        sampler = None
        if profile:
            sampler = subprocess.Popen(["/usr/bin/sample", str(child.pid), "5", "1", "-mayDie",
                                        "-file", str(output / "sample.txt")],
                                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        data = bytearray()
        handoff = None
        try:
            while time.monotonic() - start < 30:
                if select.select([child.stdout], [], [], 0.05)[0]:
                    block = os.read(child.stdout.fileno(), 65536)
                    if not block:
                        break
                    data.extend(block)
                    if b"LF_BENCH_READY\n" in data:
                        handoff = (time.monotonic() - start) * 1000
                        break
            child.communicate(b"exit\n", timeout=5)
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()
            if sampler:
                sampler.wait(timeout=15)
        if handoff is None or child.returncode:
            raise RuntimeError(f"handoff failed, status {child.returncode}; inspect {output}")
    receipts = [json.loads(line) for path in output.glob("lf-*.jsonl")
                for line in path.read_text().splitlines()]
    end = [r for r in receipts if r["event"] == "end"]
    git = [json.loads(line) for line in (output / "git.private").read_text().splitlines()]
    return {"handoff_ms": handoff, "git_processes": sum(r.get("event") == "start" for r in git),
            "connections": sum(r["connections"] for r in end),
            "statements": sum(r["statements"] for r in end), "rows": sum(r["rows"] for r in end),
            "load1": os.getloadavg()[0]}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--home", type=Path, required=True)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=20)
    parser.add_argument("--profile", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    variants = {"baseline": args.baseline.resolve()}
    if args.candidate:
        variants["candidate"] = args.candidate.resolve()
    (args.home / "config.yaml").write_text("agent: claude\n")
    # Only a Session just created by this fixture may be connected.
    measure(args.baseline.resolve(), args.home, args.repo, args.output / "prepare", None)
    with sqlite3.connect(args.home / "loopflow.db") as db:
        session = db.execute("SELECT id FROM agent_sessions ORDER BY created_at DESC, rowid DESC LIMIT 1").fetchone()[0]
    for i in range(args.samples):
        for path in ("bare", "connect"):
            order = list(variants) if i % 2 == 0 else list(reversed(variants))
            for variant in order:
                result = measure(variants[variant], args.home, args.repo,
                                 args.output / f"{path}-{i}-{variant}",
                                 session if path == "connect" else None, args.profile)
                row = {"path": path, "variant": variant, "sample": i, **result}
                with (args.output / "numbers.jsonl").open("a") as numbers:
                    numbers.write(json.dumps(row) + "\n")
                print(json.dumps(row), flush=True)


if __name__ == "__main__":
    main()
