//! What each recorded agent step's submitted input was made of.
//!
//! A Session capture already holds the evidence: `context.json` attributes the
//! exact assembled prompt to its sources, and `events.jsonl` retains provider
//! requests, tool results and compaction boundaries. This reader folds both
//! into one breakdown per step. It writes nothing and owns no store.
//!
//! Units differ by source and are never converted. Assembled sources and tool
//! output are cl100k tokens counted locally; `carried` and `compaction` are the
//! provider's own counts. A source the record cannot measure is `None`, not zero.

use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::engine::context_budget::{BudgetKey, ContextBudgets};
use crate::engine::prompt::count_tokens;
use crate::session_record::SessionHistory;
use crate::trace::{ContextAsset, ContextAssetKind, ContextDecision};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    /// Operating, surface, provider, repository and skill instructions.
    Instructions,
    Memory,
    Scratch,
    Goal,
    Steers,
    /// Documents, diffs, summaries, clipboard, user speech and assembly glue.
    Other,
    /// First provider request beyond the assembled input: conversation history
    /// carried by resume, plus the provider's own preamble and tool definitions.
    Carried,
    /// Tool results returned into the main conversation during the step.
    ToolOutput,
    /// Observed compaction boundaries; tokens in context before each.
    Compaction,
}

impl ContextSource {
    pub const ALL: [Self; 9] = [
        Self::Instructions,
        Self::Memory,
        Self::Scratch,
        Self::Goal,
        Self::Steers,
        Self::Other,
        Self::Carried,
        Self::ToolOutput,
        Self::Compaction,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Instructions => "instructions",
            Self::Memory => "memory",
            Self::Scratch => "scratch",
            Self::Goal => "goal",
            Self::Steers => "steers",
            Self::Other => "other",
            Self::Carried => "carried",
            Self::ToolOutput => "tool output",
            Self::Compaction => "compaction",
        }
    }

    fn of(kind: ContextAssetKind) -> Self {
        use ContextAssetKind as Kind;
        match kind {
            Kind::OperatingInstructions
            | Kind::SurfaceInstructions
            | Kind::ProviderInstructions
            | Kind::RepoInstructions
            | Kind::SkillInstructions
            | Kind::Direction => Self::Instructions,
            Kind::Memory => Self::Memory,
            Kind::Scratch => Self::Scratch,
            Kind::Goal => Self::Goal,
            Kind::Steer => Self::Steers,
            Kind::Chat
            | Kind::Summary
            | Kind::Document
            | Kind::Diff
            | Kind::Clipboard
            | Kind::UserMessage
            | Kind::Assembly => Self::Other,
        }
    }

    fn budget(self) -> Option<BudgetKey> {
        match self {
            Self::Memory => Some(BudgetKey::MemoryTokens),
            Self::Scratch => Some(BudgetKey::ScratchTokens),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceUsage {
    pub source: ContextSource,
    pub tokens: Option<u64>,
    /// Assets, steers, tool results or compactions; unknown when unrecorded.
    pub count: Option<u64>,
    /// Recorded steer authors. Empty for every other source.
    pub authors: Vec<String>,
    pub budget_tokens: Option<u64>,
    pub over_budget: bool,
}

impl SourceUsage {
    fn unmeasured(source: ContextSource) -> Self {
        Self {
            source,
            tokens: None,
            count: None,
            authors: Vec::new(),
            budget_tokens: None,
            over_budget: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepContext {
    pub input: String,
    pub session_id: String,
    pub skill: Option<String>,
    pub harness: String,
    pub observed_at: i64,
    pub sources: Vec<SourceUsage>,
    /// Everything Loopflow assembled and submitted, in cl100k tokens.
    pub assembled_tokens: Option<u64>,
    /// Input the provider reported for its first and largest requests.
    pub first_request_tokens: Option<u64>,
    pub peak_request_tokens: Option<u64>,
    pub gaps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReport {
    pub steps: Vec<StepContext>,
    pub totals: Vec<SourceUsage>,
}

impl ContextReport {
    pub fn new(steps: Vec<StepContext>) -> Self {
        let totals = ContextSource::ALL
            .into_iter()
            .map(|source| {
                let mut total = SourceUsage::unmeasured(source);
                let mut authors = BTreeSet::new();
                for row in steps
                    .iter()
                    .flat_map(|step| &step.sources)
                    .filter(|usage| usage.source == source)
                {
                    total.tokens = sum(total.tokens, row.tokens);
                    total.count = sum(total.count, row.count);
                    total.over_budget |= row.over_budget;
                    authors.extend(row.authors.iter().cloned());
                }
                total.authors = authors.into_iter().collect();
                total
            })
            .collect();
        Self { steps, totals }
    }
}

fn sum(total: Option<u64>, value: Option<u64>) -> Option<u64> {
    match (total, value) {
        (Some(total), Some(value)) => Some(total + value),
        (total, value) => total.or(value),
    }
}

/// Explain one recorded step from its capture, flagged against current budgets.
pub fn step_context(home: &Path, history: &SessionHistory) -> StepContext {
    let dir = history
        .artifact_key
        .as_deref()
        .and_then(|key| crate::session_record::record_dir(home, key));
    let mut step = match &dir {
        Some(dir) => read_capture(dir),
        None => Measured {
            gaps: vec!["this input has no retained capture".to_string()],
            ..Measured::default()
        },
    }
    .into_step(history);
    if let Some(budgets) = budgets(history) {
        flag(&mut step, &budgets);
    }
    step
}

/// Current budgets of the step's checkout; defaults when it is gone.
fn budgets(history: &SessionHistory) -> Option<ContextBudgets> {
    let checkout = [history.worktree.as_deref(), history.repo.as_deref()]
        .into_iter()
        .flatten()
        .map(Path::new)
        .find(|path| path.is_dir());
    let config = checkout
        .and_then(|path| {
            crate::engine::config::load_config(Some(path))
                .ok()
                .flatten()
        })
        .unwrap_or_default();
    ContextBudgets::resolve(
        &config,
        checkout.unwrap_or(Path::new("")),
        checkout.and(history.wave_name.as_deref()),
    )
    .ok()
}

fn flag(step: &mut StepContext, budgets: &ContextBudgets) {
    for usage in &mut step.sources {
        if let Some(key) = usage.source.budget() {
            let limit = budgets.limit(key) as u64;
            usage.budget_tokens = Some(limit);
            usage.over_budget = usage.tokens.is_some_and(|tokens| tokens > limit);
        }
    }
}

// -- Capture reading -----------------------------------------------------------

#[derive(Deserialize)]
struct ContextArtifact {
    context: RecordedContext,
}

#[derive(Deserialize)]
struct RecordedContext {
    system: Option<RecordedChannel>,
    task: RecordedChannel,
    decisions: Vec<ContextDecision>,
}

#[derive(Deserialize)]
struct RecordedChannel {
    assets: Vec<ContextAsset>,
}

#[derive(Debug, Default)]
struct Measured {
    /// Present only when `context.json` was readable.
    assembled: Option<BTreeMap<ContextSource, (u64, u64)>>,
    steers: Option<u64>,
    authors: BTreeSet<String>,
    first_request: Option<u64>,
    peak_request: Option<u64>,
    tool_results: u64,
    tool_tokens: u64,
    compactions: u64,
    compaction_tokens: Option<u64>,
    events_read: bool,
    gaps: Vec<String>,
}

fn read_capture(dir: &Path) -> Measured {
    let mut measured = Measured::default();
    match std::fs::read(dir.join("context.json")) {
        Ok(bytes) => match serde_json::from_slice::<ContextArtifact>(&bytes) {
            Ok(artifact) => measured.read_context(artifact.context),
            Err(error) => measured
                .gaps
                .push(format!("context.json is unreadable: {error}")),
        },
        Err(_) => measured
            .gaps
            .push("no context.json: assembled sources were not captured".to_string()),
    }
    match std::fs::File::open(dir.join("events.jsonl")) {
        Ok(file) => {
            measured.events_read = true;
            let mut requests = BTreeSet::new();
            for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
                if let Ok(event) = serde_json::from_str::<Value>(&line) {
                    measured.read_event(&event, &mut requests);
                }
            }
        }
        Err(_) => measured
            .gaps
            .push("no events.jsonl: provider activity was not captured".to_string()),
    }
    measured
}

impl Measured {
    fn read_context(&mut self, context: RecordedContext) {
        let mut assembled = BTreeMap::new();
        for asset in context
            .system
            .iter()
            .flat_map(|channel| &channel.assets)
            .chain(&context.task.assets)
        {
            let entry: &mut (u64, u64) =
                assembled.entry(ContextSource::of(asset.kind)).or_default();
            entry.0 += asset.attributed_tokens;
            entry.1 += 1;
        }
        let steers = context
            .decisions
            .iter()
            .filter(|decision| {
                decision
                    .source_path
                    .as_deref()
                    .is_some_and(|path| path.starts_with("steer:"))
            })
            .collect::<Vec<_>>();
        if !steers.is_empty() {
            self.steers = Some(steers.len() as u64);
            self.authors = steers
                .iter()
                .map(|decision| decision.label.clone())
                .collect();
        } else if !assembled.contains_key(&ContextSource::Steers) {
            self.steers = Some(0);
        } else {
            self.gaps
                .push("steers were submitted without recorded count or authors".to_string());
        }
        self.assembled = Some(assembled);
    }

    fn read_event(&mut self, event: &Value, requests: &mut BTreeSet<String>) {
        match event["type"].as_str() {
            Some("provider_output") if event["stream"] == "stdout" => {
                let Some(line) = event["line"].as_str() else {
                    return;
                };
                // Cheap rejection before parsing megabytes of unrelated output.
                if !line.starts_with('{') {
                    return;
                }
                if let Ok(output) = serde_json::from_str::<Value>(line) {
                    self.read_claude_output(&output, requests);
                }
            }
            Some("conversation") if event["event"]["type"] == "item_completed" => {
                if let Some(output) = event["event"]["item"]["output"].as_str() {
                    self.add_tool_result(output);
                }
            }
            Some("usage") => {
                let peak = event["usage"]["peak_input_tokens"].as_u64();
                self.peak_request = self.peak_request.max(peak);
            }
            _ => {}
        }
    }

    /// Claude's stream-json. Subagent traffic has its own context window, so
    /// only lines without a parent tool use describe this step's conversation.
    fn read_claude_output(&mut self, output: &Value, requests: &mut BTreeSet<String>) {
        if !output["parent_tool_use_id"].is_null() {
            return;
        }
        match output["type"].as_str() {
            Some("assistant") => {
                let usage = &output["message"]["usage"];
                let Some(input) = usage["input_tokens"].as_u64() else {
                    return;
                };
                // One request streams several lines repeating the same usage.
                let id = output["message"]["id"].as_str().unwrap_or_default();
                if !id.is_empty() && !requests.insert(id.to_string()) {
                    return;
                }
                let request = input
                    + usage["cache_read_input_tokens"].as_u64().unwrap_or(0)
                    + usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                self.first_request.get_or_insert(request);
                self.peak_request = self.peak_request.max(Some(request));
            }
            Some("user") => {
                for block in output["message"]["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|block| block["type"] == "tool_result")
                {
                    match &block["content"] {
                        Value::String(text) => self.add_tool_result(text),
                        Value::Array(parts) => {
                            let text = parts
                                .iter()
                                .filter_map(|part| part["text"].as_str())
                                .collect::<Vec<_>>()
                                .join("\n");
                            self.add_tool_result(&text);
                        }
                        _ => self.add_tool_result(""),
                    }
                }
            }
            Some("system") if output["subtype"] == "compact_boundary" => {
                self.compactions += 1;
                self.compaction_tokens = sum(
                    self.compaction_tokens,
                    output["compact_metadata"]["pre_tokens"].as_u64(),
                );
            }
            _ => {}
        }
    }

    fn add_tool_result(&mut self, text: &str) {
        self.tool_results += 1;
        if !text.is_empty() {
            self.tool_tokens += count_tokens(text) as u64;
        }
    }

    fn into_step(mut self, history: &SessionHistory) -> StepContext {
        let assembled_tokens = self
            .assembled
            .as_ref()
            .map(|sources| sources.values().map(|(tokens, _)| tokens).sum::<u64>());
        if self.events_read && self.first_request.is_none() {
            self.gaps.push(
                "provider reported no per-request input: carried context is unknown".to_string(),
            );
        }
        let carried = self
            .first_request
            .zip(assembled_tokens)
            .map(|(first, assembled)| first.saturating_sub(assembled));
        let events = self.events_read;
        let sources = ContextSource::ALL
            .into_iter()
            .map(|source| {
                let mut usage = SourceUsage::unmeasured(source);
                match source {
                    ContextSource::Carried => usage.tokens = carried,
                    ContextSource::ToolOutput => {
                        usage.tokens = events.then_some(self.tool_tokens);
                        usage.count = events.then_some(self.tool_results);
                    }
                    ContextSource::Compaction => {
                        usage.tokens = self.compaction_tokens;
                        usage.count = events.then_some(self.compactions);
                    }
                    _ => {
                        if let Some(assembled) = &self.assembled {
                            let (tokens, assets) =
                                assembled.get(&source).copied().unwrap_or_default();
                            usage.tokens = Some(tokens);
                            usage.count = Some(assets);
                        }
                        if source == ContextSource::Steers {
                            usage.count = self.steers;
                            usage.authors = self.authors.iter().cloned().collect();
                        }
                    }
                }
                usage
            })
            .collect();
        StepContext {
            input: history.selector().to_string(),
            session_id: history.session_id.clone(),
            skill: history.skill.clone(),
            harness: history.harness.clone(),
            observed_at: history.observed_at,
            sources,
            assembled_tokens,
            first_request_tokens: self.first_request,
            peak_request_tokens: self.peak_request,
            gaps: self.gaps,
        }
    }
}

// -- Rendering -----------------------------------------------------------------

/// One step, one source per line.
pub fn render_step(step: &StepContext) -> String {
    let mut lines = vec![
        "Context by source (cl100k tokens; carried and compaction in provider tokens):".to_string(),
    ];
    for usage in &step.sources {
        let mut detail = Vec::new();
        match (usage.source, usage.count) {
            (ContextSource::Steers, Some(count)) => {
                let by = if usage.authors.is_empty() {
                    String::new()
                } else {
                    format!(" by {}", usage.authors.join(", "))
                };
                detail.push(format!("{count} steers{by}"));
            }
            (ContextSource::ToolOutput, Some(count)) => detail.push(format!("{count} results")),
            (ContextSource::Compaction, Some(count)) => detail.push(format!("{count} compactions")),
            (ContextSource::Carried, _) if usage.tokens.is_some() => {
                detail.push("resume history and provider preamble".to_string())
            }
            _ => {}
        }
        detail.extend(budget_note(usage.budget_tokens, usage.over_budget));
        lines.push(format!(
            "  {:<14} {:>10}  {}",
            usage.source.name(),
            cell(usage.tokens),
            detail.join("; ")
        ));
    }
    lines.push(format!(
        "  {:<14} {:>10}",
        "assembled",
        cell(step.assembled_tokens)
    ));
    lines.push(format!(
        "  {:<14} {:>10}  provider tokens",
        "first request",
        cell(step.first_request_tokens)
    ));
    lines.push(format!(
        "  {:<14} {:>10}  provider tokens",
        "peak request",
        cell(step.peak_request_tokens)
    ));
    for gap in &step.gaps {
        lines.push(format!("  Unavailable: {gap}"));
    }
    lines.join("\n")
}

/// One row per step with a totals row; `*` marks a source over its budget.
pub fn render_report(report: &ContextReport) -> String {
    const WIDTH: usize = 12;
    let mut header = format!("{:<12}  {:<16}", "TIME", "STEP");
    for source in ContextSource::ALL {
        header.push_str(&format!("  {:>WIDTH$}", source.name().to_uppercase()));
    }
    header.push_str(&format!(
        "  {:>WIDTH$}  {:>WIDTH$}  INPUT",
        "ASSEMBLED", "PEAK"
    ));
    let mut lines = vec![
        "CONTEXT BY SOURCE (cl100k tokens; carried, compaction and peak in provider tokens; * over budget)".to_string(),
        header,
    ];
    let flagged = |tokens: Option<u64>, over: bool| {
        format!("{}{}", cell(tokens), if over { "*" } else { "" })
    };
    for step in &report.steps {
        let mut line = format!(
            "{:<12}  {:<16}",
            format_time(step.observed_at),
            crate::lf::output::truncate(step.skill.as_deref().unwrap_or(&step.harness), 16)
        );
        for usage in &step.sources {
            line.push_str(&format!(
                "  {:>WIDTH$}",
                flagged(usage.tokens, usage.over_budget)
            ));
        }
        line.push_str(&format!(
            "  {:>WIDTH$}  {:>WIDTH$}  {}",
            cell(step.assembled_tokens),
            cell(step.peak_request_tokens),
            crate::lf::commands::util::short_id(&step.input)
        ));
        lines.push(line);
    }
    let mut total = format!("{:<12}  {:<16}", "TOTAL", "");
    for usage in &report.totals {
        total.push_str(&format!(
            "  {:>WIDTH$}",
            flagged(usage.tokens, usage.over_budget)
        ));
    }
    lines.push(total);
    if let Some(steers) = report
        .totals
        .iter()
        .find(|usage| usage.source == ContextSource::Steers)
    {
        if let Some(count) = steers.count.filter(|count| *count > 0) {
            lines.push(format!(
                "Steers rendered: {count} across steps, by {}.",
                steers.authors.join(", ")
            ));
        }
    }
    let gaps = report
        .steps
        .iter()
        .filter(|step| !step.gaps.is_empty())
        .count();
    if gaps > 0 {
        lines.push(format!(
            "{gaps} steps have unavailable evidence; `lf monitor show <input> --context` names it."
        ));
    }
    lines.join("\n")
}

fn budget_note(budget: Option<u64>, over: bool) -> Option<String> {
    let budget = crate::lf::output::format_int(budget?);
    Some(if over {
        format!("OVER budget {budget}")
    } else {
        format!("budget {budget}")
    })
}

fn cell(tokens: Option<u64>) -> String {
    tokens.map_or_else(|| "-".to_string(), crate::lf::output::format_int)
}

fn format_time(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0)
        .map(|utc| {
            utc.with_timezone(&chrono::Local)
                .format("%b %-d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| unix.to_string())
}

#[cfg(test)]
mod tests {
    use super::{read_capture, ContextReport, ContextSource, StepContext};
    use crate::trace::{
        ContextAssetKind, ContextAssetSpec, ContextChannel, ContextDecision, ContextDecisionKind,
        ContextScope, PreparedTurnContext,
    };

    fn spec(kind: ContextAssetKind, content: &str) -> ContextAssetSpec {
        ContextAssetSpec {
            channel: ContextChannel::Task,
            kind,
            scope: ContextScope::Task,
            label: kind.as_str().to_string(),
            source_path: None,
            included_by: "test".to_string(),
            content: content.to_string(),
            match_all_occurrences: false,
        }
    }

    fn steer(id: i64, author: &str) -> ContextDecision {
        ContextDecision {
            position: id as u32,
            kind: ContextAssetKind::Steer,
            scope: ContextScope::Task,
            label: author.to_string(),
            source_path: Some(format!("steer:{id}")),
            decision: ContextDecisionKind::Included,
            reason: "rendered in the launch goal".to_string(),
            original_bytes: None,
            original_tokens: None,
            asset_position: None,
        }
    }

    fn event(seq: u64, body: serde_json::Value) -> String {
        let mut event = serde_json::json!({"schema_version": 1, "seq": seq});
        event
            .as_object_mut()
            .unwrap()
            .extend(body.as_object().unwrap().clone());
        format!("{event}\n")
    }

    fn claude(seq: u64, output: serde_json::Value) -> String {
        event(
            seq,
            serde_json::json!({"type": "provider_output", "stream": "stdout", "line": output.to_string()}),
        )
    }

    fn step(dir: &std::path::Path) -> StepContext {
        let history: crate::session_record::SessionHistory = serde_json::from_str(include_str!(
            "../../../tests/fixtures/dto/session_history_summary.json"
        ))
        .unwrap();
        read_capture(dir).into_step(&history)
    }

    fn tokens(step: &StepContext, source: ContextSource) -> Option<u64> {
        step.sources
            .iter()
            .find(|usage| usage.source == source)
            .unwrap()
            .tokens
    }

    #[test]
    fn a_step_is_explained_source_by_source_from_its_capture() {
        let dir = tempfile::tempdir().unwrap();
        let prompt = "SKILL rules\nMEMORY notes\nSCRATCH plan\nGOAL text\n<lf:steers>\n- first\n- second\n</lf:steers>";
        let context = PreparedTurnContext::from_attributed_prompts(
            "",
            prompt,
            vec![
                spec(ContextAssetKind::SkillInstructions, "SKILL rules"),
                spec(ContextAssetKind::Memory, "MEMORY notes"),
                spec(ContextAssetKind::Scratch, "SCRATCH plan"),
                spec(ContextAssetKind::Goal, "GOAL text"),
                spec(
                    ContextAssetKind::Steer,
                    "<lf:steers>\n- first\n- second\n</lf:steers>",
                ),
            ],
            vec![steer(1, "user"), steer(2, "user")],
        );
        let assembled = context.total_tokens();
        std::fs::write(
            dir.path().join("context.json"),
            serde_json::json!({"schema_version": 1, "context": context}).to_string(),
        )
        .unwrap();
        let request = |id: &str, cached: u64| {
            serde_json::json!({"type": "assistant", "parent_tool_use_id": null, "message": {"id": id,
                "usage": {"input_tokens": 10, "cache_read_input_tokens": cached, "cache_creation_input_tokens": 0}}})
        };
        let result = |parent: serde_json::Value, text: &str| {
            serde_json::json!({"type": "user", "parent_tool_use_id": parent, "message": {"content": [
                {"type": "tool_result", "content": text}]}})
        };
        let events = [
            claude(0, request("a", 990)),
            claude(1, request("a", 990)),
            claude(2, result(serde_json::Value::Null, "tool said this")),
            claude(3, result(serde_json::json!("toolu_1"), "inside a subagent")),
            claude(
                4,
                serde_json::json!({"type": "system", "subtype": "compact_boundary",
                    "compact_metadata": {"trigger": "auto", "pre_tokens": 150000}}),
            ),
            claude(5, request("b", 4990)),
        ]
        .concat();
        std::fs::write(dir.path().join("events.jsonl"), events).unwrap();

        let step = step(dir.path());

        assert_eq!(step.assembled_tokens, Some(assembled));
        assert_eq!(
            step.sources
                .iter()
                .filter(|usage| matches!(
                    usage.source,
                    ContextSource::Instructions
                        | ContextSource::Memory
                        | ContextSource::Scratch
                        | ContextSource::Goal
                        | ContextSource::Steers
                        | ContextSource::Other
                ))
                .map(|usage| usage.tokens.unwrap())
                .sum::<u64>(),
            assembled
        );
        let steers = &step.sources[4];
        assert_eq!(steers.count, Some(2));
        assert_eq!(steers.authors, ["user"]);
        assert!(steers.tokens.unwrap() > 0);
        assert_eq!(step.first_request_tokens, Some(1000));
        assert_eq!(step.peak_request_tokens, Some(5000));
        assert_eq!(
            tokens(&step, ContextSource::Carried),
            Some(1000 - assembled)
        );
        let tools = &step.sources[7];
        assert_eq!(tools.count, Some(1));
        assert!(tools.tokens.unwrap() > 0);
        let compaction = &step.sources[8];
        assert_eq!(
            (compaction.count, compaction.tokens),
            (Some(1), Some(150000))
        );
        assert!(step.gaps.is_empty());
    }

    #[test]
    fn missing_evidence_is_unknown_not_zero() {
        let dir = tempfile::tempdir().unwrap();
        let step = step(dir.path());
        assert!(step.sources.iter().all(|usage| usage.tokens.is_none()));
        assert_eq!(step.assembled_tokens, None);
        assert_eq!(step.gaps.len(), 2);
        let report = ContextReport::new(vec![step]);
        assert!(report.totals.iter().all(|usage| usage.tokens.is_none()));
    }

    #[test]
    fn budgets_flag_the_sources_they_cover() {
        let dir = tempfile::tempdir().unwrap();
        let steers = format!(
            "<lf:steers>\n{}</lf:steers>",
            "- again and again\n".repeat(40)
        );
        let context = PreparedTurnContext::from_attributed_prompts(
            "",
            &steers,
            vec![spec(ContextAssetKind::Steer, &steers)],
            Vec::new(),
        );
        std::fs::write(
            dir.path().join("context.json"),
            serde_json::json!({"schema_version": 1, "context": context}).to_string(),
        )
        .unwrap();
        let mut step = step(dir.path());
        let mut config = crate::engine::config::Config::default();
        for key in crate::engine::context_budget::BudgetKey::ALL {
            config.context_budgets.insert(key, 50);
        }
        let budgets =
            crate::engine::context_budget::ContextBudgets::resolve(&config, dir.path(), None)
                .unwrap();
        super::flag(&mut step, &budgets);
        let steers = &step.sources[4];
        assert!(!steers.over_budget);
        assert_eq!(steers.budget_tokens, None);
        assert!(!step.sources[1].over_budget);
        assert!(step
            .gaps
            .iter()
            .any(|gap| gap.contains("without recorded count")));
        assert!(!super::render_step(&step).contains("OVER budget"));
    }
}
