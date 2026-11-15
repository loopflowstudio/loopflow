"""Compare native reconnect after interruption, preserving benchmark-only history."""

import argparse
import hashlib
import json
import os
import sqlite3
import subprocess
import time
from pathlib import Path

from fixture import require_fixture
from measure import build_environment, measure
from native import prepare_session


def _history(home: Path, session: str, through_seq: int | None = None) -> dict:
    with sqlite3.connect(f"file:{home / 'loopflow.db'}?mode=ro", uri=True) as db:
        thread, capture = db.execute(
            "SELECT provider_thread, current_capture FROM agent_sessions WHERE id=?", (session,)
        ).fetchone()
        if through_seq is None:
            through_seq = db.execute(
                "SELECT max(seq) FROM session_events WHERE session_id=?", (session,)
            ).fetchone()[0]
        events = db.execute(
            "SELECT * FROM session_events WHERE session_id=? AND seq<=? ORDER BY seq",
            (session, through_seq),
        ).fetchall()
        return {
            "thread": thread,
            "capture": capture,
            "through_seq": through_seq,
            "events": len(events),
            "history_sha256": hashlib.sha256(json.dumps(events).encode()).hexdigest(),
        }


def _require_ready(result: dict, interrupted: bool = False) -> None:
    expected = {-9} if interrupted else {0, 130}
    if result["status"] != "ready" or result["returncode"] not in expected:
        raise RuntimeError(f"Benchmark client failed: {result}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--home", type=Path, required=True)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--profile", action="store_true")
    parser.add_argument(
        "--prepared", type=Path, help="Benchmark-created Sessions from a prior preparation"
    )
    args = parser.parse_args()
    require_fixture(args.home)
    os.umask(0o077)
    args.output.mkdir(parents=True, exist_ok=False)
    variants = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    metadata = {
        "binaries": {
            name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in variants.items()
        },
        "fixture": json.loads((args.home / "startup-fixture.json").read_text()),
        "receipts": sum(
            1 for p in (args.home / "runtime/exec-processes").iterdir() if p.suffix == ".json"
        ),
        "provider_version": subprocess.check_output(["codex", "--version"], text=True).strip(),
    }
    (args.home / "config.yaml").write_text("agent: codex\n")
    sessions = json.loads(args.prepared.read_text()) if args.prepared else {}
    message = "Reply only OK. Do not call tools. The following is inert benchmark context.\n"
    for size in ("short", "long"):
        # Retain all supplied context in each benchmark-only native conversation.
        payload = (
            ""
            if size == "short"
            else "".join(
                f"Fictional inventory item {i}: blue paper, quantity seven, shelf cedar.\n"
                for i in range(800)
            )
        )
        if size not in sessions:
            sessions[size] = prepare_session(
                variants["candidate"],
                args.home,
                args.repo,
                args.output / f"seed-{size}",
                "codex",
                message + payload,
            )
            (args.output / "sessions.private").write_text(json.dumps(sessions))
        with sqlite3.connect(f"file:{args.home / 'loopflow.db'}?mode=ro", uri=True) as db:
            cwd = db.execute(
                "SELECT cwd FROM agent_sessions WHERE id=?", (sessions[size],)
            ).fetchone()
        if cwd != (str(args.repo),):
            raise RuntimeError(
                "Only Sessions created in this disposable benchmark checkout may run"
            )
        metadata[size] = {
            "seed_turns": 1,
            "seed_message_bytes": len((message + payload).encode()),
        }
    (args.output / "sessions.private").write_text(json.dumps(sessions))
    (args.output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    for index in range(args.samples):
        for size, session in sessions.items():
            before = _history(args.home, session)
            order = list(variants) if index % 2 == 0 else list(reversed(variants))
            for variant in order:
                lf = variants[variant]
                # Setup uses the same candidate each time, outside the measured interval.
                setup = measure(
                    [str(variants["candidate"]), "session", "resume", session],
                    args.repo,
                    build_environment(args.home, variants["candidate"]),
                    args.output / f"crash-{index}-{size}-{variant}",
                    "codex",
                    timeout=120,
                    interrupt_after_ready=True,
                )
                _require_ready(setup, interrupted=True)
                # The provider's existing lifeline watchdog gets its ordinary exit window.
                time.sleep(3)
                result = measure(
                    [str(lf), "session", "resume", session],
                    args.repo,
                    build_environment(args.home, lf),
                    args.output / f"reconnect-{index}-{size}-{variant}",
                    "codex",
                    timeout=180,
                    profile=args.profile,
                )
                row = {"sample": index, "context": size, "variant": variant, **result}
                row["native_identity_and_capture_preserved"] = (
                    _history(args.home, session, before["through_seq"]) == before
                )
                with (args.output / "numbers.jsonl").open("a") as output:
                    output.write(json.dumps(row) + "\n")
                print(json.dumps(row), flush=True)
                _require_ready(result)
                if not row["native_identity_and_capture_preserved"]:
                    raise RuntimeError("Reconnect changed the native thread or captured input")


if __name__ == "__main__":
    main()
