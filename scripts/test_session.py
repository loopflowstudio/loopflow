#!/usr/bin/env python3
"""Session API smoke test.

Builds lfd, starts it, exercises the session lifecycle (create, input, stream,
end), and reports success/failure. Self-contained — no external lfd needed.

Usage:
    uv run python scripts/test_session.py
    uv run python scripts/test_session.py --skip-build
"""

import argparse
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

from loopflow.client import Client
from loopflow.errors import LoopflowError
from loopflow.models import SessionEventEnvelope

REPO_ROOT = Path(__file__).parent.parent
LFD_BIN = REPO_ROOT / "target" / "debug" / "lfd"


def run(cmd: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, **kwargs)


def run_capture(cmd: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True, **kwargs)


def log(msg: str) -> None:
    print(msg, flush=True)


def fail(msg: str) -> None:
    log(f"\nFAIL: {msg}")
    sys.exit(1)


# ---------------------------------------------------------------------------
# lfd lifecycle
# ---------------------------------------------------------------------------


def build_lfd() -> None:
    log("Building lfd...")
    result = run(["cargo", "build", "--bin", "lfd"], cwd=REPO_ROOT)
    if result.returncode != 0:
        fail("cargo build failed")


def kill_existing_lfd() -> None:
    result = run_capture(["lsof", "-ti", ":2486"])
    if result.returncode == 0 and result.stdout.strip():
        for pid in result.stdout.strip().splitlines():
            try:
                os.kill(int(pid), signal.SIGTERM)
            except (ValueError, ProcessLookupError):
                pass
        time.sleep(1)


def start_lfd() -> subprocess.Popen:
    kill_existing_lfd()

    env = os.environ.copy()
    env["RUST_LOG"] = "loopflow=debug,tower_http=debug"
    env["GRPC_ENABLE_FORK_SUPPORT"] = "0"
    env["GRPC_VERBOSITY"] = "ERROR"

    proc = subprocess.Popen(
        [str(LFD_BIN), "serve"],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        env=env,
    )

    # Wait for lfd to be ready.
    # Token file is written at startup; re-read each iteration since the
    # file may not exist yet on the first few tries.
    for attempt in range(30):
        time.sleep(0.5)
        token = read_token()
        if not token:
            continue
        probe = Client(base_url="http://127.0.0.1:2486", timeout=2, token=token)
        try:
            probe.status()
            log(f"lfd ready (token: {token[:8]}...)")
            return proc
        except (ConnectionError, LoopflowError):
            if attempt < 5:
                # Token file may have been written by a dying old process;
                # wait for the new lfd to overwrite it.
                continue
        finally:
            probe.close()

    proc.terminate()
    stdout = proc.stdout.read() if proc.stdout else ""
    fail(f"lfd did not become ready.\nOutput:\n{stdout[:2000]}")
    raise SystemExit(1)


def read_token() -> str | None:
    token_path = Path.home() / ".lf" / "session-token"
    try:
        t = token_path.read_text().strip()
        return t or None
    except OSError:
        return None


def make_client() -> Client:
    return Client(base_url="http://127.0.0.1:2486", timeout=10)


# ---------------------------------------------------------------------------
# Session tests
# ---------------------------------------------------------------------------


def wait_for_session_status(
    client: Client,
    session_id: str,
    expected: str,
    timeout_s: float = 20,
) -> bool:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        session = client.session(session_id)
        if session is None:
            return False
        status = session.status
        if status == expected:
            return True
        if status in {"failed", "ended"} and expected not in {"failed", "ended"}:
            return False
        time.sleep(0.25)
    return False


def read_session_events(
    client: Client,
    session_id: str,
    *,
    after_seq: int | None = None,
    timeout_s: float = 30,
    max_events: int = 100,
    stop_on_turn_completed: bool = False,
) -> list[SessionEventEnvelope]:
    events: list[SessionEventEnvelope] = []
    try:
        for event in client.stream_session_events(
            session_id,
            after_seq=after_seq,
            timeout=timeout_s,
        ):
            events.append(event)
            etype = event.event.get("type")
            if stop_on_turn_completed and etype == "turn_completed":
                break
            if len(events) >= max_events:
                break
    except ConnectionError:
        pass
    except LoopflowError as err:
        log(f"    stream failed: {err}")
    return events


def end_session(client: Client, session_id: str) -> bool:
    try:
        client.stop_session(session_id)
    except LoopflowError as err:
        log(f"    FAIL: {err}")
        return False
    log("    ok")
    return True


def test_session_lifecycle(client: Client) -> bool:
    ok = True

    # Create session
    log("\n  create session...")
    try:
        session = client.create_session("claude")
    except LoopflowError as err:
        log(f"    FAIL: {err}")
        return False
    log(f"    session_id: {session.id}")
    log(f"    status: {session.status}")

    log("  wait for active...")
    if not wait_for_session_status(client, session.id, "active", timeout_s=20):
        log("    FAIL: session did not become active")
        end_session(client, session.id)
        return False
    log("    active")

    # Get session
    log("  get session...")
    current = client.session(session.id)
    if current is None:
        log("    FAIL: session not found")
        ok = False
    else:
        log(f"    status: {current.status}")

    # Send input
    log("  send input...")
    try:
        client.send_session_input(session.id, "Say hello in exactly one sentence.")
        log("    ok")
    except LoopflowError as err:
        log(f"    FAIL: {err}")
        ok = False

    # Stream events
    log("  stream events...")
    event_types: list[str] = []
    events = read_session_events(
        client,
        session.id,
        timeout_s=60,
        max_events=50,
        stop_on_turn_completed=True,
    )
    for envelope in events:
        etype = str(envelope.event.get("type", "?"))
        event_types.append(etype)
        preview = json.dumps(envelope.event)[:120]
        log(f"    seq={envelope.seq} {etype}: {preview}")

    if "turn_completed" not in event_types:
        log("    FAIL: never saw turn_completed")
        ok = False
    else:
        log(f"    got {len(event_types)} events, turn completed")

    # Check for item events
    item_events = [e for e in event_types if e.startswith("item_")]
    if item_events:
        log(f"    item events: {', '.join(set(item_events))}")
    else:
        log("    (no item events — provider may not emit them)")

    # End session
    log("  end session...")
    if not end_session(client, session.id):
        ok = False

    return ok


def test_reconnect_replay(client: Client) -> bool:
    """Create session, send input, wait for completion, then replay events."""
    log("\n  create session for replay test...")
    try:
        session = client.create_session("claude")
    except LoopflowError as err:
        log(f"    FAIL: {err}")
        return False

    log("    wait for active...")
    if not wait_for_session_status(client, session.id, "active", timeout_s=20):
        log("    FAIL: session did not become active")
        end_session(client, session.id)
        return False

    try:
        client.send_session_input(session.id, "Say exactly: pong")
    except LoopflowError as err:
        log(f"    FAIL send: {err}")
        return False

    # Consume all events
    max_seq = -1
    events = read_session_events(
        client,
        session.id,
        timeout_s=30,
        stop_on_turn_completed=True,
    )
    event_count = len(events)
    for envelope in events:
        if envelope.seq is not None and envelope.seq > max_seq:
            max_seq = envelope.seq
        if envelope.event.get("type") == "turn_completed":
            break

    log(f"    first read: {event_count} events, max_seq={max_seq}")
    if event_count == 0:
        log("    FAIL: got no events from first read")
        end_session(client, session.id)
        return False

    # Replay from start
    replay_count = len(
        read_session_events(
            client,
            session.id,
            timeout_s=5,
            stop_on_turn_completed=False,
        )
    )

    log(f"    replay from 0: {replay_count} events")

    if replay_count < event_count:
        log(f"    FAIL: replay ({replay_count}) < original ({event_count})")
        return False

    # Replay with after_seq cursor
    partial_count = 0
    midpoint = max_seq // 2 if max_seq > 0 else 0
    partial_count = len(
        read_session_events(
            client,
            session.id,
            after_seq=midpoint,
            timeout_s=5,
            stop_on_turn_completed=False,
        )
    )

    log(f"    replay after_seq={midpoint}: {partial_count} events")

    if partial_count >= replay_count:
        log("    FAIL: partial replay should return fewer events")
        return False

    # Clean up
    end_session(client, session.id)
    log("    ok")
    return True


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> int:
    parser = argparse.ArgumentParser(description="Session API smoke test")
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()

    if not args.skip_build:
        build_lfd()

    proc = start_lfd()
    client = make_client()
    results: dict[str, bool] = {}

    try:
        log("\n=== session lifecycle ===")
        results["lifecycle"] = test_session_lifecycle(client)

        log("\n=== reconnect replay ===")
        results["replay"] = test_reconnect_replay(client)
    finally:
        client.close()
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()

    log("\n=== results ===")
    all_pass = True
    for name, passed in results.items():
        status = "PASS" if passed else "FAIL"
        log(f"  {name}: {status}")
        if not passed:
            all_pass = False

    return 0 if all_pass else 1


if __name__ == "__main__":
    sys.exit(main())
