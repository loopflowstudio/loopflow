"""Delete this RunPod pod at its deadline, or sooner when its budget runs out.

Standard library only; runs first inside the pod and depends on nothing else
that starts there. It uses the pod-scoped key RunPod injects as RUNPOD_API_KEY,
which can only touch this pod. The deadline it was given always stands; a
budget only shortens it. A clock set backwards buys no time.
"""

from __future__ import annotations

import json
import os
import time
import urllib.error
import urllib.request
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path
from typing import Any, Callable

BASE = "https://api.runpod.io/v2/"
READY = Path("/run/lf/guardian-ready")


class Unavailable(Exception):
    """No usable answer. This never means the pod is absent."""


Send = Callable[[str, str], tuple[int, bytes]]


def _send(key: str) -> Send:
    def send(method: str, url: str) -> tuple[int, bytes]:
        request = urllib.request.Request(
            url,
            method=method,
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
        status, raw = send(method, BASE + path)
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


def budget_deadline(send: Send, pod: str, max_usd: Decimal) -> datetime | None:
    """When the budget runs out at the price actually charged."""
    raw = _ask(send, "GET", f"pods/{pod}")
    if raw is None:
        return None
    price = Decimal(str(raw["cost"]))
    if price <= 0:
        return None
    created = datetime.fromisoformat(str(raw["createdAt"]).replace("Z", "+00:00"))
    return created + timedelta(hours=float(max_usd / price))


def guard(
    send: Send,
    pod: str,
    deadline: datetime,
    max_usd: Decimal,
    sleep: Callable[[float], None] = time.sleep,
    now: Callable[[], datetime] = lambda: datetime.now(UTC),
    monotonic: Callable[[], float] = time.monotonic,
) -> None:
    """Wait for the deadline, then delete this pod, for as long as that takes."""
    latest = monotonic() + max(0.0, (deadline - now()).total_seconds())
    try:
        paid_until = budget_deadline(send, pod, max_usd)
        if paid_until is not None:
            deadline = min(deadline, paid_until)
    except (Unavailable, KeyError, TypeError, ValueError, ArithmeticError):
        pass  # the deadline it was given still stands
    latest = min(latest, monotonic() + max(0.0, (deadline - now()).total_seconds()))
    while now() < deadline and monotonic() < latest:
        sleep(min(30.0, max(0.0, latest - monotonic())))
    while True:
        try:
            _ask(send, "DELETE", f"pods/{pod}")
            return
        except Unavailable:
            sleep(2.0)


def main() -> None:
    READY.parent.mkdir(parents=True, exist_ok=True)
    pod = os.environ["RUNPOD_POD_ID"]
    deadline = datetime.fromisoformat(os.environ["LF_DEADLINE"].replace("Z", "+00:00"))
    max_usd = Decimal(os.environ["LF_MAX_USD"])
    send = _send(os.environ["RUNPOD_API_KEY"])
    READY.touch()
    print(f"guardian: pod {pod} ends by {deadline.isoformat()} or ${max_usd}", flush=True)
    guard(send, pod, deadline, max_usd)
    print("guardian: deleted", flush=True)


if __name__ == "__main__":
    main()
