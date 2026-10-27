"""Hold a rented machine's deadline when no lf is alive.

Standard library only; it starts before anything else on the machine. It runs
the machine's command as its child and returns at the deadline, or sooner when
the child exits. What returning means is the provider's:

- Exit: the machine ends when its main process does (AWS's
  terminate-on-shutdown).
- Delete: on RunPod it then deletes its own pod with the pod-scoped key RunPod
  injects as RUNPOD_API_KEY, which can only touch this pod, and first reads the
  price actually charged so the budget can shorten the deadline.

The deadline it was given always stands; a budget only shortens it. A clock
set backwards buys no time.

    LF_DEADLINE=<iso> LF_CREATED=<iso> LF_MAX_USD=<usd> python3 guardian.py [-- command ...]
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from typing import Any, Callable

RUNPOD = "https://rest.runpod.io/v1/"


class Unavailable(Exception):
    """No usable answer. This never means the pod is absent."""


Send = Callable[[str, str], tuple[int, bytes]]


def _send(key: str) -> Send:
    def send(method: str, url: str) -> tuple[int, bytes]:
        request = urllib.request.Request(
            url,
            method=method,
            # RunPod's front door refuses default agent names.
            headers={"Authorization": f"Bearer {key}", "User-Agent": "loopflow-guardian"},
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as reply:  # noqa: S310
                return reply.status, reply.read(1024 * 1024)
        except urllib.error.HTTPError as error:
            return error.code, b""

    return send


def _ask(send: Send, method: str, path: str) -> Any:
    try:
        status, raw = send(method, RUNPOD + path)
    except OSError:
        raise Unavailable(f"{method} {path}: no reply") from None
    if status == 404:
        return None
    if not 200 <= status < 300:
        raise Unavailable(f"{method} {path}: status {status}")
    try:
        return json.loads(raw, parse_float=Decimal) if raw else None
    except ValueError:
        raise Unavailable(f"{method} {path}: unreadable reply") from None


def budget_deadline(send: Send, pod: str, created: datetime, max_usd: Decimal) -> datetime | None:
    """When the budget runs out at the price RunPod actually charges."""
    raw = _ask(send, "GET", f"pods/{pod}")
    if raw is None:
        return None
    price = Decimal(str(raw["costPerHr"]))
    if price <= 0:
        return None
    return created + timedelta(hours=float(max_usd / price))


def guard(
    deadline: datetime,
    done: Callable[[], bool] = lambda: False,
    sleep: Callable[[float], None] = time.sleep,
    now: Callable[[], datetime] = lambda: datetime.now(UTC),
    monotonic: Callable[[], float] = time.monotonic,
) -> None:
    """Return at the deadline, or sooner once `done`."""
    latest = monotonic() + max(0.0, (deadline - now()).total_seconds())
    while now() < deadline and monotonic() < latest and not done():
        sleep(min(30.0, max(0.0, latest - monotonic())))


def delete_pod(send: Send, pod: str, sleep: Callable[[float], None] = time.sleep) -> None:
    """Delete this pod, for as long as that takes."""
    while True:
        try:
            _ask(send, "DELETE", f"pods/{pod}")
            return
        except Unavailable:
            sleep(2.0)


def _exited(child: subprocess.Popen) -> bool:
    """Whether the child exited. As a container's PID 1 the guardian inherits
    every orphan on the machine, so it reaps them all."""
    while True:
        try:
            pid, _ = os.waitpid(-1, os.WNOHANG)
        except ChildProcessError:
            return True
        if pid == 0:
            return False
        if pid == child.pid:
            return True


def _at(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def main() -> None:
    deadline = _at(os.environ["LF_DEADLINE"])
    pod = os.environ.get("RUNPOD_POD_ID")
    # The child never sees the pod's key.
    key = os.environ.pop("RUNPOD_API_KEY", None)
    command = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    child = subprocess.Popen(command) if command else None
    send = _send(key) if pod and key else None
    if send is not None:
        try:
            created = _at(os.environ["LF_CREATED"])
            paid_until = budget_deadline(send, pod, created, Decimal(os.environ["LF_MAX_USD"]))
            if paid_until is not None:
                deadline = min(deadline, paid_until)
        except (Unavailable, KeyError, TypeError, ValueError, ArithmeticError):
            pass  # the deadline it was given still stands
    print(f"guardian: ends by {deadline.isoformat()}", flush=True)
    guard(deadline, done=lambda: child is not None and _exited(child))
    if child is not None and child.poll() is None:
        child.terminate()
        try:
            child.wait(10)
        except subprocess.TimeoutExpired:
            child.kill()
    if send is not None:
        delete_pod(send, pod)
        print("guardian: deleted", flush=True)


if __name__ == "__main__":
    main()
