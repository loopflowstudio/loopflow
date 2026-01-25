"""FastAPI HTTP server for lfd daemon request-response calls.

Runs alongside the socket server. Provides REST endpoints
for clients that prefer HTTP (webapp, simpler Swift integration).
"""

import asyncio
from pathlib import Path
from typing import Any

import uvicorn
from fastapi import FastAPI, HTTPException, Query
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel

from loopflow.lfd.agent import create_agent, list_agents
from loopflow.lfd.daemon.client import _notify_event
from loopflow.lfd.daemon.status import compute_status
from loopflow.lfd.models import Stimulus
from loopflow.lfd.worktree_state import get_worktree_state_service

# Default port - matches webapp's expected default
DEFAULT_PORT = 8765

app = FastAPI(title="lfd", description="Loopflow daemon API")

# Enable CORS for webapp
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],  # In production, restrict this
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


class LFDResponse(BaseModel):
    """Standard response format matching socket API."""

    ok: bool
    result: Any | None = None
    error: str | None = None


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
    flow: str = "ship"
    goal: list[str] = ["default"]
    area: list[str] = ["."]


@app.post("/agents", response_model=LFDResponse)
async def post_agent(
    repo: str = Query(..., description="Repository path"), request: CreateAgentRequest = None
):
    """Create a new agent."""
    repo_path = Path(repo)
    if not repo_path.exists():
        raise HTTPException(status_code=404, detail=f"Repository not found: {repo}")

    try:
        agent = create_agent(
            repo=repo_path,
            flow=request.flow if request else "ship",
            goal=request.goal if request else ["default"],
            area=request.area if request else ["."],
            name=request.name if request else None,
            stimulus=Stimulus(kind="once"),
        )

        # Notify subscribers of new agent
        await _notify_event("agent.created", {"agent_id": agent.id, "name": agent.name})

        return LFDResponse(
            ok=True,
            result={
                "agent": {
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
                    "created_at": agent.created_at.isoformat(),
                }
            },
        )
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


class UvicornServer:
    """Uvicorn server that can be started/stopped programmatically."""

    def __init__(self, host: str = "127.0.0.1", port: int = DEFAULT_PORT):
        self.config = uvicorn.Config(app, host=host, port=port, log_level="warning")
        self.server = uvicorn.Server(self.config)
        self._task: asyncio.Task | None = None

    async def start(self) -> None:
        """Start the server in a background task."""
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
