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
from loopflow.lfd.models import StepRun, StepRunStatus, WaveStatus
from loopflow.lfd.protocol_v1 import (
    protocol_version,
    step_run_to_proto,
    wave_to_proto,
    worktree_to_proto,
)
from loopflow.lfd.step_run import (
    get_waiting_step_run,
    load_step_runs,
    load_step_runs_for_repo,
    load_step_runs_for_worktree,
    save_step_run,
    update_step_run_status,
)
from loopflow.lfd.stimulus import create_stimulus, list_stimuli
from loopflow.lfd.wave import (
    clone_wave,
    create_wave,
    delete_wave,
    get_wave,
    list_waves,
    setup_wave_worktree,
    start_wave,
    stop_wave,
    update_wave,
    update_wave_status,
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


def _list_worktrees_sync(repo_path: Path) -> list[dict]:
    """List worktrees with staleness. Blocking - call from thread."""
    service = get_worktree_state_service()
    return service.list_worktrees(repo_path)


@app.get("/worktrees", response_model=LFDResponse)
async def list_worktrees(repo: str = Query(..., description="Repository path")):
    """List worktrees with staleness and recent steps."""
    repo_path = Path(repo)
    if not repo_path.exists():
        raise HTTPException(status_code=404, detail=f"Repository not found: {repo}")

    try:
        worktrees = await asyncio.to_thread(_list_worktrees_sync, repo_path)
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
    uptime, db_ok, socket_ok, all_metrics, status = _health_snapshot()
    return LFDResponse(
        ok=True,
        result={
            **status,
            "version": __version__,
            "schema_version": SCHEMA_VERSION,
            "uptime_seconds": uptime,
            "checks": {
                "database": "ok" if db_ok else "error",
                "socket": "ok" if socket_ok else "error",
            },
            "metrics": all_metrics,
        },
    )


def _health_snapshot() -> tuple[int, bool, bool, dict[str, int], dict[str, Any]]:
    uptime = int(time.time() - _start_time) if _start_time else 0

    db_ok = True
    try:
        from loopflow.lfd.db import DB_PATH

        db_ok = DB_PATH.exists()
    except Exception:
        db_ok = False

    socket_path = Path.home() / ".lf" / "lfd.sock"
    socket_ok = socket_path.exists()

    return uptime, db_ok, socket_ok, metrics.get_all(), compute_status()


# -----------------------------------------------------------------------------
# JSON-over-HTTP Compatibility Layer (v1 API)
# These endpoints mirror the gRPC API for clients that prefer JSON.
# -----------------------------------------------------------------------------


@app.get("/v1/health")
async def get_health_v1():
    """JSON-compatible health endpoint matching proto schema.

    This endpoint returns the exact structure defined in GetHealthResponse
    for clients that prefer JSON over gRPC.
    """
    uptime, db_ok, socket_ok, all_metrics, _ = _health_snapshot()

    return {
        "version": __version__,
        "schema_version": SCHEMA_VERSION,
        "uptime_seconds": uptime,
        "checks": {
            "database": db_ok,
            "socket": socket_ok,
        },
        "metrics": {
            "waves_total": all_metrics.get("waves_total", 0),
            "waves_running": all_metrics.get("waves_running", 0),
            "step_runs_active": all_metrics.get("step_runs_active", 0),
            "flow_runs_total": all_metrics.get("flow_runs_total", 0),
        },
        "protocol_version": protocol_version(),
    }


@app.get("/v1/status")
async def get_status_v1():
    """JSON-compatible status endpoint matching proto schema."""
    status = compute_status()
    return {
        "pid": status.get("pid", 0),
        "waves_defined": status.get("waves_defined", 0),
        "waves_running": status.get("waves_running", 0),
        "step_runs_active": status.get("step_runs_active", 0),
    }


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


def _list_waves_with_enrichment(repo: str) -> list[dict]:
    """List waves with worktree state enrichment. Blocking - call from thread."""
    repo_path = Path(repo)

    # Normalize to main repo (worktrees resolve to their main repo)
    repo_path = _normalize_repo_path(repo_path)

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

    return enriched


@app.get("/waves", response_model=LFDResponse)
async def get_waves(repo: str = Query(..., description="Repository path")):
    """List waves for a repository, enriched with worktree state."""
    repo_path = Path(repo)
    if not repo_path.exists():
        raise HTTPException(status_code=404, detail=f"Repository not found: {repo}")

    try:
        # Run in thread - includes git operations for normalization and worktree state
        enriched = await asyncio.to_thread(_list_waves_with_enrichment, repo)
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
    # Get primary stimulus (first one) for backwards compat
    stimuli = list_stimuli(wave_id=wave.id)
    primary_stimulus = stimuli[0] if stimuli else None

    result = {
        "id": wave.id,
        "name": wave.name,
        "area": wave.area,
        "direction": wave.direction,
        "flow": wave.flow,
        "stimulus": {
            "kind": primary_stimulus.kind if primary_stimulus else "once",
            "cron": primary_stimulus.cron if primary_stimulus else None,
        },
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
    Returns immediately; worktree setup happens in background.
    """
    repo_path = Path(repo)
    if not repo_path.exists():
        raise HTTPException(status_code=404, detail=f"Repository not found: {repo}")

    # Normalize to main repo (worktrees resolve to their main repo)
    repo_path = _normalize_repo_path(repo_path)

    try:
        # Create wave record immediately (no git ops)
        wave = create_wave(
            repo=repo_path,
            name=request.name if request else None,
            flow=request.flow if request and request.flow else "design",
            direction=request.direction if request else None,
            area=request.area if request else None,
            stimulus_kind="once",
        )

        # Background the git operations (fetch, worktree add, push)
        async def setup_worktree_background():
            await asyncio.to_thread(setup_wave_worktree, wave.id)
            await _notify_event("wave.ready", {"wave_id": wave.id, "name": wave.name})

        asyncio.create_task(setup_worktree_background())

        # Notify subscribers of new wave (before worktree is ready)
        await _notify_event("wave.created", {"wave_id": wave.id, "name": wave.name})

        return LFDResponse(ok=True, result={"wave": _wave_to_dict(wave)})
    except Exception as e:
        return LFDResponse(ok=False, error=str(e))


class StimulusRequest(BaseModel):
    """Stimulus configuration for API requests."""

    kind: str  # once, loop, watch, cron
    cron: str | None = None


class UpdateWaveRequest(BaseModel):
    area: list[str] | None = None
    direction: list[str] | None = None
    flow: str | None = None
    stimulus: StimulusRequest | None = None
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

        # Create stimulus if provided (separate entity now)
        if request.stimulus:
            create_stimulus(wave_id, request.stimulus.kind, request.stimulus.cron)

        updated = update_wave(
            wave_id,
            area=request.area,
            direction=request.direction,
            flow=request.flow,
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
    stimulus: StimulusRequest | None = None


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
            # Create stimulus if provided (separate entity now)
            if request.stimulus is not None:
                create_stimulus(wave_id, request.stimulus.kind, request.stimulus.cron)

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


# -----------------------------------------------------------------------------
# Proto v1 JSON compatibility endpoints
# -----------------------------------------------------------------------------


class CreateWaveRequestV1(BaseModel):
    repo: str
    name: str | None = None
    flow: str | None = None
    direction: list[str] | None = None
    area: list[str] | None = None
    idempotency_key: str | None = None


class UpdateWaveRequestV1(BaseModel):
    flow: str | None = None
    direction: list[str] | None = None
    area: list[str] | None = None
    stimulus: StimulusRequest | None = None
    paused: bool | None = None
    idempotency_key: str | None = None


class RunWaveRequestV1(BaseModel):
    area: list[str] | None = None
    direction: list[str] | None = None
    flow: str | None = None
    stimulus: StimulusRequest | None = None
    idempotency_key: str | None = None


class StartStepRunRequestV1(BaseModel):
    step_run: dict[str, Any]


class EndStepRunRequestV1(BaseModel):
    step_run_id: str
    status: str


def _load_repo_or_404(repo: str) -> Path:
    repo_path = Path(repo)
    if not repo_path.exists():
        raise HTTPException(status_code=404, detail=f"Repository not found: {repo}")
    return _normalize_repo_path(repo_path)


def _step_run_from_proto(step_run_data: dict[str, Any]) -> StepRun:
    status = step_run_data.get("status")
    status_map = {
        "STEP_RUNNING": "running",
        "STEP_WAITING": "waiting",
        "STEP_COMPLETED": "completed",
        "STEP_FAILED": "failed",
    }
    normalized = dict(step_run_data)
    if status in status_map:
        normalized["status"] = status_map[status]
    return StepRun.from_dict(normalized)


def _step_run_status_from_proto(status: str) -> StepRunStatus:
    status_map = {
        "STEP_RUNNING": StepRunStatus.RUNNING,
        "STEP_WAITING": StepRunStatus.WAITING,
        "STEP_COMPLETED": StepRunStatus.COMPLETED,
        "STEP_FAILED": StepRunStatus.FAILED,
    }
    return status_map.get(status, StepRunStatus(status))


@app.get("/v1/flows")
async def get_flows_v1(repo: str = Query(..., description="Repository path")):
    from loopflow.lf.flows import FlowItem, Fork, Step, list_flows, list_steps

    repo_path = _load_repo_or_404(repo)

    def step_names(items: list[FlowItem]) -> list[str]:
        names = []
        for item in items:
            if isinstance(item, Step):
                names.append(item.name)
            elif isinstance(item, Fork):
                names.append("(fork)")
            else:
                names.append("(choose)")
        return names

    builtins_steps = Path(__file__).resolve().parents[2] / "lf" / "builtins" / "steps"

    def is_builtin_step(name: str) -> bool:
        return any(path.stem == name for path in builtins_steps.glob("**/*.md"))

    flows = list_flows(repo_path)
    steps = list_steps(repo_path)

    return {
        "flows": [
            {
                "name": flow.name,
                "type": "FLOW_YAML",
                "steps": step_names(flow.steps),
            }
            for flow in flows
        ],
        "steps": [
            {"name": step, "type": "builtin" if is_builtin_step(step) else "custom"}
            for step in steps
        ],
    }


def _list_worktrees_v1_sync(repo: str) -> list[dict]:
    """List worktrees. Blocking - call from thread."""
    repo_path = _load_repo_or_404(repo)
    service = get_worktree_state_service()
    worktrees = service.list_worktrees(repo_path)
    return [worktree_to_proto(wt) for wt in worktrees]


@app.get("/v1/worktrees")
async def list_worktrees_v1(repo: str = Query(..., description="Repository path")):
    worktrees = await asyncio.to_thread(_list_worktrees_v1_sync, repo)
    return {"worktrees": worktrees}


def _list_waves_v1_sync(repo: str) -> list[dict]:
    """List waves. Blocking - call from thread."""
    repo_path = _load_repo_or_404(repo)
    waves = list_waves(repo=repo_path)
    return [wave_to_proto(wave) for wave in waves]


@app.get("/v1/waves")
async def list_waves_v1(repo: str = Query(..., description="Repository path")):
    waves = await asyncio.to_thread(_list_waves_v1_sync, repo)
    return {"waves": waves}


@app.get("/v1/waves/{wave_id}")
async def get_wave_v1(wave_id: str):
    wave = get_wave(wave_id)
    if not wave:
        raise HTTPException(status_code=404, detail=f"Wave not found: {wave_id}")
    return {"wave": wave_to_proto(wave)}


@app.post("/v1/waves")
async def create_wave_v1(request: CreateWaveRequestV1):
    repo_path = _load_repo_or_404(request.repo)

    # Create wave record immediately (no git ops)
    wave = create_wave(
        repo=repo_path,
        name=request.name,
        flow=request.flow or "design",
        direction=request.direction,
        area=request.area,
        stimulus_kind="once",
    )

    # Background the git operations
    async def setup_worktree_background():
        await asyncio.to_thread(setup_wave_worktree, wave.id)
        await _notify_event("wave.ready", {"wave_id": wave.id, "name": wave.name})

    asyncio.create_task(setup_worktree_background())

    await _notify_event("wave.created", {"wave_id": wave.id, "name": wave.name})
    return {"wave": wave_to_proto(wave)}


@app.patch("/v1/waves/{wave_id}")
async def update_wave_v1(wave_id: str, request: UpdateWaveRequestV1):
    wave = get_wave(wave_id)
    if not wave:
        raise HTTPException(status_code=404, detail=f"Wave not found: {wave_id}")

    # Create stimulus if provided (separate entity now)
    if request.stimulus:
        create_stimulus(wave_id, request.stimulus.kind, request.stimulus.cron)

    updated = update_wave(
        wave_id,
        area=request.area,
        direction=request.direction,
        flow=request.flow,
        paused=request.paused,
    )
    if not updated:
        raise HTTPException(status_code=500, detail="Failed to update wave")
    await _notify_event("wave.updated", {"wave_id": wave_id})
    return {"wave": wave_to_proto(updated)}


@app.delete("/v1/waves/{wave_id}")
async def delete_wave_v1(wave_id: str):
    wave = get_wave(wave_id)
    if not wave:
        raise HTTPException(status_code=404, detail=f"Wave not found: {wave_id}")
    deleted = delete_wave(wave_id)
    if not deleted:
        raise HTTPException(status_code=500, detail="Failed to delete wave")
    await _notify_event("wave.deleted", {"wave_id": wave_id})
    return {}


@app.post("/v1/waves/{wave_id}/clone")
async def clone_wave_v1(wave_id: str, request: CloneWaveRequest | None = None):
    wave = get_wave(wave_id)
    if not wave:
        raise HTTPException(status_code=404, detail=f"Wave not found: {wave_id}")
    name = request.name if request else None
    cloned = clone_wave(wave_id, name=name)
    if not cloned:
        raise HTTPException(status_code=500, detail="Failed to clone wave")
    await _notify_event("wave.created", {"wave_id": cloned.id, "name": cloned.name})
    return {"wave": wave_to_proto(cloned)}


@app.post("/v1/waves/{wave_id}/run")
async def run_wave_v1(wave_id: str, request: RunWaveRequestV1 | None = None):
    wave = get_wave(wave_id)
    if not wave:
        raise HTTPException(status_code=404, detail=f"Wave not found: {wave_id}")

    overrides = {}
    if request:
        if request.area is not None:
            overrides["area"] = request.area
        if request.direction is not None:
            overrides["direction"] = request.direction
        if request.flow is not None:
            overrides["flow"] = request.flow
        # Create stimulus if provided (separate entity now)
        if request.stimulus is not None:
            create_stimulus(wave_id, request.stimulus.kind, request.stimulus.cron)

    effective_area = overrides.get("area", wave.area)
    if effective_area is None:
        raise HTTPException(
            status_code=400,
            detail="No area configured. Set area first or pass as override.",
        )

    result = start_wave(wave_id, **overrides)
    if not result:
        raise HTTPException(status_code=500, detail=f"Failed to start: {result.reason}")
    await _notify_event("wave.started", {"wave_id": wave_id})
    return {"started": True, "wave_id": wave_id}


@app.post("/v1/waves/{wave_id}/stop")
async def stop_wave_v1(wave_id: str):
    wave = get_wave(wave_id)
    if not wave:
        raise HTTPException(status_code=404, detail=f"Wave not found: {wave_id}")
    stopped = stop_wave(wave_id)
    if not stopped:
        raise HTTPException(status_code=500, detail="Failed to stop wave")
    await _notify_event("wave.stopped", {"wave_id": wave_id})
    return {"stopped": True}


@app.post("/v1/waves/{wave_id}/connect")
async def connect_wave_v1(wave_id: str):
    from loopflow.lf.context import ContextConfig, format_prompt, gather_prompt_components
    from loopflow.lf.directions import resolve_directions
    from loopflow.lf.logging import write_prompt_file
    from loopflow.lfd.flow_run import get_run

    wave = get_wave(wave_id)
    if not wave:
        raise HTTPException(status_code=404, detail=f"Wave not found: {wave_id}")

    step_run = get_waiting_step_run(wave.id)
    if not step_run:
        raise HTTPException(status_code=404, detail="No waiting step run")

    update_step_run_status(step_run.id, StepRunStatus.RUNNING)
    update_wave_status(wave.id, WaveStatus.RUNNING)

    worktree_path = Path(step_run.worktree)
    direction = resolve_directions(wave.repo, wave.direction)
    context_paths = list(wave.area) if wave.area and wave.area[0] != "." else None

    components = gather_prompt_components(
        worktree_path,
        step=step_run.step,
        run_mode="interactive",
        direction=direction,
        context_config=ContextConfig(pathset=context_paths),
    )
    if not components.step:
        raise HTTPException(status_code=404, detail=f"Step not found: {step_run.step}")

    prompt = format_prompt(components)
    prompt_file = write_prompt_file(prompt)

    flow_run_id = step_run.flow_run_id
    step_index = 0
    if flow_run_id:
        flow_run = get_run(flow_run_id)
        if flow_run:
            step_index = flow_run.step_index

    return {
        "worktree": step_run.worktree,
        "step": step_run.step,
        "step_run_id": step_run.id,
        "prompt_file": prompt_file,
        "flow_run_id": flow_run_id,
        "step_index": step_index,
    }


@app.get("/v1/step_runs")
async def list_step_runs_v1():
    step_runs = load_step_runs()
    return {"step_runs": [step_run_to_proto(sr) for sr in step_runs]}


@app.get("/v1/step_runs/history")
async def get_step_run_history_v1(
    worktree: str | None = None,
    repo: str | None = None,
    limit: int | None = None,
):
    if worktree:
        step_runs = load_step_runs_for_worktree(worktree, limit or 20)
    elif repo:
        step_runs = load_step_runs_for_repo(repo, limit or 20)
    else:
        step_runs = load_step_runs()[: (limit or 20)]
    return {"step_runs": [step_run_to_proto(sr) for sr in step_runs]}


@app.post("/v1/step_runs/start")
async def start_step_run_v1(request: StartStepRunRequestV1):
    step_run = _step_run_from_proto(request.step_run)
    save_step_run(step_run)
    await _notify_event(
        "session.started",
        {
            "id": step_run.id,
            "step": step_run.step,
            "worktree": step_run.worktree,
        },
    )
    return {"id": step_run.id}


@app.post("/v1/step_runs/end")
async def end_step_run_v1(request: EndStepRunRequestV1):
    status = _step_run_status_from_proto(request.status)
    update_step_run_status(request.step_run_id, status)
    await _notify_event(
        "session.ended",
        {"id": request.step_run_id, "status": status.value},
    )
    return {"id": request.step_run_id}


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
