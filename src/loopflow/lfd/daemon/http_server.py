"""FastAPI HTTP server for lfd daemon request-response calls.

Runs alongside the socket server. Provides REST endpoints
for clients that prefer HTTP (webapp, simpler Swift integration).
"""

import asyncio
import os
from pathlib import Path
from typing import Any

import uvicorn
from fastapi import FastAPI, HTTPException, Query
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel

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
    from loopflow.lfd.agent import list_agents
    from loopflow.lfd.models import AgentStatus
    from loopflow.lfd.step_run import load_step_runs

    agents = list_agents()
    step_runs = load_step_runs(active_only=True)
    running_agents = [a for a in agents if a.status == AgentStatus.RUNNING]

    return LFDResponse(
        ok=True,
        result={
            "pid": os.getpid(),
            "agents_defined": len(agents),
            "agents_running": len(running_agents),
            "step_runs_active": len(step_runs),
        },
    )


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
