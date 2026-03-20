# Loopflow Context Summary

## Scope

Summarized from:

- `rust/loopflow/src/lfd/`
- `rust/loopflow/src/lfd/http/`
- `rust/loopflow/src/engine/`
- `rust/loopflow/src/engine/builtins/steps/`
- `rust/loopflow/src/engine/builtins/flows/`
- `python/loopflow/`

## High-Level Layout

- `rust/loopflow/src/engine/`: local execution/runtime layer. Defines the flow DSL, built-in step/flow loading, prompt/context assembly, agent command launch, git/worktree helpers, stream parsing, and config loading.
- `rust/loopflow/src/lfd/`: daemon/service layer. Owns persistent state, HTTP API, scheduler/executor, trigger orchestration, session management, provider auth, token/secrets handling, repo registration, and queue reconciliation.
- `python/loopflow/`: thin typed client + Typer CLI for the `lfd` HTTP API.

Core split:

- `engine` is about "how to run work".
- `lfd` is about "what state exists" + "how work is orchestrated and exposed".

The ubiquitous ID type is:

```rust
pub struct LfdId(String); // UUID-backed, serde/sql friendly
```

## Core Rust Domain Types

### Status / mode enums

```rust
enum WaveStatus { Idle, Running, Waiting, Paused, Failed }
enum WaveRunStatus { Unspecified, Pending, Running, Waiting, Completed, Failed }
enum WaveRunStackStatus { Active, Superseded, Merged }
enum LivePrState { Unknown, Open, Closed, Merged }
enum WaveMode { Loop, Cron, Manual }
enum QueueBlockReason { MissingPr, WaveRunning, ScratchDirty, RebaseConflict, PromotionFailed }

enum Signal { Repo, Wave, CiFailure }
enum ActivationOutcome { Queued, Coalesced, Dropped, Dispatched }

enum AgentStatus { Unspecified, Running, Waiting, Completed, Failed }

enum AttentionKind { InteractiveStep, Algedonic }
enum AttentionStatus { Surfaced, Viewed, Resolved }

enum SessionStatus { Starting, Active, Ending, Ended, Failed }
enum TurnStatus { Completed, Interrupted, Failed }
enum ItemStatus { InProgress, Completed, Failed, Declined }
```

### Waves, runs, triggers, repos

Source: `rust/loopflow/src/lfd/types/wave.rs`, `trigger.rs`, `repo.rs`

```rust
pub struct Wave {
    pub id: LfdId,
    pub name: String,
    pub repo: String,
    pub mode: WaveMode,
    pub primary_flow: String,
    pub cron: Option<String>,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub status: WaveStatus,
    pub iteration: u32,
    pub cycle_start_iteration: u32,
    pub created_at: Option<OffsetDateTime>,
    pub serialized: bool,
}

pub struct PullRequest {
    pub url: String,
    pub number: Option<u32>,
    pub state: Option<String>,
    pub title: Option<String>,
    pub branch: Option<String>,
}

pub struct WaveRunSnapshot {
    pub repo: String,
    pub flow: String,
    pub direction: Vec<String>,
    pub area: Vec<String>,
}

pub struct WaveRun {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub snapshot: WaveRunSnapshot,
    pub iteration: u32,
    pub step_index: u32,
    pub status: WaveRunStatus,
    pub worktree: String,
    pub branch: String,
    pub started_at: Option<OffsetDateTime>,
    pub ended_at: Option<OffsetDateTime>,
    pub error: Option<String>,
    pub flow_parents: Vec<String>,
    pub activation_log_id: Option<LfdId>,
    pub parent_run_id: Option<LfdId>,
    pub parent_pr_number: Option<u32>,
    pub stack_position: u32,
    pub stack_group_id: String,
    pub stack_status: WaveRunStackStatus,
    pub lineage_inferred: bool,
    pub target_branch: String,
    pub repair_of: Option<LfdId>,
    pub pr: Option<PullRequest>,
}

pub struct LivePullRequestState {
    pub repo_id: String,
    pub pr_number: u32,
    pub state: LivePrState,
    pub is_draft: bool,
    pub head_ref: String,
    pub head_sha: String,
    pub base_ref: String,
    pub updated_at: OffsetDateTime,
    pub merged_at: Option<OffsetDateTime>,
    pub synced_at: OffsetDateTime,
}

pub struct QueueBlock {
    pub wave_id: LfdId,
    pub run_id: LfdId,
    pub reason: QueueBlockReason,
    pub attempted_at: OffsetDateTime,
    pub conflict_files: Vec<String>,
    pub error: Option<String>,
}

pub struct QueueMergeEvent {
    pub wave_id: LfdId,
    pub pr_number: u32,
    pub merged_at: OffsetDateTime,
    pub processed_at: OffsetDateTime,
}

pub struct Trigger {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub source_wave_id: Option<LfdId>,
    pub signal: Signal,
    pub flow: Option<String>,
    pub last_main_sha: Option<String>,
    pub last_triggered_at: Option<i64>,
    pub created_at: Option<OffsetDateTime>,
    pub enabled: bool,
    pub max_iterations: Option<u32>,
}

pub struct PendingActivation {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub trigger_id: Option<LfdId>,
    pub reason: String,
    pub from_sha: String,
    pub to_sha: String,
    pub queued_at: i64,
    pub target_branch: String,
}

pub struct ActivationLog {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub trigger_id: Option<LfdId>,
    pub reason: String,
    pub outcome: ActivationOutcome,
    pub created_at: i64,
}

pub struct RepoId(String); // validated "owner/repo"

pub struct Repo {
    pub path: String,
    pub repo_id: RepoId,
    pub name: String,
    pub added_at: OffsetDateTime,
}

pub struct RepoEdge {
    pub parent_repo_id: RepoId,
    pub child_repo_id: RepoId,
}
```

Notes:

- `Wave.primary_flow` defaults to `"ship-roadmap"`.
- `Wave.serialized` switches between sequential queued execution and concurrent git-coordinated execution.
- `CI_FIX_FLOW` is the exact trigger flow name `"ci-fix"`.
- `Repo.repo_id` is canonical GitHub `"owner/repo"`, distinct from local filesystem `path`.

### Agent, attention, summaries, chat memory

Source: `rust/loopflow/src/lfd/types/agent.rs`, `attention.rs`, `summary.rs`, `chat_memory.rs`, `chat_message.rs`

```rust
pub struct AgentRun {
    pub id: LfdId,
    pub step: String,
    pub repo: String,
    pub worktree: String,
    pub wave_run_id: Option<LfdId>,
    pub status: AgentStatus,
    pub started_at: Option<OffsetDateTime>,
    pub ended_at: Option<OffsetDateTime>,
    pub pid: Option<u32>,
    pub container_id: Option<String>,
    pub agent: String,
    pub run_mode: String,
}

pub struct AttentionItem {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub run_id: Option<LfdId>,
    pub kind: AttentionKind,
    pub status: AttentionStatus,
    pub title: String,
    pub summary: String,
    pub context: serde_json::Value,
    pub surfaced_at: OffsetDateTime,
    pub viewed_at: Option<OffsetDateTime>,
    pub resolved_at: Option<OffsetDateTime>,
}

pub struct Summary {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub content: String,
    pub source_hash: String,
    pub token_budget: u32,
    pub agent: String,
    pub created_at: Option<OffsetDateTime>,
}

pub struct ChatMemoryBlock {
    pub wave_id: LfdId,
    pub name: String,
    pub content: String,
    pub position: u32,
    pub updated_at: Option<OffsetDateTime>,
}

pub struct ChatMessage {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub role: String,
    pub content: String,
    pub created_at: OffsetDateTime,
}
```

### Session model

Source: `rust/loopflow/src/lfd/sessions/types.rs`

```rust
pub struct FileEdit {
    pub path: String,
    pub kind: Option<String>,
    pub diff: Option<String>,
}

pub enum SessionItem {
    Command { id, command, cwd, status, output, exit_code, duration_ms },
    File    { id, changes, status },
    Message { id, text, phase },
    Thought { id, text },
    Tool    { id, name, status, input, output },
}

pub enum ItemDelta {
    Output { content: String },
    PlanText { content: String },
}

pub struct TurnUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub model: Option<String>,
    pub cost_usd: Option<f64>,
}

pub struct DocumentEntry {
    pub path: String,
    pub source: String,
    pub tokens: u64,
}

pub struct ContextSnapshot {
    pub sources: HashMap<String, u64>,
    pub source_counts: HashMap<String, u64>,
    pub documents: Vec<DocumentEntry>,
    pub budget: u64,
    pub total: u64,
    pub diff_tier: String,
    pub step_name: Option<String>,
    pub direction_names: Vec<String>,
    pub area_name: Option<String>,
    pub wave_name: Option<String>,
    pub has_clipboard: bool,
}

pub enum SessionEvent {
    TurnStarted { turn_id },
    TurnCompleted { turn_id, status },
    TurnUsage { turn_id, usage },
    ContextSnapshot { snapshot },
    ItemStarted { turn_id, item },
    ItemUpdated { turn_id, item_id, data },
    ItemCompleted { turn_id, item },
    TextDelta { turn_id, content },
    ReasoningDelta { turn_id, content },
    DiffUpdated { turn_id, diff },
    SuggestedActions { turn_id, actions },
    StatusChanged { status },
    Error { code, message },
    ProviderSessionId { provider_session_id },
}

pub struct SessionConfig {
    pub step: String,
    pub repo_root: String,
    pub directions: Vec<String>,
    pub area: Option<String>,
    pub wave: Option<String>,
    pub message: Option<String>,
    pub surface: Option<Surface>,
    pub agent: Option<String>,
    pub cwd: Option<String>,
    pub max_turns: Option<u32>,
    pub yolo_mode: bool,
    pub client_has_ui: Option<bool>,
    pub client_compact: Option<bool>,
}

pub struct Session {
    pub id: LfdId,
    pub harness: String,
    pub status: SessionStatus,
    pub wave_run_id: Option<String>,
    pub provider_session_id: Option<String>,
    pub config: SessionConfig,
    pub created_at: OffsetDateTime,
    pub ended_at: Option<OffsetDateTime>,
}

pub struct PersistedSessionEvent {
    pub session_id: LfdId,
    pub seq: i64,
    pub event: SessionEvent,
    pub created_at: OffsetDateTime,
}

pub struct CreateSessionParams {
    pub harness: String,
    pub wave_run_id: Option<String>,
    pub config: SessionConfig,
}
```

Important pattern: sessions are persisted as `(Session, PersistedSessionEvent*)`. Usage aggregation, SSE replay/follow, and UI state are derived from the event stream rather than a separate denormalized conversation table.

## Engine Types and Prompt/Flow Model

### Flow DSL

Source: `rust/loopflow/src/engine/flow.rs`

```rust
pub struct Step {
    pub name: String,
    pub agent: Option<String>,
    pub default_agent: Option<String>,
    pub directions: Vec<String>,
    pub action_style: Option<String>,
    pub interactive: Option<bool>,
    pub content: Option<String>,
    pub fast_path: Option<String>,
}

pub enum FlowItem {
    Step(Step),
    Op(Op),
    And { branches: Vec<FlowItem> },
    FlowRef(String),
    Or(OrDef),
}

pub struct Op {
    pub command: String,
    pub args: Vec<String>,
}

pub struct OrDef {
    pub router: Option<String>,
    pub paths: HashMap<String, OrPath>,
}

pub struct OrPath {
    pub flow: Option<String>,
    pub step: Option<String>,
    pub description: String,
    pub direction: Vec<String>,
}

pub struct Flow {
    pub name: String,
    pub items: Vec<FlowItem>,
}

pub struct ConcreteStep {
    pub step: Step,
    pub flow_parents: Vec<String>,
}

pub struct ConcreteAndBranch {
    pub steps: Vec<ConcreteStep>,
    pub flow_parents: Vec<String>,
    pub label: String,
    pub directions: Vec<String>,
}

pub struct ConcreteAnd {
    pub branches: Vec<ConcreteAndBranch>,
    pub flow_parents: Vec<String>,
}

pub struct ConcreteOr {
    pub router: Option<String>,
    pub paths: HashMap<String, OrPath>,
    pub flow_parents: Vec<String>,
}

pub struct ConcreteOp {
    pub item: Op,
    pub flow_parents: Vec<String>,
}

pub enum ConcreteItem {
    Step(ConcreteStep),
    Op(ConcreteOp),
    And(ConcreteAnd),
    Or(ConcreteOr),
}

pub enum FlowAction {
    RunStep { step: ConcreteStep },
    RunOps { ops: ConcreteOp },
    WaitInteractive { step: ConcreteStep },
    And { fork: ConcreteAnd },
    Or { branch: ConcreteOr },
    Complete,
}

pub struct Direction {
    pub name: String,
    pub content: String,
    pub source: PathBuf,
}
```

Pattern:

- Repo-local lookup first: `.lf/flows`, `.lf/steps`, `.lf/directions`.
- Fallback to embedded built-ins from `rust/loopflow/src/engine/builtins/`.
- Final fallback for steps: `.agents/skills/<name>/SKILL.md`.
- A bare step name can auto-wrap into a one-step flow.
- `And` = parallel branches; `Or` = routed branch selection with an optional router step and verdict file.

### Prompt assembly

Source: `rust/loopflow/src/engine/prompt.rs`, `launch.rs`, `structured_reply.rs`

```rust
pub enum DocumentSource {
    Step,
    Direction,
    Diff,
    RepoDoc,
    Scratch,
    Wave,
    WaveMemory,
    Summary,
    Area,
    Clipboard,
}

pub struct RelatedRepoContext {
    pub repo_id: RepoId,
    pub path: PathBuf,
}

pub struct Document {
    pub path: String,
    pub content: String,
    pub source: DocumentSource,
}

pub struct ContextBreakdown {
    pub source_tokens: HashMap<DocumentSource, usize>,
    pub source_counts: HashMap<DocumentSource, usize>,
    pub documents: Vec<DocumentEntry>,
    pub system_tokens: usize,
    pub step_name: Option<String>,
    pub direction_names: Vec<String>,
    pub diff_tier: DiffTier,
    pub diff_file_count: usize,
    pub area_name: Option<String>,
    pub area_doc_count: usize,
    pub has_clipboard: bool,
    pub wave_name: Option<String>,
}

pub struct GatherSpec {
    pub sources: Vec<DocumentSource>,
    pub repo_root: PathBuf,
    pub files: Vec<String>,
    pub area: Option<String>,
    pub wave: Option<String>,
    pub related_repos: Vec<RelatedRepoContext>,
}

pub struct GatherContextOpts {
    pub repo_root: PathBuf,
    pub step: Option<String>,
    pub message: Option<String>,
    pub surface: Surface,
    pub directions: Vec<String>,
    pub files: Vec<String>,
    pub sources: Vec<DocumentSource>,
    pub area: Option<String>,
    pub wave: Option<String>,
    pub related_repos: Vec<RelatedRepoContext>,
}

pub enum PromptFormatMode { Full, Context, Task }
pub enum Surface { Cli, ConcertoMac, ConcertoIphone, Headless }
pub enum DiffTier { UnifiedDiff, StatOnly, None }

pub struct PromptComponents {
    pub surface: Surface,
    pub docs: Vec<Document>,
    pub diff: Option<String>,
    pub diff_files: Vec<Document>,
    pub step: Option<Step>,
    pub repo_root: String,
    pub clipboard: Option<String>,
    pub directions: Vec<Direction>,
    pub summaries: Vec<Document>,
    pub wave_memory: Option<Document>,
    pub wave: Option<String>,
    pub loopflow_doc: Option<String>,
    pub voice_doc: Option<String>,
    pub message: Option<String>,
    pub diff_tier: DiffTier,
    pub diff_file_count: usize,
    pub area_docs: Vec<Document>,
    pub area: Option<String>,
}

pub struct ContextSourceOverrides {
    pub lfdocs: Option<bool>,
    pub diff_files: Option<bool>,
    pub diff: Option<bool>,
    pub clipboard: Option<bool>,
}

pub struct LaunchPromptInput {
    pub repo_root: PathBuf,
    pub step: Option<String>,
    pub resolved_step: Option<Step>,
    pub surface: Surface,
    pub directions: Vec<String>,
    pub area: Option<String>,
    pub wave: Option<String>,
    pub message: Option<String>,
    pub agent: Option<String>,
    pub cwd: Option<PathBuf>,
    pub max_turns: Option<u32>,
    pub yolo_mode: bool,
    pub include_config_directions: bool,
    pub include_config_area: bool,
    pub source_overrides: ContextSourceOverrides,
    pub summary: Option<String>,
    pub client_context: ClientContext,
    pub related_repos: Vec<RelatedRepoContext>,
}

pub struct PreparedLaunchPrompt {
    pub config: AgentConfig,
    pub components: PromptComponents,
    pub breakdown: ContextBreakdown,
    pub prompt: String,
}

pub struct StructuredReply {
    pub name: String,
    pub description: String,
    pub guidance: String,
}

pub struct ClientContext {
    pub has_ui: bool,
    pub compact: bool,
}
```

Critical prompt pipeline:

1. Resolve config from `~/.lf/config.yaml` + `.lf/config.yaml`.
2. Merge directions from step frontmatter + config + request.
3. Gather sources (`repo docs`, `diff`, `clipboard`, `wave`, `area`, related repos).
4. Budget into `DEFAULT_CONTEXT_BUDGET = 75_000`.
5. Build `system_prompt` + `task_prompt`.
6. Attach structured reply guidance when UI context requires it.

Direct quote paths/constants:

- `".lf/config.yaml"`
- `"~/.lf/config.yaml"`
- `".lf/steps"`
- `".lf/flows"`
- `".lf/directions"`
- `".lf/fork-manifest.json"`
- `"wave-summary"`

### Agent launch / git / worktrees

Source: `rust/loopflow/src/engine/agent.rs`, `git.rs`, `worktree.rs`, `worktrees.rs`, `naming.rs`

```rust
pub struct AgentConfig {
    pub system_prompt: String,
    pub task_prompt: String,
    pub agent: Option<String>,      // "claude:opus", "codex", "opencode:provider/model", ...
    pub max_turns: Option<u32>,
    pub cwd: Option<PathBuf>,
    pub skip_permissions: bool,
    pub structured_replies: Vec<StructuredReply>,
    pub directive_relay: Option<PathBuf>,
}

pub struct ProcessConfig {
    pub auto: bool,
    pub stream: bool,
    pub context_file: Option<PathBuf>,
    pub stream_format: StreamFormat,
    pub timeout: Option<Duration>,
}

pub struct AgentCapabilities {
    pub chrome: bool,
}

pub struct ClaudeArgs {
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub system_prompt_file: Option<PathBuf>,
    pub skip_permissions: bool,
    pub max_turns: Option<u32>,
    pub stream: bool,
    pub chrome: bool,
    pub resume_id: Option<String>,
}

pub struct LaunchResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub struct RebaseResult {
    pub success: bool,
    pub conflicts: Option<Vec<PathBuf>>,
    pub new_head: Option<String>,
}

pub struct BranchInfo {
    pub old_branch: String,
    pub old_head: String,
    pub new_branch: String,
}

pub enum LandStrategy { SquashMerge, LocalMerge }

pub struct LandResult {
    pub merged_commit: String,
    pub branch_deleted: bool,
}

pub struct WorktreeState {
    pub branch: Option<String>,
    pub path: PathBuf,
    pub base_branch: Option<String>,
    pub merged: bool,
    pub squash_merged: bool,
    pub prunable: bool,
    pub fresh: bool,
    pub dirty: bool,
    pub remote_gone: bool,
}

pub struct CreateWorktreeResult {
    pub path: PathBuf,
    pub branch: String,
    pub base_branch: Option<String>,
    pub base_commit: Option<String>,
}

pub struct BranchNameParts {
    pub user: Option<String>,
    pub name: String,
    pub timestamp: Option<String>,
    pub words: Option<String>,
}
```

Key operational facts:

- Harnesses supported in code paths: `claude`, `codex`, `gemini`, `opencode`.
- Branch/worktree naming is schema-driven via `BranchNameConfig.schema_` (default `"{user}.{name}.{timestamp}"`).
- Worktree sibling naming uses `repo_name.wave_name` style directories via `worktree_path_with_config`.
- `try_fast_path(cmd, cwd)` can execute a shell fast-path before spawning an LLM.
- `StreamParser` normalizes stream-json from Claude, Codex, Gemini, and OpenCode into `StreamEvent::{Text, ToolUse, Result}`.

## Store Layer

Source: `rust/loopflow/src/lfd/store/mod.rs`

Storage abstraction:

```rust
pub enum StorageConfig {
    Sqlite { path: PathBuf },
    Postgres { database_url: String },
}

pub struct Store { /* backend-dispatch wrapper */ }
pub type SharedStore = Arc<Store>;

pub struct SessionFilters {
    pub wave: Option<String>,
    pub flow: Option<String>,
    pub step: Option<String>,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

pub enum CredentialType { OAuth, ApiKey }

pub struct ProviderToken {
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub login: Option<String>,
    pub updated_at: i64,
    pub credential_type: CredentialType,
}

pub struct SecretsProviderConfig {
    pub provider: String,
    pub project: Option<String>,
    pub config: Option<String>,
    pub updated_at: i64,
}
```

Trait split:

- `WaveStateStore`: waves, runs, PR state, attention, queue blocks, triggers, activations, summaries, chat memory/messages.
- `RepoStore`: registered repos + parent/child edges.
- `ExecutionStore`: fork runs + `AgentRun` lifecycle.
- `SessionStore`: `Session`, status, provider session IDs, event append/list/filter.
- `TokenStore`: provider OAuth/API tokens.
- `SecretsProviderStore`: selected secrets-provider config.
- `StoreAdmin`: health check + schema version.

Top-level helpers:

```rust
pub async fn open_store(cfg: &StorageConfig) -> StoreResult<Store>;
pub async fn migrate_store(cfg: &StorageConfig, status_only: bool) -> StoreResult<String>;
```

Pattern: `Store` is the single service dependency injected into `HttpState`, `WaveExecutor`, `SessionManager`, and provider/secrets flows. SQLite and Postgres dispatch is explicit in `store/mod.rs`.

## Orchestration Types

### Executor and queue

Source: `rust/loopflow/src/lfd/executor/mod.rs`, `executor/wave/mod.rs`, `queue.rs`, `triggers/mod.rs`

```rust
pub trait AgentExecutor {
    async fn run(&self, cmd: Vec<String>, cwd: &Path, context: AgentRunContext<'_>) -> Result<i32>;
    async fn terminate(&self, agent_id: &str) -> Result<()>;
    async fn write_to_workspace(&self, cwd: &Path, relative_path: &str, content: &[u8]) -> Result<()>;
    async fn remove_from_workspace(&self, cwd: &Path, relative_path: &str) -> Result<()>;
    async fn cleanup_ephemeral_worktree(&self, _repo: &Path, worktree: &Path) -> Result<()>;
    async fn recover_startup(&self, _output: &OutputHub) -> Result<StartupRecovery>;
    async fn ensure_wave_workspace(&self, _wave: &Wave) -> Result<()>;
    async fn cleanup_wave_workspace(&self, _wave: &Wave) -> Result<()>;
}

pub struct WaveExecutor {
    store: SharedStore,
    scheduler: Arc<Scheduler>,
    output: OutputHub,
    runner: Arc<dyn AgentExecutor>,
    event_hub: EventHub,
    sessions: SessionManager,
    executor_type: ExecutorType,
    github_config: GitHubConfig,
}

pub enum QueueTrigger { RunCompleted, WebhookMerged, Poll }
pub enum QueueRole { Ready, Draft, Blocked, Merged, Superseded }
pub enum QueueNextAction { OpenPr, ResolveConflict, CombinePrs, AwaitMerge }

pub struct QueueRunView {
    pub role: QueueRole,
    pub block_reason: Option<QueueBlockReason>,
    pub blocked_at: Option<OffsetDateTime>,
    pub next_action: QueueNextAction,
}
```

Primary orchestration APIs:

```rust
pub async fn create_parallel_wave_run(...);
pub async fn create_wave_run_with_id(...);
pub fn ensure_wave_worktree(main_repo: &Path, wave_name: &str) -> anyhow::Result<(String, String)>;

pub async fn reconcile_wave_queue(...);
pub async fn handle_pr_merged(...);
pub fn project_queue_views<F>(...);

pub fn spawn_activation_dispatcher(...);
pub async fn enqueue_pending_activation(...);
pub async fn spawn_immediate_activation(...);
pub async fn dispatch_wave_if_ready(...);

pub fn spawn_loop_ticker(...);
pub fn spawn_watch_poller(...);
pub fn spawn_cron_poller(...);
pub fn spawn_queue_reconciler(...);
pub fn spawn_summary_refresh(...);
pub fn spawn_token_refresh(...);
pub fn spawn_ci_failure_handler(...);
pub fn spawn_recovery_loop(...);
```

Pattern:

- Triggers create `PendingActivation` or directly spawn runs.
- `WaveExecutor.execute(run_id)` advances the expanded flow item-by-item.
- Queue reconciliation projects stack PR state into `QueueRunView`.
- Output is broadcast through `OutputHub`; control-plane events through `EventHub`.

## Event Types

Source: `rust/loopflow/src/lfd/types/event.rs`, `lfd/events.rs`

`EventHub` is a `tokio::sync::broadcast` bus over `Event`.

Important `Event` families:

- connection: `Connected`, `Ping`
- auth: `auth.flow_started`, `auth.connected`, `auth.failed`, `auth.disconnected`, `auth.token_refreshed`, `auth.refresh_failed`, `auth.refresh_required`
- secrets: `secrets.connected`, `secrets.synced`, `secrets.disconnected`
- waves: `WaveCreated`, `WaveUpdated`, `WaveDeleted`, `WaveStarted`, `WaveStopped`, `WaveWaiting`, `CiFailure`, `ActivationQueued`, `ActivationCoalesced`, `ActivationDropped`
- worktree: `WorktreeUpdated`
- agents: `AgentStarted`, `AgentEnded`
- attention: `AttentionCreated`, `AttentionUpdated`, `AttentionResolved`
- output: `OutputLine`

## HTTP API

### Shared API state and DTOs

Source: `rust/loopflow/src/lfd/http/state.rs`, `dto.rs`, `mod.rs`

```rust
pub struct HttpState {
    pub store: SharedStore,
    pub scheduler: Arc<Scheduler>,
    pub executor: Arc<WaveExecutor>,
    pub event_hub: EventHub,
    pub output_hub: OutputHub,
    pub provider_auth: ProviderAuthService,
    pub auth: AuthProvider,
    pub registration: Option<RegistrationClient>,
    pub started_at: OffsetDateTime,
    pub github: GitHubConfig,
    pub http_security: HttpSecurityConfig,
    pub auth_failure_throttle: AuthFailureThrottle,
    pub ci_failure_cache: Arc<Mutex<HashSet<String>>>,
    pub sessions: SessionManager,
}

pub struct ListResponse<T> {
    pub object: String, // always "list"
    pub data: Vec<T>,
    pub has_more: bool,
}

pub struct WaveDto { /* Wave + git/PR/flow/trigger projection */ }
pub struct WaveRunDto { /* WaveRun + PR state + queue view projection */ }
pub struct TriggerDto { /* Trigger transport shape */ }
pub struct AttentionItemDto { /* Attention transport shape */ }
pub struct SessionUsageDto { /* aggregated token usage for one session */ }
pub struct WaveUsageDto { /* aggregate across a wave */ }
pub struct UsageSummaryDto { /* grouped aggregate */ }
pub struct UsageTimeseriesDto { /* bucketed aggregate */ }
pub struct WorktreeDto { pub path, pub branch, pub merged, pub prunable, pub wave_id }
pub struct RepoDto { pub path, pub name, pub repo_id, pub wave_count, pub registered, pub added_at }
```

Router shape (`router(state)`):

- unauthenticated: `/health`, `/metrics`, `/hooks/git`, `/v0/hooks/github`
- authenticated: `/status`, `/ws`, `/v0/...`

Exact route families under `/v0`:

- `/auth`
- `/secrets`
- `/providers`
- `/flows`
- `/repos`
- `/sessions`
- `/attention`
- `/waves`
- `/usage`
- `/tokens/revoke`
- `/wave_runs`
- `/worktrees`

Security patterns:

- auth middleware wraps all non-hook API routes
- query params containing auth-like keys are rejected
- body limits differ for normal JSON vs hook payloads
- error text is sanitized with `sanitize_operator_message` for untrusted content

### Route modules

Source: `rust/loopflow/src/lfd/http/routes/*`

```rust
// Waves
pub async fn list_waves_handler(...);
pub async fn create_wave_handler(...);
pub async fn get_wave_handler(...);
pub async fn update_wave_handler(...);
pub async fn delete_wave_handler(...);
pub async fn run_wave_handler(...);
pub async fn check_wave_ci_handler(...);
pub async fn add_trigger_handler(...);
pub async fn remove_trigger_handler(...);
pub async fn list_triggers_handler(...);
pub async fn list_activations_handler(...);
pub async fn stop_wave_handler(...);
pub async fn restart_step_handler(...);
pub async fn continue_wave_handler(...);
pub async fn land_wave_handler(...);
pub async fn next_wave_handler(...);
pub async fn combine_wave_handler(...);
pub async fn get_wave_file_diff_handler(...);

// Sessions
pub async fn create_session_handler(...);
pub async fn get_session_handler(...);
pub async fn send_session_input_handler(...);
pub async fn stream_session_events_handler(...); // SSE replay + follow
pub async fn delete_session_handler(...);

// Wave runs / logs
pub async fn list_wave_runs_handler(...);
pub async fn list_wave_runs_for_wave_handler(...);
pub async fn wave_logs_handler(...); // replay persisted log then follow OutputHub

// Auth
pub async fn list_auth_handler(...);
pub async fn get_auth_handler(...);
pub async fn start_auth_handler(...);
pub async fn complete_auth_handler(...);
pub async fn disconnect_auth_handler(...);
pub async fn configure_credential_handler(...);

// Providers / secrets / usage / repos / attention / worktrees / hooks / tokens
pub async fn list_providers_handler(...);
pub async fn secrets_status_handler(...);
pub async fn list_projects_handler(...);
pub async fn list_configs_handler(...);
pub async fn select_secrets_handler(...);
pub async fn sync_secrets_handler(...);
pub async fn disconnect_secrets_handler(...);
pub async fn get_session_usage_handler(...);
pub async fn get_wave_usage_handler(...);
pub async fn get_usage_summary_handler(...);
pub async fn get_usage_timeseries_handler(...);
pub async fn list_repos_handler(...);
pub async fn add_repo_handler(...);
pub async fn remove_repo_handler(...);
pub async fn add_child_handler(...);
pub async fn remove_child_handler(...);
pub async fn list_children_handler(...);
pub async fn list_parents_handler(...);
pub async fn list_attention_handler(...);
pub async fn list_attention_history_handler(...);
pub async fn get_attention_handler(...);
pub async fn patch_attention_handler(...);
pub async fn list_worktrees_handler(...);
pub async fn git_hook_handler(...);
pub async fn github_webhook_handler(...);
pub async fn revoke_tokens_handler(...);
```

Special route behavior:

- `/v0/flows` introspects repo-local + built-in flows/steps/directions and returns `supported_harnesses`.
- `/v0/waves/{wave_id}/logs` is plain text, not SSE.
- `/v0/sessions/{id}/events` is SSE with replay completion sentinel `"session.replay_completed"`.
- `/v0/tokens/revoke` requires local admin auth and operates on the connection token ledger.

## Provider / GitHub / Secrets Integration

### Provider catalog

Source: `rust/loopflow/src/lfd/providers.rs`

```rust
pub struct ProviderInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub models: &'static [ModelInfo],
    pub is_default: bool,
    pub auth_provider: Option<Provider>,
    pub model_rates: &'static [ModelRate],
}

pub struct ModelInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub is_default: bool,
}

pub struct CostRates {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_read_per_mtok: f64,
    pub cache_write_per_mtok: f64,
}
```

Current catalog literals:

- `claude`: models `opus`, `sonnet`, `haiku`
- `codex`: model `codex`
- `opencode`: model `opencode`

Important API:

```rust
pub fn lookup_cost_rates(harness: &str, model: &str) -> Option<CostRates>;
pub fn merge_auth(catalog: &[ProviderInfo], snapshots: &[ProviderAuthSnapshot]) -> Vec<ProviderInfoDto>;
```

### GitHub webhook / PR state types

Source: `rust/loopflow/src/lfd/github.rs`

```rust
pub struct GitHubCheckRunEvent { pub action, pub check_run, pub repository }
pub struct GitHubPullRequestEvent { pub action, pub pull_request, pub repository }
pub struct GitHubPushEvent { pub git_ref, pub before, pub after, pub repository }

pub struct GitHubPullRequestState {
    pub number: u32,
    pub state: LivePrState,
    pub is_draft: bool,
    pub head_ref: String,
    pub head_sha: String,
    pub base_ref: String,
    pub updated_at: OffsetDateTime,
    pub merged_at: Option<OffsetDateTime>,
}
```

Key functions:

```rust
pub fn into_live_pull_request_state(repo_id: String, pull_request: GitHubPullRequestState, synced_at: OffsetDateTime) -> LivePullRequestState;
pub fn verify_webhook_signature(secret: &str, body: &[u8], signature_header: &str) -> bool;
pub fn github_repo_from_local(repo_path: &Path) -> Option<String>;
pub fn github_repo_from_remote_url(remote: &str) -> Option<String>;
pub async fn poll_check_runs(repo_full_name: &str, branch: &str, token: &str) -> Result<Vec<CheckRun>, String>;
pub async fn fetch_pull_request(...);
```

### Secrets provider

Source: `rust/loopflow/src/lfd/secrets.rs`

```rust
pub struct SuppliedKey {
    pub env_name: String,
    pub provider: String,
    pub present: bool,
}

pub struct DopplerProject {
    pub slug: String,
    pub name: String,
}

pub struct DopplerConfig {
    pub name: String,
    pub environment: String,
}

pub struct SecretsProviderStatus {
    pub provider: String,
    pub connected: bool,
    pub project: Option<String>,
    pub config: Option<String>,
    pub keys: Vec<SuppliedKey>,
}
```

Exact mappings in code:

- `ANTHROPIC_API_KEY -> Provider::Claude`
- `OPENAI_API_KEY -> Provider::Codex`

Primary APIs:

```rust
pub async fn list_projects(store: &SharedStore) -> Result<Vec<DopplerProject>, SecretsError>;
pub async fn list_configs(store: &SharedStore, project: &str) -> Result<Vec<DopplerConfig>, SecretsError>;
pub async fn fetch_secrets(token: &str, project: &str, config: &str) -> Result<HashMap<String, String>, SecretsError>;
pub fn smart_default_config(configs: &[DopplerConfig]) -> Option<&DopplerConfig>;
pub async fn sync_secrets(store: &SharedStore, config: &SecretsProviderConfig, event_hub: Option<&EventHub>) -> Result<Vec<SuppliedKey>, SecretsError>;
pub async fn clear_secrets_credentials(store: &SharedStore, event_hub: Option<&EventHub>);
pub async fn secrets_status(store: &SharedStore) -> SecretsProviderStatus;
```

## Python Client Surface

### Pydantic models

Source: `python/loopflow/models.py`

Python mirrors a smaller transport subset of the Rust DTOs:

```python
class Trigger(BaseModel):
    id: Optional[str]
    signal: str
    source_wave_id: Optional[str]
    flow: Optional[str]
    max_iterations: Optional[int]

class PullRequest(BaseModel):
    url: str
    number: Optional[int]
    state: Optional[str]
    title: Optional[str]
    branch: Optional[str]

class WaveRun(BaseModel):
    id: str
    wave_id: str
    iteration: int
    step_index: int
    status: str
    local_worktree: str
    remote_branch: str
    pr: Optional[PullRequest]
    started_at: Optional[datetime]
    ended_at: Optional[datetime]
    error: Optional[str]
    flow_parents: list[str]

class FlowStep(BaseModel):
    type: str
    name: str

class Wave(BaseModel):
    id: str
    name: str
    repo: str
    mode: str = "loop"
    primary_flow: str = "ship-roadmap"
    cron: Optional[str]
    direction: list[str]
    area: list[str]
    triggers: list[Trigger]
    status: str
    iteration: int
    local_worktree: Optional[str]
    remote_branch: Optional[str]
    commits: list[CommitEntry]
    diff_stat: Optional[str]
    flow_steps: list[FlowStep]
    active_run: Optional[WaveRun]
    created_at: Optional[datetime]
    branch: Optional[str]
    pr_url: Optional[str]
    pr_state: Optional[str]

class SessionConfig(BaseModel):
    agent: Optional[str]
    cwd: Optional[str]
    system_prompt: Optional[str]
    max_turns: Optional[int]
    yolo_mode: bool = False

class Session(BaseModel):
    id: str
    object: str = "session"
    harness: str
    status: str
    wave_run_id: Optional[str]
    provider_session_id: Optional[str]
    config: SessionConfig
    created_at: Optional[datetime]
    ended_at: Optional[datetime]

class SessionEventEnvelope(BaseModel):
    seq: Optional[int]
    event: dict[str, Any]
```

Notable mismatch: Python `SessionConfig` is narrower than Rust `SessionConfig`; it exposes only a subset of fields.

### `Client` API

Source: `python/loopflow/client.py`, `api.py`

Environment resolution:

- base URL: `LFD_URL` or `http://{LFD_HOST}:{LFD_PORT}` with defaults `127.0.0.1:2486`
- auth token: `LFD_TOKEN`, else `~/.lf/session-token` for local URLs

Primary `Client` methods:

```python
def health(self) -> dict[str, Any]
def status(self) -> dict[str, Any]

def auth_status(self, provider: Optional[str] = None) -> list[AuthProviderStatus] | AuthProviderStatus
def start_auth(self, provider: str) -> AuthFlow
def complete_auth(self, provider: str, code: str) -> None
def disconnect_auth(self, provider: str) -> AuthProviderStatus
def configure_api_key(self, provider: str, api_key: str) -> AuthProviderStatus

def providers(self) -> list[ProviderInfo]
def revoke_connection_tokens(self, prefix: Optional[str] = None, revoke_all: bool = False) -> int
def usage_summary(...) -> UsageSummary

def waves(self, repo: Optional[str] = None) -> list[Wave]
def wave(self, name_or_id: str) -> Optional[Wave]
def create_wave(self, name: str, repo: str, flow=None, direction=None, area=None) -> Wave
def update_wave(self, name_or_id: str, flow=None, direction=None, area=None, status=None) -> Wave
def delete_wave(self, name_or_id: str) -> None
def run_wave(self, name_or_id: str, flow=None, direction=None, area=None) -> dict[str, Any]
def add_trigger(self, name_or_id: str, signal: str, flow=None, source_wave_id=None, max_iterations=None) -> dict[str, Any]
def remove_trigger(self, name_or_id: str, trigger_id: str) -> dict[str, Any]
def stop_wave(self, name_or_id: str) -> dict[str, Any]
def land_wave(self, name_or_id: str, strict=None, local=None, create_pr=None, worktree=None) -> dict[str, Any]
def next_wave(self, name_or_id: str) -> dict[str, Any]
def wave_runs(self, wave_id=None, repo=None, limit=None) -> list[WaveRun]
def wave_logs(self, name_or_id: str) -> Iterator[str]

def list_repos(self) -> list[Repo]
def add_repo(self, path: str) -> Repo
def remove_repo(self, path: str) -> None
def add_child(self, owner: str, repo: str, child_owner: str, child_repo: str) -> None
def remove_child(self, owner: str, repo: str, child_owner: str, child_repo: str) -> None
def list_children(self, owner: str, repo: str) -> list[Repo]
def list_parents(self, owner: str, repo: str) -> list[Repo]

def create_session(self, harness: str, wave_run_id: Optional[str] = None, config: Optional[SessionConfig] = None) -> Session
def session(self, session_id: str) -> Optional[Session]
def send_session_input(self, session_id: str, content: str) -> Session
def stop_session(self, session_id: str) -> Session
def stream_session_events(self, session_id: str, after_seq: Optional[int] = None, timeout: float = 60.0) -> Iterator[SessionEventEnvelope]
```

`python/loopflow/api.py` is just a module-level convenience wrapper around a singleton `Client`.

Error model:

```python
class LoopflowError(Exception): ...
class WaveAlreadyRunning(LoopflowError): ...
```

## Python CLI

Source: `python/loopflow/cli.py`

Typer apps:

- `app`
- `auth_app`
- `repos_app`
- `token_app`

Exact command surface:

```text
lfq
lfq list
lfq show <wave>
lfq create <name> <repo>
lfq run <wave>
lfq stop <wave>
lfq delete <wave>
lfq land <wave>
lfq logs <wave>
lfq usage [--wave|--flow|--step|--model|--source|--group-by|--billing|--json]
lfq providers

lfq auth status
lfq auth github
lfq auth claude
lfq auth codex
lfq auth zen
lfq auth asana
lfq auth linear
lfq auth disconnect <provider>
lfq auth configure <provider>

lfq token revoke [PREFIX] [--all]

lfq repos
lfq repos add <path>
lfq repos rm <path>
lfq repos children <owner/repo>
lfq repos parents <owner/repo>
lfq repos add-child <parent> <child>
lfq repos rm-child <parent> <child>
```

CLI characteristics:

- pretty tables via `rich`
- `--json` supported broadly
- OAuth browser flow helpers in CLI
- repo relationships use exact `"owner/repo"` parsing

## Built-In Flows and Steps

### Built-in flow names

From `rust/loopflow/src/engine/builtins/flows/`:

- `code/build`
- `code/deploy`
- `code/design-and-ship`
- `code/grind`
- `code/incident`
- `code/integrate`
- `code/pair`
- `code/qa-deploy`
- `code/qa-fix`
- `code/reorg`
- `code/ship-roadmap-play`
- `code/ship-roadmap`
- `code/ship-wave`
- `code/ship`
- `code/start`
- `ops/release`
- `plan/wave-expand`
- `plan/wave-polish`
- `plan/wave-reduce`
- `scan/scan`
- `tend/tend-tune`
- `tend/tend`

The README in `rust/loopflow/src/engine/builtins/flows/README.md` documents representative step sequences such as:

- `build`: `implement -> compress -> lint -> gate -> update-wave`
- `ship`: `design -> build -> review -> land`
- `pair`: `design -> build`
- `grind`: `research -> iterate -> build -> gate`
- `incident`: `debug -> 5whys -> build`
- `wave-reduce`, `wave-polish`, `wave-expand`: `and(...) -> update-wave`

### Built-in step names

From `rust/loopflow/src/engine/builtins/steps/`:

- code: `ci-fix`, `compress`, `debug`, `gate`, `implement`, `integrate-upstream`, `qa`, `triage`
- interactive: `code-review`, `demo`, `design`, `explore`, `refine`, `review-design`
- ops: `commit`, `init`, `land`, `lint`, `pr`, `rebase`, `release-notes`, `release`, `split-wave`, `synthesize`, `update-wave`, `validate`
- plan: `5whys`, `expand`, `ingest`, `iterate`, `kickoff`, `polish`, `reduce`, `research`
- namespaced scan/tend: `scan/scan-plan`, `scan/scan-report`, `tend/apply-chord`, `tend/assess`, `tend/draft-chord`, `tend/review-chord`, `tend/scan-waves`

`engine/builtins.rs` exposes:

```rust
pub fn get_builtin_step(name: &str) -> Option<&'static str>;
pub fn get_builtin_flow(name: &str) -> Option<&'static str>;
pub fn get_builtin_direction(name: &str) -> Option<&'static str>;
pub fn get_builtin_ops_prompt(name: &str) -> Option<&'static str>;
pub fn builtin_step_names() -> Vec<&'static str>;
pub fn builtin_flow_names() -> Vec<&'static str>;
pub fn builtin_flow_entries() -> impl Iterator<Item = (&'static str, &'static str)>;
```

## Key Patterns To Keep In Mind

- `Wave` is the long-lived plan/config object; `WaveRun` is one execution instance with a frozen `snapshot`.
- Git/PR state is treated as projection data layered onto core store records. DTOs enrich `Wave`/`WaveRun` with current worktree, diff, PR, live PR state, and queue role.
- Repo relationships are a graph over registered repos (`RepoEdge`). Prompt gathering and some orchestration can include related repos.
- Prompt context is source-tagged and token-budgeted. `ContextBreakdown` / `ContextSnapshot` are first-class outputs, not debugging leftovers.
- Interactive sessions and headless wave execution share the same prompt-prep primitives (`prepare_launch_prompt`, `gather_context`, built-in steps/flows).
- Built-ins are embedded in the Rust binary but repo-local `.lf/*` content overrides them.
- HTTP endpoints mostly return DTO projections, not raw store structs.
- The Python package is intentionally thin; real behavior lives in Rust.
- Queue / merge-stack support is a major concept: stack position, stack group, live PR state, queue block reasons, and `QueueRunView` are core to wave lifecycle.
- Secrets, provider auth, session usage, and token revocation are not separate subsystems; they feed the same central `Store` / `HttpState` / event buses.

## Most Important Exact Names

- paths: `".lf/config.yaml"`, `".lf/steps"`, `".lf/flows"`, `".lf/directions"`, `".lf/fork-manifest.json"`, `"rust/loopflow/src/lfd/http/routes/waves.rs"`, `"python/loopflow/client.py"`
- commands: `"git"`, `"gh"`, `"claude"`, `"codex"`, `"gemini"`, `"opencode"`, `"lfq"`
- route prefixes: `"/v0/waves"`, `"/v0/sessions"`, `"/v0/usage/summary"`, `"/v0/providers"`, `"/v0/repos"`, `"/health"`, `"/status"`, `"/ws"`
- core literals: `"ship-roadmap"`, `"ci-fix"`, `"main"`, `"suggest_actions"`
