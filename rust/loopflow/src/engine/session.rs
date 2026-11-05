//! The `session` namespace's asks: what agents wait on a person for, and
//! answering one without a terminal.
//!
//! An ask is read, never detected here: a Session asks when Rust's one
//! Available judgment (`store::sqlite::sessions`) says it waits on a person,
//! and a Task asks when it stands at an interactive step of its graph with no
//! run carrying it. Desktop's Available state is [`ask_of`] too.

use serde::{Deserialize, Serialize};

use super::{Error, ErrorCode, Result};
use crate::id::LfSessionId;
use crate::lf::commands::watch::{Subscription, Transport, Watched, ACTIVITY_CLOCK};
use crate::program_status::{Kind, State};
use crate::session::{SessionFilter, SessionSummary};
use crate::store::sqlite::SqliteStore;

/// Where an ask was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AskSource {
    /// The terminal's validated Program Status: the program reports itself
    /// blocked, or an interactive one idle.
    Report,
    /// The provider's own stream: a question it asked, a turn it handed back,
    /// or quiet with nothing running.
    Provider,
    /// The Task stands at an interactive step of its graph.
    Step,
}

/// One thing a person is waited on for. Wire type; no defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ask {
    /// The conversation an answer goes to; null for a Task at a step that
    /// has no conversation.
    pub session: Option<LfSessionId>,
    /// The Session's name, or the Task's title for a step.
    pub name: String,
    /// The Task's identifier, such as `LOO-1`.
    pub task: Option<String>,
    /// The harness the Session runs, such as `claude`.
    pub agent: Option<String>,
    /// Since when, Unix seconds: the provider's last signal, or the Task's
    /// arrival at its step. Null when nothing recorded it.
    pub since: Option<i64>,
    /// What is asked, as far as Loopflow sees it: a blocked report's message,
    /// the provider's last turn, or the step's name. Null when Loopflow has
    /// no text.
    pub question: Option<String>,
    /// What a blocked report asks for: `permission`, `question` or `auth`.
    pub kind: Option<Kind>,
    pub source: AskSource,
}

impl Ask {
    /// One line of `lf session asks`.
    pub fn render(&self) -> String {
        let place = match (&self.session, &self.task) {
            (Some(session), _) => session.to_string(),
            (None, Some(task)) => task.clone(),
            (None, None) => "-".into(),
        };
        let mut line = format!("{place}  {}", self.name);
        if let Some(task) = self.task.as_ref().filter(|_| self.session.is_some()) {
            line.push_str(&format!(" ({task})"));
        }
        if let Some(agent) = &self.agent {
            line.push_str(&format!(" · {agent}"));
        }
        match &self.question {
            Some(question) => {
                let question = question.lines().last().unwrap_or_default();
                line.push_str(&format!("\n  {question}"));
            }
            None => line.push_str("\n  (no question text)"),
        }
        line
    }
}

/// What `session.answer` delivered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answered {
    pub session: LfSessionId,
    /// Bytes of text the owner took, the submitting key not counted.
    pub bytes: usize,
    /// Enter followed the text.
    pub submitted: bool,
}

/// The ask a Session's reading poses, when it waits on a person.
pub(crate) fn ask_of(session: &SessionSummary) -> Option<Ask> {
    if !session.available {
        return None;
    }
    let report = session.program_status.as_ref();
    let blocked = report.and_then(|status| {
        status
            .records
            .iter()
            .rev()
            .find(|record| record.state == State::Blocked)
    });
    let question = blocked
        .and_then(|record| record.msg.clone().or_else(|| record.title.clone()))
        .or_else(|| session.preview.as_ref().map(|preview| preview.text.clone()))
        .filter(|text| !text.trim().is_empty());
    Some(Ask {
        session: Some(session.id.clone()),
        name: session.title.clone(),
        task: session.task_identifier.clone(),
        agent: session.provider.clone(),
        since: session.activity_observed_at,
        question,
        kind: blocked.and_then(|record| record.kind),
        // Available reads a current report when there is one, else the stream.
        source: if report.is_some() {
            AskSource::Report
        } else {
            AskSource::Provider
        },
    })
}

/// Every ask on this Machine at `now`, longest waiting first.
pub(crate) fn asks_in(store: &SqliteStore, now: i64) -> crate::store::StoreResult<Vec<Ask>> {
    let filter = SessionFilter {
        available: true,
        interactive: None,
        limit: 0,
        ..SessionFilter::default()
    };
    let mut asks: Vec<Ask> = store
        .session_summaries(&filter, now)?
        .iter()
        .filter_map(ask_of)
        .collect();
    asks.extend(
        store
            .tasks_at_interactive_steps()?
            .into_iter()
            .map(|waiting| Ask {
                session: waiting.conversation,
                name: waiting.title,
                task: waiting.identifier,
                agent: None,
                since: Some(waiting.since),
                question: Some(waiting.step.name),
                kind: None,
                source: AskSource::Step,
            }),
    );
    asks.sort_by(|a, b| {
        (a.since.is_none(), a.since, &a.session, &a.task).cmp(&(
            b.since.is_none(),
            b.since,
            &b.session,
            &b.task,
        ))
    });
    Ok(asks)
}

/// `session.asks`: the questions agents wait on, a complete list.
pub fn asks() -> Result<Vec<Ask>> {
    let Some(registry) = crate::store::read_existing_registry().map_err(unavailable)? else {
        return Ok(Vec::new());
    };
    asks_in(&registry.sqlite, now()).map_err(unavailable)
}

/// `session.asks` as a subscription: the whole list whenever it may change.
pub fn watch_asks(transport: Transport) -> anyhow::Result<Option<Subscription>> {
    // A clock tick reads again only when an AgentProcess lived or died or a
    // Session went quiet long enough to become available; commits read at once.
    let watched =
        Watched::every(ACTIVITY_CLOCK).probing(|store| Ok(store.session_observations(now())?));
    crate::lf::commands::watch::start(
        (),
        watched,
        |store, ()| async move {
            let Some(store) = store else {
                return Ok(Vec::new());
            };
            Ok(crate::store::run_sqlite(&store.sqlite, |sqlite| asks_in(&sqlite, now())).await?)
        },
        transport,
    )
}

/// `session.answer`: type `text` into the Session's terminal, then Enter with
/// `submit`, through its owner as a viewer would. Input control is taken only
/// while no other viewer holds it and released after; uncertain input is
/// never sent again.
pub fn answer(session: &str, text: &str, submit: bool) -> Result<Answered> {
    if text.is_empty() && !submit {
        return Err(Error::new(
            ErrorCode::Invalid,
            "an answer needs text, `submit`, or both",
        ));
    }
    let registry = crate::store::read_existing_registry()
        .map_err(unavailable)?
        .ok_or_else(|| {
            Error::new(ErrorCode::Unavailable, "Machine registry is unavailable")
                .with(serde_json::json!({ "resource": "machine_registry" }))
        })?;
    let store = &registry.sqlite;
    let ended = |message: String| {
        Error::new(ErrorCode::NotFound, message).with(serde_json::json!({ "session": session }))
    };
    let Some(record) = store.session(session).map_err(unavailable)? else {
        return Err(ended(format!("no Session {session} exists")));
    };
    if record.completed_at.is_some() {
        return Err(ended(format!("Session {session} has ended")));
    }
    let Some(owner) = store.session_transport(session).map_err(unavailable)? else {
        // A running provider with no recorded owner is one Loopflow cannot
        // type into; one with no running provider has ended.
        let live = store
            .session_summary(session, now())
            .map_err(unavailable)?
            .is_some_and(|summary| {
                summary.agent_process_evidence == crate::journal::ProcessIdentityEvidence::Live
            });
        return Err(if live {
            Error::new(
                ErrorCode::Unavailable,
                format!("Session {session} runs without a Loopflow owner to deliver input to"),
            )
            .with(serde_json::json!({ "session": session }))
        } else {
            ended(format!("Session {session} has no running owner"))
        });
    };
    match crate::transport::answer(&owner, text.as_bytes(), submit) {
        Ok(crate::transport::Answer::Delivered { .. }) => Ok(Answered {
            session: record.id,
            bytes: text.len(),
            submitted: submit,
        }),
        Ok(crate::transport::Answer::Busy) => Err(Error::new(
            ErrorCode::Conflict,
            format!("another viewer holds Session {session}'s input; nothing was sent"),
        )
        .with(serde_json::json!({ "session": session }))),
        Ok(crate::transport::Answer::Ended) => Err(Error::new(
            ErrorCode::NotFound,
            format!("Session {session}'s owner is not running; nothing was sent"),
        )
        .with(serde_json::json!({ "session": session, "owner": owner }))),
        Err(error) => Err(Error::outcome_unknown(&error)),
    }
}

fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn unavailable(error: impl Into<anyhow::Error>) -> Error {
    Error::refused(&error.into(), ErrorCode::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::{asks_in, Answered, Ask, AskSource};
    use crate::durable::TaskId;
    use crate::program_status::{Kind, Records, Report, State};
    use crate::session::SessionActivity;
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn results_match_the_shared_fixtures_without_defaults() {
        let reading: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/server/session_asks_reading.json"
        ))
        .unwrap();
        let body = &reading["frame"]["body"];
        let asks: Vec<Ask> = serde_json::from_value(body.clone()).unwrap();
        assert_eq!(&serde_json::to_value(&asks).unwrap(), body);
        assert_eq!(asks[0].kind, Some(Kind::Permission));
        assert_eq!(asks[1].source, AskSource::Step);
        assert_eq!(
            asks[0].render(),
            "session_5e55105e000040008000000000000001  Migration review (LOO-1) · claude\n  Apply the migration to production?"
        );
        let response: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/server/session_answered.json"
        ))
        .unwrap();
        let answered: Answered = serde_json::from_value(response["result"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&answered).unwrap(), response["result"]);
        assert!(answered.submitted);
        // A missing field is an error, never a default.
        assert!(
            serde_json::from_str::<Ask>(r#"{"session":null,"name":"x","task":null,"agent":null,"since":null,"question":null,"kind":null}"#).is_err()
        );
        assert!(serde_json::from_str::<Answered>(r#"{"session":"s","bytes":3}"#).is_err());
    }

    /// A Session whose live provider asked for approval, one whose terminal
    /// reports a blocked question, one still working, and a Task at its
    /// interactive step: three asks, longest waiting first.
    #[test]
    fn asks_read_available_sessions_and_tasks_at_interactive_steps() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let pid = std::process::id();
        let started = crate::journal::process_started_at(pid).unwrap().unwrap();
        for (session, observed_at, pending_input) in
            [("approval", 30, 1), ("blocked", 20, 0), ("working", 10, 0)]
        {
            store.test_session(session, &crate::session_record::new_artifact_key());
            let attachment = store
                .claim_session_attachment(session, None, &store.test_lf_process(), true)
                .unwrap();
            store
                .record_agent_process_identity(session, &attachment, pid, started)
                .unwrap();
            store
                .record_session_activity(
                    session,
                    &attachment,
                    &SessionActivity {
                        preview: None,
                        observed_at,
                        open_tools: 1,
                        pending_input,
                        yielded: false,
                    },
                )
                .unwrap();
            if session == "blocked" {
                let agent = Some(&attachment.agent_process_id);
                store
                    .begin_program_status(session, agent, "terminal")
                    .unwrap();
                let records = Records {
                    seen: true,
                    records: vec![Report {
                        state: State::Blocked,
                        id: None,
                        kind: Some(Kind::Question),
                        progress: None,
                        app: None,
                        title: Some("Question".into()),
                        msg: Some("Which schema wins?".into()),
                    }],
                };
                store
                    .record_program_status(session, agent, "terminal", 1, &records)
                    .unwrap();
            }
        }
        let task = store
            .create_task(&crate::work::task::NewTask {
                id: TaskId::new(),
                repository_id: store.ensure_repository("/asks").unwrap(),
                project_id: None,
                title: "Ship the asks".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let graph = serde_json::json!({"name": "captured", "steps": [{"Skill":{"skill":{"name":"demo"},"id":"review","interactive":true,"returns":null,"sources":[]}}]});
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,?2,'review',5)",
                rusqlite::params![task.id.as_str(), graph.to_string()],
            )
            .unwrap();

        let asks = asks_in(&store, 40).unwrap();
        let summary: Vec<_> = asks
            .iter()
            .map(|ask| {
                (
                    ask.session.as_ref().map(|id| id.to_string()),
                    ask.since,
                    ask.question.as_deref(),
                    ask.source,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                (None, Some(5), Some("review"), AskSource::Step),
                (
                    Some("blocked".into()),
                    Some(20),
                    Some("Which schema wins?"),
                    AskSource::Report
                ),
                (Some("approval".into()), Some(30), None, AskSource::Provider),
            ]
        );
        assert_eq!(asks[0].name, "Ship the asks");
        assert!(asks[0].task.is_some());
        assert_eq!(asks[1].kind, Some(Kind::Question));

        // Desktop's Available is the same reading.
        let listed = store
            .session_summaries(
                &crate::session::SessionFilter {
                    interactive: None,
                    ..Default::default()
                },
                40,
            )
            .unwrap();
        for summary in &listed {
            assert_eq!(
                super::ask_of(summary).is_some(),
                summary.id != "working",
                "{}",
                summary.id
            );
        }
    }
}
