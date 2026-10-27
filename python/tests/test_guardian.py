"""The rented machine's guardian: it ends the machine whatever else fails."""

import importlib.util
import json
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path

import pytest

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


def _runpod(price: str = "0.24", fail_deletes: int = 0, read_fails: bool = False):
    calls = []
    remaining = [fail_deletes]

    def send(method: str, url: str) -> tuple[int, bytes]:
        calls.append((method, url))
        if method == "GET":
            if read_fails:
                raise OSError("no reply")
            return 200, json.dumps({"id": "pod1", "costPerHr": price}).encode()
        if remaining[0] > 0:
            remaining[0] -= 1
            return 503, b""
        return 200, b""

    return send, calls


def _guard(clock: Clock, deadline: datetime, done=lambda: False) -> datetime:
    guardian.guard(deadline, done, clock.sleep, lambda: clock.wall, lambda: clock.mono)
    return clock.wall


def test_returns_at_the_deadline() -> None:
    assert _guard(Clock(), START + timedelta(hours=2)) == START + timedelta(hours=2)


def test_returns_when_the_machine_command_exits() -> None:
    clock = Clock()
    ended = _guard(clock, START + timedelta(hours=2), done=lambda: clock.mono >= 60)
    assert ended < START + timedelta(minutes=2)


def test_the_charged_price_shortens_the_deadline() -> None:
    send, _ = _runpod(price="0.50")
    paid_until = guardian.budget_deadline(send, "pod1", START, Decimal("0.50"))
    assert paid_until == START + timedelta(hours=1)


def test_an_unreadable_price_is_unavailable_not_absent() -> None:
    send, _ = _runpod(read_fails=True)
    with pytest.raises(guardian.Unavailable):
        guardian.budget_deadline(send, "pod1", START, Decimal("0.01"))


def test_a_clock_set_backwards_buys_no_time() -> None:
    clock = Clock()

    def sleep(seconds: float) -> None:
        clock.mono += seconds
        clock.wall -= timedelta(seconds=seconds)

    guardian.guard(
        START + timedelta(minutes=10), lambda: False, sleep, lambda: clock.wall, lambda: clock.mono
    )
    assert clock.mono == 600


def test_delete_retries_until_it_succeeds() -> None:
    send, calls = _runpod(fail_deletes=3)
    guardian.delete_pod(send, "pod1", sleep=lambda _: None)
    assert calls == [("DELETE", guardian.RUNPOD + "pods/pod1")] * 4
