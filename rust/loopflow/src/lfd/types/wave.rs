//! Wave and Run types.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;
use crate::lfd::types::money::{Money, SpendCap};

fn default_workers() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WaveStatus {
    #[default]
    Idle = 1,
    Running = 2,
    Waiting = 3,
    Paused = 4,
    Failed = 5,
}

impl WaveStatus {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Idle,
            2 => Self::Running,
            3 => Self::Waiting,
            4 => Self::Paused,
            5 => Self::Failed,
            _ => Self::Idle,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Paused => "paused",
            Self::Failed => "failed",
        }
    }
}

impl std::str::FromStr for WaveStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "idle" => Ok(Self::Idle),
            "running" => Ok(Self::Running),
            "waiting" => Ok(Self::Waiting),
            "paused" => Ok(Self::Paused),
            "failed" => Ok(Self::Failed),
            _ => Err(format!("unknown wave status: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    #[default]
    Unspecified = 0,
    Pending = 1,
    Running = 2,
    Waiting = 3,
    Completed = 4,
    Failed = 5,
}

impl RunStatus {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Pending,
            2 => Self::Running,
            3 => Self::Waiting,
            4 => Self::Completed,
            5 => Self::Failed,
            _ => Self::Unspecified,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunStackStatus {
    #[default]
    Active = 1,
    Superseded = 2,
    Merged = 3,
}

impl RunStackStatus {
    pub fn from_i32(value: i32) -> Self {
        match value {
            2 => Self::Superseded,
            3 => Self::Merged,
            _ => Self::Active,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LivePrState {
    #[default]
    Unknown = 0,
    Open = 1,
    Closed = 2,
    Merged = 3,
}

impl LivePrState {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Open,
            2 => Self::Closed,
            3 => Self::Merged,
            _ => Self::Unknown,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Open => "open",
            Self::Closed => "closed",
            Self::Merged => "merged",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WaveMode {
    #[default]
    Loop,
    Manual,
}

impl WaveMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Loop => "loop",
            Self::Manual => "manual",
        }
    }
}

impl std::str::FromStr for WaveMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "loop" => Ok(Self::Loop),
            "manual" => Ok(Self::Manual),
            _ => Err(format!("unknown wave mode: {value}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveCron {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub flow: String,
    pub schedule: String,
    pub last_triggered_at: Option<i64>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueBlockReason {
    MissingPr,
    WaveRunning,
    ScratchDirty,
    RebaseConflict,
    PromotionFailed,
}

impl QueueBlockReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingPr => "missing_pr",
            Self::WaveRunning => "wave_running",
            Self::ScratchDirty => "scratch_dirty",
            Self::RebaseConflict => "rebase_conflict",
            Self::PromotionFailed => "promotion_failed",
        }
    }
}

impl std::str::FromStr for QueueBlockReason {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "missing_pr" => Ok(Self::MissingPr),
            "wave_running" => Ok(Self::WaveRunning),
            "scratch_dirty" => Ok(Self::ScratchDirty),
            "rebase_conflict" => Ok(Self::RebaseConflict),
            "promotion_failed" => Ok(Self::PromotionFailed),
            _ => Err(format!("unknown queue block reason: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    pub id: LfdId,
    pub name: String,
    pub mode: WaveMode,
    pub primary_flow: String,
    pub goal: String,
    pub metrics: Vec<String>,
    #[serde(default)]
    pub crons: Vec<WaveCron>,
    /// Per-repo execution state (worktree/branch/status/iteration), stitched from
    /// `wave_repos` on read. Execution state lives here; the wave carries only
    /// identity plus the wave-level `paused` flag.
    #[serde(default)]
    pub repos: Vec<RepoWork>,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    /// Wave-level pause flag. Rolled into `status()` (a paused wave reports
    /// `Paused` regardless of per-repo state).
    pub paused: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    /// Maximum number of active runs this wave can have at once.
    #[serde(default = "default_workers")]
    pub workers: u32,
    /// Hard spend ceiling for this wave. `None` means uncapped (safety floor
    /// opt-in). Crossing the cap pauses the wave and blocks to a human.
    #[serde(default)]
    pub spend_cap: Option<SpendCap>,
    /// Cumulative agent cost accrued by this wave's runs, in cents. Grows as
    /// runs finish and report cost; compared against `spend_cap.rate`.
    #[serde(default)]
    pub spent: Money,
}

impl Wave {
    pub fn new(id: LfdId, name: String, repo: String) -> Self {
        Self {
            id,
            name,
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            crons: Vec::new(),
            repos: vec![RepoWork {
                repo,
                worktree: String::new(),
                branch: String::new(),
                status: WaveStatus::Idle,
                iteration: 0,
                cycle_start_iteration: 0,
                position: 0,
            }],
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: default_workers(),
            spend_cap: None,
            spent: Money::ZERO,
        }
    }

    pub fn id(&self) -> &LfdId {
        &self.id
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    /// Primary repo path — the first `RepoWork`'s repo, `""` when a wave carries
    /// no repos (shouldn't happen outside construction).
    pub fn repo(&self) -> &str {
        self.repos.first().map(|r| r.repo.as_str()).unwrap_or("")
    }

    pub fn mode(&self) -> WaveMode {
        self.mode
    }

    pub fn primary_flow(&self) -> &String {
        &self.primary_flow
    }

    pub fn goal(&self) -> &str {
        &self.goal
    }

    pub fn metrics(&self) -> &Vec<String> {
        &self.metrics
    }

    pub fn direction(&self) -> &Vec<String> {
        &self.direction
    }

    pub fn area(&self) -> &Vec<String> {
        &self.area
    }

    /// Wave-level status, rolled up over `repos`. `Paused` is a wave-level flag;
    /// otherwise the status is derived from the per-repo execution state: any
    /// running wins, then failed, then waiting, else idle.
    pub fn status(&self) -> WaveStatus {
        if self.paused {
            return WaveStatus::Paused;
        }
        if self.repos.iter().any(|r| r.status == WaveStatus::Running) {
            WaveStatus::Running
        } else if self.repos.iter().any(|r| r.status == WaveStatus::Failed) {
            WaveStatus::Failed
        } else if self.repos.iter().any(|r| r.status == WaveStatus::Waiting) {
            WaveStatus::Waiting
        } else {
            WaveStatus::Idle
        }
    }

    /// Max iteration across `repos`.
    pub fn iteration(&self) -> u32 {
        self.repos.iter().map(|r| r.iteration).max().unwrap_or(0)
    }

    pub fn cycle_start_iteration(&self) -> u32 {
        self.repos
            .iter()
            .map(|r| r.cycle_start_iteration)
            .max()
            .unwrap_or(0)
    }

    /// Set the wave's execution status. Toggles the wave-level `paused` flag and
    /// writes the status onto every `RepoWork`, so reads through `status()` and
    /// per-repo readers agree.
    pub fn set_status(&mut self, status: WaveStatus) {
        self.paused = status == WaveStatus::Paused;
        for repo in &mut self.repos {
            repo.status = status;
        }
    }

    /// Mutable access to the `RepoWork` for `repo`, falling back to the primary
    /// repo when there's no exact match. Used by the executor to record per-repo
    /// status/iteration after dispatching a run.
    pub fn repo_work_mut(&mut self, repo: &str) -> Option<&mut RepoWork> {
        if let Some(pos) = self.repos.iter().position(|r| r.repo == repo) {
            return self.repos.get_mut(pos);
        }
        self.repos.first_mut()
    }

    pub fn created_at(&self) -> Option<OffsetDateTime> {
        self.created_at
    }

    pub fn workers(&self) -> u32 {
        self.workers
    }

    pub fn spend_cap(&self) -> Option<SpendCap> {
        self.spend_cap
    }

    pub fn spent(&self) -> Money {
        self.spent
    }

    /// Accrue one run's cost onto the wave's running total. Returns the new
    /// total so callers can log it.
    pub fn accrue_spend(&mut self, cost: Money) -> Money {
        self.spent = self.spent.saturating_add(cost);
        self.spent
    }

    /// Why the wave should pause, given a just-finished run's `run_cost`.
    /// `None` means keep going. Checks the per-iteration ceiling against the
    /// single run and the cumulative ceiling against total spend-to-date.
    pub fn spend_pause_reason(&self, run_cost: Money) -> Option<SpendPause> {
        let cap = self.spend_cap?;
        if cap.iteration_exceeded(run_cost) {
            return Some(SpendPause::PerIteration {
                cost: run_cost,
                cap: cap.per_iteration,
            });
        }
        if cap.rate_exceeded(self.spent) {
            return Some(SpendPause::Rate {
                spent: self.spent,
                cap: cap.rate,
            });
        }
        None
    }
}

/// Which spend ceiling a wave crossed. Carries the numbers for the block's
/// human-facing summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendPause {
    /// A single iteration cost more than `per_iteration` allows.
    PerIteration { cost: Money, cap: Money },
    /// Cumulative spend reached the `rate` ceiling.
    Rate { spent: Money, cap: Money },
}

impl SpendPause {
    pub fn summary(&self) -> String {
        match self {
            SpendPause::PerIteration { cost, cap } => format!(
                "One iteration spent {cost}, over the per-iteration cap of {cap}. \
                 Wave paused; review before resuming."
            ),
            SpendPause::Rate { spent, cap } => format!(
                "Wave has spent {spent}, at or over its spend cap of {cap}. \
                 Wave paused; raise the cap or resume to continue."
            ),
        }
    }
}

/// Per-repo execution state for a wave. Waves own identity; each `RepoWork`
/// carries the worktree/branch/status/iteration for one repo the wave runs in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoWork {
    pub repo: String,
    /// Worktree path, `""` when none.
    pub worktree: String,
    /// Branch name, `""` when none.
    pub branch: String,
    pub status: WaveStatus,
    pub iteration: u32,
    pub cycle_start_iteration: u32,
    pub position: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub url: String,
    pub number: Option<u32>,
    pub state: Option<String>,
    pub title: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub repo: String,
    pub flow: String,
    pub task: Option<String>,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub iteration: u32,
    pub step_index: u32,
    pub status: RunStatus,
    pub worktree: String,
    pub branch: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ended_at: Option<OffsetDateTime>,
    pub error: Option<String>,
    pub flow_parents: Vec<String>,
    #[serde(skip)]
    pub execution_cursor: Option<String>,
    pub activation_log_id: Option<LfdId>,
    pub parent_run_id: Option<LfdId>,
    pub parent_pr_number: Option<u32>,
    pub stack_position: u32,
    pub stack_group_id: String,
    #[serde(default)]
    pub stack_status: RunStackStatus,
    #[serde(default)]
    pub lineage_inferred: bool,
    /// The branch this run targets. "main" means new branch off main (produce
    /// PR). Any other value means check out that branch and push to it (no PR).
    #[serde(default = "default_target_branch")]
    pub target_branch: String,
    /// When set, this run is a repair attempt for the referenced failed run.
    /// The executor uses this to decide whether to escalate (algedonic signal)
    /// or attempt another repair on failure.
    pub repair_of: Option<LfdId>,
    /// The pull request created or associated with this run.
    /// Set when the run creates a PR (auto-create or land --create-pr).
    pub pr: Option<PullRequest>,
}

fn default_target_branch() -> String {
    "main".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivePullRequestState {
    pub repo_id: String,
    pub pr_number: u32,
    pub state: LivePrState,
    pub is_draft: bool,
    pub head_ref: String,
    pub head_sha: String,
    pub base_ref: String,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub merged_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub synced_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueBlock {
    pub wave_id: LfdId,
    pub run_id: LfdId,
    pub reason: QueueBlockReason,
    #[serde(with = "time::serde::rfc3339")]
    pub attempted_at: OffsetDateTime,
    pub conflict_files: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueMergeEvent {
    pub wave_id: LfdId,
    pub pr_number: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub merged_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub processed_at: OffsetDateTime,
}

impl Run {
    pub fn new(id: LfdId, wave_id: LfdId) -> Self {
        let stack_group_id = wave_id.to_string();
        Self {
            id,
            wave_id,
            repo: String::new(),
            flow: String::new(),
            task: None,
            direction: Vec::new(),
            area: Vec::new(),
            iteration: 0,
            step_index: 0,
            status: RunStatus::Pending,
            worktree: String::new(),
            branch: String::new(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            execution_cursor: None,
            activation_log_id: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id,
            stack_status: RunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: None,
        }
    }
}

#[cfg(test)]
mod spend_tests {
    use super::*;

    fn capped_wave(rate_cents: i64, per_iteration_cents: i64) -> Wave {
        let mut wave = Wave::new(LfdId::new(), "budget".to_string(), "/tmp/repo".to_string());
        wave.spend_cap = Some(SpendCap {
            rate: Money::from_cents(rate_cents),
            per_iteration: Money::from_cents(per_iteration_cents),
        });
        wave
    }

    #[test]
    fn uncapped_wave_never_pauses() {
        let mut wave = Wave::new(LfdId::new(), "w".to_string(), "/tmp/repo".to_string());
        wave.accrue_spend(Money::from_cents(999_999));
        assert_eq!(wave.spend_pause_reason(Money::from_cents(999_999)), None);
    }

    #[test]
    fn accrue_accumulates() {
        let mut wave = capped_wave(1000, 0);
        wave.accrue_spend(Money::from_cents(300));
        wave.accrue_spend(Money::from_cents(250));
        assert_eq!(wave.spent(), Money::from_cents(550));
    }

    #[test]
    fn pauses_when_cumulative_cap_crossed() {
        let mut wave = capped_wave(500, 0);
        wave.accrue_spend(Money::from_cents(500));
        assert!(matches!(
            wave.spend_pause_reason(Money::from_cents(100)),
            Some(SpendPause::Rate { .. })
        ));
    }

    #[test]
    fn per_iteration_catches_pathological_run() {
        // Cumulative is fine, but one run blew past the per-iteration cap.
        let mut wave = capped_wave(10_000, 200);
        wave.accrue_spend(Money::from_cents(300));
        assert!(matches!(
            wave.spend_pause_reason(Money::from_cents(300)),
            Some(SpendPause::PerIteration { .. })
        ));
    }

    #[test]
    fn stays_running_below_both_caps() {
        let mut wave = capped_wave(1000, 500);
        wave.accrue_spend(Money::from_cents(200));
        assert_eq!(wave.spend_pause_reason(Money::from_cents(200)), None);
    }
}
