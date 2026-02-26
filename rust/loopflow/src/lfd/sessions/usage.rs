use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

use crate::lfd::sessions::types::{
    ContextSnapshot, PersistedSessionEvent, Session, SessionEvent, TurnUsage,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TokenTotals {
    pub input: u64,
    pub output: u64,
    pub reasoning: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl TokenTotals {
    fn add_turn_usage(&mut self, usage: &TurnUsage) {
        self.input += usage.input_tokens;
        self.output += usage.output_tokens;
        self.reasoning += usage.reasoning_tokens.unwrap_or(0);
        self.cache_read += usage.cache_read_tokens.unwrap_or(0);
        self.cache_write += usage.cache_write_tokens.unwrap_or(0);
    }

    fn merge(&mut self, other: &TokenTotals) {
        self.input += other.input;
        self.output += other.output;
        self.reasoning += other.reasoning;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionUsageAggregate {
    pub tokens: TokenTotals,
    pub turns: u64,
    pub context: Option<ContextSnapshot>,
    pub models: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct StepUsageAggregate {
    #[serde(flatten)]
    pub tokens: TokenTotals,
    pub sessions: u64,
    pub turns: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WaveUsageAggregate {
    pub tokens: TokenTotals,
    pub sessions: u64,
    pub turns: u64,
    pub models: BTreeMap<String, u64>,
    pub by_step: BTreeMap<String, StepUsageAggregate>,
}

#[derive(Debug, Clone)]
pub struct UsageSessionData {
    pub session: Session,
    pub events: Vec<PersistedSessionEvent>,
    pub wave_id: Option<String>,
    pub flow: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    Wave,
    Flow,
    Step,
    Model,
    Source,
}

impl GroupBy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wave => "wave",
            Self::Flow => "flow",
            Self::Step => "step",
            Self::Model => "model",
            Self::Source => "source",
        }
    }
}

impl std::str::FromStr for GroupBy {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "wave" => Ok(Self::Wave),
            "flow" => Ok(Self::Flow),
            "step" => Ok(Self::Step),
            "model" => Ok(Self::Model),
            "source" => Ok(Self::Source),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsageSummaryGroupAggregate {
    pub key: String,
    pub tokens: TokenTotals,
    pub sessions: u64,
    pub turns: u64,
}

pub fn aggregate_wave_usage(
    _wave_id: &str,
    sessions: &[(Session, Vec<PersistedSessionEvent>)],
) -> WaveUsageAggregate {
    let mut aggregate = WaveUsageAggregate::default();

    for (session, events) in sessions {
        let session_usage = aggregate_session_events(events, None);

        aggregate.sessions += 1;
        aggregate.turns += session_usage.turns;
        aggregate.tokens.merge(&session_usage.tokens);
        merge_model_counts(&mut aggregate.models, &session_usage.models);

        let step = step_key(session);
        let step_usage = aggregate.by_step.entry(step).or_default();
        step_usage.sessions += 1;
        step_usage.turns += session_usage.turns;
        step_usage.tokens.merge(&session_usage.tokens);
    }

    aggregate
}

pub fn aggregate_summary(
    group_by: GroupBy,
    sessions: &[UsageSessionData],
    model_filter: Option<&str>,
) -> Vec<UsageSummaryGroupAggregate> {
    match group_by {
        GroupBy::Wave => aggregate_summary_by_key(sessions, model_filter, |data| {
            data.wave_id
                .clone()
                .or_else(|| data.session.config.wave.clone())
                .unwrap_or_else(|| "unknown".to_string())
        }),
        GroupBy::Flow => aggregate_summary_by_key(sessions, model_filter, |data| {
            data.flow.clone().unwrap_or_else(|| "unknown".to_string())
        }),
        GroupBy::Step => {
            aggregate_summary_by_key(sessions, model_filter, |data| step_key(&data.session))
        }
        GroupBy::Model => aggregate_summary_by_model(sessions, model_filter),
        GroupBy::Source => aggregate_summary_by_source(sessions),
    }
}

pub fn aggregate_session_events(
    events: &[PersistedSessionEvent],
    model_filter: Option<&str>,
) -> SessionUsageAggregate {
    let mut aggregate = SessionUsageAggregate::default();

    for event in events {
        match &event.event {
            SessionEvent::TurnUsage { usage, .. } => {
                if let Some(filter) = model_filter {
                    if usage.model.as_deref() != Some(filter) {
                        continue;
                    }
                }

                aggregate.turns += 1;
                aggregate.tokens.add_turn_usage(usage);
                if let Some(model) = usage.model.as_ref() {
                    *aggregate.models.entry(model.clone()).or_insert(0) += 1;
                }
            }
            SessionEvent::ContextSnapshot { snapshot } => {
                if aggregate.context.is_none() {
                    aggregate.context = Some(snapshot.clone());
                }
            }
            _ => {}
        }
    }

    aggregate
}

fn aggregate_summary_by_key(
    sessions: &[UsageSessionData],
    model_filter: Option<&str>,
    key_fn: impl Fn(&UsageSessionData) -> String,
) -> Vec<UsageSummaryGroupAggregate> {
    let mut groups: BTreeMap<String, UsageSummaryGroupAggregate> = BTreeMap::new();

    for data in sessions {
        let usage = aggregate_session_events(&data.events, model_filter);
        if model_filter.is_some() && usage.turns == 0 {
            continue;
        }

        let key = key_fn(data);
        let group = groups
            .entry(key.clone())
            .or_insert(UsageSummaryGroupAggregate {
                key,
                tokens: TokenTotals::default(),
                sessions: 0,
                turns: 0,
            });
        group.tokens.merge(&usage.tokens);
        group.sessions += 1;
        group.turns += usage.turns;
    }

    groups.into_values().collect()
}

fn aggregate_summary_by_model(
    sessions: &[UsageSessionData],
    model_filter: Option<&str>,
) -> Vec<UsageSummaryGroupAggregate> {
    let mut groups: BTreeMap<String, UsageSummaryGroupAggregate> = BTreeMap::new();

    for data in sessions {
        let mut models_seen = HashSet::new();

        for event in &data.events {
            let SessionEvent::TurnUsage { usage, .. } = &event.event else {
                continue;
            };
            let Some(model) = usage.model.as_deref() else {
                continue;
            };
            if let Some(filter) = model_filter {
                if model != filter {
                    continue;
                }
            }

            let group = groups
                .entry(model.to_string())
                .or_insert(UsageSummaryGroupAggregate {
                    key: model.to_string(),
                    tokens: TokenTotals::default(),
                    sessions: 0,
                    turns: 0,
                });
            group.tokens.add_turn_usage(usage);
            group.turns += 1;
            if models_seen.insert(model.to_string()) {
                group.sessions += 1;
            }
        }
    }

    groups.into_values().collect()
}

fn aggregate_summary_by_source(sessions: &[UsageSessionData]) -> Vec<UsageSummaryGroupAggregate> {
    let mut groups: BTreeMap<String, UsageSummaryGroupAggregate> = BTreeMap::new();

    for data in sessions {
        let session_usage = aggregate_session_events(&data.events, None);
        let Some(snapshot) = session_usage.context else {
            continue;
        };

        for (source, source_tokens) in snapshot.sources {
            let group = groups
                .entry(source.clone())
                .or_insert(UsageSummaryGroupAggregate {
                    key: source,
                    tokens: TokenTotals::default(),
                    sessions: 0,
                    turns: 0,
                });
            group.tokens.input += source_tokens;
            group.sessions += 1;
            group.turns += session_usage.turns;
        }
    }

    groups.into_values().collect()
}

fn step_key(session: &Session) -> String {
    let step = session.config.step.trim();
    if step.is_empty() {
        return "unknown".to_string();
    }
    step.to_string()
}

fn merge_model_counts(target: &mut BTreeMap<String, u64>, source: &BTreeMap<String, u64>) {
    for (model, count) in source {
        *target.entry(model.clone()).or_insert(0) += count;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::lfd::id::LfdId;
    use crate::lfd::sessions::types::{SessionConfig, SessionStatus};
    use time::OffsetDateTime;

    #[test]
    fn aggregate_session_events_sums_turn_usage_and_context() {
        let events = vec![
            context_event(HashMap::from([
                ("step".to_string(), 100),
                ("diff".to_string(), 400),
            ])),
            turn_usage_event(TurnUsage {
                input_tokens: 120,
                output_tokens: 25,
                reasoning_tokens: Some(10),
                cache_read_tokens: Some(5),
                cache_write_tokens: Some(3),
                model: Some("claude-sonnet-4".to_string()),
                cost_usd: None,
            }),
            turn_usage_event(TurnUsage {
                input_tokens: 80,
                output_tokens: 20,
                reasoning_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                model: Some("claude-sonnet-4".to_string()),
                cost_usd: None,
            }),
        ];

        let usage = aggregate_session_events(&events, None);
        assert_eq!(usage.tokens.input, 200);
        assert_eq!(usage.tokens.output, 45);
        assert_eq!(usage.tokens.reasoning, 10);
        assert_eq!(usage.tokens.cache_read, 5);
        assert_eq!(usage.tokens.cache_write, 3);
        assert_eq!(usage.turns, 2);
        assert_eq!(usage.models.get("claude-sonnet-4"), Some(&2));
        assert_eq!(
            usage
                .context
                .expect("context snapshot should exist")
                .sources
                .get("diff"),
            Some(&400)
        );
    }

    #[test]
    fn aggregate_wave_usage_rolls_up_step_breakdown() {
        let session_a = test_session("implement");
        let session_b = test_session("gate");
        let wave_usage = aggregate_wave_usage(
            "wave-1",
            &[
                (
                    session_a,
                    vec![turn_usage_event(TurnUsage {
                        input_tokens: 150,
                        output_tokens: 30,
                        reasoning_tokens: Some(5),
                        cache_read_tokens: None,
                        cache_write_tokens: None,
                        model: Some("claude-sonnet-4".to_string()),
                        cost_usd: None,
                    })],
                ),
                (
                    session_b,
                    vec![turn_usage_event(TurnUsage {
                        input_tokens: 90,
                        output_tokens: 12,
                        reasoning_tokens: None,
                        cache_read_tokens: Some(8),
                        cache_write_tokens: Some(4),
                        model: Some("claude-haiku-4-5".to_string()),
                        cost_usd: None,
                    })],
                ),
            ],
        );

        assert_eq!(wave_usage.sessions, 2);
        assert_eq!(wave_usage.turns, 2);
        assert_eq!(wave_usage.tokens.input, 240);
        assert_eq!(wave_usage.models.get("claude-sonnet-4"), Some(&1));
        assert_eq!(wave_usage.models.get("claude-haiku-4-5"), Some(&1));

        let implement = wave_usage.by_step.get("implement").expect("implement step");
        assert_eq!(implement.sessions, 1);
        assert_eq!(implement.tokens.input, 150);

        let gate = wave_usage.by_step.get("gate").expect("gate step");
        assert_eq!(gate.sessions, 1);
        assert_eq!(gate.tokens.input, 90);
    }

    #[test]
    fn aggregate_summary_by_model_honors_model_filter() {
        let records = vec![UsageSessionData {
            session: test_session("implement"),
            events: vec![
                turn_usage_event(TurnUsage {
                    input_tokens: 100,
                    output_tokens: 20,
                    reasoning_tokens: None,
                    cache_read_tokens: None,
                    cache_write_tokens: None,
                    model: Some("claude-sonnet-4".to_string()),
                    cost_usd: None,
                }),
                turn_usage_event(TurnUsage {
                    input_tokens: 60,
                    output_tokens: 8,
                    reasoning_tokens: None,
                    cache_read_tokens: None,
                    cache_write_tokens: None,
                    model: Some("claude-haiku-4-5".to_string()),
                    cost_usd: None,
                }),
            ],
            wave_id: Some("wave-alpha".to_string()),
            flow: Some("build".to_string()),
        }];

        let groups = aggregate_summary(GroupBy::Model, &records, Some("claude-haiku-4-5"));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, "claude-haiku-4-5");
        assert_eq!(groups[0].tokens.input, 60);
        assert_eq!(groups[0].sessions, 1);
        assert_eq!(groups[0].turns, 1);
    }

    #[test]
    fn aggregate_summary_by_source_uses_context_snapshot() {
        let records = vec![UsageSessionData {
            session: test_session("implement"),
            events: vec![
                context_event(HashMap::from([
                    ("step".to_string(), 120),
                    ("diff".to_string(), 340),
                ])),
                turn_usage_event(TurnUsage {
                    input_tokens: 200,
                    output_tokens: 40,
                    reasoning_tokens: None,
                    cache_read_tokens: None,
                    cache_write_tokens: None,
                    model: Some("claude-sonnet-4".to_string()),
                    cost_usd: None,
                }),
            ],
            wave_id: Some("wave-alpha".to_string()),
            flow: Some("build".to_string()),
        }];

        let groups = aggregate_summary(GroupBy::Source, &records, None);
        let diff = groups
            .iter()
            .find(|group| group.key == "diff")
            .expect("diff");
        assert_eq!(diff.tokens.input, 340);
        assert_eq!(diff.sessions, 1);
        assert_eq!(diff.turns, 1);
    }

    fn test_session(step: &str) -> Session {
        Session {
            id: LfdId::new(),
            harness: "claude".to_string(),
            status: SessionStatus::Ended,
            wave_run_id: None,
            provider_session_id: None,
            config: SessionConfig {
                step: step.to_string(),
                repo_root: "/tmp/repo".to_string(),
                ..Default::default()
            },
            created_at: OffsetDateTime::now_utc(),
            ended_at: None,
        }
    }

    fn context_event(sources: HashMap<String, u64>) -> PersistedSessionEvent {
        PersistedSessionEvent {
            session_id: LfdId::new(),
            seq: 0,
            event: SessionEvent::ContextSnapshot {
                snapshot: ContextSnapshot {
                    sources,
                    budget: 200_000,
                    total: 460,
                    diff_tier: "UnifiedDiff".to_string(),
                },
            },
            created_at: OffsetDateTime::now_utc(),
        }
    }

    fn turn_usage_event(usage: TurnUsage) -> PersistedSessionEvent {
        PersistedSessionEvent {
            session_id: LfdId::new(),
            seq: 1,
            event: SessionEvent::TurnUsage {
                turn_id: "turn_1".to_string(),
                usage,
            },
            created_at: OffsetDateTime::now_utc(),
        }
    }
}
