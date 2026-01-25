"""FastAPI HTTP server for lfd daemon request-response calls.

Runs alongside the socket server. Provides REST endpoints
for clients that prefer HTTP (webapp, simpler Swift integration).
"""

import asyncio
import time
from pathlib import Path
from typing import Any

import uvicorn
from fastapi import FastAPI, HTTPException, Query
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel

from loopflow import __version__
from loopflow.lfd.agent import (
    create_agent,
    delete_agent,
    get_agent,
    list_agents,
    start_agent,
    stop_agent,
    update_agent,
)
from loopflow.lfd.daemon import metrics
from loopflow.lfd.daemon.client import _notify_event
from loopflow.lfd.daemon.status import compute_status
from loopflow.lfd.migrations.baseline import SCHEMA_VERSION
from loopflow.lfd.models import Stimulus
from loopflow.lfd.worktree_state import get_worktree_state_service

# Default port - matches webapp's expected default
DEFAULT_PORT = 8765

# Track server start time for uptime calculation
_start_time: float | None = None

app = FastAPI(title="lfd", description="Loopflow daemon API")

# Enable CORS for webapp
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],  # In production, restrict this
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.middleware("http")
async def count_requests(request, call_next):
    """Count HTTP requests for metrics."""
    metrics.increment("http_requests")
    return await call_next(request)


class LFDResponse(BaseModel):
    """Standard response format matching socket API."""

    ok: bool
    result: Any | None = None
    error: str | None = None
    version: str = __version__


@app.get("/worktrees", response_model=LFDResponse)
async def list_worktrees(repo: str = Query(..., description="Repository path")):
    """List worktrees with staleness and recent steps."""
    repo_path = Path(repo)
    if not repo_path.exists():
        raise HTTPException(status_code=404, detail=f"Repository not found: {repo}")

    try:
        service = get_worktree_state_service()
        worktrees = service.list_worktrees(repo_path)
        return LFDResponse(ok=True, result={"worktrees": worktrees})
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


@app.get("/status", response_model=LFDResponse)
async def get_status():
    """Basic health check and daemon status."""
    return LFDResponse(ok=True, result=compute_status())


@app.get("/health", response_model=LFDResponse)
async def get_health():
    """Detailed health check for diagnostics."""
    global _start_time
    uptime = time.time() - _start_time if _start_time else 0

    # Check database accessibility
    db_ok = True
    try:
        from loopflow.lfd.db import DB_PATH

        db_ok = DB_PATH.exists()
    except Exception:
        db_ok = False

    # Check socket exists
    socket_path = Path.home() / ".lf" / "lfd.sock"
    socket_ok = socket_path.exists()

    status = compute_status()
    return LFDResponse(
        ok=True,
        result={
            **status,
            "version": __version__,
            "schema_version": SCHEMA_VERSION,
            "uptime_seconds": int(uptime),
            "checks": {
                "database": "ok" if db_ok else "error",
                "socket": "ok" if socket_ok else "error",
            },
            "metrics": metrics.get_all(),
        },
    )


@app.get("/agents", response_model=LFDResponse)
async def get_agents(repo: str = Query(..., description="Repository path")):
    """List agents for a repository."""
    repo_path = Path(repo)
    if not repo_path.exists():
        raise HTTPException(status_code=404, detail=f"Repository not found: {repo}")

    try:
        agents = list_agents(repo=repo_path)
        return LFDResponse(
            ok=True,
            result={
                "agents": [
                    {
                        "id": a.id,
                        "name": a.name,
                        "flow": a.flow,
                        "goal": a.goal,
                        "area": a.area,
                        "repo": str(a.repo),
                        "stimulus": {"kind": a.stimulus.kind, "cron": a.stimulus.cron},
                        "status": a.status.value,
                        "iteration": a.iteration,
                        "worktree": str(a.worktree) if a.worktree else None,
                        "branch": a.branch,
                        "pr_limit": a.pr_limit,
                        "merge_mode": a.merge_mode.value,
                        "pid": a.pid,
                        "created_at": a.created_at.isoformat(),
                    }
                    for a in agents
                ]
            },
        )
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


class CreateAgentRequest(BaseModel):
    name: str | None = None
    flow: str | None = None
    goal: list[str] | None = None
    area: list[str] | None = None


def _agent_to_dict(agent) -> dict:
    """Convert agent to API response dict."""
    return {
        "id": agent.id,
        "name": agent.name,
        "flow": agent.flow,
        "goal": agent.goal,
        "area": agent.area,
        "repo": str(agent.repo),
        "stimulus": {"kind": agent.stimulus.kind, "cron": agent.stimulus.cron},
        "status": agent.status.value,
        "iteration": agent.iteration,
        "worktree": str(agent.worktree) if agent.worktree else None,
        "branch": agent.branch,
        "pr_limit": agent.pr_limit,
        "merge_mode": agent.merge_mode.value,
        "pid": agent.pid,
        "created_at": agent.created_at.isoformat(),
    }


@app.post("/agents", response_model=LFDResponse)
async def post_agent(
    repo: str = Query(..., description="Repository path"), request: CreateAgentRequest = None
):
    """Create a new agent.

    Accepts minimal body - even empty creates an agent with generated name.
    """
    repo_path = Path(repo)
    if not repo_path.exists():
        raise HTTPException(status_code=404, detail=f"Repository not found: {repo}")

    try:
        agent = create_agent(
            repo=repo_path,
            name=request.name if request else None,
            flow=request.flow if request and request.flow else "ship",
            goal=request.goal if request else None,
            area=request.area if request else None,
            stimulus=Stimulus(kind="once"),
        )

        # Notify subscribers of new agent
        await _notify_event("agent.created", {"agent_id": agent.id, "name": agent.name})

        return LFDResponse(ok=True, result={"agent": _agent_to_dict(agent)})
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


class UpdateAgentRequest(BaseModel):
    area: list[str] | None = None
    goal: list[str] | None = None
    flow: str | None = None


@app.patch("/agents/{agent_id}", response_model=LFDResponse)
async def patch_agent(agent_id: str, request: UpdateAgentRequest):
    """Update an agent's configuration.

    Accepts any subset of fields: area, goal, flow.
    """
    try:
        agent = get_agent(agent_id)
        if not agent:
            raise HTTPException(status_code=404, detail=f"Agent not found: {agent_id}")

        updated = update_agent(
            agent_id,
            area=request.area,
            goal=request.goal,
            flow=request.flow,
        )

        if not updated:
            return LFDResponse(ok=False, error="Failed to update agent")

        # Notify subscribers of agent update
        await _notify_event("agent.updated", {"agent_id": agent_id})

        return LFDResponse(ok=True, result={"agent": _agent_to_dict(updated)})
    except HTTPException:
        raise
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


@app.get("/agents/{agent_id}", response_model=LFDResponse)
async def get_agent_by_id(agent_id: str):
    """Get a single agent by ID."""
    try:
        agent = get_agent(agent_id)
        if not agent:
            raise HTTPException(status_code=404, detail=f"Agent not found: {agent_id}")

        return LFDResponse(ok=True, result={"agent": _agent_to_dict(agent)})
    except HTTPException:
        raise
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


@app.delete("/agents/{agent_id}", response_model=LFDResponse)
async def delete_agent_by_id(agent_id: str):
    """Delete an agent."""
    try:
        agent = get_agent(agent_id)
        if not agent:
            raise HTTPException(status_code=404, detail=f"Agent not found: {agent_id}")

        deleted = delete_agent(agent_id)
        if not deleted:
            return LFDResponse(ok=False, error="Failed to delete agent")

        # Notify subscribers
        await _notify_event("agent.deleted", {"agent_id": agent_id})

        return LFDResponse(ok=True, result={"deleted": True})
    except HTTPException:
        raise
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


class RunAgentRequest(BaseModel):
    stimulus: str = "once"  # once, loop, watch, cron
    cron: str | None = None
    path: str | None = None  # optional watch path override


@app.post("/agents/{agent_id}/run", response_model=LFDResponse)
async def run_agent(agent_id: str, request: RunAgentRequest = None):
    """Run an agent.

    Validates configuration before starting:
    - area must be set (required)
    - flow uses default if not set

    Stimulus can be: once, loop, watch, cron.
    """
    try:
        agent = get_agent(agent_id)
        if not agent:
            raise HTTPException(status_code=404, detail=f"Agent not found: {agent_id}")

        # Validate: area must be set
        if agent.area is None:
            return LFDResponse(ok=False, error="Agent has no area configured. Set area first.")

        # Update stimulus
        stim_kind = request.stimulus if request else "once"
        stim_cron = request.cron if request and stim_kind == "cron" else None
        update_agent(agent_id, stimulus=Stimulus(kind=stim_kind, cron=stim_cron))

        # Start the agent
        result = start_agent(agent_id)

        if result:
            # Notify subscribers
            await _notify_event("agent.started", {"agent_id": agent_id})
            return LFDResponse(ok=True, result={"started": True, "agent_id": agent_id})
        else:
            return LFDResponse(ok=False, error=f"Failed to start: {result.reason}")
    except HTTPException:
        raise
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


@app.post("/agents/{agent_id}/stop", response_model=LFDResponse)
async def stop_agent_by_id(agent_id: str):
    """Stop a running agent."""
    try:
        agent = get_agent(agent_id)
        if not agent:
            raise HTTPException(status_code=404, detail=f"Agent not found: {agent_id}")

        stopped = stop_agent(agent_id)
        if stopped:
            await _notify_event("agent.stopped", {"agent_id": agent_id})
            return LFDResponse(ok=True, result={"stopped": True})
        else:
            return LFDResponse(ok=False, error="Failed to stop agent")
    except HTTPException:
        raise
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


class UvicornServer:
    """Uvicorn server that can be started/stopped programmatically."""

    def __init__(self, host: str = "127.0.0.1", port: int = DEFAULT_PORT):
        # Note: uvicorn already sets SO_REUSEADDR by default
        self.config = uvicorn.Config(app, host=host, port=port, log_level="warning")
        self.server = uvicorn.Server(self.config)
        self._task: asyncio.Task | None = None

    async def start(self) -> None:
        """Start the server in a background task."""
        global _start_time
        _start_time = time.time()
        self._task = asyncio.create_task(self.server.serve())
        # Wait a bit for server to be ready
        await asyncio.sleep(0.1)

    async def stop(self) -> None:
        """Stop the server."""
        self.server.should_exit = True
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass


async def start_http_server(port: int = DEFAULT_PORT) -> UvicornServer:
    """Start the FastAPI server. Returns server for cleanup."""
    server = UvicornServer(port=port)
    await server.start()
    return server
