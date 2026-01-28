"""gRPC server for lfd daemon control plane.

Implements ControlService from control.proto. Runs alongside
the socket server on port 50051 (default).
"""

import asyncio
import time
from datetime import datetime
from pathlib import Path

import grpc
from google.protobuf import timestamp_pb2

from loopflow import __version__
from loopflow.lfd.daemon import metrics
from loopflow.lfd.daemon.manager import Manager
from loopflow.lfd.daemon.status import compute_status
from loopflow.lfd.migrations.baseline import SCHEMA_VERSION
from loopflow.lfd.models import StepRun, StepRunStatus
from loopflow.lfd.step_run import (
    load_step_runs,
    load_step_runs_for_repo,
    load_step_runs_for_worktree,
    save_step_run,
    update_step_run_status,
)
from loopflow.lfd.worktree_state import get_worktree_state_service
from loopflow.proto.loopflow.control.v1 import control_pb2, control_pb2_grpc

PROTOCOL_VERSION = control_pb2.ProtocolVersion(major=1, minor=0, patch=0)

# Default gRPC port
DEFAULT_GRPC_PORT = 50051


def _now_timestamp() -> timestamp_pb2.Timestamp:
    """Create a protobuf Timestamp for the current time."""
    ts = timestamp_pb2.Timestamp()
    ts.FromDatetime(datetime.utcnow())
    return ts


class ControlServiceServicer(control_pb2_grpc.ControlServiceServicer):
    """Implementation of ControlService gRPC interface."""

    def __init__(self, manager: Manager, broadcast_callback=None, start_time: float = None):
        self.manager = manager
        self._broadcast = broadcast_callback
        self._start_time = start_time or time.time()
        # Subscribers: writer -> list of patterns
        self._subscribers: dict = {}

    async def GetStatus(self, request, context):
        """Return basic daemon status."""
        metrics.increment("grpc_requests")
        status = compute_status()
        return control_pb2.GetStatusResponse(
            pid=status.get("pid", 0),
            waves_defined=status.get("waves_defined", 0),
            waves_running=status.get("waves_running", 0),
            step_runs_active=status.get("step_runs_active", 0),
        )

    async def GetHealth(self, request, context):
        """Return detailed health check with protocol version."""
        metrics.increment("grpc_requests")

        # Check database
        db_ok = True
        try:
            from loopflow.lfd.db import DB_PATH

            db_ok = DB_PATH.exists()
        except Exception:
            db_ok = False

        # Check socket
        socket_path = Path.home() / ".lf" / "lfd.sock"
        socket_ok = socket_path.exists()

        uptime = int(time.time() - self._start_time)
        all_metrics = metrics.get_all()

        return control_pb2.GetHealthResponse(
            version=__version__,
            schema_version=SCHEMA_VERSION,
            uptime_seconds=uptime,
            checks=control_pb2.HealthChecks(
                database=db_ok,
                socket=socket_ok,
            ),
            metrics=control_pb2.HealthMetrics(
                waves_total=all_metrics.get("waves_total", 0),
                waves_running=all_metrics.get("waves_running", 0),
                step_runs_active=all_metrics.get("step_runs_active", 0),
                flow_runs_total=all_metrics.get("flow_runs_total", 0),
            ),
            protocol_version=PROTOCOL_VERSION,
        )

    async def ListStepRuns(self, request, context):
        """List all active step runs."""
        metrics.increment("grpc_requests")
        step_runs = load_step_runs()
        return control_pb2.ListStepRunsResponse(
            step_runs=[_step_run_to_proto(sr) for sr in step_runs]
        )

    async def GetStepRunHistory(self, request, context):
        """Return step run history for a worktree or repo."""
        metrics.increment("grpc_requests")
        limit = request.limit if request.limit else 20

        if request.worktree:
            step_runs = load_step_runs_for_worktree(request.worktree, limit)
        elif request.repo:
            step_runs = load_step_runs_for_repo(request.repo, limit)
        else:
            step_runs = load_step_runs()[:limit]

        return control_pb2.GetStepRunHistoryResponse(
            step_runs=[_step_run_to_proto(sr) for sr in step_runs]
        )

    async def StartStepRun(self, request, context):
        """Record a step run start."""
        metrics.increment("grpc_requests")
        sr = request.step_run

        step_run = StepRun(
            id=sr.id,
            step=sr.step,
            repo=sr.repo,
            worktree=sr.worktree,
            flow_run_id=sr.flow_run_id if sr.HasField("flow_run_id") else None,
            wave_id=sr.wave_id if sr.HasField("wave_id") else None,
            status=StepRunStatus(sr.status) if sr.status else StepRunStatus.RUNNING,
            started_at=sr.started_at.ToDatetime() if sr.HasField("started_at") else datetime.now(),
            model=sr.model,
            run_mode=sr.run_mode,
        )
        save_step_run(step_run)

        if self._broadcast:
            await self._broadcast_event(
                "session.started",
                control_pb2.SessionStartedEvent(
                    id=step_run.id,
                    step=step_run.step,
                    worktree=step_run.worktree,
                ),
            )

        return control_pb2.StartStepRunResponse(id=step_run.id)

    async def EndStepRun(self, request, context):
        """Record a step run end."""
        metrics.increment("grpc_requests")

        if not request.step_run_id:
            context.set_code(grpc.StatusCode.INVALID_ARGUMENT)
            context.set_details("Missing step_run_id")
            return control_pb2.EndStepRunResponse()

        status = StepRunStatus(request.status) if request.status else StepRunStatus.COMPLETED
        update_step_run_status(request.step_run_id, status)

        if self._broadcast:
            await self._broadcast_event(
                "session.ended",
                control_pb2.SessionEndedEvent(
                    id=request.step_run_id,
                    status=status.value,
                ),
            )

        return control_pb2.EndStepRunResponse(id=request.step_run_id)

    async def GetSchedulerStatus(self, request, context):
        """Return scheduler status."""
        metrics.increment("grpc_requests")
        status = self.manager.get_status()
        return control_pb2.GetSchedulerStatusResponse(
            slots_used=status.get("slots_used", 0),
            slots_total=status.get("slots_total", 0),
            outstanding=status.get("outstanding", 0),
            outstanding_limit=status.get("outstanding_limit", 0),
            running=status.get("running", []),
        )

    async def AcquireSlot(self, request, context):
        """Try to acquire a scheduler slot."""
        metrics.increment("grpc_requests")

        if not request.run_id:
            context.set_code(grpc.StatusCode.INVALID_ARGUMENT)
            context.set_details("Missing run_id")
            return control_pb2.AcquireSlotResponse()

        acquired, reason = self.manager.acquire(request.run_id)

        if acquired and self._broadcast:
            await self._broadcast_event(
                "scheduler.slot.acquired",
                control_pb2.SchedulerSlotAcquiredEvent(
                    run_id=request.run_id,
                    slots_used=self.manager.slots_used(),
                ),
            )

        return control_pb2.AcquireSlotResponse(
            acquired=acquired,
            reason=reason,
            slots_used=self.manager.slots_used(),
        )

    async def ReleaseSlot(self, request, context):
        """Release a scheduler slot."""
        metrics.increment("grpc_requests")

        if not request.run_id:
            context.set_code(grpc.StatusCode.INVALID_ARGUMENT)
            context.set_details("Missing run_id")
            return control_pb2.ReleaseSlotResponse()

        self.manager.release(request.run_id)

        if self._broadcast:
            await self._broadcast_event(
                "scheduler.slot.released",
                control_pb2.SchedulerSlotReleasedEvent(
                    run_id=request.run_id,
                    slots_used=self.manager.slots_used(),
                ),
            )

        return control_pb2.ReleaseSlotResponse(slots_used=self.manager.slots_used())

    async def ListWorktrees(self, request, context):
        """List worktrees for a repository."""
        metrics.increment("grpc_requests")

        if not request.repo:
            context.set_code(grpc.StatusCode.INVALID_ARGUMENT)
            context.set_details("Missing repo parameter")
            return control_pb2.ListWorktreesResponse()

        repo_path = Path(request.repo)
        if not repo_path.exists():
            context.set_code(grpc.StatusCode.NOT_FOUND)
            context.set_details(f"Repository not found: {request.repo}")
            return control_pb2.ListWorktreesResponse()

        try:
            service = get_worktree_state_service()
            worktrees = service.list_worktrees(repo_path)
            return control_pb2.ListWorktreesResponse(
                worktrees=[_worktree_to_proto(wt) for wt in worktrees]
            )
        except Exception as e:
            context.set_code(grpc.StatusCode.INTERNAL)
            context.set_details(str(e))
            return control_pb2.ListWorktreesResponse()

    async def NotifyWorktreeChanged(self, request, context):
        """Handle notification that a worktree changed."""
        metrics.increment("grpc_requests")

        if not request.repo or not request.branch:
            context.set_code(grpc.StatusCode.INVALID_ARGUMENT)
            context.set_details("Missing repo or branch parameter")
            return control_pb2.NotifyWorktreeChangedResponse()

        repo_path = Path(request.repo)
        service = get_worktree_state_service()
        service.invalidate(repo_path)

        reason = request.reason if request.reason else "changed"

        if self._broadcast:
            worktree_status = service.get_one(repo_path, request.branch)
            await self._broadcast_event(
                "worktree.updated",
                control_pb2.WorktreeUpdatedEvent(
                    branch=request.branch,
                    reason=_worktree_reason_to_proto(reason),
                    repo=str(repo_path),
                    worktree=_worktree_to_proto(worktree_status) if worktree_status else None,
                ),
            )

        return control_pb2.NotifyWorktreeChangedResponse(
            branch=request.branch,
            reason=reason,
        )

    async def Notify(self, request, context):
        """Accept external events and broadcast to subscribers."""
        metrics.increment("grpc_requests")

        if not request.event:
            context.set_code(grpc.StatusCode.INVALID_ARGUMENT)
            context.set_details("Missing event parameter")
            return control_pb2.NotifyResponse()

        # For now, just acknowledge - actual event broadcasting
        # happens through the socket server's broadcast mechanism
        return control_pb2.NotifyResponse(event=request.event)

    async def StreamOutput(self, request, context):
        """Accept output lines and broadcast to subscribers."""
        metrics.increment("grpc_requests")

        if not request.step_run_id or request.text is None:
            context.set_code(grpc.StatusCode.INVALID_ARGUMENT)
            context.set_details("Missing step_run_id or text parameter")
            return control_pb2.StreamOutputResponse()

        if self._broadcast:
            await self._broadcast_event(
                "output.line",
                control_pb2.OutputLineEvent(
                    session_id=request.step_run_id,
                    text=request.text,
                ),
            )

        return control_pb2.StreamOutputResponse()

    async def Subscribe(self, request, context):
        """Stream events matching the given patterns."""
        metrics.increment("grpc_requests")
        # Event streaming is complex and requires integration with
        # the socket server's broadcast mechanism. For now, we just
        # yield nothing - the socket server handles event streaming.
        # This will be wired up in a future iteration.
        while not context.cancelled():
            await asyncio.sleep(1)

    async def _broadcast_event(self, event_type: str, payload) -> None:
        """Broadcast an event through the socket server's mechanism."""
        if self._broadcast:
            # Convert protobuf message to dict
            from google.protobuf.json_format import MessageToDict

            from loopflow.lfd.daemon.protocol import Event

            data = MessageToDict(payload, preserving_proto_field_name=True)

            await self._broadcast(Event(event_type, data))


def _step_run_to_proto(sr: StepRun) -> control_pb2.StepRun:
    """Convert StepRun model to protobuf message."""
    proto = control_pb2.StepRun(
        id=sr.id,
        step=sr.step,
        repo=sr.repo,
        worktree=sr.worktree,
        status=_step_status_to_proto(sr.status),
        model=sr.model or "",
        run_mode=sr.run_mode or "",
    )

    if sr.flow_run_id:
        proto.flow_run_id = sr.flow_run_id
    if sr.wave_id:
        proto.wave_id = sr.wave_id
    if sr.started_at:
        proto.started_at.FromDatetime(sr.started_at)
    if sr.ended_at:
        proto.ended_at.FromDatetime(sr.ended_at)
    if sr.pid:
        proto.pid = sr.pid

    return proto


def _step_status_to_proto(status: StepRunStatus) -> int:
    """Convert StepRunStatus to protobuf enum value."""
    mapping = {
        StepRunStatus.RUNNING: control_pb2.STEP_RUNNING,
        StepRunStatus.WAITING: control_pb2.STEP_WAITING,
        StepRunStatus.COMPLETED: control_pb2.STEP_COMPLETED,
        StepRunStatus.FAILED: control_pb2.STEP_FAILED,
    }
    return mapping.get(status, control_pb2.STEP_RUN_STATUS_UNSPECIFIED)


def _worktree_reason_to_proto(reason: str) -> int:
    """Convert worktree change reason string to protobuf enum."""
    mapping = {
        "commit": control_pb2.WORKTREE_COMMIT,
        "checkout": control_pb2.WORKTREE_CHECKOUT,
        "changed": control_pb2.WORKTREE_CHANGED,
        "draft_pr_created": control_pb2.WORKTREE_DRAFT_PR_CREATED,
        "ci_updated": control_pb2.WORKTREE_CI_UPDATED,
        "merged": control_pb2.WORKTREE_MERGED,
        "pr_state_changed": control_pb2.WORKTREE_PR_STATE_CHANGED,
    }
    return mapping.get(reason, control_pb2.WORKTREE_CHANGE_REASON_UNSPECIFIED)


def _worktree_to_proto(wt: dict) -> control_pb2.WorktreeState:
    """Convert worktree dict to protobuf message."""
    working_tree = wt.get("working_tree", {})
    main = wt.get("main", {})
    remote = wt.get("remote", {})
    ci = wt.get("ci", {})
    diff = working_tree.get("diff_vs_main", {})

    proto = control_pb2.WorktreeState(
        branch=wt.get("branch", ""),
        path=wt.get("path", ""),
        base_branch=wt.get("base_branch", "main"),
        working_tree=control_pb2.WorkingTreeStatus(
            staged=working_tree.get("staged", False),
            modified=working_tree.get("modified", False),
            untracked=working_tree.get("untracked", False),
            diff_added=diff.get("added", 0),
            diff_deleted=diff.get("deleted", 0),
        ),
        main=control_pb2.MainStatus(
            ahead=main.get("ahead", 0),
            behind=main.get("behind", 0),
        ),
        remote=control_pb2.RemoteStatus(
            name=remote.get("name"),
            ahead=remote.get("ahead", 0),
            behind=remote.get("behind", 0),
        ),
        ci=control_pb2.CIStatus(
            source=ci.get("source"),
            url=ci.get("url"),
            state=_pr_state_to_proto(ci.get("state")),
            ci_state=_ci_state_to_proto(ci.get("ci_state")),
        ),
        prunable=wt.get("prunable", False),
        staleness=_staleness_to_proto(wt.get("staleness")),
        staleness_days=wt.get("staleness_days"),
        recent_steps=[_recent_step_to_proto(s) for s in wt.get("recent_steps", [])],
    )

    if wt.get("operation_state"):
        proto.operation_state = _operation_state_to_proto(wt["operation_state"])

    return proto


def _pr_state_to_proto(state: str | None) -> int:
    """Convert PR state string to protobuf enum."""
    if not state:
        return control_pb2.PR_STATE_UNSPECIFIED
    mapping = {
        "OPEN": control_pb2.PR_OPEN,
        "MERGED": control_pb2.PR_MERGED,
        "CLOSED": control_pb2.PR_CLOSED,
    }
    return mapping.get(state.upper(), control_pb2.PR_STATE_UNSPECIFIED)


def _ci_state_to_proto(state: str | None) -> int:
    """Convert CI state string to protobuf enum."""
    if not state:
        return control_pb2.CI_STATE_UNSPECIFIED
    mapping = {
        "SUCCESS": control_pb2.CI_SUCCESS,
        "PENDING": control_pb2.CI_PENDING,
        "FAILURE": control_pb2.CI_FAILURE,
    }
    return mapping.get(state.upper(), control_pb2.CI_STATE_UNSPECIFIED)


def _staleness_to_proto(staleness: str | None) -> int:
    """Convert staleness string to protobuf enum."""
    if not staleness:
        return control_pb2.STALENESS_UNSPECIFIED
    mapping = {
        "merged": control_pb2.STALENESS_MERGED,
        "remote_deleted": control_pb2.STALENESS_REMOTE_DELETED,
    }
    return mapping.get(staleness.lower(), control_pb2.STALENESS_UNSPECIFIED)


def _operation_state_to_proto(state: str | None) -> int:
    """Convert operation state string to protobuf enum."""
    if not state:
        return control_pb2.OPERATION_STATE_UNSPECIFIED
    mapping = {
        "rebase": control_pb2.OPERATION_REBASE,
        "merge": control_pb2.OPERATION_MERGE,
    }
    return mapping.get(state.lower(), control_pb2.OPERATION_STATE_UNSPECIFIED)


def _recent_step_to_proto(step: dict) -> control_pb2.RecentStepRun:
    """Convert recent step dict to protobuf message."""
    proto = control_pb2.RecentStepRun(
        id=step.get("id", ""),
        step=step.get("step", ""),
        status=_step_status_str_to_proto(step.get("status")),
    )
    if step.get("started_at"):
        proto.started_at.FromDatetime(datetime.fromisoformat(step["started_at"]))
    if step.get("ended_at"):
        proto.ended_at.FromDatetime(datetime.fromisoformat(step["ended_at"]))
    return proto


def _step_status_str_to_proto(status: str | None) -> int:
    """Convert step status string to protobuf enum."""
    if not status:
        return control_pb2.STEP_RUN_STATUS_UNSPECIFIED
    mapping = {
        "running": control_pb2.STEP_RUNNING,
        "waiting": control_pb2.STEP_WAITING,
        "completed": control_pb2.STEP_COMPLETED,
        "failed": control_pb2.STEP_FAILED,
    }
    return mapping.get(status.lower(), control_pb2.STEP_RUN_STATUS_UNSPECIFIED)


class GrpcServer:
    """Async gRPC server wrapper for the control plane."""

    def __init__(
        self,
        port: int = DEFAULT_GRPC_PORT,
        manager: Manager = None,
        broadcast_callback=None,
    ):
        self.port = port
        self.manager = manager
        self._broadcast = broadcast_callback
        self._server: grpc.aio.Server | None = None
        self._start_time = time.time()

    async def start(self) -> None:
        """Start the gRPC server."""
        self._server = grpc.aio.server()

        servicer = ControlServiceServicer(
            manager=self.manager,
            broadcast_callback=self._broadcast,
            start_time=self._start_time,
        )
        control_pb2_grpc.add_ControlServiceServicer_to_server(servicer, self._server)

        self._server.add_insecure_port(f"[::]:{self.port}")
        await self._server.start()

    async def stop(self) -> None:
        """Stop the gRPC server gracefully."""
        if self._server:
            await self._server.stop(grace=5)


async def start_grpc_server(
    port: int = DEFAULT_GRPC_PORT,
    manager: Manager = None,
    broadcast_callback=None,
) -> GrpcServer:
    """Start the gRPC server. Returns server for cleanup."""
    server = GrpcServer(
        port=port,
        manager=manager,
        broadcast_callback=broadcast_callback,
    )
    await server.start()
    return server
