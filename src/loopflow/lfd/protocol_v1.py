"""Helpers for proto v1 JSON compatibility."""

from __future__ import annotations

from datetime import datetime
from typing import Any

from loopflow.lfd.models import MergeMode, StepRun, StepRunStatus, Stimulus, Wave, WaveStatus

PROTO_VERSION = (1, 0, 0)


_STIMULUS_KIND = {
    "once": "STIMULUS_ONCE",
    "loop": "STIMULUS_LOOP",
    "watch": "STIMULUS_WATCH",
    "cron": "STIMULUS_CRON",
}

_WAVE_STATUS = {
    WaveStatus.IDLE: "WAVE_IDLE",
    WaveStatus.RUNNING: "WAVE_RUNNING",
    WaveStatus.WAITING: "WAVE_WAITING",
    WaveStatus.ERROR: "WAVE_ERROR",
}

_MERGE_MODE = {
    MergeMode.PR: "MERGE_PR",
    MergeMode.LAND: "MERGE_LAND",
}

_STEP_RUN_STATUS = {
    StepRunStatus.RUNNING: "STEP_RUNNING",
    StepRunStatus.WAITING: "STEP_WAITING",
    StepRunStatus.COMPLETED: "STEP_COMPLETED",
    StepRunStatus.FAILED: "STEP_FAILED",
}

_OPERATION_STATE = {
    "rebase": "OPERATION_REBASE",
    "merge": "OPERATION_MERGE",
}

_PR_STATE = {
    "open": "PR_OPEN",
    "merged": "PR_MERGED",
    "closed": "PR_CLOSED",
    "draft": "PR_OPEN",
}

_CI_STATE = {
    "success": "CI_SUCCESS",
    "pending": "CI_PENDING",
    "failure": "CI_FAILURE",
}

_STALENESS = {
    "merged": "STALENESS_MERGED",
    "remote_deleted": "STALENESS_REMOTE_DELETED",
}


def protocol_version() -> dict[str, int]:
    return {
        "major": PROTO_VERSION[0],
        "minor": PROTO_VERSION[1],
        "patch": PROTO_VERSION[2],
    }


def stimulus_to_proto(stimulus: Stimulus | None) -> dict[str, Any]:
    if not stimulus:
        return {"kind": "STIMULUS_KIND_UNSPECIFIED"}
    kind = _STIMULUS_KIND.get(stimulus.kind, "STIMULUS_KIND_UNSPECIFIED")
    result: dict[str, Any] = {"kind": kind}
    if stimulus.kind == "cron" and stimulus.cron:
        result["cron"] = stimulus.cron
    return result


def wave_to_proto(wave: Wave, stimuli: list[Stimulus] | None = None) -> dict[str, Any]:
    """Convert Wave to proto-compatible dict.

    Note: stimulus is now a separate entity. Pass stimuli list to include them.
    The proto still expects a single stimulus field for backwards compat,
    so we use the first stimulus if available.
    """
    data: dict[str, Any] = {
        "id": wave.id,
        "name": wave.name,
        "repo": str(wave.repo),
        "flow": wave.flow,
        "direction": wave.direction or [],
        "area": wave.area or [],
        "paused": wave.paused,
        "status": _WAVE_STATUS.get(wave.status, "WAVE_STATUS_UNSPECIFIED"),
        "iteration": wave.iteration,
        "pr_limit": wave.pr_limit,
        "merge_mode": _MERGE_MODE.get(wave.merge_mode, "MERGE_MODE_UNSPECIFIED"),
        "created_at": _format_timestamp(wave.created_at),
        "consecutive_failures": wave.consecutive_failures,
        "pending_activations": wave.pending_activations,
        "step_index": wave.step_index,
    }

    # Note: Wave proto no longer has stimulus or last_main_sha fields
    # These are now on the Stimulus message

    _maybe(data, "worktree", str(wave.worktree) if wave.worktree else None)
    _maybe(data, "branch", wave.branch)
    _maybe(data, "pid", wave.pid)
    return data


def step_run_to_proto(step_run: StepRun) -> dict[str, Any]:
    data: dict[str, Any] = {
        "id": step_run.id,
        "step": step_run.step,
        "repo": step_run.repo,
        "worktree": step_run.worktree,
        "status": _STEP_RUN_STATUS.get(step_run.status, "STEP_RUN_STATUS_UNSPECIFIED"),
        "started_at": _format_timestamp(step_run.started_at),
        "model": step_run.model,
        "run_mode": step_run.run_mode,
    }

    _maybe(data, "flow_run_id", step_run.flow_run_id)
    _maybe(data, "wave_id", step_run.wave_id)
    _maybe(data, "ended_at", _format_timestamp(step_run.ended_at) if step_run.ended_at else None)
    _maybe(data, "pid", step_run.pid)
    return data


def worktree_to_proto(worktree_state: dict[str, Any]) -> dict[str, Any]:
    working_tree = worktree_state.get("working_tree", {})
    diff = working_tree.get("diff_vs_main", {})

    data: dict[str, Any] = {
        "branch": worktree_state.get("branch", ""),
        "path": worktree_state.get("path", ""),
        "base_branch": worktree_state.get("base_branch") or "",
        "working_tree": {
            "staged": bool(working_tree.get("staged")),
            "modified": bool(working_tree.get("modified")),
            "untracked": bool(working_tree.get("untracked")),
            "diff_added": int(diff.get("added", 0)),
            "diff_deleted": int(diff.get("deleted", 0)),
        },
        "main": {
            "ahead": int(worktree_state.get("main", {}).get("ahead", 0)),
            "behind": int(worktree_state.get("main", {}).get("behind", 0)),
        },
        "remote": {
            "ahead": int(worktree_state.get("remote", {}).get("ahead", 0)),
            "behind": int(worktree_state.get("remote", {}).get("behind", 0)),
        },
        "ci": {},
        "prunable": bool(worktree_state.get("prunable", False)),
        "recent_steps": [
            _recent_step_to_proto(step) for step in worktree_state.get("recent_steps", [])
        ],
    }

    _maybe(data, "main_state", worktree_state.get("main_state"))

    remote_name = worktree_state.get("remote", {}).get("name")
    _maybe(data["remote"], "name", remote_name)

    operation_state = worktree_state.get("operation_state")
    _maybe(data, "operation_state", _OPERATION_STATE.get(operation_state))

    ci = worktree_state.get("ci", {})
    _maybe(data["ci"], "source", ci.get("source"))
    _maybe(data["ci"], "url", ci.get("url"))
    _maybe(data["ci"], "state", _PR_STATE.get(ci.get("state", "")))
    _maybe(data["ci"], "ci_state", _CI_STATE.get(ci.get("ci_state", "")))
    if not data["ci"]:
        data.pop("ci", None)

    staleness = worktree_state.get("staleness")
    _maybe(data, "staleness", _STALENESS.get(staleness))
    _maybe(data, "staleness_days", worktree_state.get("staleness_days"))

    return data


def _recent_step_to_proto(step: dict[str, Any]) -> dict[str, Any]:
    data: dict[str, Any] = {
        "id": step.get("id", ""),
        "step": step.get("step", ""),
        "status": _step_run_status_name(step.get("status")),
    }

    _maybe(data, "started_at", step.get("startedAt"))
    _maybe(data, "ended_at", step.get("endedAt"))
    return data


def _step_run_status_name(status: str | None) -> str:
    if not status:
        return "STEP_RUN_STATUS_UNSPECIFIED"
    try:
        return _STEP_RUN_STATUS[StepRunStatus(status)]
    except ValueError:
        return "STEP_RUN_STATUS_UNSPECIFIED"


def _format_timestamp(value: datetime | None) -> str | None:
    if value is None:
        return None
    if value.tzinfo is None:
        return value.isoformat() + "Z"
    return value.isoformat()


def _maybe(target: dict[str, Any], key: str, value: Any) -> None:
    if value is not None:
        target[key] = value
