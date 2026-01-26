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
from loopflow.lfd.daemon import metrics
from loopflow.lfd.daemon.client import _notify_event
from loopflow.lfd.daemon.status import compute_status
from loopflow.lfd.migrations.baseline import SCHEMA_VERSION
from loopflow.lfd.models import Stimulus
from loopflow.lfd.wave import (
    clone_wave,
    create_wave,
    delete_wave,
    get_wave,
    list_waves,
    start_wave,
    stop_wave,
    update_wave,
)
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


@app.get("/waves", response_model=LFDResponse)
async def get_waves(repo: str = Query(..., description="Repository path")):
    """List waves for a repository, enriched with worktree state."""
    repo_path = Path(repo)
    if not repo_path.exists():
        raise HTTPException(status_code=404, detail=f"Repository not found: {repo}")

    # Normalize to main repo (worktrees resolve to their main repo)
    repo_path = _normalize_repo_path(repo_path)

    try:
        waves = list_waves(repo=repo_path)

        # Get worktree state service for enrichment
        wt_service = get_worktree_state_service()

        enriched = []
        for wave in waves:
            # Look up worktree state if wave has a branch
            wt_state = None
            if wave.branch:
                wt_state = wt_service.get_one(repo_path, wave.branch)
            enriched.append(_wave_to_dict(wave, wt_state))

        return LFDResponse(ok=True, result={"waves": enriched})
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


class CreateWaveRequest(BaseModel):
    name: str | None = None
    flow: str | None = None
    direction: list[str] | None = None
    area: list[str] | None = None


def _wave_to_dict(wave, worktree_state: dict | None = None) -> dict:
    """Convert wave to API response dict, enriched with worktree state."""
    result = {
        "id": wave.id,
        "name": wave.name,
        "area": wave.area,
        "direction": wave.direction,
        "flow": wave.flow,
        "stimulus": {"kind": wave.stimulus.kind, "cron": wave.stimulus.cron},
        "paused": wave.paused,
        "repo": str(wave.repo),
        "status": wave.status.value,
        "iteration": wave.iteration,
        "worktree": str(wave.worktree) if wave.worktree else None,
        "branch": wave.branch,
        "pr_limit": wave.pr_limit,
        "merge_mode": wave.merge_mode.value,
        "pid": wave.pid,
        "created_at": wave.created_at.isoformat(),
    }

    # Enrich with worktree state if available
    if worktree_state:
        wt = worktree_state.get("working_tree", {})
        main = worktree_state.get("main", {})
        remote = worktree_state.get("remote", {})
        ci = worktree_state.get("ci", {})
        diff = wt.get("diff_vs_main", {})

        result.update(
            {
                # Git status
                "is_dirty": wt.get("staged") or wt.get("modified") or wt.get("untracked") or False,
                "is_rebasing": worktree_state.get("operation_state") == "rebase",
                "is_merging": worktree_state.get("operation_state") == "merge",
                "has_diff": (diff.get("added", 0) + diff.get("deleted", 0)) > 0,
                # Ahead/behind
                "ahead_main": main.get("ahead", 0),
                "behind_main": main.get("behind", 0),
                "ahead_remote": remote.get("ahead", 0),
                "behind_remote": remote.get("behind", 0),
                # PR
                "pr_url": ci.get("url"),
                "pr_number": _extract_pr_number(ci.get("url")),
                "pr_state": ci.get("state"),
                # Staleness
                "staleness": worktree_state.get("staleness"),
                "staleness_days": worktree_state.get("staleness_days"),
                # Recent steps
                "recent_steps": worktree_state.get("recent_steps", []),
            }
        )

    return result


def _extract_pr_number(url: str | None) -> int | None:
    """Extract PR number from GitHub PR URL."""
    if not url:
        return None
    import re

    match = re.search(r"/pull/(\d+)", url)
    return int(match.group(1)) if match else None


@app.post("/waves", response_model=LFDResponse)
async def post_wave(
    repo: str = Query(..., description="Repository path"), request: CreateWaveRequest = None
):
    """Create a new wave.

    Accepts minimal body - even empty creates a wave with generated name.
    """
    repo_path = Path(repo)
    if not repo_path.exists():
        raise HTTPException(status_code=404, detail=f"Repository not found: {repo}")

    # Normalize to main repo (worktrees resolve to their main repo)
    repo_path = _normalize_repo_path(repo_path)

    try:
        wave = create_wave(
            repo=repo_path,
            name=request.name if request else None,
            flow=request.flow if request and request.flow else "design",
            direction=request.direction if request else None,
            area=request.area if request else None,
            stimulus=Stimulus(kind="once"),
        )

        # Notify subscribers of new wave
        await _notify_event("wave.created", {"wave_id": wave.id, "name": wave.name})

        return LFDResponse(ok=True, result={"wave": _wave_to_dict(wave)})
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


class StimulusUpdate(BaseModel):
    kind: str  # once, loop, watch, cron
    cron: str | None = None


class UpdateWaveRequest(BaseModel):
    area: list[str] | None = None
    direction: list[str] | None = None
    flow: str | None = None
    stimulus: StimulusUpdate | None = None
    paused: bool | None = None


@app.patch("/waves/{wave_id}", response_model=LFDResponse)
async def patch_wave(wave_id: str, request: UpdateWaveRequest):
    """Update a wave's configuration.

    Accepts any subset of fields: area, direction, flow, stimulus, paused.
    Stimulus is an object: {kind: "once"|"loop"|"watch"|"cron", cron?: string}
    """
    try:
        wave = get_wave(wave_id)
        if not wave:
            raise HTTPException(status_code=404, detail=f"Wave not found: {wave_id}")

        # Build stimulus if provided
        stimulus = None
        if request.stimulus:
            stimulus = Stimulus(kind=request.stimulus.kind, cron=request.stimulus.cron)

        updated = update_wave(
            wave_id,
            area=request.area,
            direction=request.direction,
            flow=request.flow,
            stimulus=stimulus,
            paused=request.paused,
        )

        if not updated:
            return LFDResponse(ok=False, error="Failed to update wave")

        # Notify subscribers of wave update
        await _notify_event("wave.updated", {"wave_id": wave_id})

        return LFDResponse(ok=True, result={"wave": _wave_to_dict(updated)})
    except HTTPException:
        raise
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


@app.get("/waves/{wave_id}", response_model=LFDResponse)
async def get_wave_by_id(wave_id: str):
    """Get a single wave by ID."""
    try:
        wave = get_wave(wave_id)
        if not wave:
            raise HTTPException(status_code=404, detail=f"Wave not found: {wave_id}")

        return LFDResponse(ok=True, result={"wave": _wave_to_dict(wave)})
    except HTTPException:
        raise
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


@app.delete("/waves/{wave_id}", response_model=LFDResponse)
async def delete_wave_by_id(wave_id: str):
    """Delete a wave."""
    try:
        wave = get_wave(wave_id)
        if not wave:
            raise HTTPException(status_code=404, detail=f"Wave not found: {wave_id}")

        deleted = delete_wave(wave_id)
        if not deleted:
            return LFDResponse(ok=False, error="Failed to delete wave")

        # Notify subscribers
        await _notify_event("wave.deleted", {"wave_id": wave_id})

        return LFDResponse(ok=True, result={"deleted": True})
    except HTTPException:
        raise
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


class CloneWaveRequest(BaseModel):
    name: str | None = None  # optional name for clone


@app.post("/waves/{wave_id}/clone", response_model=LFDResponse)
async def clone_wave_endpoint(wave_id: str, request: CloneWaveRequest | None = None):
    """Clone a wave with a new name.

    Creates a copy with same config but fresh state (paused, no worktree).
    """
    try:
        wave = get_wave(wave_id)
        if not wave:
            raise HTTPException(status_code=404, detail=f"Wave not found: {wave_id}")

        name = request.name if request else None
        cloned = clone_wave(wave_id, name=name)

        if not cloned:
            return LFDResponse(ok=False, error="Failed to clone wave")

        # Notify subscribers
        await _notify_event("wave.created", {"wave_id": cloned.id, "name": cloned.name})

        return LFDResponse(ok=True, result={"wave": _wave_to_dict(cloned)})
    except HTTPException:
        raise
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


class RunWaveRequest(BaseModel):
    """Optional overrides for a single run (doesn't change wave config)."""

    area: list[str] | None = None
    direction: list[str] | None = None
    flow: str | None = None
    stimulus: StimulusUpdate | None = None


@app.post("/waves/{wave_id}/run", response_model=LFDResponse)
async def run_wave(wave_id: str, request: RunWaveRequest | None = None):
    """Run a wave.

    Optional body params are one-time overrides for this run only.
    Order: area, direction, flow, stimulus - any can be overridden.

    These overrides do NOT modify the wave's persistent configuration.
    """
    try:
        wave = get_wave(wave_id)
        if not wave:
            raise HTTPException(status_code=404, detail=f"Wave not found: {wave_id}")

        # Build overrides dict
        overrides = {}
        if request:
            if request.area is not None:
                overrides["area"] = request.area
            if request.direction is not None:
                overrides["direction"] = request.direction
            if request.flow is not None:
                overrides["flow"] = request.flow
            if request.stimulus is not None:
                overrides["stimulus"] = Stimulus(
                    kind=request.stimulus.kind, cron=request.stimulus.cron
                )

        # Check area (from wave or override)
        effective_area = overrides.get("area", wave.area)
        if effective_area is None:
            return LFDResponse(
                ok=False, error="No area configured. Set area first or pass as override."
            )

        # Start the wave with optional overrides
        result = start_wave(wave_id, **overrides)

        if result:
            await _notify_event("wave.started", {"wave_id": wave_id})
            return LFDResponse(ok=True, result={"started": True, "wave_id": wave_id})
        else:
            return LFDResponse(ok=False, error=f"Failed to start: {result.reason}")
    except HTTPException:
        raise
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


@app.post("/waves/{wave_id}/stop", response_model=LFDResponse)
async def stop_wave_by_id(wave_id: str):
    """Stop a running wave."""
    try:
        wave = get_wave(wave_id)
        if not wave:
            raise HTTPException(status_code=404, detail=f"Wave not found: {wave_id}")

        stopped = stop_wave(wave_id)
        if stopped:
            await _notify_event("wave.stopped", {"wave_id": wave_id})
            return LFDResponse(ok=True, result={"stopped": True})
        else:
            return LFDResponse(ok=False, error="Failed to stop wave")
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
