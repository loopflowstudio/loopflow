//! Per-request Linear accounting for one Machine, off unless this Machine's
//! `~/.lf/config.yaml` says
//!
//! ```yaml
//! logs:
//!   linear_requests: true
//! ```
//!
//! Each request then appends one JSON line to `logs/linear-requests.jsonl`:
//! what was asked, by which lf command and Process, and the allowance Linear
//! reported back. `lf history linear` totals them. Nothing reaches the terminal.
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::cooldown::{Allowance, Allowances};

pub const FILE_NAME: &str = "linear-requests.jsonl";
/// The log rolls over to one `.1` predecessor past this size: about a week of heavy use.
const ROLL_BYTES: u64 = 32 * 1024 * 1024;

/// What became of one request lf meant to send.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestOutcome {
    /// Sent, and Linear answered within its limits.
    Sent,
    /// Sent, and Linear refused it for its rate limit.
    RateLimited,
    /// Not sent: the shared cooldown was still running.
    Paused,
    /// Not sent: a non-urgent request held back while the budget is low.
    Deferred,
    /// Sent, but no response arrived.
    Failed,
}

impl RequestOutcome {
    pub fn spent(self) -> bool {
        matches!(self, Self::Sent | Self::RateLimited | Self::Failed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinearRequestRecord {
    pub at_ms: i64,
    /// The GraphQL operation name, such as `TeamChanges` or `SyncComment`.
    pub operation: String,
    pub mutation: bool,
    /// Whether the request could spend the reserved budget.
    pub urgent: bool,
    pub outcome: RequestOutcome,
    pub status: Option<u16>,
    pub duration_ms: Option<u64>,
    /// The calling command, such as `lf task status`; never its arguments.
    pub command: Option<String>,
    pub lf_process_id: Option<String>,
    pub repository: Option<String>,
    /// The Linear object the request names: an issue, comment or Project.
    pub target: Option<String>,
    pub requests: Option<Allowance>,
    pub complexity: Option<Allowance>,
    pub cost: Option<i64>,
}

/// Whether this Machine records Linear requests.
pub fn enabled(home: &Path) -> bool {
    crate::config::machine_flag(home, &["logs", "linear_requests"])
}

pub fn log_path(home: &Path) -> PathBuf {
    home.join("logs").join(FILE_NAME)
}

/// One request about to be made, recorded once its outcome is known.
#[derive(Debug)]
pub(super) struct PendingRequest {
    path: PathBuf,
    started: std::time::Instant,
    record: LinearRequestRecord,
}

impl PendingRequest {
    /// `None` when this Machine does not record requests.
    pub(super) fn start(
        home: &Path,
        query: &str,
        variables: &Value,
        repository: Option<&Path>,
        urgent: bool,
    ) -> Option<Self> {
        if !enabled(home) {
            return None;
        }
        let (mutation, operation) = operation(query);
        let process = crate::journal::current_lf_process_id();
        let command = crate::journal::current_command().map(|command| {
            crate::lf::commands::top::command_label(Some(&command), crate::process::ProcessKind::Lf)
        });
        let target = ["issueId", "id"]
            .into_iter()
            .find_map(|key| variables.get(key)?.as_str())
            .map(str::to_owned);
        Some(Self {
            path: log_path(home),
            started: std::time::Instant::now(),
            record: LinearRequestRecord {
                at_ms: super::cooldown::now_ms(),
                operation,
                mutation,
                urgent,
                outcome: RequestOutcome::Sent,
                status: None,
                duration_ms: None,
                command,
                lf_process_id: process.map(|id| id.to_string()),
                repository: repository.map(|path| path.display().to_string()),
                target,
                requests: None,
                complexity: None,
                cost: None,
            },
        })
    }

    pub(super) fn finish(mut self, exchange: Exchange) {
        let record = &mut self.record;
        record.outcome = exchange.outcome;
        record.status = exchange.status;
        if exchange.outcome.spent() {
            record.duration_ms = Some(self.started.elapsed().as_millis() as u64);
        }
        record.requests = exchange.allowances.requests;
        record.complexity = exchange.allowances.complexity;
        record.cost = exchange.allowances.cost;
        if let Err(error) = append(&self.path, record) {
            tracing::debug!(%error, "Linear request log unavailable");
        }
    }
}

/// What became of one request, and what Linear's answer reported.
#[derive(Debug)]
pub(super) struct Exchange {
    pub(super) outcome: RequestOutcome,
    pub(super) status: Option<u16>,
    pub(super) allowances: Allowances,
}

impl Exchange {
    /// A request Linear never answered.
    pub(super) fn unanswered(outcome: RequestOutcome) -> Self {
        Self {
            outcome,
            status: None,
            allowances: Allowances::default(),
        }
    }
}

/// `query Name(...)` or `mutation Name(...)`: whether it writes, and its name.
fn operation(query: &str) -> (bool, String) {
    let mut words = query
        .trim_start()
        .split(|c: char| c.is_whitespace() || c == '(' || c == '{')
        .filter(|word| !word.is_empty());
    let kind = words.next().unwrap_or_default();
    let name = words
        .next()
        .filter(|_| matches!(kind, "query" | "mutation"))
        .unwrap_or("anonymous");
    (kind == "mutation", name.to_owned())
}

fn append(path: &Path, record: &LinearRequestRecord) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().expect("request log has a parent"))?;
    if std::fs::metadata(path).is_ok_and(|meta| meta.len() > ROLL_BYTES) {
        std::fs::rename(path, path.with_extension("jsonl.1"))?;
    }
    let mut line = serde_json::to_vec(record).map_err(std::io::Error::other)?;
    line.push(b'\n');
    // One write per line keeps concurrent processes' appends whole.
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(&line)
}

/// Every recorded request at or after `since_ms`, oldest first.
pub fn read_since(home: &Path, since_ms: i64) -> std::io::Result<Vec<LinearRequestRecord>> {
    let path = log_path(home);
    let mut records = Vec::new();
    for path in [path.with_extension("jsonl.1"), path] {
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        records.extend(
            text.lines()
                .filter_map(|line| serde_json::from_str::<LinearRequestRecord>(line).ok())
                .filter(|record| record.at_ms >= since_ms),
        );
    }
    records.sort_by_key(|record| record.at_ms);
    Ok(records)
}

/// Requests counted together: those that reached Linear, those held back by
/// the cooldown or the reserve, and the complexity Linear charged.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Tally {
    pub spent: u64,
    pub held: u64,
    pub cost: i64,
}

impl Tally {
    fn add(&mut self, record: &LinearRequestRecord) {
        if record.outcome.spent() {
            self.spent += 1;
        } else {
            self.held += 1;
        }
        self.cost += record.cost.unwrap_or(0);
    }
}

/// Requests one command made through one operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CallerTotal {
    pub command: String,
    pub operation: String,
    #[serde(flatten)]
    pub tally: Tally,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HourTotal {
    pub start_ms: i64,
    #[serde(flatten)]
    pub tally: Tally,
}

/// Recorded Linear requests over one window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RequestSummary {
    pub since_ms: i64,
    pub outcomes: BTreeMap<RequestOutcome, u64>,
    pub hours: Vec<HourTotal>,
    /// Heaviest first.
    pub callers: Vec<CallerTotal>,
    /// The allowance Linear reported most recently.
    pub requests: Option<Allowance>,
    pub complexity: Option<Allowance>,
}

pub fn summarize(since_ms: i64, records: &[LinearRequestRecord]) -> RequestSummary {
    let mut outcomes = BTreeMap::new();
    let mut hours: BTreeMap<i64, Tally> = BTreeMap::new();
    let mut callers: BTreeMap<(String, String), Tally> = BTreeMap::new();
    for record in records {
        *outcomes.entry(record.outcome).or_default() += 1;
        let hour = record.at_ms - record.at_ms.rem_euclid(3_600_000);
        hours.entry(hour).or_default().add(record);
        let command = record.command.clone().unwrap_or_else(|| "lf".into());
        callers
            .entry((command, record.operation.clone()))
            .or_default()
            .add(record);
    }
    let mut callers: Vec<_> = callers
        .into_iter()
        .map(|((command, operation), tally)| CallerTotal {
            command,
            operation,
            tally,
        })
        .collect();
    // Heaviest first; the map's order breaks ties by command and operation.
    callers.sort_by_key(|caller| {
        let Tally { spent, held, cost } = caller.tally;
        std::cmp::Reverse((spent, cost, held))
    });
    let latest =
        |pick: fn(&LinearRequestRecord) -> Option<Allowance>| records.iter().rev().find_map(pick);
    RequestSummary {
        since_ms,
        outcomes,
        hours: hours
            .into_iter()
            .map(|(start_ms, tally)| HourTotal { start_ms, tally })
            .collect(),
        callers,
        requests: latest(|record| record.requests),
        complexity: latest(|record| record.complexity),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_names_the_graphql_operation() {
        assert_eq!(
            operation("mutation SyncComment($id: String!) { x }"),
            (true, "SyncComment".into())
        );
        assert_eq!(
            operation("\n query Viewer {\n viewer { id } }"),
            (false, "Viewer".into())
        );
        assert_eq!(operation("{ viewer { id } }"), (false, "anonymous".into()));
    }

    fn record(
        at_ms: i64,
        command: &str,
        operation: &str,
        outcome: RequestOutcome,
    ) -> LinearRequestRecord {
        LinearRequestRecord {
            at_ms,
            operation: operation.into(),
            mutation: false,
            urgent: false,
            outcome,
            status: None,
            duration_ms: None,
            command: Some(command.into()),
            lf_process_id: None,
            repository: None,
            target: None,
            requests: None,
            complexity: None,
            cost: Some(2),
        }
    }

    #[test]
    fn summary_ranks_callers_and_totals_hours() {
        let records = [
            record(
                3_600_000,
                "lf task status",
                "TeamChanges",
                RequestOutcome::Sent,
            ),
            record(
                3_700_000,
                "lf task status",
                "TeamChanges",
                RequestOutcome::Sent,
            ),
            record(
                7_300_000,
                "lf task comment",
                "SyncComment",
                RequestOutcome::Sent,
            ),
            record(
                7_400_000,
                "lf task status",
                "TeamChanges",
                RequestOutcome::Deferred,
            ),
        ];
        let summary = summarize(0, &records);
        assert_eq!(summary.callers[0].command, "lf task status");
        assert_eq!(
            (
                summary.callers[0].tally.spent,
                summary.callers[0].tally.held
            ),
            (2, 1)
        );
        assert_eq!(summary.hours.len(), 2);
        assert_eq!(
            (summary.hours[1].tally.spent, summary.hours[1].tally.held),
            (1, 1)
        );
        assert_eq!(summary.outcomes[&RequestOutcome::Deferred], 1);
    }
}
