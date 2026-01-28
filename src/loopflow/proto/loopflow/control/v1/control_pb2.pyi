import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class StimulusKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    STIMULUS_KIND_UNSPECIFIED: _ClassVar[StimulusKind]
    STIMULUS_ONCE: _ClassVar[StimulusKind]
    STIMULUS_LOOP: _ClassVar[StimulusKind]
    STIMULUS_WATCH: _ClassVar[StimulusKind]
    STIMULUS_CRON: _ClassVar[StimulusKind]

class WaveStatus(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    WAVE_STATUS_UNSPECIFIED: _ClassVar[WaveStatus]
    WAVE_IDLE: _ClassVar[WaveStatus]
    WAVE_RUNNING: _ClassVar[WaveStatus]
    WAVE_WAITING: _ClassVar[WaveStatus]
    WAVE_ERROR: _ClassVar[WaveStatus]

class MergeMode(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    MERGE_MODE_UNSPECIFIED: _ClassVar[MergeMode]
    MERGE_PR: _ClassVar[MergeMode]
    MERGE_LAND: _ClassVar[MergeMode]

class FlowType(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    FLOW_TYPE_UNSPECIFIED: _ClassVar[FlowType]
    FLOW_YAML: _ClassVar[FlowType]
    FLOW_PYTHON: _ClassVar[FlowType]

class CIState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CI_STATE_UNSPECIFIED: _ClassVar[CIState]
    CI_SUCCESS: _ClassVar[CIState]
    CI_PENDING: _ClassVar[CIState]
    CI_FAILURE: _ClassVar[CIState]

class PRState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    PR_STATE_UNSPECIFIED: _ClassVar[PRState]
    PR_OPEN: _ClassVar[PRState]
    PR_MERGED: _ClassVar[PRState]
    PR_CLOSED: _ClassVar[PRState]

class OperationState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    OPERATION_STATE_UNSPECIFIED: _ClassVar[OperationState]
    OPERATION_REBASE: _ClassVar[OperationState]
    OPERATION_MERGE: _ClassVar[OperationState]

class Staleness(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    STALENESS_UNSPECIFIED: _ClassVar[Staleness]
    STALENESS_MERGED: _ClassVar[Staleness]
    STALENESS_REMOTE_DELETED: _ClassVar[Staleness]

class StepRunStatus(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    STEP_RUN_STATUS_UNSPECIFIED: _ClassVar[StepRunStatus]
    STEP_RUNNING: _ClassVar[StepRunStatus]
    STEP_WAITING: _ClassVar[StepRunStatus]
    STEP_COMPLETED: _ClassVar[StepRunStatus]
    STEP_FAILED: _ClassVar[StepRunStatus]

class WorktreeChangeReason(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    WORKTREE_CHANGE_REASON_UNSPECIFIED: _ClassVar[WorktreeChangeReason]
    WORKTREE_COMMIT: _ClassVar[WorktreeChangeReason]
    WORKTREE_CHECKOUT: _ClassVar[WorktreeChangeReason]
    WORKTREE_CHANGED: _ClassVar[WorktreeChangeReason]
    WORKTREE_DRAFT_PR_CREATED: _ClassVar[WorktreeChangeReason]
    WORKTREE_CI_UPDATED: _ClassVar[WorktreeChangeReason]
    WORKTREE_MERGED: _ClassVar[WorktreeChangeReason]
    WORKTREE_PR_STATE_CHANGED: _ClassVar[WorktreeChangeReason]

class WaitingReason(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    WAITING_REASON_UNSPECIFIED: _ClassVar[WaitingReason]
    WAITING_INTERACTIVE_STEP: _ClassVar[WaitingReason]
    WAITING_PR_LIMIT: _ClassVar[WaitingReason]

STIMULUS_KIND_UNSPECIFIED: StimulusKind
STIMULUS_ONCE: StimulusKind
STIMULUS_LOOP: StimulusKind
STIMULUS_WATCH: StimulusKind
STIMULUS_CRON: StimulusKind
WAVE_STATUS_UNSPECIFIED: WaveStatus
WAVE_IDLE: WaveStatus
WAVE_RUNNING: WaveStatus
WAVE_WAITING: WaveStatus
WAVE_ERROR: WaveStatus
MERGE_MODE_UNSPECIFIED: MergeMode
MERGE_PR: MergeMode
MERGE_LAND: MergeMode
FLOW_TYPE_UNSPECIFIED: FlowType
FLOW_YAML: FlowType
FLOW_PYTHON: FlowType
CI_STATE_UNSPECIFIED: CIState
CI_SUCCESS: CIState
CI_PENDING: CIState
CI_FAILURE: CIState
PR_STATE_UNSPECIFIED: PRState
PR_OPEN: PRState
PR_MERGED: PRState
PR_CLOSED: PRState
OPERATION_STATE_UNSPECIFIED: OperationState
OPERATION_REBASE: OperationState
OPERATION_MERGE: OperationState
STALENESS_UNSPECIFIED: Staleness
STALENESS_MERGED: Staleness
STALENESS_REMOTE_DELETED: Staleness
STEP_RUN_STATUS_UNSPECIFIED: StepRunStatus
STEP_RUNNING: StepRunStatus
STEP_WAITING: StepRunStatus
STEP_COMPLETED: StepRunStatus
STEP_FAILED: StepRunStatus
WORKTREE_CHANGE_REASON_UNSPECIFIED: WorktreeChangeReason
WORKTREE_COMMIT: WorktreeChangeReason
WORKTREE_CHECKOUT: WorktreeChangeReason
WORKTREE_CHANGED: WorktreeChangeReason
WORKTREE_DRAFT_PR_CREATED: WorktreeChangeReason
WORKTREE_CI_UPDATED: WorktreeChangeReason
WORKTREE_MERGED: WorktreeChangeReason
WORKTREE_PR_STATE_CHANGED: WorktreeChangeReason
WAITING_REASON_UNSPECIFIED: WaitingReason
WAITING_INTERACTIVE_STEP: WaitingReason
WAITING_PR_LIMIT: WaitingReason

class ProtocolVersion(_message.Message):
    __slots__ = ("major", "minor", "patch")
    MAJOR_FIELD_NUMBER: _ClassVar[int]
    MINOR_FIELD_NUMBER: _ClassVar[int]
    PATCH_FIELD_NUMBER: _ClassVar[int]
    major: int
    minor: int
    patch: int
    def __init__(
        self, major: _Optional[int] = ..., minor: _Optional[int] = ..., patch: _Optional[int] = ...
    ) -> None: ...

class ErrorDetail(_message.Message):
    __slots__ = (
        "code",
        "message",
        "retryable",
        "retry_after_seconds",
        "trace_id",
        "idempotency_key",
    )
    CODE_FIELD_NUMBER: _ClassVar[int]
    MESSAGE_FIELD_NUMBER: _ClassVar[int]
    RETRYABLE_FIELD_NUMBER: _ClassVar[int]
    RETRY_AFTER_SECONDS_FIELD_NUMBER: _ClassVar[int]
    TRACE_ID_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    code: str
    message: str
    retryable: bool
    retry_after_seconds: int
    trace_id: str
    idempotency_key: str
    def __init__(
        self,
        code: _Optional[str] = ...,
        message: _Optional[str] = ...,
        retryable: bool = ...,
        retry_after_seconds: _Optional[int] = ...,
        trace_id: _Optional[str] = ...,
        idempotency_key: _Optional[str] = ...,
    ) -> None: ...

class GetStatusRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class GetStatusResponse(_message.Message):
    __slots__ = ("pid", "waves_defined", "waves_running", "step_runs_active")
    PID_FIELD_NUMBER: _ClassVar[int]
    WAVES_DEFINED_FIELD_NUMBER: _ClassVar[int]
    WAVES_RUNNING_FIELD_NUMBER: _ClassVar[int]
    STEP_RUNS_ACTIVE_FIELD_NUMBER: _ClassVar[int]
    pid: int
    waves_defined: int
    waves_running: int
    step_runs_active: int
    def __init__(
        self,
        pid: _Optional[int] = ...,
        waves_defined: _Optional[int] = ...,
        waves_running: _Optional[int] = ...,
        step_runs_active: _Optional[int] = ...,
    ) -> None: ...

class GetHealthRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class GetHealthResponse(_message.Message):
    __slots__ = (
        "version",
        "schema_version",
        "uptime_seconds",
        "checks",
        "metrics",
        "protocol_version",
    )
    VERSION_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_VERSION_FIELD_NUMBER: _ClassVar[int]
    UPTIME_SECONDS_FIELD_NUMBER: _ClassVar[int]
    CHECKS_FIELD_NUMBER: _ClassVar[int]
    METRICS_FIELD_NUMBER: _ClassVar[int]
    PROTOCOL_VERSION_FIELD_NUMBER: _ClassVar[int]
    version: str
    schema_version: int
    uptime_seconds: int
    checks: HealthChecks
    metrics: HealthMetrics
    protocol_version: ProtocolVersion
    def __init__(
        self,
        version: _Optional[str] = ...,
        schema_version: _Optional[int] = ...,
        uptime_seconds: _Optional[int] = ...,
        checks: _Optional[_Union[HealthChecks, _Mapping]] = ...,
        metrics: _Optional[_Union[HealthMetrics, _Mapping]] = ...,
        protocol_version: _Optional[_Union[ProtocolVersion, _Mapping]] = ...,
    ) -> None: ...

class HealthChecks(_message.Message):
    __slots__ = ("database", "socket")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    SOCKET_FIELD_NUMBER: _ClassVar[int]
    database: bool
    socket: bool
    def __init__(self, database: bool = ..., socket: bool = ...) -> None: ...

class HealthMetrics(_message.Message):
    __slots__ = ("waves_total", "waves_running", "step_runs_active", "flow_runs_total")
    WAVES_TOTAL_FIELD_NUMBER: _ClassVar[int]
    WAVES_RUNNING_FIELD_NUMBER: _ClassVar[int]
    STEP_RUNS_ACTIVE_FIELD_NUMBER: _ClassVar[int]
    FLOW_RUNS_TOTAL_FIELD_NUMBER: _ClassVar[int]
    waves_total: int
    waves_running: int
    step_runs_active: int
    flow_runs_total: int
    def __init__(
        self,
        waves_total: _Optional[int] = ...,
        waves_running: _Optional[int] = ...,
        step_runs_active: _Optional[int] = ...,
        flow_runs_total: _Optional[int] = ...,
    ) -> None: ...

class Stimulus(_message.Message):
    __slots__ = ("kind", "cron")
    KIND_FIELD_NUMBER: _ClassVar[int]
    CRON_FIELD_NUMBER: _ClassVar[int]
    kind: StimulusKind
    cron: str
    def __init__(
        self, kind: _Optional[_Union[StimulusKind, str]] = ..., cron: _Optional[str] = ...
    ) -> None: ...

class Wave(_message.Message):
    __slots__ = (
        "id",
        "name",
        "repo",
        "flow",
        "direction",
        "area",
        "stimulus",
        "paused",
        "status",
        "iteration",
        "worktree",
        "branch",
        "pr_limit",
        "merge_mode",
        "pid",
        "created_at",
        "last_main_sha",
        "consecutive_failures",
        "pending_activations",
    )
    ID_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    REPO_FIELD_NUMBER: _ClassVar[int]
    FLOW_FIELD_NUMBER: _ClassVar[int]
    DIRECTION_FIELD_NUMBER: _ClassVar[int]
    AREA_FIELD_NUMBER: _ClassVar[int]
    STIMULUS_FIELD_NUMBER: _ClassVar[int]
    PAUSED_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    ITERATION_FIELD_NUMBER: _ClassVar[int]
    WORKTREE_FIELD_NUMBER: _ClassVar[int]
    BRANCH_FIELD_NUMBER: _ClassVar[int]
    PR_LIMIT_FIELD_NUMBER: _ClassVar[int]
    MERGE_MODE_FIELD_NUMBER: _ClassVar[int]
    PID_FIELD_NUMBER: _ClassVar[int]
    CREATED_AT_FIELD_NUMBER: _ClassVar[int]
    LAST_MAIN_SHA_FIELD_NUMBER: _ClassVar[int]
    CONSECUTIVE_FAILURES_FIELD_NUMBER: _ClassVar[int]
    PENDING_ACTIVATIONS_FIELD_NUMBER: _ClassVar[int]
    id: str
    name: str
    repo: str
    flow: str
    direction: _containers.RepeatedScalarFieldContainer[str]
    area: _containers.RepeatedScalarFieldContainer[str]
    stimulus: Stimulus
    paused: bool
    status: WaveStatus
    iteration: int
    worktree: str
    branch: str
    pr_limit: int
    merge_mode: MergeMode
    pid: int
    created_at: _timestamp_pb2.Timestamp
    last_main_sha: str
    consecutive_failures: int
    pending_activations: int
    def __init__(
        self,
        id: _Optional[str] = ...,
        name: _Optional[str] = ...,
        repo: _Optional[str] = ...,
        flow: _Optional[str] = ...,
        direction: _Optional[_Iterable[str]] = ...,
        area: _Optional[_Iterable[str]] = ...,
        stimulus: _Optional[_Union[Stimulus, _Mapping]] = ...,
        paused: bool = ...,
        status: _Optional[_Union[WaveStatus, str]] = ...,
        iteration: _Optional[int] = ...,
        worktree: _Optional[str] = ...,
        branch: _Optional[str] = ...,
        pr_limit: _Optional[int] = ...,
        merge_mode: _Optional[_Union[MergeMode, str]] = ...,
        pid: _Optional[int] = ...,
        created_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...,
        last_main_sha: _Optional[str] = ...,
        consecutive_failures: _Optional[int] = ...,
        pending_activations: _Optional[int] = ...,
    ) -> None: ...

class ListWavesRequest(_message.Message):
    __slots__ = ("repo",)
    REPO_FIELD_NUMBER: _ClassVar[int]
    repo: str
    def __init__(self, repo: _Optional[str] = ...) -> None: ...

class ListWavesResponse(_message.Message):
    __slots__ = ("waves",)
    WAVES_FIELD_NUMBER: _ClassVar[int]
    waves: _containers.RepeatedCompositeFieldContainer[Wave]
    def __init__(self, waves: _Optional[_Iterable[_Union[Wave, _Mapping]]] = ...) -> None: ...

class GetWaveRequest(_message.Message):
    __slots__ = ("wave_id",)
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    wave_id: str
    def __init__(self, wave_id: _Optional[str] = ...) -> None: ...

class GetWaveResponse(_message.Message):
    __slots__ = ("wave",)
    WAVE_FIELD_NUMBER: _ClassVar[int]
    wave: Wave
    def __init__(self, wave: _Optional[_Union[Wave, _Mapping]] = ...) -> None: ...

class CreateWaveRequest(_message.Message):
    __slots__ = ("repo", "name", "flow", "direction", "area", "idempotency_key")
    REPO_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    FLOW_FIELD_NUMBER: _ClassVar[int]
    DIRECTION_FIELD_NUMBER: _ClassVar[int]
    AREA_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    repo: str
    name: str
    flow: str
    direction: _containers.RepeatedScalarFieldContainer[str]
    area: _containers.RepeatedScalarFieldContainer[str]
    idempotency_key: str
    def __init__(
        self,
        repo: _Optional[str] = ...,
        name: _Optional[str] = ...,
        flow: _Optional[str] = ...,
        direction: _Optional[_Iterable[str]] = ...,
        area: _Optional[_Iterable[str]] = ...,
        idempotency_key: _Optional[str] = ...,
    ) -> None: ...

class CreateWaveResponse(_message.Message):
    __slots__ = ("wave",)
    WAVE_FIELD_NUMBER: _ClassVar[int]
    wave: Wave
    def __init__(self, wave: _Optional[_Union[Wave, _Mapping]] = ...) -> None: ...

class UpdateWaveRequest(_message.Message):
    __slots__ = ("wave_id", "flow", "direction", "area", "stimulus", "paused", "idempotency_key")
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    FLOW_FIELD_NUMBER: _ClassVar[int]
    DIRECTION_FIELD_NUMBER: _ClassVar[int]
    AREA_FIELD_NUMBER: _ClassVar[int]
    STIMULUS_FIELD_NUMBER: _ClassVar[int]
    PAUSED_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    wave_id: str
    flow: str
    direction: _containers.RepeatedScalarFieldContainer[str]
    area: _containers.RepeatedScalarFieldContainer[str]
    stimulus: Stimulus
    paused: bool
    idempotency_key: str
    def __init__(
        self,
        wave_id: _Optional[str] = ...,
        flow: _Optional[str] = ...,
        direction: _Optional[_Iterable[str]] = ...,
        area: _Optional[_Iterable[str]] = ...,
        stimulus: _Optional[_Union[Stimulus, _Mapping]] = ...,
        paused: bool = ...,
        idempotency_key: _Optional[str] = ...,
    ) -> None: ...

class UpdateWaveResponse(_message.Message):
    __slots__ = ("wave",)
    WAVE_FIELD_NUMBER: _ClassVar[int]
    wave: Wave
    def __init__(self, wave: _Optional[_Union[Wave, _Mapping]] = ...) -> None: ...

class DeleteWaveRequest(_message.Message):
    __slots__ = ("wave_id",)
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    wave_id: str
    def __init__(self, wave_id: _Optional[str] = ...) -> None: ...

class DeleteWaveResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class CloneWaveRequest(_message.Message):
    __slots__ = ("wave_id", "name", "idempotency_key")
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    wave_id: str
    name: str
    idempotency_key: str
    def __init__(
        self,
        wave_id: _Optional[str] = ...,
        name: _Optional[str] = ...,
        idempotency_key: _Optional[str] = ...,
    ) -> None: ...

class CloneWaveResponse(_message.Message):
    __slots__ = ("wave",)
    WAVE_FIELD_NUMBER: _ClassVar[int]
    wave: Wave
    def __init__(self, wave: _Optional[_Union[Wave, _Mapping]] = ...) -> None: ...

class RunWaveRequest(_message.Message):
    __slots__ = ("wave_id", "area", "direction", "flow", "stimulus", "idempotency_key")
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    AREA_FIELD_NUMBER: _ClassVar[int]
    DIRECTION_FIELD_NUMBER: _ClassVar[int]
    FLOW_FIELD_NUMBER: _ClassVar[int]
    STIMULUS_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    wave_id: str
    area: _containers.RepeatedScalarFieldContainer[str]
    direction: _containers.RepeatedScalarFieldContainer[str]
    flow: str
    stimulus: Stimulus
    idempotency_key: str
    def __init__(
        self,
        wave_id: _Optional[str] = ...,
        area: _Optional[_Iterable[str]] = ...,
        direction: _Optional[_Iterable[str]] = ...,
        flow: _Optional[str] = ...,
        stimulus: _Optional[_Union[Stimulus, _Mapping]] = ...,
        idempotency_key: _Optional[str] = ...,
    ) -> None: ...

class RunWaveResponse(_message.Message):
    __slots__ = ("started", "wave_id")
    STARTED_FIELD_NUMBER: _ClassVar[int]
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    started: bool
    wave_id: str
    def __init__(self, started: bool = ..., wave_id: _Optional[str] = ...) -> None: ...

class StopWaveRequest(_message.Message):
    __slots__ = ("wave_id", "force")
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    FORCE_FIELD_NUMBER: _ClassVar[int]
    wave_id: str
    force: bool
    def __init__(self, wave_id: _Optional[str] = ..., force: bool = ...) -> None: ...

class StopWaveResponse(_message.Message):
    __slots__ = ("stopped",)
    STOPPED_FIELD_NUMBER: _ClassVar[int]
    stopped: bool
    def __init__(self, stopped: bool = ...) -> None: ...

class ConnectWaveRequest(_message.Message):
    __slots__ = ("wave_id",)
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    wave_id: str
    def __init__(self, wave_id: _Optional[str] = ...) -> None: ...

class ConnectWaveResponse(_message.Message):
    __slots__ = ("worktree", "step", "step_run_id", "prompt_file", "flow_run_id", "step_index")
    WORKTREE_FIELD_NUMBER: _ClassVar[int]
    STEP_FIELD_NUMBER: _ClassVar[int]
    STEP_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    PROMPT_FILE_FIELD_NUMBER: _ClassVar[int]
    FLOW_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    STEP_INDEX_FIELD_NUMBER: _ClassVar[int]
    worktree: str
    step: str
    step_run_id: str
    prompt_file: str
    flow_run_id: str
    step_index: int
    def __init__(
        self,
        worktree: _Optional[str] = ...,
        step: _Optional[str] = ...,
        step_run_id: _Optional[str] = ...,
        prompt_file: _Optional[str] = ...,
        flow_run_id: _Optional[str] = ...,
        step_index: _Optional[int] = ...,
    ) -> None: ...

class FlowInfo(_message.Message):
    __slots__ = ("name", "type", "steps")
    NAME_FIELD_NUMBER: _ClassVar[int]
    TYPE_FIELD_NUMBER: _ClassVar[int]
    STEPS_FIELD_NUMBER: _ClassVar[int]
    name: str
    type: FlowType
    steps: _containers.RepeatedScalarFieldContainer[str]
    def __init__(
        self,
        name: _Optional[str] = ...,
        type: _Optional[_Union[FlowType, str]] = ...,
        steps: _Optional[_Iterable[str]] = ...,
    ) -> None: ...

class StepInfo(_message.Message):
    __slots__ = ("name", "type")
    NAME_FIELD_NUMBER: _ClassVar[int]
    TYPE_FIELD_NUMBER: _ClassVar[int]
    name: str
    type: str
    def __init__(self, name: _Optional[str] = ..., type: _Optional[str] = ...) -> None: ...

class ListFlowsRequest(_message.Message):
    __slots__ = ("repo",)
    REPO_FIELD_NUMBER: _ClassVar[int]
    repo: str
    def __init__(self, repo: _Optional[str] = ...) -> None: ...

class ListFlowsResponse(_message.Message):
    __slots__ = ("flows", "steps")
    FLOWS_FIELD_NUMBER: _ClassVar[int]
    STEPS_FIELD_NUMBER: _ClassVar[int]
    flows: _containers.RepeatedCompositeFieldContainer[FlowInfo]
    steps: _containers.RepeatedCompositeFieldContainer[StepInfo]
    def __init__(
        self,
        flows: _Optional[_Iterable[_Union[FlowInfo, _Mapping]]] = ...,
        steps: _Optional[_Iterable[_Union[StepInfo, _Mapping]]] = ...,
    ) -> None: ...

class WorkingTreeStatus(_message.Message):
    __slots__ = ("staged", "modified", "untracked", "diff_added", "diff_deleted")
    STAGED_FIELD_NUMBER: _ClassVar[int]
    MODIFIED_FIELD_NUMBER: _ClassVar[int]
    UNTRACKED_FIELD_NUMBER: _ClassVar[int]
    DIFF_ADDED_FIELD_NUMBER: _ClassVar[int]
    DIFF_DELETED_FIELD_NUMBER: _ClassVar[int]
    staged: bool
    modified: bool
    untracked: bool
    diff_added: int
    diff_deleted: int
    def __init__(
        self,
        staged: bool = ...,
        modified: bool = ...,
        untracked: bool = ...,
        diff_added: _Optional[int] = ...,
        diff_deleted: _Optional[int] = ...,
    ) -> None: ...

class MainStatus(_message.Message):
    __slots__ = ("ahead", "behind")
    AHEAD_FIELD_NUMBER: _ClassVar[int]
    BEHIND_FIELD_NUMBER: _ClassVar[int]
    ahead: int
    behind: int
    def __init__(self, ahead: _Optional[int] = ..., behind: _Optional[int] = ...) -> None: ...

class RemoteStatus(_message.Message):
    __slots__ = ("name", "ahead", "behind")
    NAME_FIELD_NUMBER: _ClassVar[int]
    AHEAD_FIELD_NUMBER: _ClassVar[int]
    BEHIND_FIELD_NUMBER: _ClassVar[int]
    name: str
    ahead: int
    behind: int
    def __init__(
        self, name: _Optional[str] = ..., ahead: _Optional[int] = ..., behind: _Optional[int] = ...
    ) -> None: ...

class CIStatus(_message.Message):
    __slots__ = ("source", "url", "state", "ci_state")
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    URL_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    CI_STATE_FIELD_NUMBER: _ClassVar[int]
    source: str
    url: str
    state: PRState
    ci_state: CIState
    def __init__(
        self,
        source: _Optional[str] = ...,
        url: _Optional[str] = ...,
        state: _Optional[_Union[PRState, str]] = ...,
        ci_state: _Optional[_Union[CIState, str]] = ...,
    ) -> None: ...

class RecentStepRun(_message.Message):
    __slots__ = ("id", "step", "status", "started_at", "ended_at")
    ID_FIELD_NUMBER: _ClassVar[int]
    STEP_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    STARTED_AT_FIELD_NUMBER: _ClassVar[int]
    ENDED_AT_FIELD_NUMBER: _ClassVar[int]
    id: str
    step: str
    status: StepRunStatus
    started_at: _timestamp_pb2.Timestamp
    ended_at: _timestamp_pb2.Timestamp
    def __init__(
        self,
        id: _Optional[str] = ...,
        step: _Optional[str] = ...,
        status: _Optional[_Union[StepRunStatus, str]] = ...,
        started_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...,
        ended_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...,
    ) -> None: ...

class WorktreeState(_message.Message):
    __slots__ = (
        "branch",
        "path",
        "base_branch",
        "working_tree",
        "main",
        "main_state",
        "remote",
        "operation_state",
        "ci",
        "prunable",
        "staleness",
        "staleness_days",
        "recent_steps",
    )
    BRANCH_FIELD_NUMBER: _ClassVar[int]
    PATH_FIELD_NUMBER: _ClassVar[int]
    BASE_BRANCH_FIELD_NUMBER: _ClassVar[int]
    WORKING_TREE_FIELD_NUMBER: _ClassVar[int]
    MAIN_FIELD_NUMBER: _ClassVar[int]
    MAIN_STATE_FIELD_NUMBER: _ClassVar[int]
    REMOTE_FIELD_NUMBER: _ClassVar[int]
    OPERATION_STATE_FIELD_NUMBER: _ClassVar[int]
    CI_FIELD_NUMBER: _ClassVar[int]
    PRUNABLE_FIELD_NUMBER: _ClassVar[int]
    STALENESS_FIELD_NUMBER: _ClassVar[int]
    STALENESS_DAYS_FIELD_NUMBER: _ClassVar[int]
    RECENT_STEPS_FIELD_NUMBER: _ClassVar[int]
    branch: str
    path: str
    base_branch: str
    working_tree: WorkingTreeStatus
    main: MainStatus
    main_state: str
    remote: RemoteStatus
    operation_state: OperationState
    ci: CIStatus
    prunable: bool
    staleness: Staleness
    staleness_days: int
    recent_steps: _containers.RepeatedCompositeFieldContainer[RecentStepRun]
    def __init__(
        self,
        branch: _Optional[str] = ...,
        path: _Optional[str] = ...,
        base_branch: _Optional[str] = ...,
        working_tree: _Optional[_Union[WorkingTreeStatus, _Mapping]] = ...,
        main: _Optional[_Union[MainStatus, _Mapping]] = ...,
        main_state: _Optional[str] = ...,
        remote: _Optional[_Union[RemoteStatus, _Mapping]] = ...,
        operation_state: _Optional[_Union[OperationState, str]] = ...,
        ci: _Optional[_Union[CIStatus, _Mapping]] = ...,
        prunable: bool = ...,
        staleness: _Optional[_Union[Staleness, str]] = ...,
        staleness_days: _Optional[int] = ...,
        recent_steps: _Optional[_Iterable[_Union[RecentStepRun, _Mapping]]] = ...,
    ) -> None: ...

class ListWorktreesRequest(_message.Message):
    __slots__ = ("repo",)
    REPO_FIELD_NUMBER: _ClassVar[int]
    repo: str
    def __init__(self, repo: _Optional[str] = ...) -> None: ...

class ListWorktreesResponse(_message.Message):
    __slots__ = ("worktrees",)
    WORKTREES_FIELD_NUMBER: _ClassVar[int]
    worktrees: _containers.RepeatedCompositeFieldContainer[WorktreeState]
    def __init__(
        self, worktrees: _Optional[_Iterable[_Union[WorktreeState, _Mapping]]] = ...
    ) -> None: ...

class NotifyWorktreeChangedRequest(_message.Message):
    __slots__ = ("repo", "branch", "reason")
    REPO_FIELD_NUMBER: _ClassVar[int]
    BRANCH_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    repo: str
    branch: str
    reason: str
    def __init__(
        self, repo: _Optional[str] = ..., branch: _Optional[str] = ..., reason: _Optional[str] = ...
    ) -> None: ...

class NotifyWorktreeChangedResponse(_message.Message):
    __slots__ = ("branch", "reason")
    BRANCH_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    branch: str
    reason: str
    def __init__(self, branch: _Optional[str] = ..., reason: _Optional[str] = ...) -> None: ...

class GetSchedulerStatusRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class GetSchedulerStatusResponse(_message.Message):
    __slots__ = ("slots_used", "slots_total", "outstanding", "outstanding_limit", "running")
    SLOTS_USED_FIELD_NUMBER: _ClassVar[int]
    SLOTS_TOTAL_FIELD_NUMBER: _ClassVar[int]
    OUTSTANDING_FIELD_NUMBER: _ClassVar[int]
    OUTSTANDING_LIMIT_FIELD_NUMBER: _ClassVar[int]
    RUNNING_FIELD_NUMBER: _ClassVar[int]
    slots_used: int
    slots_total: int
    outstanding: int
    outstanding_limit: int
    running: _containers.RepeatedScalarFieldContainer[str]
    def __init__(
        self,
        slots_used: _Optional[int] = ...,
        slots_total: _Optional[int] = ...,
        outstanding: _Optional[int] = ...,
        outstanding_limit: _Optional[int] = ...,
        running: _Optional[_Iterable[str]] = ...,
    ) -> None: ...

class AcquireSlotRequest(_message.Message):
    __slots__ = ("run_id",)
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    run_id: str
    def __init__(self, run_id: _Optional[str] = ...) -> None: ...

class AcquireSlotResponse(_message.Message):
    __slots__ = ("acquired", "reason", "slots_used")
    ACQUIRED_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    SLOTS_USED_FIELD_NUMBER: _ClassVar[int]
    acquired: bool
    reason: str
    slots_used: int
    def __init__(
        self, acquired: bool = ..., reason: _Optional[str] = ..., slots_used: _Optional[int] = ...
    ) -> None: ...

class ReleaseSlotRequest(_message.Message):
    __slots__ = ("run_id",)
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    run_id: str
    def __init__(self, run_id: _Optional[str] = ...) -> None: ...

class ReleaseSlotResponse(_message.Message):
    __slots__ = ("slots_used",)
    SLOTS_USED_FIELD_NUMBER: _ClassVar[int]
    slots_used: int
    def __init__(self, slots_used: _Optional[int] = ...) -> None: ...

class StepRun(_message.Message):
    __slots__ = (
        "id",
        "step",
        "repo",
        "worktree",
        "flow_run_id",
        "wave_id",
        "status",
        "started_at",
        "ended_at",
        "pid",
        "model",
        "run_mode",
    )
    ID_FIELD_NUMBER: _ClassVar[int]
    STEP_FIELD_NUMBER: _ClassVar[int]
    REPO_FIELD_NUMBER: _ClassVar[int]
    WORKTREE_FIELD_NUMBER: _ClassVar[int]
    FLOW_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    STARTED_AT_FIELD_NUMBER: _ClassVar[int]
    ENDED_AT_FIELD_NUMBER: _ClassVar[int]
    PID_FIELD_NUMBER: _ClassVar[int]
    MODEL_FIELD_NUMBER: _ClassVar[int]
    RUN_MODE_FIELD_NUMBER: _ClassVar[int]
    id: str
    step: str
    repo: str
    worktree: str
    flow_run_id: str
    wave_id: str
    status: StepRunStatus
    started_at: _timestamp_pb2.Timestamp
    ended_at: _timestamp_pb2.Timestamp
    pid: int
    model: str
    run_mode: str
    def __init__(
        self,
        id: _Optional[str] = ...,
        step: _Optional[str] = ...,
        repo: _Optional[str] = ...,
        worktree: _Optional[str] = ...,
        flow_run_id: _Optional[str] = ...,
        wave_id: _Optional[str] = ...,
        status: _Optional[_Union[StepRunStatus, str]] = ...,
        started_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...,
        ended_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...,
        pid: _Optional[int] = ...,
        model: _Optional[str] = ...,
        run_mode: _Optional[str] = ...,
    ) -> None: ...

class ListStepRunsRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ListStepRunsResponse(_message.Message):
    __slots__ = ("step_runs",)
    STEP_RUNS_FIELD_NUMBER: _ClassVar[int]
    step_runs: _containers.RepeatedCompositeFieldContainer[StepRun]
    def __init__(
        self, step_runs: _Optional[_Iterable[_Union[StepRun, _Mapping]]] = ...
    ) -> None: ...

class GetStepRunHistoryRequest(_message.Message):
    __slots__ = ("worktree", "repo", "limit")
    WORKTREE_FIELD_NUMBER: _ClassVar[int]
    REPO_FIELD_NUMBER: _ClassVar[int]
    LIMIT_FIELD_NUMBER: _ClassVar[int]
    worktree: str
    repo: str
    limit: int
    def __init__(
        self,
        worktree: _Optional[str] = ...,
        repo: _Optional[str] = ...,
        limit: _Optional[int] = ...,
    ) -> None: ...

class GetStepRunHistoryResponse(_message.Message):
    __slots__ = ("step_runs",)
    STEP_RUNS_FIELD_NUMBER: _ClassVar[int]
    step_runs: _containers.RepeatedCompositeFieldContainer[StepRun]
    def __init__(
        self, step_runs: _Optional[_Iterable[_Union[StepRun, _Mapping]]] = ...
    ) -> None: ...

class StartStepRunRequest(_message.Message):
    __slots__ = ("step_run",)
    STEP_RUN_FIELD_NUMBER: _ClassVar[int]
    step_run: StepRun
    def __init__(self, step_run: _Optional[_Union[StepRun, _Mapping]] = ...) -> None: ...

class StartStepRunResponse(_message.Message):
    __slots__ = ("id",)
    ID_FIELD_NUMBER: _ClassVar[int]
    id: str
    def __init__(self, id: _Optional[str] = ...) -> None: ...

class EndStepRunRequest(_message.Message):
    __slots__ = ("step_run_id", "status")
    STEP_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    step_run_id: str
    status: StepRunStatus
    def __init__(
        self, step_run_id: _Optional[str] = ..., status: _Optional[_Union[StepRunStatus, str]] = ...
    ) -> None: ...

class EndStepRunResponse(_message.Message):
    __slots__ = ("id",)
    ID_FIELD_NUMBER: _ClassVar[int]
    id: str
    def __init__(self, id: _Optional[str] = ...) -> None: ...

class SubscribeRequest(_message.Message):
    __slots__ = ("patterns",)
    PATTERNS_FIELD_NUMBER: _ClassVar[int]
    patterns: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, patterns: _Optional[_Iterable[str]] = ...) -> None: ...

class Event(_message.Message):
    __slots__ = (
        "event",
        "timestamp",
        "session_started",
        "session_ended",
        "output_line",
        "worktree_updated",
        "worktree_pruned",
        "wave_created",
        "wave_updated",
        "wave_deleted",
        "wave_started",
        "wave_stopped",
        "wave_activated",
        "wave_waiting",
        "scheduler_slot_acquired",
        "scheduler_slot_released",
    )
    EVENT_FIELD_NUMBER: _ClassVar[int]
    TIMESTAMP_FIELD_NUMBER: _ClassVar[int]
    SESSION_STARTED_FIELD_NUMBER: _ClassVar[int]
    SESSION_ENDED_FIELD_NUMBER: _ClassVar[int]
    OUTPUT_LINE_FIELD_NUMBER: _ClassVar[int]
    WORKTREE_UPDATED_FIELD_NUMBER: _ClassVar[int]
    WORKTREE_PRUNED_FIELD_NUMBER: _ClassVar[int]
    WAVE_CREATED_FIELD_NUMBER: _ClassVar[int]
    WAVE_UPDATED_FIELD_NUMBER: _ClassVar[int]
    WAVE_DELETED_FIELD_NUMBER: _ClassVar[int]
    WAVE_STARTED_FIELD_NUMBER: _ClassVar[int]
    WAVE_STOPPED_FIELD_NUMBER: _ClassVar[int]
    WAVE_ACTIVATED_FIELD_NUMBER: _ClassVar[int]
    WAVE_WAITING_FIELD_NUMBER: _ClassVar[int]
    SCHEDULER_SLOT_ACQUIRED_FIELD_NUMBER: _ClassVar[int]
    SCHEDULER_SLOT_RELEASED_FIELD_NUMBER: _ClassVar[int]
    event: str
    timestamp: _timestamp_pb2.Timestamp
    session_started: SessionStartedEvent
    session_ended: SessionEndedEvent
    output_line: OutputLineEvent
    worktree_updated: WorktreeUpdatedEvent
    worktree_pruned: WorktreePrunedEvent
    wave_created: WaveCreatedEvent
    wave_updated: WaveUpdatedEvent
    wave_deleted: WaveDeletedEvent
    wave_started: WaveStartedEvent
    wave_stopped: WaveStoppedEvent
    wave_activated: WaveActivatedEvent
    wave_waiting: WaveWaitingEvent
    scheduler_slot_acquired: SchedulerSlotAcquiredEvent
    scheduler_slot_released: SchedulerSlotReleasedEvent
    def __init__(
        self,
        event: _Optional[str] = ...,
        timestamp: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...,
        session_started: _Optional[_Union[SessionStartedEvent, _Mapping]] = ...,
        session_ended: _Optional[_Union[SessionEndedEvent, _Mapping]] = ...,
        output_line: _Optional[_Union[OutputLineEvent, _Mapping]] = ...,
        worktree_updated: _Optional[_Union[WorktreeUpdatedEvent, _Mapping]] = ...,
        worktree_pruned: _Optional[_Union[WorktreePrunedEvent, _Mapping]] = ...,
        wave_created: _Optional[_Union[WaveCreatedEvent, _Mapping]] = ...,
        wave_updated: _Optional[_Union[WaveUpdatedEvent, _Mapping]] = ...,
        wave_deleted: _Optional[_Union[WaveDeletedEvent, _Mapping]] = ...,
        wave_started: _Optional[_Union[WaveStartedEvent, _Mapping]] = ...,
        wave_stopped: _Optional[_Union[WaveStoppedEvent, _Mapping]] = ...,
        wave_activated: _Optional[_Union[WaveActivatedEvent, _Mapping]] = ...,
        wave_waiting: _Optional[_Union[WaveWaitingEvent, _Mapping]] = ...,
        scheduler_slot_acquired: _Optional[_Union[SchedulerSlotAcquiredEvent, _Mapping]] = ...,
        scheduler_slot_released: _Optional[_Union[SchedulerSlotReleasedEvent, _Mapping]] = ...,
    ) -> None: ...

class SessionStartedEvent(_message.Message):
    __slots__ = ("id", "step", "worktree", "wave_id", "flow_run_id", "step_index")
    ID_FIELD_NUMBER: _ClassVar[int]
    STEP_FIELD_NUMBER: _ClassVar[int]
    WORKTREE_FIELD_NUMBER: _ClassVar[int]
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    FLOW_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    STEP_INDEX_FIELD_NUMBER: _ClassVar[int]
    id: str
    step: str
    worktree: str
    wave_id: str
    flow_run_id: str
    step_index: int
    def __init__(
        self,
        id: _Optional[str] = ...,
        step: _Optional[str] = ...,
        worktree: _Optional[str] = ...,
        wave_id: _Optional[str] = ...,
        flow_run_id: _Optional[str] = ...,
        step_index: _Optional[int] = ...,
    ) -> None: ...

class SessionEndedEvent(_message.Message):
    __slots__ = ("id", "status", "wave_id", "flow_run_id", "flow_will_continue")
    ID_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    FLOW_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    FLOW_WILL_CONTINUE_FIELD_NUMBER: _ClassVar[int]
    id: str
    status: str
    wave_id: str
    flow_run_id: str
    flow_will_continue: bool
    def __init__(
        self,
        id: _Optional[str] = ...,
        status: _Optional[str] = ...,
        wave_id: _Optional[str] = ...,
        flow_run_id: _Optional[str] = ...,
        flow_will_continue: bool = ...,
    ) -> None: ...

class OutputLineEvent(_message.Message):
    __slots__ = ("session_id", "text")
    SESSION_ID_FIELD_NUMBER: _ClassVar[int]
    TEXT_FIELD_NUMBER: _ClassVar[int]
    session_id: str
    text: str
    def __init__(self, session_id: _Optional[str] = ..., text: _Optional[str] = ...) -> None: ...

class WorktreeUpdatedEvent(_message.Message):
    __slots__ = ("branch", "reason", "repo", "worktree", "changes")
    BRANCH_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    REPO_FIELD_NUMBER: _ClassVar[int]
    WORKTREE_FIELD_NUMBER: _ClassVar[int]
    CHANGES_FIELD_NUMBER: _ClassVar[int]
    branch: str
    reason: WorktreeChangeReason
    repo: str
    worktree: WorktreeState
    changes: WorktreeChanges
    def __init__(
        self,
        branch: _Optional[str] = ...,
        reason: _Optional[_Union[WorktreeChangeReason, str]] = ...,
        repo: _Optional[str] = ...,
        worktree: _Optional[_Union[WorktreeState, _Mapping]] = ...,
        changes: _Optional[_Union[WorktreeChanges, _Mapping]] = ...,
    ) -> None: ...

class WorktreeChanges(_message.Message):
    __slots__ = ("ci_state", "pr_state")
    CI_STATE_FIELD_NUMBER: _ClassVar[int]
    PR_STATE_FIELD_NUMBER: _ClassVar[int]
    ci_state: CIState
    pr_state: PRState
    def __init__(
        self,
        ci_state: _Optional[_Union[CIState, str]] = ...,
        pr_state: _Optional[_Union[PRState, str]] = ...,
    ) -> None: ...

class WorktreePrunedEvent(_message.Message):
    __slots__ = ("branch", "repo")
    BRANCH_FIELD_NUMBER: _ClassVar[int]
    REPO_FIELD_NUMBER: _ClassVar[int]
    branch: str
    repo: str
    def __init__(self, branch: _Optional[str] = ..., repo: _Optional[str] = ...) -> None: ...

class WaveCreatedEvent(_message.Message):
    __slots__ = ("wave_id", "name")
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    wave_id: str
    name: str
    def __init__(self, wave_id: _Optional[str] = ..., name: _Optional[str] = ...) -> None: ...

class WaveUpdatedEvent(_message.Message):
    __slots__ = ("wave_id",)
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    wave_id: str
    def __init__(self, wave_id: _Optional[str] = ...) -> None: ...

class WaveDeletedEvent(_message.Message):
    __slots__ = ("wave_id",)
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    wave_id: str
    def __init__(self, wave_id: _Optional[str] = ...) -> None: ...

class WaveStartedEvent(_message.Message):
    __slots__ = ("wave_id",)
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    wave_id: str
    def __init__(self, wave_id: _Optional[str] = ...) -> None: ...

class WaveStoppedEvent(_message.Message):
    __slots__ = ("wave_id",)
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    wave_id: str
    def __init__(self, wave_id: _Optional[str] = ...) -> None: ...

class WaveActivatedEvent(_message.Message):
    __slots__ = ("wave_id", "stimulus")
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    STIMULUS_FIELD_NUMBER: _ClassVar[int]
    wave_id: str
    stimulus: StimulusKind
    def __init__(
        self, wave_id: _Optional[str] = ..., stimulus: _Optional[_Union[StimulusKind, str]] = ...
    ) -> None: ...

class WaveWaitingEvent(_message.Message):
    __slots__ = ("wave_id", "step", "step_run_id", "flow_run_id", "step_index", "reason")
    WAVE_ID_FIELD_NUMBER: _ClassVar[int]
    STEP_FIELD_NUMBER: _ClassVar[int]
    STEP_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    FLOW_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    STEP_INDEX_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    wave_id: str
    step: str
    step_run_id: str
    flow_run_id: str
    step_index: int
    reason: WaitingReason
    def __init__(
        self,
        wave_id: _Optional[str] = ...,
        step: _Optional[str] = ...,
        step_run_id: _Optional[str] = ...,
        flow_run_id: _Optional[str] = ...,
        step_index: _Optional[int] = ...,
        reason: _Optional[_Union[WaitingReason, str]] = ...,
    ) -> None: ...

class SchedulerSlotAcquiredEvent(_message.Message):
    __slots__ = ("run_id", "slots_used")
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    SLOTS_USED_FIELD_NUMBER: _ClassVar[int]
    run_id: str
    slots_used: int
    def __init__(self, run_id: _Optional[str] = ..., slots_used: _Optional[int] = ...) -> None: ...

class SchedulerSlotReleasedEvent(_message.Message):
    __slots__ = ("run_id", "slots_used")
    RUN_ID_FIELD_NUMBER: _ClassVar[int]
    SLOTS_USED_FIELD_NUMBER: _ClassVar[int]
    run_id: str
    slots_used: int
    def __init__(self, run_id: _Optional[str] = ..., slots_used: _Optional[int] = ...) -> None: ...

class NotifyRequest(_message.Message):
    __slots__ = ("event", "data")
    EVENT_FIELD_NUMBER: _ClassVar[int]
    DATA_FIELD_NUMBER: _ClassVar[int]
    event: str
    data: bytes
    def __init__(self, event: _Optional[str] = ..., data: _Optional[bytes] = ...) -> None: ...

class NotifyResponse(_message.Message):
    __slots__ = ("event",)
    EVENT_FIELD_NUMBER: _ClassVar[int]
    event: str
    def __init__(self, event: _Optional[str] = ...) -> None: ...

class StreamOutputRequest(_message.Message):
    __slots__ = ("step_run_id", "text")
    STEP_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    TEXT_FIELD_NUMBER: _ClassVar[int]
    step_run_id: str
    text: str
    def __init__(self, step_run_id: _Optional[str] = ..., text: _Optional[str] = ...) -> None: ...

class StreamOutputResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...
