"""Worker for continuous execution.

Runs iterations of an agent until stopped or paused.
Coordinates with the daemon manager for global concurrency limits.
"""

import json
import socket
import sys
import time
import uuid
from pathlib import Path

from loopflow.lfd.agent import (
    count_outstanding,
    get_agent,
    update_agent_consecutive_failures,
    update_agent_iteration,
    update_agent_pid,
    update_agent_status,
)
from loopflow.lfd.daemon.client import notify_event
from loopflow.lfd.execution.runner import run_iteration
from loopflow.lfd.logging import worker_log
from loopflow.lfd.models import Agent, AgentStatus

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"
MANAGER_POLL_INTERVAL = 30  # seconds between slot checks

# Retry and circuit breaker config
MAX_RETRIES = 3
RETRY_BACKOFF_SECONDS = 30
CIRCUIT_BREAKER_THRESHOLD = 5


def _emit_circuit_breaker(agent: Agent, failures: int) -> None:
    """Emit circuit breaker event and log error."""
    worker_log.error(f"[{agent.short_id()}] circuit breaker: {failures} consecutive failures")
    notify_event(
        "agent.circuit_breaker",
        {
            "agent_id": agent.id,
            "area": agent.area_display,
            "failures": failures,
            "threshold": CIRCUIT_BREAKER_THRESHOLD,
        },
    )


def _manager_call(method: str, params: dict | None = None) -> dict | None:
    """Make a synchronous call to the daemon manager.

    Returns the result dict on success, None on connection failure.
    """
    if not SOCKET_PATH.exists():
        return None

    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(5.0)
        sock.connect(str(SOCKET_PATH))

        request = {"method": method}
        if params:
            request["params"] = params

        sock.sendall((json.dumps(request) + "\n").encode())

        response_data = b""
        while b"\n" not in response_data:
            chunk = sock.recv(1024)
            if not chunk:
                break
            response_data += chunk

        sock.close()

        if response_data:
            response = json.loads(response_data.decode().strip())
            if response.get("ok"):
                return response.get("result", {})
        return None
    except Exception:
        return None


def _manager_acquire(run_id: str) -> tuple[bool, str | None]:
    """Try to acquire a manager slot.

    Returns (acquired, reason) when the daemon is available.
    """
    result = _manager_call("scheduler.acquire", {"run_id": run_id})
    if result is None:
        # Daemon not running, allow iteration (standalone mode)
        return True, None
    return result.get("acquired", False), result.get("reason")


def _manager_release(run_id: str) -> None:
    """Release a manager slot."""
    _manager_call("scheduler.release", {"run_id": run_id})


def run_agent_iterations(agent: Agent) -> None:
    """Run agent iterations until PR limit is reached or error occurs."""
    short_id = agent.short_id()
    consecutive_failures = agent.consecutive_failures

    worker_log.info(
        f"[{short_id}] starting: mode={agent.mode} flow={agent.flow} "
        f"area={agent.area_display} iteration={agent.iteration}"
    )

    # Check circuit breaker on startup
    if consecutive_failures >= CIRCUIT_BREAKER_THRESHOLD:
        _emit_circuit_breaker(agent, consecutive_failures)
        update_agent_status(agent.id, AgentStatus.ERROR)
        update_agent_pid(agent.id, None)
        return

    while True:
        outstanding = count_outstanding(agent)
        if outstanding >= agent.pr_limit:
            worker_log.info(f"[{short_id}] waiting: {outstanding}/{agent.pr_limit} PRs outstanding")
            update_agent_status(agent.id, AgentStatus.WAITING)
            notify_event(
                "agent.waiting",
                {
                    "agent_id": agent.id,
                    "area": agent.area_display,
                    "outstanding": outstanding,
                    "limit": agent.pr_limit,
                },
            )
            break

        iteration = agent.iteration + 1
        run_id = str(uuid.uuid4())

        worker_log.info(f"[{short_id}] starting iteration {iteration}")

        # Wait for manager slot (global concurrency)
        while True:
            acquired, reason = _manager_acquire(run_id)
            if acquired:
                break
            worker_log.debug(f"[{short_id}] waiting for slot: {reason}")
            notify_event(
                "scheduler.waiting",
                {
                    "agent_id": agent.id,
                    "area": agent.area_display,
                    "reason": reason or "concurrency",
                },
            )
            time.sleep(MANAGER_POLL_INTERVAL)

        try:
            success = _run_with_retry(agent, iteration, run_id)
            if success:
                worker_log.info(f"[{short_id}] iteration {iteration} completed successfully")
                # Reset failures on success
                if consecutive_failures > 0:
                    worker_log.info(f"[{short_id}] resetting failures from {consecutive_failures}")
                    consecutive_failures = 0
                    update_agent_consecutive_failures(agent.id, 0)

                update_agent_iteration(agent.id, iteration)
                agent.iteration = iteration
            else:
                # Increment failures
                consecutive_failures += 1
                worker_log.warning(
                    f"[{short_id}] iteration {iteration} failed "
                    f"(consecutive_failures={consecutive_failures})"
                )
                update_agent_consecutive_failures(agent.id, consecutive_failures)

                if consecutive_failures >= CIRCUIT_BREAKER_THRESHOLD:
                    _emit_circuit_breaker(agent, consecutive_failures)

                update_agent_status(agent.id, AgentStatus.ERROR)
                break
        except Exception as e:
            consecutive_failures += 1
            update_agent_consecutive_failures(agent.id, consecutive_failures)

            worker_log.error(
                f"[{short_id}] iteration {iteration} raised exception: {e}",
                exc_info=True,
            )

            notify_event(
                "agent.error",
                {
                    "agent_id": agent.id,
                    "area": agent.area_display,
                    "error": str(e),
                    "consecutive_failures": consecutive_failures,
                },
            )

            if consecutive_failures >= CIRCUIT_BREAKER_THRESHOLD:
                _emit_circuit_breaker(agent, consecutive_failures)

            update_agent_status(agent.id, AgentStatus.ERROR)
            break
        finally:
            _manager_release(run_id)

    worker_log.info(f"[{short_id}] stopped: status={agent.status.value}")
    update_agent_pid(agent.id, None)


def _run_with_retry(agent: Agent, iteration: int, run_id: str) -> bool:
    """Run iteration with retry and backoff."""
    short_id = agent.short_id()
    last_error = None

    for attempt in range(MAX_RETRIES):
        try:
            worker_log.debug(f"[{short_id}] attempt {attempt + 1}/{MAX_RETRIES}")
            success = run_iteration(agent, iteration, run_id)
            if success:
                return True

            # run_iteration returned False (failure without exception)
            if attempt < MAX_RETRIES - 1:
                worker_log.warning(
                    f"[{short_id}] attempt {attempt + 1} failed, "
                    f"retrying in {RETRY_BACKOFF_SECONDS}s"
                )
                notify_event(
                    "agent.retry",
                    {
                        "agent_id": agent.id,
                        "area": agent.area_display,
                        "attempt": attempt + 1,
                        "max_retries": MAX_RETRIES,
                        "backoff": RETRY_BACKOFF_SECONDS,
                    },
                )
                time.sleep(RETRY_BACKOFF_SECONDS)
            else:
                worker_log.error(f"[{short_id}] all {MAX_RETRIES} attempts failed")
                return False

        except Exception as e:
            last_error = e
            if attempt < MAX_RETRIES - 1:
                worker_log.warning(
                    f"[{short_id}] attempt {attempt + 1} raised {type(e).__name__}: {e}, "
                    f"retrying in {RETRY_BACKOFF_SECONDS}s"
                )
                notify_event(
                    "agent.retry",
                    {
                        "agent_id": agent.id,
                        "area": agent.area_display,
                        "attempt": attempt + 1,
                        "max_retries": MAX_RETRIES,
                        "backoff": RETRY_BACKOFF_SECONDS,
                        "error": str(e),
                    },
                )
                time.sleep(RETRY_BACKOFF_SECONDS)
            else:
                worker_log.error(
                    f"[{short_id}] all {MAX_RETRIES} attempts exhausted, last error: {e}"
                )

    # All retries exhausted
    if last_error:
        raise last_error
    return False


def main() -> None:
    """Entry point for background worker."""
    if len(sys.argv) < 3:
        print("Usage: python -m loopflow.lfd.execution.worker agent <agent_id>", file=sys.stderr)
        sys.exit(1)

    cmd = sys.argv[1]
    agent_id = sys.argv[2]

    if cmd != "agent":
        print(f"Unknown command: {cmd}", file=sys.stderr)
        sys.exit(1)

    agent = get_agent(agent_id)
    if not agent:
        print(f"Agent not found: {agent_id}", file=sys.stderr)
        sys.exit(1)

    run_agent_iterations(agent)


if __name__ == "__main__":
    main()
