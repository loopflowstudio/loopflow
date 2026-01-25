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
    clone_agent,
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


@app.get("/flows", response_model=LFDResponse)
async def get_flows(repo: str = Query(..., description="Repository path")):
    """List available flows and steps for a repository."""
    from loopflow.lf.flows import FlowItem, Fork, Step, list_flows, list_steps

    repo_path = Path(repo)
    if not repo_path.exists():
        raise HTTPException(status_code=404, detail=f"Repository not found: {repo}")

    def step_names(items: list[FlowItem]) -> list[str]:
        """Extract step names from flow items."""
        names = []
        for item in items:
            if isinstance(item, Step):
                names.append(item.name)
            elif isinstance(item, Fork):
                names.append("(fork)")
            else:
                names.append("(choose)")
        return names

    try:
        flows = list_flows(repo_path)
        steps = list_steps(repo_path)

        return LFDResponse(
            ok=True,
            result={
                "flows": [
                    {
                        "name": f.name,
                        "type": "flow",
                        "steps": step_names(f.steps),
                    }
                    for f in flows
                ],
                "steps": [{"name": s, "type": "step"} for s in steps],
            },
        )
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


def _normalize_repo_path(repo: Path) -> Path:
    """Normalize repo path - resolve worktrees to main repo."""
    import subprocess

    result = subprocess.run(
        ["git", "rev-parse", "--git-common-dir"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return repo
    git_dir = Path(result.stdout.strip())
    if not git_dir.is_absolute():
        git_dir = (repo / git_dir).resolve()
    return git_dir.parent


@app.get("/agents", response_model=LFDResponse)
async def get_agents(repo: str = Query(..., description="Repository path")):
    """List agents for a repository."""
    repo_path = Path(repo)
    if not repo_path.exists():
        raise HTTPException(status_code=404, detail=f"Repository not found: {repo}")

    # Normalize to main repo (worktrees resolve to their main repo)
    repo_path = _normalize_repo_path(repo_path)

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
        "area": agent.area,
        "goal": agent.goal,
        "flow": agent.flow,
        "stimulus": {"kind": agent.stimulus.kind, "cron": agent.stimulus.cron},
        "paused": agent.paused,
        "repo": str(agent.repo),
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

    # Normalize to main repo (worktrees resolve to their main repo)
    repo_path = _normalize_repo_path(repo_path)

    try:
        agent = create_agent(
            repo=repo_path,
            name=request.name if request else None,
            flow=request.flow if request and request.flow else "design",
            goal=request.goal if request else None,
            area=request.area if request else None,
            stimulus=Stimulus(kind="once"),
        )

        # Notify subscribers of new agent
        await _notify_event("agent.created", {"agent_id": agent.id, "name": agent.name})

        return LFDResponse(ok=True, result={"agent": _agent_to_dict(agent)})
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


class StimulusUpdate(BaseModel):
    kind: str  # once, loop, watch, cron
    cron: str | None = None


class UpdateAgentRequest(BaseModel):
    area: list[str] | None = None
    goal: list[str] | None = None
    flow: str | None = None
    stimulus: StimulusUpdate | None = None
    paused: bool | None = None


@app.patch("/agents/{agent_id}", response_model=LFDResponse)
async def patch_agent(agent_id: str, request: UpdateAgentRequest):
    """Update an agent's configuration.

    Accepts any subset of fields: area, goal, flow, stimulus, paused.
    Stimulus is an object: {kind: "once"|"loop"|"watch"|"cron", cron?: string}
    """
    try:
        agent = get_agent(agent_id)
        if not agent:
            raise HTTPException(status_code=404, detail=f"Agent not found: {agent_id}")

        # Build stimulus if provided
        stimulus = None
        if request.stimulus:
            stimulus = Stimulus(kind=request.stimulus.kind, cron=request.stimulus.cron)

        updated = update_agent(
            agent_id,
            area=request.area,
            goal=request.goal,
            flow=request.flow,
            stimulus=stimulus,
            paused=request.paused,
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


class CloneAgentRequest(BaseModel):
    name: str | None = None  # optional name for clone


@app.post("/agents/{agent_id}/clone", response_model=LFDResponse)
async def clone_agent_endpoint(agent_id: str, request: CloneAgentRequest | None = None):
    """Clone an agent with a new name.

    Creates a copy with same config but fresh state (paused, no worktree).
    """
    try:
        agent = get_agent(agent_id)
        if not agent:
            raise HTTPException(status_code=404, detail=f"Agent not found: {agent_id}")

        name = request.name if request else None
        cloned = clone_agent(agent_id, name=name)

        if not cloned:
            return LFDResponse(ok=False, error="Failed to clone agent")

        # Notify subscribers
        await _notify_event("agent.created", {"agent_id": cloned.id, "name": cloned.name})

        return LFDResponse(ok=True, result={"agent": _agent_to_dict(cloned)})
    except HTTPException:
        raise
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


class RunAgentRequest(BaseModel):
    """Optional overrides for a single run (doesn't change agent config)."""

    area: list[str] | None = None
    goal: list[str] | None = None
    flow: str | None = None
    stimulus: StimulusUpdate | None = None


@app.post("/agents/{agent_id}/run", response_model=LFDResponse)
async def run_agent(agent_id: str, request: RunAgentRequest | None = None):
    """Run an agent.

    Optional body params are one-time overrides for this run only.
    Order: area, goal, flow, stimulus - any can be overridden.

    These overrides do NOT modify the agent's persistent configuration.
    """
    try:
        agent = get_agent(agent_id)
        if not agent:
            raise HTTPException(status_code=404, detail=f"Agent not found: {agent_id}")

        # Build overrides dict
        overrides = {}
        if request:
            if request.area is not None:
                overrides["area"] = request.area
            if request.goal is not None:
                overrides["goal"] = request.goal
            if request.flow is not None:
                overrides["flow"] = request.flow
            if request.stimulus is not None:
                overrides["stimulus"] = Stimulus(
                    kind=request.stimulus.kind, cron=request.stimulus.cron
                )

        # Check area (from agent or override)
        effective_area = overrides.get("area", agent.area)
        if effective_area is None:
            return LFDResponse(
                ok=False, error="No area configured. Set area first or pass as override."
            )

        # Start the agent with optional overrides
        result = start_agent(agent_id, **overrides)

        if result:
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
