//! Conversation identity and execution history. An invocation never owns feedback.

use serde::{Deserialize, Serialize};

use crate::durable::TaskId;
use crate::id::{AgentSessionId, LfSessionId, WaveId};

/// Attribution captured before a native request, never from its delayed reply.
/// This is observation context, not a capability to write to the provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionTurnOrigin {
    pub session_id: LfSessionId,
    pub lf_process_id: crate::id::LfProcessId,
    pub agent_process_id: crate::id::LfProcessId,
    pub captured_event: Option<i64>,
    pub task_id: Option<String>,
    pub wave_id: Option<String>,
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
    Captured,
    Started,
    Usage,
    Completed,
    /// Final provider output for an exact native turn; completion is separate.
    Output,
    /// Evidence without an exact native turn; cannot settle a Graph.
    Observed,
}

/// Captured or imported evidence; no execution identity or authority.
#[derive(Debug)]
pub(crate) struct SessionObservation {
    pub artifact_key: String,
    pub source: String,
    pub observed_at: i64,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub payload: serde_json::Value,
}

impl SessionEventKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Captured => "captured",
            Self::Started => "started",
            Self::Usage => "usage",
            Self::Completed => "completed",
            Self::Output => "output",
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
    pub ready_summary: Option<String>,
    pub completed_at: Option<i64>,
    pub created_at: i64,
}

impl LfSession {
    pub(crate) fn input_key(&self) -> anyhow::Result<&String> {
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
pub(crate) const AVAILABILITY_QUIET_SECONDS: i64 = 120;

/// Bounded provider text observed on the current native turn. Display evidence only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    Repository(crate::repository::CanonicalRepo),
    Wave(WaveId),
    Task(TaskId),
}

/// The Work a conversation names. Admission fills a Task's Wave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionWork {
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub source: WorkSource,
}

/// How this attempt received its work attribution; absent for unrecorded history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkSource {
    Declared,
    Checkout,
    Inherited,
    Bound,
}

/// SQL selection for conversation inventory; mode and completion are independent.
#[derive(Debug, Clone)]
pub struct SessionFilter {
    pub orphan: bool,
    pub repo: Option<String>,
    pub task: Option<String>,
    pub search: Option<String>,
    pub interactive: Option<bool>,
    /// Only conversations available for a person, chosen before paging.
    pub available: bool,
    pub history: bool,
    pub limit: usize,
    pub offset: usize,
    /// Present for stable-ID pages; empty starts the first page.
    pub after: Option<String>,
}

impl Default for SessionFilter {
    fn default() -> Self {
        Self {
            orphan: false,
            repo: None,
            task: None,
            search: None,
            interactive: Some(true),
            available: false,
            history: false,
            limit: 100,
            offset: 0,
            after: None,
        }
    }
}

/// Passive inventory values. Request, transcript, capture and native identity
/// validation belong to exact detail/actions, never to this row projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionSummary {
    pub preview: Option<SessionPreview>,
    pub terminal_id: Option<String>,
    /// Its execution owner runs with no viewer attached.
    pub unviewed: bool,
    pub primary_scope: Option<String>,
    pub agent_process_evidence: crate::journal::ProcessIdentityEvidence,
    /// Available for a person, as of the read's clock.
    pub available: bool,
    pub program_status: Option<crate::program_status::Records>,
    /// None for a conversation Loopflow never attached to.
    pub agent_process_id: Option<crate::id::LfProcessId>,
    pub task_terminal: bool,
    /// Its Task names it as the Task's primary conversation.
    pub task_primary: bool,
    pub task_ids: Vec<TaskId>,
    pub captured: Option<i64>,
    pub id: LfSessionId,
    pub artifact_key: Option<String>,
    pub title: String,
    pub title_source: TitleSource,
    pub ready_summary: Option<String>,
    pub completed_at: Option<i64>,
    pub interactive: bool,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub graph_lf_process_id: Option<String>,
    pub cwd: std::path::PathBuf,
    pub skill: Option<String>,
    pub provider: Option<String>,
    /// The login its current conversation is pinned to.
    pub account: Option<crate::store::ProviderAccountId>,
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

/// A Graph as its Graph process records it; Current says nothing about a live process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphProcessSummaryState {
    /// The Graph process has no recorded exit.
    Current,
    Completed,
    /// The Graph process exited before the Graph's last step.
    Stopped,
}

impl GraphProcessSummaryState {
    /// What a Graph process's recorded outcome and exit time say of its Graph.
    pub(crate) fn of_process(outcome: Option<&str>, completed_at: Option<i64>) -> Self {
        match (outcome, completed_at) {
            (Some("succeeded"), _) => Self::Completed,
            (None, None) => Self::Current,
            _ => Self::Stopped,
        }
    }
}

/// Read-local admission context. Immutable observations override current-input
/// fallback fields; attribution is selected from the input's original evidence.
#[derive(Debug)]
pub(crate) struct HistoryCapture {
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
    pub provider: Option<String>,
    pub model: Option<String>,
    pub interactive: bool,
}

pub(crate) fn conversation(
    cwd: &std::path::Path,
    agent: Option<&str>,
    skill: &str,
    title: String,
) -> anyhow::Result<LfSession> {
    let skill = crate::graph::load_skill(skill, cwd)?.name;
    let wave = crate::work::wave::context::resolve_ambient_wave_name();
    let agent = crate::ops::task::resolve_task_agent(
        cwd,
        wave.as_deref(),
        &crate::config::PersonalSettings::read(Some(cwd), wave.as_deref()),
        agent,
        None,
    );
    let (provider, model) = crate::config::parse_agent(&agent);
    Ok(LfSession {
        captured: None,
        id: uuid::Uuid::new_v4().simple().to_string().into(),
        artifact_key: None,
        caller_artifact_key: None,
        input_published: false,
        cwd: cwd.to_path_buf(),
        skill: Some(skill),
        provider: Some(provider),
        model,
        node: None,
        iterations: None,
        task_id: None,
        wave_id: None,
        graph_lf_process_id: None,
        work_source: None,
        bound_at: None,
        interactive: true,
        repo: None,
        title,
        title_source: TitleSource::Generated,
        request: None,
        ready_summary: None,
        completed_at: None,
        created_at: crate::store::rows::now_unix(),
    })
}
