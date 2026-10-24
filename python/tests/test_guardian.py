"""The rented pod's guardian: it deletes the pod whatever else fails."""

import importlib.util
import json
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path

GUARDIAN = Path(__file__).parents[2] / "rust/loopflow/src/rental/guardian.py"
_spec = importlib.util.spec_from_file_location("guardian", GUARDIAN)
guardian = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(guardian)

START = datetime(2026, 10, 21, 20, 0, tzinfo=UTC)


class Clock:
    def __init__(self) -> None:
        self.wall = START
        self.mono = 0.0

    def sleep(self, seconds: float) -> None:
        self.wall += timedelta(seconds=seconds)
        self.mono += seconds


def _provider(price: str = "0.24", fail_deletes: int = 0, read_fails: bool = False):
    calls = []
    remaining = [fail_deletes]

    def send(method: str, url: str) -> tuple[int, bytes]:
        calls.append((method, url))
        if method == "GET":
            if read_fails:
                raise OSError("no reply")
            pod = {"cost": price, "createdAt": START.isoformat()}
            return 200, json.dumps(pod).encode()
        if remaining[0] > 0:
            remaining[0] -= 1
            return 503, b""
        return 200, b""

    return send, calls


def _run(send, clock: Clock, deadline: datetime, max_usd: str) -> datetime:
    guardian.guard(
        send,
        "pod1",
        deadline,
        Decimal(max_usd),
        clock.sleep,
        lambda: clock.wall,
        lambda: clock.mono,
    )
    return clock.wall


def test_deletes_at_the_deadline() -> None:
    send, calls = _provider()
    ended = _run(send, Clock(), START + timedelta(hours=2), "10")
    assert ended == START + timedelta(hours=2)
    assert calls[-1] == ("DELETE", guardian.BASE + "pods/pod1")


def test_budget_shortens_the_deadline() -> None:
    send, _ = _provider(price="0.50")
    ended = _run(send, Clock(), START + timedelta(hours=4), "0.50")
    assert ended == START + timedelta(hours=1)


def test_unreadable_budget_keeps_the_given_deadline() -> None:
    send, _ = _provider(read_fails=True)
    ended = _run(send, Clock(), START + timedelta(minutes=30), "0.01")
    assert ended == START + timedelta(minutes=30)


def test_a_clock_set_backwards_buys_no_time() -> None:
    send, calls = _provider()
    clock = Clock()

    def sleep(seconds: float) -> None:
        clock.mono += seconds
        clock.wall -= timedelta(seconds=seconds)

    guardian.guard(
        send,
        "pod1",
        START + timedelta(minutes=10),
        Decimal("10"),
        sleep,
        lambda: clock.wall,
        lambda: clock.mono,
    )
    assert clock.mono == 600
    assert calls[-1][0] == "DELETE"


def test_delete_retries_until_it_succeeds() -> None:
    send, calls = _provider(fail_deletes=3)
    _run(send, Clock(), START, "10")
    assert [method for method, _ in calls].count("DELETE") == 4
