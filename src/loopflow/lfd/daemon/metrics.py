"""Simple in-memory metrics for lfd daemon."""

# Counters - reset on daemon restart
_counters: dict[str, int] = {
    "events_broadcast": 0,
    "socket_requests": 0,
    "http_requests": 0,
}


def increment(name: str, amount: int = 1) -> None:
    """Increment a counter."""
    if name in _counters:
        _counters[name] += amount


def get_all() -> dict[str, int]:
    """Get all counter values."""
    return _counters.copy()
