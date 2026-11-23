use serde::{Deserialize, Serialize};

use loopflow_model::durable::TaskId;
use loopflow_model::id::{AgentSessionId, LfSessionId, WaveId};

/// Attribution captured before a native request, never from its delayed reply.
/// This is observation context, not a capability to write to the provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTurnOrigin {
    pub session_id: LfSessionId,
    pub lf_process_id: loopflow_model::id::LfProcessId,
    pub agent_process_id: loopflow_model::id::AgentProcessId,
    pub captured_event: Option<i64>,
    pub task_id: Option<String>,
    pub wave_id: Option<String>,
    pub project_id: Option<String>,
    pub repository: Option<String>,
    pub skill: Option<String>,
    pub provider: Option<String>,
    /// The Graph and step the input was dispatched for.
    pub graph: Option<String>,
    pub step: Option<String>,
}

/// One provider turn as `history.turns` keeps it: inserted when it starts,
/// completed in place when it ends. Its grouping is what it was dispatched
/// for; a bind made while it ran does not move it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Turn {
    pub id: i64,
    pub session_id: LfSessionId,
    pub thread: AgentSessionId,
    pub turn: String,
    pub captured_event: Option<i64>,
    pub lf_process_id: Option<String>,
    pub agent_process_id: Option<String>,
    pub task_id: Option<String>,
    pub wave_id: Option<String>,
    pub project_id: Option<String>,
    pub repository: Option<String>,
    pub skill: Option<String>,
    pub graph: Option<String>,
    pub step: Option<String>,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    /// How the provider says it ended: `completed`, `failed`, `interrupted`.
    pub status: Option<String>,
    pub duration_ms: Option<i64>,
    /// The provider's error or errors, as it reported them.
    pub error: Option<serde_json::Value>,
    pub provider: Option<String>,
    pub usage: Option<RecordedUsage>,
    pub context: Option<TurnContext>,
}

/// One turn's tokens, resolved from its provider's reading before they are
/// kept: a sum over turns needs no counter logic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordedUsage {
    pub observed_at: i64,
    /// The provider marked the reading as its last for the turn.
    #[serde(rename = "final")]
    pub final_reading: bool,
    /// Counted from the first report seen, not from the turn's start.
    pub partial: bool,
    pub usage: loopflow_model::chat::types::TurnUsage,
}

/// Immutable native evidence. Missing start, attribution or usage stays missing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionEvent {
    pub seq: i64,
    pub session_id: LfSessionId,
    // Retain the published DTO key and historical event encoding.
    #[serde(rename = "provider_thread")]
    pub agent_session: Option<AgentSessionId>,
    pub provider_turn: Option<String>,
    pub kind: SessionEventKind,
    pub agent_process_id: Option<String>,
    pub lf_process_id: Option<String>,
    pub task_id: Option<String>,
    pub wave_id: Option<String>,
    pub observed_at: i64,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEventKind {
    /// An input, kept in the store as Session state.
    Captured,
    /// Session history; provider turns are in `history.turns`.
    Observed,
}

/// Captured or imported evidence; no execution identity or authority.
#[derive(Debug)]
pub struct SessionObservation {
    pub artifact_key: String,
    pub source: String,
    pub observed_at: i64,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub payload: serde_json::Value,
}

impl SessionEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Captured => "captured",
            Self::Observed => "observed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LfSession {
    pub captured: Option<i64>,
    pub id: LfSessionId,
    /// Immutable captured input; absent until the first explicit input reservation.
    pub artifact_key: Option<String>,
    /// Causal input reference; grants neither attachment nor Graph authority.
    pub caller_artifact_key: Option<String>,
    pub input_published: bool,
    /// Selected working directory; existing Process records retain their launch directory.
    pub cwd: std::path::PathBuf,
    pub skill: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub node: Option<u32>,
    pub iterations: Option<Vec<Vec<u32>>>,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    /// The Graph whose step captured the current input; derived, never stored.
    pub graph_lf_process_id: Option<String>,
    pub work_source: Option<WorkSource>,
    /// Time of a prospective bind; absent for admission or unknown historical timing.
    pub bound_at: Option<i64>,
    pub interactive: bool,
    /// Canonical local repository at admission; absent when unknown or taskless outside Git.
    pub repo: Option<String>,
    pub title: String,
    pub title_source: TitleSource,
    /// Retained request context from historical conversations.
    pub request: Option<String>,
    pub completed_at: Option<i64>,
    pub created_at: i64,
}

impl LfSession {
    pub fn input_key(&self) -> anyhow::Result<&String> {
        self.artifact_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Session {} has no captured input", self.id))
    }
}

/// A Session assigned to a Task after it began.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBind {
    pub session_id: LfSessionId,
    pub at: i64,
    /// The Task's issue identifier.
    pub task: String,
    pub wave: Option<String>,
}

/// Quiet time after which a conversation with no unresolved tool call waits
/// on a person. A long silent provider step can read as Available.
pub const AVAILABILITY_QUIET_SECONDS: i64 = 120;

/// Bounded provider text observed on the current native turn. Display evidence only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionPreview {
    pub provider_thread: AgentSessionId,
    pub provider_turn: String,
    pub observed_at: i64,
    pub source: String,
    pub text: String,
}

/// What a Session's attached LfProcess last read from its provider's own stream. One row
/// per Session, replaced by whichever attachment currently owns that stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionActivity {
    pub preview: Option<SessionPreview>,
    pub observed_at: i64,
    /// Tool calls started and not yet answered.
    pub open_tools: usize,
    /// Questions the provider asked a person and has no answer to.
    pub pending_input: usize,
    /// The provider handed its last turn back successfully.
    pub yielded: bool,
}

/// Whether a Session's provider is still making progress: its last provider
/// signal and the launching process's last CPU sample of the provider body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionLiveness {
    pub observed_at: i64,
    pub cpu_sampled_at: Option<i64>,
    /// Since when the body and its tools used no CPU; absent when the last
    /// sample could not read them.
    pub cpu_quiet_since: Option<i64>,
}

/// What filled one provider turn's context, retained on its `completed`
/// receipt. A request figure is absent when the provider reported none.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnContext {
    pub first_request_tokens: Option<u64>,
    pub peak_request_tokens: Option<u64>,
    pub tool_results: u64,
    /// Tool output in cl100k tokens.
    pub tool_output_tokens: u64,
    pub compactions: u64,
    pub compaction_tokens: Option<u64>,
}

impl TurnContext {
    pub fn request(&mut self, input_tokens: u64) {
        self.first_request_tokens.get_or_insert(input_tokens);
        self.peak_request_tokens = self.peak_request_tokens.max(Some(input_tokens));
    }

    pub fn compacted(&mut self, pre_tokens: Option<u64>) {
        self.compactions += 1;
        if let Some(pre_tokens) = pre_tokens {
            *self.compaction_tokens.get_or_insert(0) += pre_tokens;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleSource {
    Generated,
    Human,
}

/// What a conversation is the one ongoing conversation of. A repository's or
/// Wave's row carries the scope's identity; a Task names its own, which stays
/// one of the Task's conversations. Primary grants no Graph or process authority.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrimaryScope {
    Repository(crate::address::CanonicalRepo),
    Wave(WaveId),
    Task(TaskId),
}

impl PrimaryScope {
    /// Admission and replacement serialize on the same saved scope identity.
    pub fn lock_key(&self) -> String {
        match self {
            Self::Repository(repo) => format!("primary:repository:{repo}"),
            Self::Wave(wave) => format!("primary:wave:{wave}"),
            Self::Task(task) => format!("primary:task:{task}"),
        }
    }
}

/// The Work a conversation names. Admission fills a Task's Wave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionWork {
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub source: WorkSource,
}

/// How this attempt received its work attribution; absent for unrecorded history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkSource {
    Declared,
    Checkout,
    Inherited,
    Bound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum SessionPrimary {
    All,
    Task,
    Wave,
    Repository,
}

/// Inventory selection; SQL filters metadata, then the list resolves checkout membership.
#[derive(Debug, Clone)]
pub struct SessionFilter {
    pub orphan: bool,
    pub repo: Option<String>,
    pub task: Option<String>,
    /// Attribution or resolved checkout membership, selected before pagination.
    pub wave: Option<WaveId>,
    pub primary: Option<SessionPrimary>,
    pub search: Option<String>,
    /// Include headless Sessions alongside interactive conversations.
    pub headless: bool,
    /// Only conversations available for a person, chosen before paging.
    pub available: bool,
    /// Also list raw conversations: providers lf never launched, which have no Session.
    pub raw: bool,
    pub limit: usize,
    /// Present for stable-ID pages; empty starts the first page.
    pub after: Option<String>,
    /// Only Sessions on `harness[:model]`.
    pub on: Option<String>,
}

impl Default for SessionFilter {
    fn default() -> Self {
        Self {
            on: None,
            orphan: false,
            repo: None,
            task: None,
            wave: None,
            primary: None,
            search: None,
            headless: false,
            available: false,
            raw: true,
            limit: 100,
            after: None,
        }
    }
}

/// Passive inventory values. Request, transcript, capture and native identity
/// validation belong to exact detail/actions, never to this row projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub transition: Option<crate::sqlite::provider_transitions::ProviderTransition>,
    pub preview: Option<SessionPreview>,
    pub terminal_id: Option<String>,
    /// Its held terminal or detached process runs with no viewer attached.
    pub unviewed: bool,
    /// Its agent runs in a terminal a holder keeps; any number of views attach.
    pub held: bool,
    pub primary_scope: Option<String>,
    pub agent_process_evidence: crate::ProcessIdentityEvidence,
    /// When its current agent's end was recorded, and how; `None` while it
    /// runs, before the first, or when nobody recorded the end.
    pub agent_end: Option<AgentEnd>,
    /// Starts by the server's recovery pass since it last finished a turn.
    pub starts_since_turn: u32,
    /// When the pass last started it.
    pub last_started_at: Option<i64>,
    /// Why the pass's last start failed; `None` once one succeeded.
    pub start_failure: Option<String>,
    /// Available for a person, as of the read's clock.
    pub available: bool,
    pub program_status: Option<loopflow_model::program_status::Records>,
    /// None for a conversation Loopflow never attached to.
    pub agent_process_id: Option<loopflow_model::id::AgentProcessId>,
    pub task_terminal: bool,
    /// The Task whose hold keeps this primary Session down.
    pub paused_task: Option<TaskId>,
    /// Its Task names it as the Task's primary conversation.
    pub task_primary: bool,
    /// It is an open Task's conversation in the Task's checkout on this
    /// Machine: one the server's recovery pass reads. A Task checked out
    /// elsewhere, a completed conversation, or one moved to another Machine,
    /// is not.
    pub in_task_checkout: bool,
    /// Its held terminal is open and its agent's pid is not recorded yet: a
    /// start under way, which has no liveness to read.
    pub launching: bool,
    pub task_ids: Vec<TaskId>,
    pub captured: Option<i64>,
    pub id: LfSessionId,
    pub artifact_key: Option<String>,
    /// The current input records a native resume reference; opening validates it.
    pub has_provider_history: bool,
    pub title: String,
    pub title_source: TitleSource,
    pub completed_at: Option<i64>,
    pub interactive: bool,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub graph_lf_process_id: Option<String>,
    pub cwd: std::path::PathBuf,
    pub skill: Option<String>,
    pub provider: Option<String>,
    /// Historical attribution: the saved login its conversation last recorded; never the current native login.
    pub account: Option<crate::ProviderAccountId>,
    pub model: Option<String>,
    pub node: Option<u32>,
    pub iterations: Option<Vec<Vec<u32>>>,
    pub graph: Option<GraphProcessSummary>,
    /// Whether this Session's step is the last its Graph launched.
    pub graph_step_latest: bool,
    pub independent: bool,
    pub wave_name: Option<String>,
    pub task_identifier: Option<String>,
}

/// An agent's recorded end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentEnd {
    pub at: i64,
    /// The provider's exit, as its launcher saw it; `None` when nobody
    /// saw it: the holder died, or the Machine restarted.
    pub exit_code: Option<i32>,
}

/// The person quit: `/exit` or Ctrl+D (0), or Ctrl+C at the prompt (130).
/// Every other exit, and an end nobody saw, is a death.
pub fn quit_by_person(exit_code: Option<i32>) -> bool {
    matches!(exit_code, Some(0 | 130))
}

/// A Graph as its Graph process records it; Current says nothing about a live process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GraphProcessSummary {
    /// The Graph process.
    pub id: String,
    pub name: String,
    pub state: GraphProcessSummaryState,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    /// When its latest step started, or its Graph process exited.
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GraphProcessSummaryState {
    /// The Graph process has no recorded exit.
    Current,
    Completed,
    /// The Graph process exited before the Graph's last step.
    Stopped,
}

impl GraphProcessSummaryState {
    /// What a Graph run's result says of its Graph.
    pub fn of_run(result: crate::graph_process::ExecutionResult) -> Self {
        use crate::graph_process::ExecutionResult;
        match result {
            ExecutionResult::Succeeded => Self::Completed,
            ExecutionResult::Running => Self::Current,
            ExecutionResult::Interrupted | ExecutionResult::Failed | ExecutionResult::Unknown => {
                Self::Stopped
            }
        }
    }
}

/// Read-local admission context. Immutable observations override current-input
/// fallback fields; attribution is selected from the input's original evidence.
#[derive(Debug)]
pub struct HistoryCapture {
    pub captured: Option<i64>,
    pub id: LfSessionId,
    pub current_capture: Option<i64>,
    pub caller_artifact_key: Option<String>,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub observed_at: i64,
    pub work_source: Option<WorkSource>,
    pub cwd: std::path::PathBuf,
    pub repo: Option<String>,
    pub skill: Option<String>,
    /// SHA-256 of the skill text this input launched with; absent for another input's.
    pub skill_version: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub interactive: bool,
}

/// Keep generated names on the same Task label as every other projection.
pub fn task_session_title(
    title: &str,
    id: &loopflow_model::durable::TaskId,
    label: &str,
    previous: Option<&str>,
) -> String {
    let hex = id.as_str().trim_start_matches("task_");
    let purpose = title
        .split_whitespace()
        .filter(|word| {
            let token = word.trim_matches(|ch: char| !ch.is_ascii_alphanumeric());
            let local = token
                .strip_prefix("lf-")
                .or_else(|| token.strip_prefix("task_"))
                .unwrap_or(token);
            token != label
                && previous != Some(token)
                && !(local.len() >= 6
                    && local.bytes().all(|ch| ch.is_ascii_hexdigit())
                    && hex.starts_with(&local.to_ascii_lowercase()))
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("{label} {purpose}")
        .trim()
        .chars()
        .take(80)
        .collect()
}
