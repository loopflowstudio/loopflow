# Rust Codebase Summary

Workspace: `rust/loopflow` (main crate) + `rust/loopflow-test-support` (test helpers).
Edition 2021, MIT license.

## Binary Targets

| Binary | Entry | Purpose |
|--------|-------|---------|
| `lf` | `src/bin/lf.rs` | CLI — run steps/flows, git ops, worktree management |
| `lfd` | `src/bin/lfd.rs` | Daemon — persistent wave execution, HTTP API on `127.0.0.1:2486` |
| `lf-prompt` | `src/bin/lf-prompt.rs` | Standalone prompt assembly |
| `lf-agent` | `src/bin/lf-agent.rs` | Standalone agent runner |

## Top-Level Modules (`src/lib.rs`)

```
pub mod agent;   // Anthropic API client, tool trait/registry, agentic turn loop
pub mod chat;    // Chat protocol types, turn completion validation
pub mod engine;  // Flow/step/direction loading, prompt assembly, agent launching, git ops, stream parsing
pub mod lf;      // CLI definition (clap), command dispatch
pub mod lfd;     // Daemon: store, scheduler, executor, HTTP, triggers, events
pub mod ops;     // Git workflow operations: commit, pr, land, rebase, next, abandon, combine
```

---

## `agent` — Anthropic API + Tool System

### `agent::anthropic` — API Client Types

```rust
pub struct Request {
    pub model: String,
    pub max_tokens: u32,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
}

pub struct Message { pub role: String, pub content: MessageContent }
pub enum MessageContent { Text(String), Blocks(Vec<ContentBlock>) }

pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String },
}

pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

pub struct Response {
    pub id: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: Option<String>,  // "end_turn" | "tool_use"
    pub usage: Usage,
}

pub struct Usage { pub input_tokens: u32, pub output_tokens: u32 }

pub enum ApiError {
    MissingApiKey,
    Http { status: u16, body: String },
    Network(String),
    Parse(String),
}

pub fn default_request(model: &str, max_tokens: u32) -> Request;
pub async fn call(request: &Request) -> Result<Response, ApiError>;
```

### `agent::registry` — Tool Trait + Registry

```rust
pub struct ToolResult {
    pub output: String,
    pub event: Option<AgentEvent>,
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn definition(&self) -> ToolDefinition;
    fn call(&self, input: &serde_json::Value) -> ToolResult;
}

pub struct ToolRegistry { tools: Vec<Box<dyn Tool>> }

impl ToolRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, tool: Box<dyn Tool>);
    pub fn definitions(&self) -> Vec<ToolDefinition>;
    pub fn dispatch(&self, name: &str, input: &serde_json::Value) -> Option<ToolResult>;
}
```

### `agent::turn` — Agentic Turn Loop

```rust
pub struct TurnConfig {
    pub max_iterations: u32,
    pub timeout: Duration,
    pub system: Option<String>,
}

pub struct TurnResult {
    pub response: String,
    pub iterations: u32,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

pub enum TurnError {
    MaxIterations(u32),
    Timeout(Duration),
    Api(ApiError),
    NoTextResponse,
}

/// Run agentic loop: send prompt, dispatch tool calls, repeat until text response or limit.
pub async fn run(
    prompt: &str,
    config: &TurnConfig,
    registry: &ToolRegistry,
) -> Result<TurnResult, TurnError>;
```

### `agent::tools` — Built-in Tools

`GetCurrentTime` and `Calculate` built-in tools. `pub fn default_registry() -> ToolRegistry`.

---

## `chat` — Chat Protocol

### `chat::contract` — Protocol Types

```rust
pub enum UserMessagePhase { Progress, Final }

pub struct SendMessageArgs { pub content: String, pub phase: UserMessagePhase }
pub struct WorkspaceSnapshot { pub branch: String, pub head_sha_at_start: String }
pub struct ChatTurnRequest { pub wave_id: String, pub content: String, pub token_history_budget: usize }

pub struct MemoryEditLog { pub op: String, pub block: String, pub detail: String }
pub struct ToolCallLog { pub tool: String, pub args: String, pub result_summary: String }
pub struct ContextSnapshot { pub memory_tokens: usize, pub history_tokens: usize, pub total_tokens: usize }

pub struct ChatTurnResult {
    pub id: String,
    pub response: String,
    pub final_message_seen: bool,
    pub memory_edits: Vec<MemoryEditLog>,
    pub tool_calls: Vec<ToolCallLog>,
    pub context: Option<ContextSnapshot>,
    pub snapshot: Option<WorkspaceSnapshot>,
}

/// Events streamed during a chat turn.
pub enum AgentEvent {
    Message { content: String, phase: UserMessagePhase },
    ToolCall { tool: String, args: String },
    ToolResult { tool: String, summary: String },
    MemoryEdit { op: String, block: String, detail: String },
    Done { context: Option<ContextSnapshot> },
    Failed { code: String, message: String },
}
```

### `chat::completion` — Turn Validation

```rust
pub enum CompletionError { MissingFinalMessage, MultipleFinalMessages, FinalMessageOnFailedTurn }

pub fn validate_turn_completion(events: &[AgentEvent]) -> Result<(), CompletionError>;
pub fn final_message_count(events: &[AgentEvent]) -> usize;
```

---

## `engine` — Core Engine

### `engine::flow` — Flow/Step/Direction System

```rust
pub struct Step {
    pub name: String,
    pub model: Option<String>,
    pub directions: Vec<String>,
    pub interactive: bool,
    pub content: String,
}

pub enum FlowItem {
    Step(Step),
    Fork { branches: Vec<FlowItem>, select: ForkSelect },
    FlowRef(String),
}

pub enum ForkSelect { All, One, Prompt { prompt: String } }

pub struct Flow { pub name: String, pub items: Vec<FlowItem> }

/// Concrete items after flow expansion (FlowRef resolved, flow_parents tracked).
pub struct ConcreteStep { pub step: Step, pub flow_parents: Vec<String> }
pub struct ConcreteFork {
    pub branches: Vec<Vec<ConcreteItem>>,
    pub select: ForkSelect,
    pub flow_parents: Vec<String>,
}
pub enum ConcreteItem { Step(ConcreteStep), Fork(ConcreteFork) }

pub enum FlowAction {
    RunStep { step: ConcreteStep },
    WaitInteractive { step: ConcreteStep },
    Fork { fork: ConcreteFork },
    Complete,
}

pub struct Direction { pub name: String, pub content: String, pub source: PathBuf }

pub fn load_flow(name: &str, repo: &Path) -> Result<Flow, LoadError>;
pub fn expand_flow(flow: &Flow, repo: &Path) -> Result<Vec<ConcreteItem>, LoadError>;
pub fn load_step(name: &str, repo: &Path) -> Result<Step, LoadError>;
pub fn load_direction(name: &str, repo: &Path) -> Result<Direction, LoadError>;
pub fn next_action(items: &[ConcreteItem], step_index: usize) -> FlowAction;
```

**Lookup paths** (checked in order):
- Steps: `.lf/steps/`, `.claude/commands/`, `~/.lf/steps/`, `~/.claude/commands/`, builtins
- Flows: `.lf/flows/`, `~/.lf/flows/`, builtins
- Directions: `.lf/directions/`, `~/.lf/directions/`, builtins

### `engine::config` — Configuration

```rust
pub struct Config {
    pub agent_model: Option<String>,
    pub yolo: Option<bool>,
    pub chrome: Option<bool>,
    pub push: Option<bool>,
    pub pr: Option<bool>,
    pub land: Option<bool>,
    pub context: Vec<String>,
    pub exclude: Vec<String>,
    pub ide: Option<IdeConfig>,
    pub interactive: Option<bool>,
    pub include_loopflow_doc: Option<bool>,
    pub lfdocs: Vec<String>,
    pub diff: Option<bool>,
    pub diff_files: Option<bool>,
    pub paste: Option<bool>,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub summaries: Vec<SummaryConfig>,
    pub summary_tokens: Option<usize>,
    pub skill_sources: Vec<SkillSourceConfig>,
    pub branch_names: Option<BranchNameConfig>,
    pub lint_check: Option<String>,
    pub autoprune: Option<AutopruneConfig>,
    pub budgets: Option<BudgetConfig>,
    pub rlm_model: Option<String>,
    pub rlm_max_parallel: Option<usize>,
    pub rlm_max_depth: Option<usize>,
}

pub struct BudgetConfig {
    pub area: Option<usize>,    // default 50_000
    pub docs: Option<usize>,    // default 30_000
    pub diff: Option<usize>,    // default 20_000
}

pub struct BranchNameConfig {
    pub schema_: String,  // default: "{user}.{name}.{timestamp}"
}

pub struct SummaryConfig { pub path: String, pub tokens: Option<usize>, pub model: Option<String> }
pub struct SkillSourceConfig { pub name: String, pub prefix: String, pub path: String }
pub struct AutopruneConfig { pub enabled: Option<bool>, pub poll_interval_seconds: Option<u64> }
pub struct IdeConfig { pub warp: Option<bool>, pub cursor: Option<bool>, pub workspace: Option<String> }

/// Parse "provider/model" into (model, provider).
pub fn parse_model(model: &str) -> (String, Option<String>);
/// Load config from repo `.lf/config.yaml`, merged with global `~/.lf/config.yaml`.
pub fn load_config(repo_root: Option<&Path>) -> Result<Option<Config>, LoadError>;
pub fn load_config_or_default(repo_root: Option<&Path>) -> Config;
```

Config merge: repo `.lf/config.yaml` overrides global `~/.lf/config.yaml` scalars; `context`, `exclude`, `skill_sources`, `summaries` lists combine additively.

### `engine::prompt` — Context Gathering + Prompt Assembly

```rust
pub const DEFAULT_CONTEXT_BUDGET: usize = 75_000;

pub enum DiffTier { UnifiedDiff, StatOnly, None }

pub struct Document { pub path: String, pub content: String, pub category: String }

pub struct ContextBreakdown {
    pub step: usize, pub direction: usize, pub system: usize,
    pub diff: usize, pub docs: usize, pub area: usize, pub clipboard: usize,
    // ... additional budget tracking fields
}

pub struct GatherContextOpts {
    pub repo_root: PathBuf,
    pub step: Step,
    pub message: Option<String>,
    pub run_mode: String,
    pub directions: Vec<Direction>,
    pub files: Vec<String>,
    pub lfdocs: Vec<String>,
    pub diff_files: Option<bool>,
    pub diff: Option<bool>,
    pub clipboard: Option<String>,
    pub area: Vec<String>,
    pub wave: Option<Wave>,
}

pub struct PromptComponents {
    pub run_mode: String,
    pub docs: Vec<Document>,
    pub diff: Option<String>,
    pub diff_files: Option<String>,
    pub step: Step,
    pub repo_root: PathBuf,
    pub clipboard: Option<String>,
    pub directions: Vec<Direction>,
    pub summaries: Vec<Document>,
    pub wave: Option<Wave>,
    pub loopflow_doc: Option<String>,
    pub message: Option<String>,
    pub diff_tier: DiffTier,
    pub diff_file_count: usize,
    pub area_docs: Vec<Document>,
    pub area: Vec<String>,
}

pub fn count_tokens(text: &str) -> usize;
pub fn trim_context_with_breakdown(components: PromptComponents, max_tokens: usize) -> (PromptComponents, ContextBreakdown);
pub fn gather_context(opts: &GatherContextOpts) -> Result<PromptComponents, CoreError>;
pub fn format_prompt(components: &PromptComponents) -> String;
pub fn format_context_prompt(components: &PromptComponents) -> String;
pub fn format_task_prompt(components: &PromptComponents) -> String;
pub fn write_prompt_log(repo_root: &Path, prompt: &str, step_name: &str, flow_parents: &[String]) -> Result<PathBuf, CoreError>;
pub fn drop_native_instruction_docs(components: &mut PromptComponents, repo_root: &Path);
```

**Trimming priority** (first to shed): `area_docs` > `summaries` > `docs` > `diff_files` > `diff` > `clipboard`. Step and direction content are never trimmed.

Prompt XML tags: `<lf:step:name>`, `<lf:direction:name>`, `<lf:docs>`, `<lf:diff>`, `<lf:area>`, `<lf:clipboard>`, `<lf:message>`.

### `engine::agent` — Agent Launching

```rust
pub struct LaunchResult { pub exit_code: i32, pub stdout: String, pub stderr: String }

pub struct LaunchConfig {
    pub auto: bool,
    pub stream: bool,
    pub skip_permissions: bool,
    pub model_variant: Option<String>,
    pub chrome: bool,
    pub cwd: Option<PathBuf>,
    pub context_file: Option<PathBuf>,
    pub stream_format: Option<StreamFormat>,
}

pub trait Runner: Send + Sync {
    fn launch(&self, model: &str, prompt: &str, config: &LaunchConfig) -> Result<LaunchResult, CoreError>;
}
pub struct DefaultRunner;

pub fn build_claude_command(config: &LaunchConfig) -> Vec<String>;
pub fn build_codex_command(config: &LaunchConfig) -> Vec<String>;
pub fn build_gemini_command(config: &LaunchConfig) -> Vec<String>;
pub fn build_opencode_command(config: &LaunchConfig) -> Vec<String>;
pub fn build_model_command(model: &str, config: &LaunchConfig) -> Vec<String>;
pub fn build_agent_command(model: &str, prompt: &str, config: &LaunchConfig) -> Vec<String>;
pub fn launch_agent(model: &str, prompt: &str, config: &LaunchConfig) -> Result<LaunchResult, CoreError>;
pub fn check_cli_available(cli: &str) -> bool;
pub fn seed_rlm_env(config: &Config);
```

Supported agents: Claude (`claude`), Codex (`codex`), Gemini (`gemini`), OpenCode (`opencode`).

### `engine::stream` — Multi-Agent Stream Parser

```rust
pub enum StreamEvent {
    Text(String),
    ToolUse { name: String, summary: String },
    Result { subtype: ResultSubtype, cost_usd: Option<f64>, duration_secs: Option<f64> },
}

pub enum ResultSubtype { Success, Error }
pub enum ParseResult { Events(Vec<StreamEvent>), Skipped, Passthrough }
pub enum StreamFormat { Raw, Human(bool /* color */) }

pub struct StreamParser;
impl StreamParser {
    pub fn new() -> Self;
    pub fn feed_line(&mut self, line: &str) -> ParseResult;
}

pub fn render_event(event: &StreamEvent, use_color: bool) -> (String, String);
pub fn format_event(event: &StreamEvent, use_color: bool);
```

Handles Claude, Codex, Gemini, OpenCode JSON output formats.

### `engine::error` — Error Types

```rust
pub enum StoreError { RunNotFound, StepRunNotFound, Other(String) }
pub enum LoadError { FlowNotFound, StepNotFound, DirectionNotFound, InvalidFlow, InvalidStep, InvalidDirection, Io }
pub enum CoreError { FlowNotFound, StepNotFound, InvalidFlow, ExecutionFailed, WorktreeError, StoreError, IoError }
pub enum GitError { CommandFailed { command, stderr }, Io(String) }
```

### `engine::event` — Engine Events

```rust
pub enum EngineEvent {
    StepStarted { run_id, step, timestamp },
    StepCompleted { run_id, step, exit_code, timestamp },
    StepFailed { run_id, step, error, timestamp },
    FlowCompleted { run_id, timestamp },
    FlowFailed { run_id, error, timestamp },
}
```

### `engine::git` — Git Operations

```rust
pub struct RebaseResult { pub success: bool, pub conflicts: Vec<String>, pub new_head: Option<String> }
pub struct BranchInfo { pub old_branch: String, pub old_head: String, pub new_branch: String }
pub enum LandStrategy { SquashMerge, LocalMerge }
pub struct LandResult { pub merged_commit: String, pub branch_deleted: bool }
```

~40 functions: `fetch`, `is_ancestor`, `merge_base`, `checkout`, `checkout_new_branch`, `push_with_upstream`, `delete_remote_branch`, `delete_local_branch`, `branch_rename`, `current_branch`, `get_default_branch`, `is_clean`, `stage_all`, `commit`, `pr_exists`, `pr_create_draft`, `pr_merge_squash_auto`, `sync_main`, `worktree_remove`, `worktree_move`, `worktree_add`, `rev_parse`, `is_squash_merged`, `rebase`, `create_branch`, `push`, `land`, `hash_areas`.

### `engine::worktree` — Low-Level Worktree Ops

```rust
pub fn create_worktree(repo: &Path, worktree: &Path, branch: &str) -> Result<(), CoreError>;
pub fn remove_worktree(worktree: &Path, force_delete_branch: bool) -> Result<(), CoreError>;
pub fn find_worktree_root(path: &Path) -> Result<String, CoreError>;
```

### `engine::worktrees` — High-Level Worktree Management

```rust
pub struct WorktreeState {
    pub branch: Option<String>,
    pub path: PathBuf,
    pub base_branch: Option<String>,
    pub merged: bool,
    pub prunable: bool,
}

pub struct CreateWorktreeResult {
    pub path: PathBuf,
    pub branch: String,
    pub base_branch: Option<String>,
    pub base_commit: Option<String>,
}

pub fn main_repo_root(repo: &Path) -> Result<PathBuf, GitError>;
pub fn worktree_path(repo: &Path, name: &str) -> PathBuf;
pub fn worktree_short_name(repo: &Path) -> Option<String>;
pub fn branch_exists(repo: &Path, branch: &str) -> Result<bool, GitError>;
pub fn list_worktrees(repo: &Path) -> Result<Vec<WorktreeState>, GitError>;
pub fn create_with_schema(
    repo: &Path,
    short_name: &str,
    base: Option<&str>,
    branch_config: Option<&BranchNameConfig>,
) -> Result<CreateWorktreeResult, GitError>;
pub fn schedule_upstream_sync(worktree: PathBuf, branch: String);
pub fn preserve_worktree(repo: &Path, worktree: &Path) -> Result<PathBuf, GitError>;
```

### `engine::fork` — Fork Execution

```rust
pub struct ForkManifest { pub branches: Vec<ForkManifestBranch> }

pub struct ForkManifestBranch {
    pub index: usize,
    pub step: String,
    pub direction: String,
    pub worktree: String,
    pub branch: String,
    pub exit_code: i32,
}

pub fn fork_worktree_path(repo: &Path, index: usize) -> PathBuf;
pub fn merge_directions(base: &[String], extra: &[String]) -> Vec<String>;
pub fn write_fork_manifest(repo: &Path, branches: &[ForkManifestBranch]) -> Result<PathBuf, CoreError>;
pub fn cleanup_fork_worktrees(manifest_path: Option<&Path>, worktrees: &[PathBuf]);
```

### `engine::naming` — Branch Name Generation

```rust
pub fn sanitize_for_branch(value: &str) -> String;
pub fn generate_word_pair() -> String;  // e.g., "magical-musical"
pub fn format_branch_name(short_name: &str, config: &Config, repo: &Path) -> Result<String, GitError>;
```

Schema placeholders: `{user}`, `{name}`, `{timestamp}`, `{words}`.

### `engine::builtins` — Embedded Resources

```rust
pub const LOOPFLOW_DOC: &str = include_str!("builtins/LOOPFLOW.md");
pub const RLM_DOC: &str = include_str!("builtins/RLM.md");

pub fn get_builtin_step(name: &str) -> Option<&'static str>;
pub fn get_builtin_flow(name: &str) -> Option<&'static str>;
pub fn get_builtin_direction(name: &str) -> Option<&'static str>;
pub fn get_builtin_ops_prompt(name: &str) -> Option<&'static str>;
pub fn builtin_step_names() -> Vec<&'static str>;
pub fn builtin_flow_names() -> Vec<&'static str>;
pub fn builtin_direction_names() -> Vec<&'static str>;
```

### `engine::command` — Shell Command Execution

```rust
pub struct CommandError {
    pub command: String,
    pub args: Vec<String>,
    pub status: Option<i32>,
    pub stderr: String,
    pub message: String,
}

pub fn run_command(cmd: &mut Command) -> Result<Output, CommandError>;
```

### `engine::clipboard`

```rust
pub fn read() -> Option<String>;
pub fn write(text: &str) -> Result<(), std::io::Error>;
```

### `engine::platform`

```rust
pub fn open_url(url: &str);
pub fn kill_process(pid: u32);
```

---

## `lf` — CLI

### `lf::mod` — CLI Definition (clap)

```rust
pub struct Cli {
    pub command: Option<Commands>,
    pub list: bool,             // -l
    pub direction: Vec<String>, // -d
    pub area: Vec<String>,      // -a
    pub clipboard: bool,        // -c
    pub model: Option<String>,  // -m
    pub yolo: bool,
    pub interactive: bool,      // -i
    pub batch: bool,            // -b
    pub web: bool,
    pub chrome: bool,
    pub no_chrome: bool,
    pub wave: Option<String>,   // -w
}

pub enum Commands {
    Run { name: String, args: Vec<String> },
    Inline { prompt: String },       // lf : "do something"
    Ops { op: OpsCommand },
    External(Vec<String>),           // fallback to step name
}

pub enum OpsCommand {
    Cp, Doctor, Rebase, Push, Land, Pr, Sync, Next, Commit, Abandon,
    Wt { cmd: WtCommand },
    Shell { cmd: ShellCommand },
}

pub enum WtCommand { Create, Switch, List, Prune, Remove, Ci }
pub enum ShellCommand { Init, Install, Directive }
```

Entry point (`bin/lf.rs`) reorders args so flags like `-c` can appear after step name (`lf debug -c` → `lf -c debug`).

### `lf::commands` — Command Dispatch

```rust
// flow.rs — Execute a multi-step flow
pub fn run(flow: &Flow, message: Option<&str>, cli: &Cli, repo: &Path) -> Result<()>;

// run.rs — Execute a single step
pub fn run(step: Option<&str>, message: Option<&str>, cli: &Cli) -> Result<()>;

// list.rs — List available steps/flows/directions
pub fn show_all() -> Result<()>;

// ops/mod.rs — Dispatch git workflow operations
pub fn run(op: &OpsCommand) -> Result<()>;
```

---

## `lfd` — Daemon

### `lfd::id` — ID Newtype

```rust
pub struct LfdId(String);  // UUID v4

impl LfdId {
    pub fn new() -> Self;                              // Generate random UUID
    pub fn parse(value: &str) -> Result<Self, IdError>; // Validate UUID format
    pub fn as_str(&self) -> &str;
    pub(crate) fn from_raw(value: impl Into<String>) -> Self;
}
// Implements: Display, FromStr, Serialize, Deserialize, rusqlite ToSql/FromSql, tokio_postgres ToSql/FromSql
```

### `lfd::types` — Domain Types

Re-exports: `Agent`, `AgentStatus`, `Event`, `PendingActivation`, `Stimulus`, `StimulusKind`, `Summary`, `PullRequest`, `SidecarKind`, `Wave`, `WaveRun`, `WaveRunKind`, `WaveRunSnapshot`, `WaveRunStatus`, `WaveStatus`.

#### Wave & WaveRun (`types/wave.rs`)

```rust
pub enum WaveStatus { Idle = 1, Running = 2, Waiting = 3, Paused = 4, Failed = 5 }
pub enum WaveRunStatus { Unspecified = 0, Pending = 1, Running = 2, Waiting = 3, Completed = 4, Failed = 5 }
pub enum WaveRunKind { Main = 1, Sidecar = 2 }
pub enum SidecarKind { CiFix = 1 }

pub struct Wave {
    pub id: LfdId,
    pub name: String,              // human label, unique
    pub repo: String,              // absolute repo path
    pub flow: String,              // flow name (e.g. "default")
    pub direction: Vec<String>,    // direction names
    pub area: Vec<String>,         // area paths
    pub status: WaveStatus,
    pub iteration: u32,            // bumped each run
    pub created_at: Option<OffsetDateTime>,
}

pub struct WaveRun {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub snapshot: WaveRunSnapshot, // frozen config at run start
    pub iteration: u32,
    pub step_index: u32,           // current position in flow
    pub status: WaveRunStatus,
    pub worktree: String,          // absolute path
    pub branch: String,
    pub started_at: Option<OffsetDateTime>,
    pub ended_at: Option<OffsetDateTime>,
    pub error: Option<String>,
    pub flow_parents: Vec<String>,
    pub run_kind: WaveRunKind,
    pub sidecar_kind: Option<SidecarKind>,
}

pub struct WaveRunSnapshot {
    pub repo: String, pub flow: String,
    pub direction: Vec<String>, pub area: Vec<String>,
    pub pr: Option<PullRequest>,
}

pub struct PullRequest {
    pub url: String,
    pub number: Option<u32>,
    pub state: Option<String>,
    pub title: Option<String>,
    pub branch: Option<String>,
}
```

#### Agent (`types/agent.rs`)

```rust
pub enum AgentStatus { Unspecified = 0, Running = 1, Waiting = 2, Completed = 3, Failed = 4 }

pub struct Agent {
    pub id: LfdId,
    pub step: String,
    pub repo: String,
    pub worktree: String,
    pub wave_run_id: LfdId,
    pub status: AgentStatus,
    pub started_at: Option<OffsetDateTime>,
    pub ended_at: Option<OffsetDateTime>,
    pub pid: Option<u32>,
    pub container_id: Option<String>,
    pub model: Option<String>,
    pub run_mode: Option<String>,
}
```

#### Stimulus (`types/stimulus.rs`)

```rust
pub enum StimulusKind { Unspecified = 0, Once = 1, Loop = 2, Watch = 3, Cron = 4 }

pub struct Stimulus {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub kind: StimulusKind,
    pub cron: Option<String>,
    pub last_main_sha: Option<String>,
    pub last_triggered_at: Option<OffsetDateTime>,
    pub created_at: Option<OffsetDateTime>,
    pub enabled: bool,
}

pub struct PendingActivation {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub stimulus_id: LfdId,
    pub from_sha: Option<String>,
    pub to_sha: Option<String>,
    pub queued_at: Option<OffsetDateTime>,
}
```

#### Event (`types/event.rs`) — WebSocket Streaming

```rust
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Connected { timestamp },
    Ping,
    WaveCreated { wave_id, name, timestamp },
    WaveUpdated { wave_id, timestamp },
    WaveDeleted { wave_id, timestamp },
    WaveStarted { wave_id, wave_run_id, timestamp },
    WaveStopped { wave_id, timestamp },
    WaveWaiting { wave_id, wave_run_id, step, timestamp },
    CiFailure { wave_id, wave_run_id, pr_number, branch, commit_sha, check_name, logs_url, timestamp },
    WorktreeUpdated { worktree, repo, branch, timestamp },
    AgentStarted { agent_id, step, worktree, timestamp },
    AgentEnded { agent_id, status, timestamp },
    OutputLine { wave_id, agent_id, text, timestamp },
}
```

#### Summary (`types/summary.rs`)

```rust
pub struct Summary {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub content: String,
    pub source_hash: String,
    pub token_budget: u32,
    pub model: String,
    pub created_at: Option<OffsetDateTime>,
}
```

### `lfd::store` — Database Layer

```rust
pub type SharedStore = Arc<dyn RunStore>;

pub struct ForkRun {
    pub id: LfdId,
    pub wave_run_id: LfdId,
    pub step_index: u32,
    pub branch_index: u32,
    pub status: ForkRunStatus,
    pub worktree: String,
}

pub enum ForkRunStatus { Pending = 0, Running = 1, Completed = 2, Failed = 3 }

pub trait RunStore: Send + Sync {
    // --- Wave ---
    fn list_waves(&self) -> StoreResult<Vec<Wave>>;
    fn get_wave(&self, id: &LfdId) -> StoreResult<Wave>;
    fn get_wave_by_name(&self, name: &str) -> StoreResult<Wave>;
    fn create_wave(&self, wave: &Wave) -> StoreResult<()>;
    fn update_wave(&self, wave: &Wave) -> StoreResult<()>;
    fn delete_wave(&self, id: &LfdId) -> StoreResult<()>;

    // --- WaveRun ---
    fn list_wave_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<WaveRun>>;
    fn get_wave_run(&self, id: &LfdId) -> StoreResult<WaveRun>;
    fn get_active_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>>;
    fn get_latest_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>>;
    fn create_wave_run(&self, run: &WaveRun) -> StoreResult<()>;
    fn update_wave_run(&self, run: &WaveRun) -> StoreResult<()>;
    fn fail_orphaned_runs(&self) -> StoreResult<u32>;

    // --- Stimulus ---
    fn list_stimuli(&self, wave_id: &LfdId) -> StoreResult<Vec<Stimulus>>;
    fn list_stimuli_by_kind(&self, kind: StimulusKind) -> StoreResult<Vec<Stimulus>>;
    fn get_stimulus(&self, id: &LfdId) -> StoreResult<Stimulus>;
    fn create_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()>;
    fn update_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()>;
    fn delete_stimulus(&self, id: &LfdId) -> StoreResult<()>;
    fn delete_stimuli_for_wave(&self, wave_id: &LfdId) -> StoreResult<()>;

    // --- PendingActivation ---
    fn list_pending_activations(&self) -> StoreResult<Vec<PendingActivation>>;
    fn create_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()>;
    fn update_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()>;
    fn delete_pending_activations(&self, stimulus_id: &LfdId) -> StoreResult<()>;
    fn get_pending_for_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<Vec<PendingActivation>>;

    // --- ForkRun ---
    fn list_fork_runs(&self, wave_run_id: &LfdId) -> StoreResult<Vec<ForkRun>>;
    fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()>;
    fn delete_fork_runs(&self, wave_run_id: &LfdId) -> StoreResult<()>;

    // --- Agent ---
    fn list_agents(&self) -> StoreResult<Vec<Agent>>;
    fn list_agent_history(&self, wave_run_id: &LfdId) -> StoreResult<Vec<Agent>>;
    fn get_agent(&self, id: &LfdId) -> StoreResult<Agent>;
    fn get_waiting_agent_for_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Agent>>;
    fn start_agent(&self, agent: &Agent) -> StoreResult<()>;
    fn update_agent_status(&self, id: &LfdId, status: AgentStatus) -> StoreResult<()>;
    fn end_agent(&self, id: &LfdId, status: AgentStatus) -> StoreResult<()>;
    fn get_active_agents_for_wave(&self, wave_id: &LfdId) -> StoreResult<Vec<Agent>>;
    fn end_active_agent_for_wave(&self, wave_id: &LfdId, status: AgentStatus) -> StoreResult<()>;
    fn get_stuck_agents(&self) -> StoreResult<Vec<Agent>>;

    // --- Summary ---
    fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>>;
    fn upsert_summary(&self, summary: &Summary) -> StoreResult<()>;
}
```

**Implementations:**
- `SqliteStore` — `SqliteStore::new(path: &Path)`, default `~/.lf/lfd.db`
- `PostgresStore` — `PostgresStore::connect(database_url)`, `migrate(database_url)`, `migrate_async(database_url)`, `migrate_status_async(database_url)`

### `lfd::scheduler` — Slot-Based Concurrency

```rust
pub struct Scheduler {
    max_slots: usize,
    semaphore: Arc<Semaphore>,
    active: Mutex<HashMap<String, OwnedSemaphorePermit>>,
    sessions: Mutex<HashSet<String>>,
}

impl Scheduler {
    pub fn new(max_slots: usize) -> Self;
    pub fn max_slots(&self) -> usize;
    pub fn slots_used(&self) -> u32;
    pub async fn acquire(&self, run_id: &str) -> (bool, Option<String>);
    pub fn release(&self, run_id: &str) -> u32;
    pub fn register_session(&self, wave_id: &str) -> bool;
    pub fn unregister_session(&self, wave_id: &str);
    pub fn has_active_session(&self, wave_id: &str) -> bool;
    pub fn start_loops(
        self: Arc<Self>,
        store: SharedStore,
        executor: WaveExecutor,
        event_hub: EventHub,
        cancel: CancellationToken,
    ) -> Vec<JoinHandle<()>>;
}
```

### `lfd::executor` — Wave Execution Engine

```rust
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    async fn run(&self, cmd: Vec<String>, cwd: &Path, wave_id: &str, agent_id: &str, wave_run_id: &str, output: &OutputHub) -> Result<i32>;
    async fn terminate(&self, agent_id: &str) -> Result<()>;
    async fn recover_startup(&self, output: &OutputHub) -> Result<StartupRecovery>;
    async fn cleanup_wave(&self, wave: &Wave) -> Result<()>;
}

pub struct StartupRecovery {
    pub orphaned_runs_failed: u32,
    pub rehydrated_agents: u32,
    pub lost_agents_failed: u32,
    pub orphaned_containers_removed: u32,
}

pub enum EphemeralOwnerKind { Fork, Sidecar }

pub struct EphemeralWorktree {
    pub path: String,
    pub owner_kind: EphemeralOwnerKind,
    pub owner_id: String,
}

pub struct JanitorReport { pub removed: u32, pub active: u32, pub errors: u32 }

pub struct CiFailure {
    pub wave_id: LfdId,
    pub wave_run_id: LfdId,
    pub pr_number: u32,
    pub branch: String,
    pub commit_sha: String,
    pub check_name: String,
    pub logs_url: String,
}

pub struct LocalProcessExecutor { /* tracks PIDs */ }
pub struct DockerExecutor { /* tracks container IDs, credential mounts */ }

pub struct WaveExecutor {
    store: SharedStore,
    scheduler: Arc<Scheduler>,
    output: OutputHub,
    runner: Arc<dyn AgentExecutor>,
    event_hub: EventHub,
    executor_type: ExecutorType,
}

impl WaveExecutor {
    pub fn new(store, scheduler, output, event_hub, config) -> Result<Self>;
    pub fn executor_type(&self) -> ExecutorType;
    pub async fn recover_startup(&self) -> Result<StartupRecovery>;
    pub async fn cleanup_wave_workspace(&self, wave: &Wave) -> Result<()>;
    pub async fn terminate_agent(&self, agent_id: &LfdId) -> Result<()>;
    pub async fn run_worktree_janitor(&self, repo_roots: &[PathBuf]) -> Result<JanitorReport>;
    pub async fn spawn_ci_fix_agent(&self, failure: &CiFailure) -> Result<()>;
    pub async fn execute(&self, run_id: &LfdId) -> Result<()>;
    pub async fn ensure_summary_fresh(&self, wave: &Wave, run: &WaveRun) -> Result<()>;
}

pub fn create_wave_run_with_id(store: &SharedStore, wave: &Wave, run_id: &LfdId) -> Result<WaveRun, StoreError>;
pub fn ensure_wave_worktree(main_repo: &Path, wave_name: &str) -> anyhow::Result<(String, String)>;
```

### `lfd::events` — Event Broadcasting

```rust
pub struct EventHub {
    sender: broadcast::Sender<Event>,
}

impl EventHub {
    pub fn new(buffer: usize) -> Self;
    pub fn send(&self, event: Event);
    pub fn subscribe(&self) -> broadcast::Receiver<Event>;
}
```

### `lfd::triggers` — Background Trigger Loops

```rust
pub fn spawn_loop_ticker(scheduler, store, executor, event_hub, cancel) -> JoinHandle<()>;
pub fn spawn_watch_poller(store, executor, scheduler, event_hub, cancel) -> JoinHandle<()>;
pub fn spawn_cron_poller(store, executor, scheduler, event_hub, cancel) -> JoinHandle<()>;
pub fn spawn_ci_failure_handler(executor, event_hub, cancel) -> JoinHandle<()>;
pub fn spawn_recovery_loop(store, executor, cancel) -> JoinHandle<()>;
pub fn spawn_summary_refresh(store, executor, event_hub, cancel) -> JoinHandle<()>;
pub fn spawn_run_task_with_slot(store, executor, scheduler, event_hub, run);
```

### `lfd::http` — HTTP API (axum)

Axum router at `127.0.0.1:2486`. Auth middleware on API routes. Wave ID params accept UUID or wave name.

```
Unauthenticated:
  GET  /health
  GET  /metrics
  POST /hooks/git
  POST /v0/hooks/github

Authenticated:
  GET  /status
  GET  /ws                              → WebSocket event stream

  GET  /v0/flows
  GET  /v0/waves
  POST /v0/waves
  GET  /v0/waves/:wave_id
  PUT  /v0/waves/:wave_id
  DELETE /v0/waves/:wave_id
  POST /v0/waves/:wave_id/run
  POST /v0/waves/:wave_id/stop
  POST /v0/waves/:wave_id/continue
  POST /v0/waves/:wave_id/land
  POST /v0/waves/:wave_id/next
  POST /v0/waves/:wave_id/check-ci
  POST /v0/waves/:wave_id/combine
  POST /v0/waves/:wave_id/stimulus
  DELETE /v0/waves/:wave_id/stimulus/:sid
  GET  /v0/waves/:wave_id/stimuli
  GET  /v0/waves/:wave_id/runs
  GET  /v0/waves/:wave_id/logs
  GET  /v0/wave_runs
```

#### HTTP DTOs (`http/dto.rs`)

```rust
pub struct HealthResponse { pub status, uptime_seconds, database, waves_running, agents_active, registration }
pub struct StatusResponse { pub pid, waves_defined, waves_running, agents_active, slots_used, slots_total, registration }
pub struct ListResponse<T> { pub object: String, pub data: Vec<T>, pub has_more: bool }

pub struct WaveDto {
    pub id, object, name, repo, flow: String,
    pub direction, area: Vec<String>,
    pub status: String, pub iteration: u32,
    pub created_at: Option<String>,
    pub local_worktree, remote_branch: Option<String>,
    pub commits: Vec<CommitEntryDto>,
    pub diff_stat: Option<String>,
    pub flow_steps: Vec<String>,
    pub open_pr_count: u32,
    pub stimuli: Vec<StimulusDto>,
    pub active_run: Option<WaveRunDto>,
}

pub struct WaveRunDto {
    pub id, object, wave_id, flow, repo: String,
    pub direction, area: Vec<String>,
    pub iteration, step_index: u32,
    pub status: String,
    pub local_worktree, remote_branch: String,
    pub pr: Option<PullRequestDto>,
    pub started_at, ended_at, error: Option<String>,
    pub flow_parents: Vec<String>,
}

pub struct RunWaveResponse { pub started: bool, pub wave_id: String, pub wave_run_id: Option<String> }
pub struct StopWaveResponse { pub stopped: bool }
pub struct ContinueWaveResponse { pub continued: bool, pub wave_id, wave_run_id: String }
pub struct LandWaveResponse { pub merged: bool }
pub struct NextWaveResponse { pub new_branch: String }
pub struct CombineResponse { pub ok: bool, pub result: CombineResponseResult }
pub struct DeletedResourceResponse { pub id, object: String, pub deleted: bool }
```

### `lfd::service` — System Service Management

```rust
// Platform-specific launchd (macOS) / systemd (Linux) management.
pub fn install() -> Result<(), Box<dyn std::error::Error>>;
pub fn uninstall() -> Result<(), Box<dyn std::error::Error>>;
pub fn start() -> Result<(), Box<dyn std::error::Error>>;
pub fn stop() -> Result<(), Box<dyn std::error::Error>>;
pub fn status() -> Result<(), Box<dyn std::error::Error>>;
```

### `lfd::sessions` — PTY Execution

```rust
pub struct PtyCommand { /* program, args, cwd */ }

impl PtyCommand {
    pub fn new(program: impl Into<String>) -> Self;
    pub fn arg(self, value: impl Into<String>) -> Self;
    pub fn cwd(self, cwd: impl Into<PathBuf>) -> Self;
}

pub fn run_pty_command(command: PtyCommand) -> Result<i32, SessionError>;
```

### Other `lfd` Modules

- `lfd::auth` — `AuthContext` for JWT-based request authentication
- `lfd::config` — `LfdConfig` with auth, database, executor settings
- `lfd::credentials` — `load_jwt()` from `~/.lf/credentials.json`
- `lfd::registration` — `RegistrationClient` + `ConnectionValidator` for `auth.loopflow.studio`
- `lfd::github` — GitHub webhook handling
- `lfd::machine_id` — `machine_id()`, `machine_name()` for registration
- `lfd::obs` — Observability/metrics setup
- `lfd::output` — `OutputHub` for agent output streaming/storage

---

## `ops` — Git Workflow Operations

```rust
pub type OpsResult<T> = Result<T, OpsError>;

pub enum OpsError {
    Git(GitError), Core(CoreError), Load(LoadError), Io(std::io::Error),
    CommandFailed { command, stderr }, AgentFailed(String),
    Parse(String), LintFailed, Message(String),
}

pub struct CommitOptions { pub repo: PathBuf, pub message: Option<String> }
pub struct PrOptions { pub repo: PathBuf, pub push: bool, pub land: bool, pub draft: bool }
pub struct PrResult { pub url: String, pub created: bool }
pub struct PrInfo { pub number: u32, pub url: String, pub title: String, pub state: String }
pub struct LandOptions { pub repo: PathBuf, pub strategy: LandStrategy }
pub struct LandResult { pub merged_commit: String, pub branch_deleted: bool }
pub struct NextOptions { pub repo: PathBuf, pub name: Option<String> }
pub struct NextResult { pub path: PathBuf, pub branch: String }
pub struct RebaseOptions { pub repo: PathBuf }
pub struct RebaseResult { pub success: bool }
pub struct AbandonOptions { pub repo: PathBuf, pub force: bool }
pub struct CombineOptions { pub repo: PathBuf, pub pr_numbers: Vec<u32> }
pub struct CombineResult { pub combined_branch: String, pub pr_url: String }
pub struct Message { pub title: String, pub body: String }
```

| Function | Description |
|----------|-------------|
| `commit_workflow(&CommitOptions)` | Stage, generate message, commit |
| `commit_workflow_traced(&CommitOptions)` | Commit with op tracing |
| `create_or_update_pr(&PrOptions)` | Create draft or update existing PR |
| `current_pr(&Path)` | PR info for current branch |
| `update_pr(&Path, &str, &str)` | Update PR title/body |
| `land(&LandOptions)` | Squash-merge PR, clean up branch |
| `mark_ready(&Path)` | Mark PR ready for review |
| `next_branch(&NextOptions)` | Create next iteration branch |
| `rebase_with_recovery(&RebaseOptions)` | Rebase with conflict recovery |
| `abandon_branch(&AbandonOptions)` | Delete branch and close PR |
| `combine_prs(&CombineOptions)` | Merge multiple PRs into one |
| `generate_commit_message(&Path)` | AI-generated commit message |
| `generate_pr_message(&Path)` | AI-generated PR title+body |
| `ensure_lint_passes(&Path, &str)` | Run lint, fail if non-zero |

### `ops::trace` — Operation Tracing

```rust
pub struct OpTrace { pub op: String, pub prompt_hash: String, pub response: String }
pub struct Tracer { /* records or replays traces */ }
pub struct MockResponses { /* preloaded responses keyed by prompt hash */ }
pub fn hash_prompt(&str) -> String;   // SHA-256 truncated
pub fn trace_enabled() -> bool;       // checks LF_TRACE env var
```

### `ops::progress` — Progress Reporting

```rust
pub trait Progress: Send + Sync {
    fn status(&self, message: &str);
    fn complete(&self, message: &str);
    fn fail(&self, message: &str);
}

pub struct NullProgress;  // No-op implementation
```

---

## Daemon Entry (`bin/lfd.rs`)

Env vars: `LFD_HTTP_ADDR` (default `127.0.0.1:2486`), `LFD_DB_PATH` (default `~/.lf/lfd.db`), `LFD_STORAGE` (`sqlite`/`postgres`), `LFD_DATABASE_URL`, `LFD_MAX_SLOTS` (default `num_cpus / 2`), `LFD_GITHUB_TOKEN`.

Subcommands: `migrate [--status]`, `install`, `uninstall`, `start`, `stop`, `status`.

---

## Key Patterns

1. **Newtype IDs**: `LfdId(String)` wraps UUIDs — all domain entities use this, not raw strings
2. **Integer-backed enums**: Status/kind enums use explicit discriminants (`Idle = 1`) with `from_i32`/`as_i32` for DB, `#[serde(rename_all = "snake_case")]` for JSON
3. **Trait-based DI**: `RunStore`, `AgentExecutor`, `Runner`, `Progress`, `Tool` — swapped for testing
4. **Arc sharing**: `SharedStore = Arc<dyn RunStore>`, scheduler/executor/event_hub all `Arc`-wrapped
5. **Snapshot immutability**: `WaveRunSnapshot` freezes config at run creation
6. **Config merge**: Global + repo YAML; additive list keys, scalar overrides
7. **Flow as data**: YAML-defined step sequences with fork/ref support
8. **Worktree isolation**: Each wave run gets its own git worktree; janitor cleans orphans
9. **Event-driven UI**: `EventHub` broadcasts to WebSocket clients; `OutputHub` streams agent output
10. **Builtin embedding**: Steps/flows/directions compiled in via `include_str!`
11. **Multi-agent support**: `build_*_command` functions produce CLI args for Claude, Codex, Gemini, OpenCode
12. **Prompt budget**: Token counting via tiktoken, per-section caps, tiered diff inclusion
13. **Dual storage backends**: `RunStore` trait abstracts SQLite (local) and PostgreSQL (production)

---

## Trigger System (`lfd/triggers/`)

Six background loops spawned by `Scheduler::start_loops`:

| Loop | Description |
|------|-------------|
| `loop_ticker` | Checks idle waves with Loop stimuli, starts runs |
| `watch_poller` | Polls `origin/main` for new commits, queues PendingActivations |
| `cron_poller` | Evaluates cron expressions, fires on schedule |
| `ci_failure_handler` | Listens for CiFailure events, spawns sidecar debug runs |
| `recovery_loop` | Detects stuck agents, fails orphaned runs |
| `summary_refresh` | Regenerates wave summaries when source hash changes |

---

## File Map

```
rust/loopflow/src/
├── lib.rs
├── bin/{lf,lfd,lf-prompt,lf-agent}.rs
├── agent/{mod,anthropic,registry,tools,turn}.rs
├── chat/{mod,contract,completion}.rs
├── engine/{mod,agent,builtins,clipboard,command,config,error,event,flow,fork,git,naming,platform,prompt,stream,worktree,worktrees}.rs
├── lf/{mod,discovery,output}.rs
│   └── commands/{mod,flow,list,run,util}.rs
│       └── ops/mod.rs
├── lfd/{mod,auth,config,credentials,events,executor,github,id,machine_id,obs,output,registration,scheduler,sessions}.rs
│   ├── http/{mod,dto,state}.rs
│   │   └── routes/{mod,flows,hooks,system,wave_runs,waves,ws}.rs
│   ├── service/{mod,macos,linux}.rs
│   ├── store/{mod,migrations,postgres,rows,sqlite}.rs
│   ├── triggers/{mod,ci_failure,common,cron,loop_ticker,recovery,summary_refresh,watch}.rs
│   └── types/{mod,agent,event,stimulus,summary,wave}.rs
└── ops/{mod,abandon,combine,commit,error,land,lint,messages,next,pr,progress,rebase,trace,util}.rs

rust/loopflow/tests/{agent,combine,commit,config,context,discovery,flow}_tests.rs
rust/loopflow-test-support/src/lib.rs
```

## Key Dependencies

```toml
clap = "4.5"           # CLI parsing (derive)
serde = "1.0"          # serialization
tokio = "1"            # async runtime
axum = "0.7"           # HTTP server
reqwest = "0.12"       # HTTP client
rusqlite = "0.31"      # SQLite (bundled)
tokio-postgres = "0.7" # Postgres
git2 = "0.18"          # Git operations
tiktoken-rs = "0.6"    # Token counting
bollard = "0.17"       # Docker API
thiserror = "1.0"      # Error derives
time = "0.3"           # Timestamps (OffsetDateTime)
uuid = "1"             # UUID generation
portable-pty = "0.8"   # PTY for agent sessions
```
