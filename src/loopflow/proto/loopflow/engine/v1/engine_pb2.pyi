import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class DiffMode(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    DIFF_MODE_UNSPECIFIED: _ClassVar[DiffMode]
    DIFF_FILES: _ClassVar[DiffMode]
    DIFF_RAW: _ClassVar[DiffMode]
    DIFF_NONE: _ClassVar[DiffMode]

class StepEventType(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    STEP_EVENT_TYPE_UNSPECIFIED: _ClassVar[StepEventType]
    STEP_STARTED: _ClassVar[StepEventType]
    STEP_OUTPUT: _ClassVar[StepEventType]
    STEP_COMPLETED: _ClassVar[StepEventType]
    STEP_FAILED: _ClassVar[StepEventType]
    STEP_COMMITTED: _ClassVar[StepEventType]
    STEP_PUSHED: _ClassVar[StepEventType]

class FlowEventType(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    FLOW_EVENT_TYPE_UNSPECIFIED: _ClassVar[FlowEventType]
    FLOW_STARTED: _ClassVar[FlowEventType]
    FLOW_STEP_STARTED: _ClassVar[FlowEventType]
    FLOW_STEP_COMPLETED: _ClassVar[FlowEventType]
    FLOW_STEP_FAILED: _ClassVar[FlowEventType]
    FLOW_FORK_STARTED: _ClassVar[FlowEventType]
    FLOW_FORK_THREAD_COMPLETED: _ClassVar[FlowEventType]
    FLOW_FORK_COMPLETED: _ClassVar[FlowEventType]
    FLOW_SYNTHESIZE_STARTED: _ClassVar[FlowEventType]
    FLOW_SYNTHESIZE_COMPLETED: _ClassVar[FlowEventType]
    FLOW_CHOOSE_STARTED: _ClassVar[FlowEventType]
    FLOW_CHOOSE_DECIDED: _ClassVar[FlowEventType]
    FLOW_COMPLETED: _ClassVar[FlowEventType]
    FLOW_FAILED: _ClassVar[FlowEventType]
    FLOW_COMMITTED: _ClassVar[FlowEventType]
    FLOW_PR_CREATED: _ClassVar[FlowEventType]
    FLOW_WAITING: _ClassVar[FlowEventType]

class FlowRunStatus(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    FLOW_RUN_STATUS_UNSPECIFIED: _ClassVar[FlowRunStatus]
    FLOW_RUN_RUNNING: _ClassVar[FlowRunStatus]
    FLOW_RUN_WAITING: _ClassVar[FlowRunStatus]
    FLOW_RUN_COMPLETED: _ClassVar[FlowRunStatus]
    FLOW_RUN_FAILED: _ClassVar[FlowRunStatus]

class TickResult(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    TICK_RESULT_UNSPECIFIED: _ClassVar[TickResult]
    TICK_STEP_COMPLETE: _ClassVar[TickResult]
    TICK_FLOW_COMPLETE: _ClassVar[TickResult]
    TICK_WAITING_INTERACTIVE: _ClassVar[TickResult]
    TICK_STEP_FAILED: _ClassVar[TickResult]

class ForkEventType(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    FORK_EVENT_TYPE_UNSPECIFIED: _ClassVar[ForkEventType]
    FORK_STARTED: _ClassVar[ForkEventType]
    FORK_THREAD_STARTED: _ClassVar[ForkEventType]
    FORK_THREAD_OUTPUT: _ClassVar[ForkEventType]
    FORK_THREAD_COMPLETED: _ClassVar[ForkEventType]
    FORK_THREAD_FAILED: _ClassVar[ForkEventType]
    FORK_COMPLETED: _ClassVar[ForkEventType]

class SynthesizeEventType(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    SYNTHESIZE_EVENT_TYPE_UNSPECIFIED: _ClassVar[SynthesizeEventType]
    SYNTHESIZE_STARTED: _ClassVar[SynthesizeEventType]
    SYNTHESIZE_OUTPUT: _ClassVar[SynthesizeEventType]
    SYNTHESIZE_COMPLETED: _ClassVar[SynthesizeEventType]
    SYNTHESIZE_FAILED: _ClassVar[SynthesizeEventType]

DIFF_MODE_UNSPECIFIED: DiffMode
DIFF_FILES: DiffMode
DIFF_RAW: DiffMode
DIFF_NONE: DiffMode
STEP_EVENT_TYPE_UNSPECIFIED: StepEventType
STEP_STARTED: StepEventType
STEP_OUTPUT: StepEventType
STEP_COMPLETED: StepEventType
STEP_FAILED: StepEventType
STEP_COMMITTED: StepEventType
STEP_PUSHED: StepEventType
FLOW_EVENT_TYPE_UNSPECIFIED: FlowEventType
FLOW_STARTED: FlowEventType
FLOW_STEP_STARTED: FlowEventType
FLOW_STEP_COMPLETED: FlowEventType
FLOW_STEP_FAILED: FlowEventType
FLOW_FORK_STARTED: FlowEventType
FLOW_FORK_THREAD_COMPLETED: FlowEventType
FLOW_FORK_COMPLETED: FlowEventType
FLOW_SYNTHESIZE_STARTED: FlowEventType
FLOW_SYNTHESIZE_COMPLETED: FlowEventType
FLOW_CHOOSE_STARTED: FlowEventType
FLOW_CHOOSE_DECIDED: FlowEventType
FLOW_COMPLETED: FlowEventType
FLOW_FAILED: FlowEventType
FLOW_COMMITTED: FlowEventType
FLOW_PR_CREATED: FlowEventType
FLOW_WAITING: FlowEventType
FLOW_RUN_STATUS_UNSPECIFIED: FlowRunStatus
FLOW_RUN_RUNNING: FlowRunStatus
FLOW_RUN_WAITING: FlowRunStatus
FLOW_RUN_COMPLETED: FlowRunStatus
FLOW_RUN_FAILED: FlowRunStatus
TICK_RESULT_UNSPECIFIED: TickResult
TICK_STEP_COMPLETE: TickResult
TICK_FLOW_COMPLETE: TickResult
TICK_WAITING_INTERACTIVE: TickResult
TICK_STEP_FAILED: TickResult
FORK_EVENT_TYPE_UNSPECIFIED: ForkEventType
FORK_STARTED: ForkEventType
FORK_THREAD_STARTED: ForkEventType
FORK_THREAD_OUTPUT: ForkEventType
FORK_THREAD_COMPLETED: ForkEventType
FORK_THREAD_FAILED: ForkEventType
FORK_COMPLETED: ForkEventType
SYNTHESIZE_EVENT_TYPE_UNSPECIFIED: SynthesizeEventType
SYNTHESIZE_STARTED: SynthesizeEventType
SYNTHESIZE_OUTPUT: SynthesizeEventType
SYNTHESIZE_COMPLETED: SynthesizeEventType
SYNTHESIZE_FAILED: SynthesizeEventType

class ClipboardContent(_message.Message):
    __slots__ = ("text", "image", "image_mime_type")
    TEXT_FIELD_NUMBER: _ClassVar[int]
    IMAGE_FIELD_NUMBER: _ClassVar[int]
    IMAGE_MIME_TYPE_FIELD_NUMBER: _ClassVar[int]
    text: str
    image: bytes
    image_mime_type: str
    def __init__(
        self,
        text: _Optional[str] = ...,
        image: _Optional[bytes] = ...,
        image_mime_type: _Optional[str] = ...,
    ) -> None: ...

class FilesetConfig(_message.Message):
    __slots__ = ("paths", "exclude", "token_limit", "parent_docs")
    PATHS_FIELD_NUMBER: _ClassVar[int]
    EXCLUDE_FIELD_NUMBER: _ClassVar[int]
    TOKEN_LIMIT_FIELD_NUMBER: _ClassVar[int]
    PARENT_DOCS_FIELD_NUMBER: _ClassVar[int]
    paths: _containers.RepeatedScalarFieldContainer[str]
    exclude: _containers.RepeatedScalarFieldContainer[str]
    token_limit: int
    parent_docs: bool
    def __init__(
        self,
        paths: _Optional[_Iterable[str]] = ...,
        exclude: _Optional[_Iterable[str]] = ...,
        token_limit: _Optional[int] = ...,
        parent_docs: bool = ...,
    ) -> None: ...

class BudgetConfig(_message.Message):
    __slots__ = ("area", "docs", "diff", "total")
    AREA_FIELD_NUMBER: _ClassVar[int]
    DOCS_FIELD_NUMBER: _ClassVar[int]
    DIFF_FIELD_NUMBER: _ClassVar[int]
    TOTAL_FIELD_NUMBER: _ClassVar[int]
    area: int
    docs: int
    diff: int
    total: int
    def __init__(
        self,
        area: _Optional[int] = ...,
        docs: _Optional[int] = ...,
        diff: _Optional[int] = ...,
        total: _Optional[int] = ...,
    ) -> None: ...

class ContextConfig(_message.Message):
    __slots__ = ("diff_mode", "files", "area", "lfdocs", "clipboard", "summaries", "budgets")
    DIFF_MODE_FIELD_NUMBER: _ClassVar[int]
    FILES_FIELD_NUMBER: _ClassVar[int]
    AREA_FIELD_NUMBER: _ClassVar[int]
    LFDOCS_FIELD_NUMBER: _ClassVar[int]
    CLIPBOARD_FIELD_NUMBER: _ClassVar[int]
    SUMMARIES_FIELD_NUMBER: _ClassVar[int]
    BUDGETS_FIELD_NUMBER: _ClassVar[int]
    diff_mode: DiffMode
    files: FilesetConfig
    area: str
    lfdocs: bool
    clipboard: bool
    summaries: bool
    budgets: BudgetConfig
    def __init__(
        self,
        diff_mode: _Optional[_Union[DiffMode, str]] = ...,
        files: _Optional[_Union[FilesetConfig, _Mapping]] = ...,
        area: _Optional[str] = ...,
        lfdocs: bool = ...,
        clipboard: bool = ...,
        summaries: bool = ...,
        budgets: _Optional[_Union[BudgetConfig, _Mapping]] = ...,
    ) -> None: ...

class Document(_message.Message):
    __slots__ = ("path", "content", "category")
    PATH_FIELD_NUMBER: _ClassVar[int]
    CONTENT_FIELD_NUMBER: _ClassVar[int]
    CATEGORY_FIELD_NUMBER: _ClassVar[int]
    path: str
    content: str
    category: str
    def __init__(
        self,
        path: _Optional[str] = ...,
        content: _Optional[str] = ...,
        category: _Optional[str] = ...,
    ) -> None: ...

class Direction(_message.Message):
    __slots__ = ("name", "content", "source")
    NAME_FIELD_NUMBER: _ClassVar[int]
    CONTENT_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    name: str
    content: str
    source: str
    def __init__(
        self,
        name: _Optional[str] = ...,
        content: _Optional[str] = ...,
        source: _Optional[str] = ...,
    ) -> None: ...

class StepFile(_message.Message):
    __slots__ = ("name", "content", "config", "source")
    NAME_FIELD_NUMBER: _ClassVar[int]
    CONTENT_FIELD_NUMBER: _ClassVar[int]
    CONFIG_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    name: str
    content: str
    config: StepConfig
    source: str
    def __init__(
        self,
        name: _Optional[str] = ...,
        content: _Optional[str] = ...,
        config: _Optional[_Union[StepConfig, _Mapping]] = ...,
        source: _Optional[str] = ...,
    ) -> None: ...

class StepConfig(_message.Message):
    __slots__ = (
        "interactive",
        "include",
        "exclude",
        "model",
        "direction",
        "chrome",
        "diff_files",
        "context",
        "area",
    )
    INTERACTIVE_FIELD_NUMBER: _ClassVar[int]
    INCLUDE_FIELD_NUMBER: _ClassVar[int]
    EXCLUDE_FIELD_NUMBER: _ClassVar[int]
    MODEL_FIELD_NUMBER: _ClassVar[int]
    DIRECTION_FIELD_NUMBER: _ClassVar[int]
    CHROME_FIELD_NUMBER: _ClassVar[int]
    DIFF_FILES_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    AREA_FIELD_NUMBER: _ClassVar[int]
    interactive: bool
    include: _containers.RepeatedScalarFieldContainer[str]
    exclude: _containers.RepeatedScalarFieldContainer[str]
    model: str
    direction: _containers.RepeatedScalarFieldContainer[str]
    chrome: bool
    diff_files: bool
    context: _containers.RepeatedScalarFieldContainer[str]
    area: str
    def __init__(
        self,
        interactive: bool = ...,
        include: _Optional[_Iterable[str]] = ...,
        exclude: _Optional[_Iterable[str]] = ...,
        model: _Optional[str] = ...,
        direction: _Optional[_Iterable[str]] = ...,
        chrome: bool = ...,
        diff_files: bool = ...,
        context: _Optional[_Iterable[str]] = ...,
        area: _Optional[str] = ...,
    ) -> None: ...

class PromptComponents(_message.Message):
    __slots__ = (
        "run_mode",
        "docs",
        "diff",
        "diff_files",
        "step",
        "repo_root",
        "clipboard",
        "loopflow_doc",
        "directions",
        "image_files",
        "summaries",
    )
    RUN_MODE_FIELD_NUMBER: _ClassVar[int]
    DOCS_FIELD_NUMBER: _ClassVar[int]
    DIFF_FIELD_NUMBER: _ClassVar[int]
    DIFF_FILES_FIELD_NUMBER: _ClassVar[int]
    STEP_FIELD_NUMBER: _ClassVar[int]
    REPO_ROOT_FIELD_NUMBER: _ClassVar[int]
    CLIPBOARD_FIELD_NUMBER: _ClassVar[int]
    LOOPFLOW_DOC_FIELD_NUMBER: _ClassVar[int]
    DIRECTIONS_FIELD_NUMBER: _ClassVar[int]
    IMAGE_FILES_FIELD_NUMBER: _ClassVar[int]
    SUMMARIES_FIELD_NUMBER: _ClassVar[int]
    run_mode: str
    docs: _containers.RepeatedCompositeFieldContainer[Document]
    diff: str
    diff_files: _containers.RepeatedCompositeFieldContainer[Document]
    step: StepFile
    repo_root: str
    clipboard: ClipboardContent
    loopflow_doc: str
    directions: _containers.RepeatedCompositeFieldContainer[Direction]
    image_files: _containers.RepeatedScalarFieldContainer[str]
    summaries: _containers.RepeatedCompositeFieldContainer[Document]
    def __init__(
        self,
        run_mode: _Optional[str] = ...,
        docs: _Optional[_Iterable[_Union[Document, _Mapping]]] = ...,
        diff: _Optional[str] = ...,
        diff_files: _Optional[_Iterable[_Union[Document, _Mapping]]] = ...,
        step: _Optional[_Union[StepFile, _Mapping]] = ...,
        repo_root: _Optional[str] = ...,
        clipboard: _Optional[_Union[ClipboardContent, _Mapping]] = ...,
        loopflow_doc: _Optional[str] = ...,
        directions: _Optional[_Iterable[_Union[Direction, _Mapping]]] = ...,
        image_files: _Optional[_Iterable[str]] = ...,
        summaries: _Optional[_Iterable[_Union[Document, _Mapping]]] = ...,
    ) -> None: ...

class DroppedComponent(_message.Message):
    __slots__ = ("category", "name", "tokens", "reason")
    CATEGORY_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    TOKENS_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    category: str
    name: str
    tokens: int
    reason: str
    def __init__(
        self,
        category: _Optional[str] = ...,
        name: _Optional[str] = ...,
        tokens: _Optional[int] = ...,
        reason: _Optional[str] = ...,
    ) -> None: ...

class GatherContextRequest(_message.Message):
    __slots__ = ("repo_root", "step", "inline", "step_args", "run_mode", "directions", "config")
    REPO_ROOT_FIELD_NUMBER: _ClassVar[int]
    STEP_FIELD_NUMBER: _ClassVar[int]
    INLINE_FIELD_NUMBER: _ClassVar[int]
    STEP_ARGS_FIELD_NUMBER: _ClassVar[int]
    RUN_MODE_FIELD_NUMBER: _ClassVar[int]
    DIRECTIONS_FIELD_NUMBER: _ClassVar[int]
    CONFIG_FIELD_NUMBER: _ClassVar[int]
    repo_root: str
    step: str
    inline: str
    step_args: _containers.RepeatedScalarFieldContainer[str]
    run_mode: str
    directions: _containers.RepeatedScalarFieldContainer[str]
    config: ContextConfig
    def __init__(
        self,
        repo_root: _Optional[str] = ...,
        step: _Optional[str] = ...,
        inline: _Optional[str] = ...,
        step_args: _Optional[_Iterable[str]] = ...,
        run_mode: _Optional[str] = ...,
        directions: _Optional[_Iterable[str]] = ...,
        config: _Optional[_Union[ContextConfig, _Mapping]] = ...,
    ) -> None: ...

class GatherContextResponse(_message.Message):
    __slots__ = ("components", "total_tokens")
    COMPONENTS_FIELD_NUMBER: _ClassVar[int]
    TOTAL_TOKENS_FIELD_NUMBER: _ClassVar[int]
    components: PromptComponents
    total_tokens: int
    def __init__(
        self,
        components: _Optional[_Union[PromptComponents, _Mapping]] = ...,
        total_tokens: _Optional[int] = ...,
    ) -> None: ...

class TrimContextRequest(_message.Message):
    __slots__ = ("components", "max_tokens")
    COMPONENTS_FIELD_NUMBER: _ClassVar[int]
    MAX_TOKENS_FIELD_NUMBER: _ClassVar[int]
    components: PromptComponents
    max_tokens: int
    def __init__(
        self,
        components: _Optional[_Union[PromptComponents, _Mapping]] = ...,
        max_tokens: _Optional[int] = ...,
    ) -> None: ...

class TrimContextResponse(_message.Message):
    __slots__ = ("components", "dropped", "total_tokens")
    COMPONENTS_FIELD_NUMBER: _ClassVar[int]
    DROPPED_FIELD_NUMBER: _ClassVar[int]
    TOTAL_TOKENS_FIELD_NUMBER: _ClassVar[int]
    components: PromptComponents
    dropped: _containers.RepeatedCompositeFieldContainer[DroppedComponent]
    total_tokens: int
    def __init__(
        self,
        components: _Optional[_Union[PromptComponents, _Mapping]] = ...,
        dropped: _Optional[_Iterable[_Union[DroppedComponent, _Mapping]]] = ...,
        total_tokens: _Optional[int] = ...,
    ) -> None: ...

class TokenNode(_message.Message):
    __slots__ = ("name", "tokens", "path", "children")
    NAME_FIELD_NUMBER: _ClassVar[int]
    TOKENS_FIELD_NUMBER: _ClassVar[int]
    PATH_FIELD_NUMBER: _ClassVar[int]
    CHILDREN_FIELD_NUMBER: _ClassVar[int]
    name: str
    tokens: int
    path: str
    children: _containers.RepeatedCompositeFieldContainer[TokenNode]
    def __init__(
        self,
        name: _Optional[str] = ...,
        tokens: _Optional[int] = ...,
        path: _Optional[str] = ...,
        children: _Optional[_Iterable[_Union[TokenNode, _Mapping]]] = ...,
    ) -> None: ...

class AnalyzeTokensRequest(_message.Message):
    __slots__ = ("components",)
    COMPONENTS_FIELD_NUMBER: _ClassVar[int]
    components: PromptComponents
    def __init__(self, components: _Optional[_Union[PromptComponents, _Mapping]] = ...) -> None: ...

class AnalyzeTokensResponse(_message.Message):
    __slots__ = ("root", "total_tokens", "formatted")
    ROOT_FIELD_NUMBER: _ClassVar[int]
    TOTAL_TOKENS_FIELD_NUMBER: _ClassVar[int]
    FORMATTED_FIELD_NUMBER: _ClassVar[int]
    root: TokenNode
    total_tokens: int
    formatted: str
    def __init__(
        self,
        root: _Optional[_Union[TokenNode, _Mapping]] = ...,
        total_tokens: _Optional[int] = ...,
        formatted: _Optional[str] = ...,
    ) -> None: ...

class FormatPromptRequest(_message.Message):
    __slots__ = ("components",)
    COMPONENTS_FIELD_NUMBER: _ClassVar[int]
    components: PromptComponents
    def __init__(self, components: _Optional[_Union[PromptComponents, _Mapping]] = ...) -> None: ...

class FormatPromptResponse(_message.Message):
    __slots__ = ("prompt", "tokens")
    PROMPT_FIELD_NUMBER: _ClassVar[int]
    TOKENS_FIELD_NUMBER: _ClassVar[int]
    prompt: str
    tokens: int
    def __init__(self, prompt: _Optional[str] = ..., tokens: _Optional[int] = ...) -> None: ...

class ModelSpec(_message.Message):
    __slots__ = ("backend", "variant")
    BACKEND_FIELD_NUMBER: _ClassVar[int]
    VARIANT_FIELD_NUMBER: _ClassVar[int]
    backend: str
    variant: str
    def __init__(self, backend: _Optional[str] = ..., variant: _Optional[str] = ...) -> None: ...

class RunStepRequest(_message.Message):
    __slots__ = (
        "repo_root",
        "step",
        "step_args",
        "context_config",
        "directions",
        "model",
        "skip_permissions",
        "push_enabled",
        "chrome",
        "idempotency_key",
    )
    REPO_ROOT_FIELD_NUMBER: _ClassVar[int]
    STEP_FIELD_NUMBER: _ClassVar[int]
    STEP_ARGS_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_CONFIG_FIELD_NUMBER: _ClassVar[int]
    DIRECTIONS_FIELD_NUMBER: _ClassVar[int]
    MODEL_FIELD_NUMBER: _ClassVar[int]
    SKIP_PERMISSIONS_FIELD_NUMBER: _ClassVar[int]
    PUSH_ENABLED_FIELD_NUMBER: _ClassVar[int]
    CHROME_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    repo_root: str
    step: str
    step_args: _containers.RepeatedScalarFieldContainer[str]
    context_config: ContextConfig
    directions: _containers.RepeatedScalarFieldContainer[str]
    model: ModelSpec
    skip_permissions: bool
    push_enabled: bool
    chrome: bool
    idempotency_key: str
    def __init__(
        self,
        repo_root: _Optional[str] = ...,
        step: _Optional[str] = ...,
        step_args: _Optional[_Iterable[str]] = ...,
        context_config: _Optional[_Union[ContextConfig, _Mapping]] = ...,
        directions: _Optional[_Iterable[str]] = ...,
        model: _Optional[_Union[ModelSpec, _Mapping]] = ...,
        skip_permissions: bool = ...,
        push_enabled: bool = ...,
        chrome: bool = ...,
        idempotency_key: _Optional[str] = ...,
    ) -> None: ...

class StepEvent(_message.Message):
    __slots__ = (
        "type",
        "timestamp",
        "step_run_id",
        "pid",
        "text",
        "exit_code",
        "error",
        "commit_sha",
        "commit_message",
        "remote_branch",
    )
    TYPE_FIELD_NUMBER: _ClassVar[int]
    TIMESTAMP_FIELD_NUMBER: _ClassVar[int]
    STEP_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    PID_FIELD_NUMBER: _ClassVar[int]
    TEXT_FIELD_NUMBER: _ClassVar[int]
    EXIT_CODE_FIELD_NUMBER: _ClassVar[int]
    ERROR_FIELD_NUMBER: _ClassVar[int]
    COMMIT_SHA_FIELD_NUMBER: _ClassVar[int]
    COMMIT_MESSAGE_FIELD_NUMBER: _ClassVar[int]
    REMOTE_BRANCH_FIELD_NUMBER: _ClassVar[int]
    type: StepEventType
    timestamp: _timestamp_pb2.Timestamp
    step_run_id: str
    pid: int
    text: str
    exit_code: int
    error: str
    commit_sha: str
    commit_message: str
    remote_branch: str
    def __init__(
        self,
        type: _Optional[_Union[StepEventType, str]] = ...,
        timestamp: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...,
        step_run_id: _Optional[str] = ...,
        pid: _Optional[int] = ...,
        text: _Optional[str] = ...,
        exit_code: _Optional[int] = ...,
        error: _Optional[str] = ...,
        commit_sha: _Optional[str] = ...,
        commit_message: _Optional[str] = ...,
        remote_branch: _Optional[str] = ...,
    ) -> None: ...

class RunInteractiveStepRequest(_message.Message):
    __slots__ = (
        "repo_root",
        "step",
        "step_args",
        "context_config",
        "directions",
        "model",
        "skip_permissions",
        "chrome",
    )
    REPO_ROOT_FIELD_NUMBER: _ClassVar[int]
    STEP_FIELD_NUMBER: _ClassVar[int]
    STEP_ARGS_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_CONFIG_FIELD_NUMBER: _ClassVar[int]
    DIRECTIONS_FIELD_NUMBER: _ClassVar[int]
    MODEL_FIELD_NUMBER: _ClassVar[int]
    SKIP_PERMISSIONS_FIELD_NUMBER: _ClassVar[int]
    CHROME_FIELD_NUMBER: _ClassVar[int]
    repo_root: str
    step: str
    step_args: _containers.RepeatedScalarFieldContainer[str]
    context_config: ContextConfig
    directions: _containers.RepeatedScalarFieldContainer[str]
    model: ModelSpec
    skip_permissions: bool
    chrome: bool
    def __init__(
        self,
        repo_root: _Optional[str] = ...,
        step: _Optional[str] = ...,
        step_args: _Optional[_Iterable[str]] = ...,
        context_config: _Optional[_Union[ContextConfig, _Mapping]] = ...,
        directions: _Optional[_Iterable[str]] = ...,
        model: _Optional[_Union[ModelSpec, _Mapping]] = ...,
        skip_permissions: bool = ...,
        chrome: bool = ...,
    ) -> None: ...

class RunInteractiveStepResponse(_message.Message):
    __slots__ = ("command", "env")
    COMMAND_FIELD_NUMBER: _ClassVar[int]
    ENV_FIELD_NUMBER: _ClassVar[int]
    command: _containers.RepeatedScalarFieldContainer[str]
    env: _containers.RepeatedScalarFieldContainer[str]
    def __init__(
        self, command: _Optional[_Iterable[str]] = ..., env: _Optional[_Iterable[str]] = ...
    ) -> None: ...

class FlowItem(_message.Message):
    __slots__ = ("step", "fork", "choose", "loop_until_empty")
    STEP_FIELD_NUMBER: _ClassVar[int]
    FORK_FIELD_NUMBER: _ClassVar[int]
    CHOOSE_FIELD_NUMBER: _ClassVar[int]
    LOOP_UNTIL_EMPTY_FIELD_NUMBER: _ClassVar[int]
    step: FlowStep
    fork: FlowFork
    choose: FlowChoose
    loop_until_empty: FlowLoopUntilEmpty
    def __init__(
        self,
        step: _Optional[_Union[FlowStep, _Mapping]] = ...,
        fork: _Optional[_Union[FlowFork, _Mapping]] = ...,
        choose: _Optional[_Union[FlowChoose, _Mapping]] = ...,
        loop_until_empty: _Optional[_Union[FlowLoopUntilEmpty, _Mapping]] = ...,
    ) -> None: ...

class FlowStep(_message.Message):
    __slots__ = ("name", "after", "model", "direction", "interactive")
    NAME_FIELD_NUMBER: _ClassVar[int]
    AFTER_FIELD_NUMBER: _ClassVar[int]
    MODEL_FIELD_NUMBER: _ClassVar[int]
    DIRECTION_FIELD_NUMBER: _ClassVar[int]
    INTERACTIVE_FIELD_NUMBER: _ClassVar[int]
    name: str
    after: _containers.RepeatedScalarFieldContainer[str]
    model: str
    direction: _containers.RepeatedScalarFieldContainer[str]
    interactive: bool
    def __init__(
        self,
        name: _Optional[str] = ...,
        after: _Optional[_Iterable[str]] = ...,
        model: _Optional[str] = ...,
        direction: _Optional[_Iterable[str]] = ...,
        interactive: bool = ...,
    ) -> None: ...

class ForkThread(_message.Message):
    __slots__ = ("step", "flow", "direction", "model", "area")
    STEP_FIELD_NUMBER: _ClassVar[int]
    FLOW_FIELD_NUMBER: _ClassVar[int]
    DIRECTION_FIELD_NUMBER: _ClassVar[int]
    MODEL_FIELD_NUMBER: _ClassVar[int]
    AREA_FIELD_NUMBER: _ClassVar[int]
    step: str
    flow: str
    direction: _containers.RepeatedScalarFieldContainer[str]
    model: str
    area: _containers.RepeatedScalarFieldContainer[str]
    def __init__(
        self,
        step: _Optional[str] = ...,
        flow: _Optional[str] = ...,
        direction: _Optional[_Iterable[str]] = ...,
        model: _Optional[str] = ...,
        area: _Optional[_Iterable[str]] = ...,
    ) -> None: ...

class SynthesizeConfig(_message.Message):
    __slots__ = ("direction", "area", "prompt")
    DIRECTION_FIELD_NUMBER: _ClassVar[int]
    AREA_FIELD_NUMBER: _ClassVar[int]
    PROMPT_FIELD_NUMBER: _ClassVar[int]
    direction: _containers.RepeatedScalarFieldContainer[str]
    area: _containers.RepeatedScalarFieldContainer[str]
    prompt: str
    def __init__(
        self,
        direction: _Optional[_Iterable[str]] = ...,
        area: _Optional[_Iterable[str]] = ...,
        prompt: _Optional[str] = ...,
    ) -> None: ...

class FlowFork(_message.Message):
    __slots__ = ("threads", "step", "model", "synthesize")
    THREADS_FIELD_NUMBER: _ClassVar[int]
    STEP_FIELD_NUMBER: _ClassVar[int]
    MODEL_FIELD_NUMBER: _ClassVar[int]
    SYNTHESIZE_FIELD_NUMBER: _ClassVar[int]
    threads: _containers.RepeatedCompositeFieldContainer[ForkThread]
    step: str
    model: str
    synthesize: SynthesizeConfig
    def __init__(
        self,
        threads: _Optional[_Iterable[_Union[ForkThread, _Mapping]]] = ...,
        step: _Optional[str] = ...,
        model: _Optional[str] = ...,
        synthesize: _Optional[_Union[SynthesizeConfig, _Mapping]] = ...,
    ) -> None: ...

class FlowChoose(_message.Message):
    __slots__ = ("options", "output", "prompt")
    class OptionsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: FlowItemList
        def __init__(
            self, key: _Optional[str] = ..., value: _Optional[_Union[FlowItemList, _Mapping]] = ...
        ) -> None: ...

    OPTIONS_FIELD_NUMBER: _ClassVar[int]
    OUTPUT_FIELD_NUMBER: _ClassVar[int]
    PROMPT_FIELD_NUMBER: _ClassVar[int]
    options: _containers.MessageMap[str, FlowItemList]
    output: str
    prompt: str
    def __init__(
        self,
        options: _Optional[_Mapping[str, FlowItemList]] = ...,
        output: _Optional[str] = ...,
        prompt: _Optional[str] = ...,
    ) -> None: ...

class FlowItemList(_message.Message):
    __slots__ = ("items",)
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    items: _containers.RepeatedCompositeFieldContainer[FlowItem]
    def __init__(self, items: _Optional[_Iterable[_Union[FlowItem, _Mapping]]] = ...) -> None: ...

class FlowLoopUntilEmpty(_message.Message):
    __slots__ = ("steps", "wave", "max_iterations")
    STEPS_FIELD_NUMBER: _ClassVar[int]
    WAVE_FIELD_NUMBER: _ClassVar[int]
    MAX_ITERATIONS_FIELD_NUMBER: _ClassVar[int]
    steps: _containers.RepeatedCompositeFieldContainer[FlowItem]
    wave: str
    max_iterations: int
    def __init__(
        self,
        steps: _Optional[_Iterable[_Union[FlowItem, _Mapping]]] = ...,
        wave: _Optional[str] = ...,
        max_iterations: _Optional[int] = ...,
    ) -> None: ...

class Flow(_message.Message):
    __slots__ = ("name", "steps")
    NAME_FIELD_NUMBER: _ClassVar[int]
    STEPS_FIELD_NUMBER: _ClassVar[int]
    name: str
    steps: _containers.RepeatedCompositeFieldContainer[FlowItem]
    def __init__(
        self,
        name: _Optional[str] = ...,
        steps: _Optional[_Iterable[_Union[FlowItem, _Mapping]]] = ...,
    ) -> None: ...

class RunFlowRequest(_message.Message):
    __slots__ = (
        "repo_root",
        "flow",
        "area",
        "directions",
        "context_config",
        "model",
        "skip_permissions",
        "push_enabled",
        "pr_enabled",
        "chrome",
        "idempotency_key",
    )
    REPO_ROOT_FIELD_NUMBER: _ClassVar[int]
    FLOW_FIELD_NUMBER: _ClassVar[int]
    AREA_FIELD_NUMBER: _ClassVar[int]
    DIRECTIONS_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_CONFIG_FIELD_NUMBER: _ClassVar[int]
    MODEL_FIELD_NUMBER: _ClassVar[int]
    SKIP_PERMISSIONS_FIELD_NUMBER: _ClassVar[int]
    PUSH_ENABLED_FIELD_NUMBER: _ClassVar[int]
    PR_ENABLED_FIELD_NUMBER: _ClassVar[int]
    CHROME_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    repo_root: str
    flow: str
    area: _containers.RepeatedScalarFieldContainer[str]
    directions: _containers.RepeatedScalarFieldContainer[str]
    context_config: ContextConfig
    model: ModelSpec
    skip_permissions: bool
    push_enabled: bool
    pr_enabled: bool
    chrome: bool
    idempotency_key: str
    def __init__(
        self,
        repo_root: _Optional[str] = ...,
        flow: _Optional[str] = ...,
        area: _Optional[_Iterable[str]] = ...,
        directions: _Optional[_Iterable[str]] = ...,
        context_config: _Optional[_Union[ContextConfig, _Mapping]] = ...,
        model: _Optional[_Union[ModelSpec, _Mapping]] = ...,
        skip_permissions: bool = ...,
        push_enabled: bool = ...,
        pr_enabled: bool = ...,
        chrome: bool = ...,
        idempotency_key: _Optional[str] = ...,
    ) -> None: ...

class FlowEvent(_message.Message):
    __slots__ = (
        "type",
        "timestamp",
        "flow_run_id",
        "step_name",
        "step_index",
        "exit_code",
        "error",
        "thread_index",
        "thread_direction",
        "thread_diff",
        "chosen_branch",
        "commit_sha",
        "pr_url",
        "waiting_step",
        "waiting_step_run_id",
    )
    TYPE_FIELD_NUMBER: _ClassVar[int]
    TIMESTAMP_FIELD_NUMBER: _ClassVar[int]
    FLOW_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    STEP_NAME_FIELD_NUMBER: _ClassVar[int]
    STEP_INDEX_FIELD_NUMBER: _ClassVar[int]
    EXIT_CODE_FIELD_NUMBER: _ClassVar[int]
    ERROR_FIELD_NUMBER: _ClassVar[int]
    THREAD_INDEX_FIELD_NUMBER: _ClassVar[int]
    THREAD_DIRECTION_FIELD_NUMBER: _ClassVar[int]
    THREAD_DIFF_FIELD_NUMBER: _ClassVar[int]
    CHOSEN_BRANCH_FIELD_NUMBER: _ClassVar[int]
    COMMIT_SHA_FIELD_NUMBER: _ClassVar[int]
    PR_URL_FIELD_NUMBER: _ClassVar[int]
    WAITING_STEP_FIELD_NUMBER: _ClassVar[int]
    WAITING_STEP_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    type: FlowEventType
    timestamp: _timestamp_pb2.Timestamp
    flow_run_id: str
    step_name: str
    step_index: int
    exit_code: int
    error: str
    thread_index: int
    thread_direction: str
    thread_diff: str
    chosen_branch: str
    commit_sha: str
    pr_url: str
    waiting_step: str
    waiting_step_run_id: str
    def __init__(
        self,
        type: _Optional[_Union[FlowEventType, str]] = ...,
        timestamp: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...,
        flow_run_id: _Optional[str] = ...,
        step_name: _Optional[str] = ...,
        step_index: _Optional[int] = ...,
        exit_code: _Optional[int] = ...,
        error: _Optional[str] = ...,
        thread_index: _Optional[int] = ...,
        thread_direction: _Optional[str] = ...,
        thread_diff: _Optional[str] = ...,
        chosen_branch: _Optional[str] = ...,
        commit_sha: _Optional[str] = ...,
        pr_url: _Optional[str] = ...,
        waiting_step: _Optional[str] = ...,
        waiting_step_run_id: _Optional[str] = ...,
    ) -> None: ...

class FlowRun(_message.Message):
    __slots__ = ("id", "flow", "worktree", "step_index", "status", "completed_steps")
    ID_FIELD_NUMBER: _ClassVar[int]
    FLOW_FIELD_NUMBER: _ClassVar[int]
    WORKTREE_FIELD_NUMBER: _ClassVar[int]
    STEP_INDEX_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_STEPS_FIELD_NUMBER: _ClassVar[int]
    id: str
    flow: str
    worktree: str
    step_index: int
    status: FlowRunStatus
    completed_steps: _containers.RepeatedScalarFieldContainer[str]
    def __init__(
        self,
        id: _Optional[str] = ...,
        flow: _Optional[str] = ...,
        worktree: _Optional[str] = ...,
        step_index: _Optional[int] = ...,
        status: _Optional[_Union[FlowRunStatus, str]] = ...,
        completed_steps: _Optional[_Iterable[str]] = ...,
    ) -> None: ...

class TickFlowRequest(_message.Message):
    __slots__ = (
        "flow_run_id",
        "repo_root",
        "context_config",
        "model",
        "skip_permissions",
        "idempotency_key",
    )
    FLOW_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    REPO_ROOT_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_CONFIG_FIELD_NUMBER: _ClassVar[int]
    MODEL_FIELD_NUMBER: _ClassVar[int]
    SKIP_PERMISSIONS_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    flow_run_id: str
    repo_root: str
    context_config: ContextConfig
    model: ModelSpec
    skip_permissions: bool
    idempotency_key: str
    def __init__(
        self,
        flow_run_id: _Optional[str] = ...,
        repo_root: _Optional[str] = ...,
        context_config: _Optional[_Union[ContextConfig, _Mapping]] = ...,
        model: _Optional[_Union[ModelSpec, _Mapping]] = ...,
        skip_permissions: bool = ...,
        idempotency_key: _Optional[str] = ...,
    ) -> None: ...

class TickFlowResponse(_message.Message):
    __slots__ = ("result", "flow_run", "waiting_step", "waiting_step_run_id", "prompt_file")
    RESULT_FIELD_NUMBER: _ClassVar[int]
    FLOW_RUN_FIELD_NUMBER: _ClassVar[int]
    WAITING_STEP_FIELD_NUMBER: _ClassVar[int]
    WAITING_STEP_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    PROMPT_FILE_FIELD_NUMBER: _ClassVar[int]
    result: TickResult
    flow_run: FlowRun
    waiting_step: str
    waiting_step_run_id: str
    prompt_file: str
    def __init__(
        self,
        result: _Optional[_Union[TickResult, str]] = ...,
        flow_run: _Optional[_Union[FlowRun, _Mapping]] = ...,
        waiting_step: _Optional[str] = ...,
        waiting_step_run_id: _Optional[str] = ...,
        prompt_file: _Optional[str] = ...,
    ) -> None: ...

class RunForkRequest(_message.Message):
    __slots__ = (
        "threads",
        "base_commit",
        "parent_worktree",
        "flow_name",
        "main_repo",
        "exclude",
        "skip_permissions",
        "model",
        "context",
        "idempotency_key",
    )
    THREADS_FIELD_NUMBER: _ClassVar[int]
    BASE_COMMIT_FIELD_NUMBER: _ClassVar[int]
    PARENT_WORKTREE_FIELD_NUMBER: _ClassVar[int]
    FLOW_NAME_FIELD_NUMBER: _ClassVar[int]
    MAIN_REPO_FIELD_NUMBER: _ClassVar[int]
    EXCLUDE_FIELD_NUMBER: _ClassVar[int]
    SKIP_PERMISSIONS_FIELD_NUMBER: _ClassVar[int]
    MODEL_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    threads: _containers.RepeatedCompositeFieldContainer[ForkThread]
    base_commit: str
    parent_worktree: str
    flow_name: str
    main_repo: str
    exclude: _containers.RepeatedScalarFieldContainer[str]
    skip_permissions: bool
    model: ModelSpec
    context: _containers.RepeatedScalarFieldContainer[str]
    idempotency_key: str
    def __init__(
        self,
        threads: _Optional[_Iterable[_Union[ForkThread, _Mapping]]] = ...,
        base_commit: _Optional[str] = ...,
        parent_worktree: _Optional[str] = ...,
        flow_name: _Optional[str] = ...,
        main_repo: _Optional[str] = ...,
        exclude: _Optional[_Iterable[str]] = ...,
        skip_permissions: bool = ...,
        model: _Optional[_Union[ModelSpec, _Mapping]] = ...,
        context: _Optional[_Iterable[str]] = ...,
        idempotency_key: _Optional[str] = ...,
    ) -> None: ...

class ForkResult(_message.Message):
    __slots__ = ("worktree", "config", "diff", "status", "scratch_notes")
    WORKTREE_FIELD_NUMBER: _ClassVar[int]
    CONFIG_FIELD_NUMBER: _ClassVar[int]
    DIFF_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    SCRATCH_NOTES_FIELD_NUMBER: _ClassVar[int]
    worktree: str
    config: ForkThread
    diff: str
    status: str
    scratch_notes: str
    def __init__(
        self,
        worktree: _Optional[str] = ...,
        config: _Optional[_Union[ForkThread, _Mapping]] = ...,
        diff: _Optional[str] = ...,
        status: _Optional[str] = ...,
        scratch_notes: _Optional[str] = ...,
    ) -> None: ...

class ForkEvent(_message.Message):
    __slots__ = ("type", "timestamp", "thread_index", "text", "result", "all_results")
    TYPE_FIELD_NUMBER: _ClassVar[int]
    TIMESTAMP_FIELD_NUMBER: _ClassVar[int]
    THREAD_INDEX_FIELD_NUMBER: _ClassVar[int]
    TEXT_FIELD_NUMBER: _ClassVar[int]
    RESULT_FIELD_NUMBER: _ClassVar[int]
    ALL_RESULTS_FIELD_NUMBER: _ClassVar[int]
    type: ForkEventType
    timestamp: _timestamp_pb2.Timestamp
    thread_index: int
    text: str
    result: ForkResult
    all_results: _containers.RepeatedCompositeFieldContainer[ForkResult]
    def __init__(
        self,
        type: _Optional[_Union[ForkEventType, str]] = ...,
        timestamp: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...,
        thread_index: _Optional[int] = ...,
        text: _Optional[str] = ...,
        result: _Optional[_Union[ForkResult, _Mapping]] = ...,
        all_results: _Optional[_Iterable[_Union[ForkResult, _Mapping]]] = ...,
    ) -> None: ...

class SynthesizeRequest(_message.Message):
    __slots__ = (
        "fork_results",
        "config",
        "worktree",
        "model",
        "skip_permissions",
        "idempotency_key",
    )
    FORK_RESULTS_FIELD_NUMBER: _ClassVar[int]
    CONFIG_FIELD_NUMBER: _ClassVar[int]
    WORKTREE_FIELD_NUMBER: _ClassVar[int]
    MODEL_FIELD_NUMBER: _ClassVar[int]
    SKIP_PERMISSIONS_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    fork_results: _containers.RepeatedCompositeFieldContainer[ForkResult]
    config: SynthesizeConfig
    worktree: str
    model: ModelSpec
    skip_permissions: bool
    idempotency_key: str
    def __init__(
        self,
        fork_results: _Optional[_Iterable[_Union[ForkResult, _Mapping]]] = ...,
        config: _Optional[_Union[SynthesizeConfig, _Mapping]] = ...,
        worktree: _Optional[str] = ...,
        model: _Optional[_Union[ModelSpec, _Mapping]] = ...,
        skip_permissions: bool = ...,
        idempotency_key: _Optional[str] = ...,
    ) -> None: ...

class SynthesizeEvent(_message.Message):
    __slots__ = ("type", "timestamp", "text", "exit_code", "error")
    TYPE_FIELD_NUMBER: _ClassVar[int]
    TIMESTAMP_FIELD_NUMBER: _ClassVar[int]
    TEXT_FIELD_NUMBER: _ClassVar[int]
    EXIT_CODE_FIELD_NUMBER: _ClassVar[int]
    ERROR_FIELD_NUMBER: _ClassVar[int]
    type: SynthesizeEventType
    timestamp: _timestamp_pb2.Timestamp
    text: str
    exit_code: int
    error: str
    def __init__(
        self,
        type: _Optional[_Union[SynthesizeEventType, str]] = ...,
        timestamp: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...,
        text: _Optional[str] = ...,
        exit_code: _Optional[int] = ...,
        error: _Optional[str] = ...,
    ) -> None: ...

class LoadStepRequest(_message.Message):
    __slots__ = ("name", "repo_root")
    NAME_FIELD_NUMBER: _ClassVar[int]
    REPO_ROOT_FIELD_NUMBER: _ClassVar[int]
    name: str
    repo_root: str
    def __init__(self, name: _Optional[str] = ..., repo_root: _Optional[str] = ...) -> None: ...

class LoadStepResponse(_message.Message):
    __slots__ = ("step",)
    STEP_FIELD_NUMBER: _ClassVar[int]
    step: StepFile
    def __init__(self, step: _Optional[_Union[StepFile, _Mapping]] = ...) -> None: ...

class LoadFlowRequest(_message.Message):
    __slots__ = ("name", "repo_root")
    NAME_FIELD_NUMBER: _ClassVar[int]
    REPO_ROOT_FIELD_NUMBER: _ClassVar[int]
    name: str
    repo_root: str
    def __init__(self, name: _Optional[str] = ..., repo_root: _Optional[str] = ...) -> None: ...

class LoadFlowResponse(_message.Message):
    __slots__ = ("flow",)
    FLOW_FIELD_NUMBER: _ClassVar[int]
    flow: Flow
    def __init__(self, flow: _Optional[_Union[Flow, _Mapping]] = ...) -> None: ...

class LoadDirectionRequest(_message.Message):
    __slots__ = ("name", "repo_root")
    NAME_FIELD_NUMBER: _ClassVar[int]
    REPO_ROOT_FIELD_NUMBER: _ClassVar[int]
    name: str
    repo_root: str
    def __init__(self, name: _Optional[str] = ..., repo_root: _Optional[str] = ...) -> None: ...

class LoadDirectionResponse(_message.Message):
    __slots__ = ("direction",)
    DIRECTION_FIELD_NUMBER: _ClassVar[int]
    direction: Direction
    def __init__(self, direction: _Optional[_Union[Direction, _Mapping]] = ...) -> None: ...

class GenerateCommitMessageRequest(_message.Message):
    __slots__ = ("repo_root", "diff", "model")
    REPO_ROOT_FIELD_NUMBER: _ClassVar[int]
    DIFF_FIELD_NUMBER: _ClassVar[int]
    MODEL_FIELD_NUMBER: _ClassVar[int]
    repo_root: str
    diff: str
    model: ModelSpec
    def __init__(
        self,
        repo_root: _Optional[str] = ...,
        diff: _Optional[str] = ...,
        model: _Optional[_Union[ModelSpec, _Mapping]] = ...,
    ) -> None: ...

class GenerateCommitMessageResponse(_message.Message):
    __slots__ = ("title", "body")
    TITLE_FIELD_NUMBER: _ClassVar[int]
    BODY_FIELD_NUMBER: _ClassVar[int]
    title: str
    body: str
    def __init__(self, title: _Optional[str] = ..., body: _Optional[str] = ...) -> None: ...

class GeneratePRMessageRequest(_message.Message):
    __slots__ = ("repo_root", "base_branch", "model")
    REPO_ROOT_FIELD_NUMBER: _ClassVar[int]
    BASE_BRANCH_FIELD_NUMBER: _ClassVar[int]
    MODEL_FIELD_NUMBER: _ClassVar[int]
    repo_root: str
    base_branch: str
    model: ModelSpec
    def __init__(
        self,
        repo_root: _Optional[str] = ...,
        base_branch: _Optional[str] = ...,
        model: _Optional[_Union[ModelSpec, _Mapping]] = ...,
    ) -> None: ...

class GeneratePRMessageResponse(_message.Message):
    __slots__ = ("title", "body")
    TITLE_FIELD_NUMBER: _ClassVar[int]
    BODY_FIELD_NUMBER: _ClassVar[int]
    title: str
    body: str
    def __init__(self, title: _Optional[str] = ..., body: _Optional[str] = ...) -> None: ...
