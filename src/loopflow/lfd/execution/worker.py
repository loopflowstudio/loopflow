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
    update_agent_iteration,
    update_agent_pid,
    update_agent_status,
)
from loopflow.lfd.daemon.client import notify_event
from loopflow.lfd.execution.runner import run_iteration
from loopflow.lfd.models import Agent, AgentStatus

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"
MANAGER_POLL_INTERVAL = 30  # seconds between slot checks


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
    while True:
        outstanding = count_outstanding(agent)
        if outstanding >= agent.pr_limit:
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

        # Wait for manager slot (global concurrency)
        while True:
            acquired, reason = _manager_acquire(run_id)
            if acquired:
                break
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
            success = run_iteration(agent, iteration, run_id)
            if success:
                update_agent_iteration(agent.id, iteration)
                agent.iteration = iteration
            else:
                update_agent_status(agent.id, AgentStatus.ERROR)
                break
        except Exception as e:
            notify_event(
                "agent.error",
                {
                    "agent_id": agent.id,
                    "area": agent.area_display,
                    "error": str(e),
                },
            )
            update_agent_status(agent.id, AgentStatus.ERROR)
            break
        finally:
            _manager_release(run_id)

    update_agent_pid(agent.id, None)


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
