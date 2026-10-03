//! `lf usage --binds` — what binding a Session to a Task does to usage totals.
//!
//! Prospective attribution leaves usage with the owner recorded when it was
//! spent. Post-hoc would give a bound Session's whole history to its Task.
//! This report reads both from the same immutable history and changes neither.

use std::collections::BTreeMap;

use anyhow::Result;
use serde::Serialize;
use time::OffsetDateTime;

use crate::lf::commands::util::short_id;
use crate::lf::output::{format_cost, format_int, truncate, Colors};
use crate::session::SessionBind;
use crate::session_record::SessionHistory;

#[derive(Debug, Serialize)]
pub struct BindReport {
    /// Sessions with any recorded input, bound or not.
    pub sessions: usize,
    pub bound: Vec<BoundSession>,
    /// Tasks and Waves whose totals differ between the two rules.
    pub by_task: Vec<RuleUsage>,
    pub by_wave: Vec<RuleUsage>,
}

#[derive(Debug, Serialize)]
pub struct BoundSession {
    pub session_id: String,
    pub task: String,
    pub wave: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub bound_at: OffsetDateTime,
    /// Finished before the bind under another owner.
    pub before: Usage,
    /// Running or unfinished at the bind; its split across the bind is unknown.
    pub during: Usage,
    /// Recorded to the Task itself.
    pub after: Usage,
}

#[derive(Debug, Serialize)]
pub struct RuleUsage {
    pub name: String,
    pub prospective: Usage,
    pub post_hoc: Usage,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Usage {
    pub steps: usize,
    /// Steps whose provider reported no input or no output count.
    pub unmeasured_steps: usize,
    pub input_tokens: i64,
    pub output_tokens: i64,
    /// Sum of reported costs; absent when no step reported one.
    pub cost_usd: Option<f64>,
}

impl Usage {
    fn add(&mut self, step: &Step) {
        self.steps += 1;
        match step.tokens {
            Some((input, output)) => {
                self.input_tokens += input;
                self.output_tokens += output;
            }
            None => self.unmeasured_steps += 1,
        }
        if let Some(cost) = step.cost_usd {
            *self.cost_usd.get_or_insert(0.0) += cost;
        }
    }
}

/// One captured input reduced to what the comparison measures.
#[derive(Debug)]
struct Step {
    session_id: String,
    task: Option<String>,
    wave: Option<String>,
    ended_at: Option<i64>,
    tokens: Option<(i64, i64)>,
    cost_usd: Option<f64>,
}

pub(crate) fn run(json: bool) -> Result<()> {
    let (steps, binds) = read()?;
    let report = build_report(&steps, &binds);
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print!("{}", report_text(&report));
    }
    Ok(())
}

fn read() -> Result<(Vec<Step>, Vec<SessionBind>)> {
    let database = crate::store::database_path_from_env()?;
    if !database.exists() {
        return Ok((Vec::new(), Vec::new()));
    }
    let store = crate::store::sqlite::SqliteStore::open_execs_read_only(&database)?;
    let steps = store
        .conversation_history(None, None, None, None, 0, false)?
        .iter()
        .map(step)
        .collect();
    Ok((steps, store.bound_sessions()?))
}

fn step(session: &SessionHistory) -> Step {
    Step {
        session_id: session.session_id.clone(),
        task: session.task_identifier.clone(),
        wave: session.wave_name.clone(),
        ended_at: session.recorded_at.or_else(|| {
            session
                .providers
                .iter()
                .map(|turn| turn.completed_at)
                .collect::<Option<Vec<_>>>()
                .and_then(|ended| ended.into_iter().max())
        }),
        tokens: session
            .usage
            .total_input_tokens
            .or(session.usage.input_tokens)
            .zip(session.usage.output_tokens),
        cost_usd: session.usage.cost_usd,
    }
}

fn build_report(steps: &[Step], binds: &[SessionBind]) -> BindReport {
    let mut sessions = BTreeMap::<&str, Vec<&Step>>::new();
    for step in steps {
        sessions.entry(&step.session_id).or_default().push(step);
    }
    let mut tasks = BTreeMap::<&str, Vec<&Step>>::new();
    let mut waves = BTreeMap::<&str, Vec<&Step>>::new();
    let bound = binds
        .iter()
        .map(|bind| {
            let mut session = BoundSession {
                session_id: bind.session_id.clone(),
                task: bind.task.clone(),
                wave: bind.wave.clone(),
                bound_at: OffsetDateTime::from_unix_timestamp(bind.at)
                    .expect("bind times stay in the calendar range"),
                before: Usage::default(),
                during: Usage::default(),
                after: Usage::default(),
            };
            for step in sessions.get(bind.session_id.as_str()).into_iter().flatten() {
                if step.task.as_deref() == Some(&bind.task) {
                    session.after.add(step);
                    continue;
                }
                if step.ended_at.is_some_and(|ended| ended <= bind.at) {
                    session.before.add(step);
                } else {
                    session.during.add(step);
                }
                tasks.entry(&bind.task).or_default().push(step);
                if let Some(wave) = bind
                    .wave
                    .as_deref()
                    .filter(|wave| step.wave.as_deref() != Some(wave))
                {
                    waves.entry(wave).or_default().push(step);
                }
            }
            session
        })
        .collect();
    BindReport {
        sessions: sessions.len(),
        bound,
        by_task: compared(steps, tasks, |step| step.task.as_deref()),
        by_wave: compared(steps, waves, |step| step.wave.as_deref()),
    }
}

/// Each owner's recorded total beside the total with its bound Sessions' earlier
/// steps added. Largest move first.
fn compared<'a>(
    steps: &'a [Step],
    moved: BTreeMap<&str, Vec<&Step>>,
    owner: impl Fn(&'a Step) -> Option<&'a str>,
) -> Vec<RuleUsage> {
    let mut rows: Vec<RuleUsage> = moved
        .into_iter()
        .map(|(name, moved)| {
            let mut prospective = Usage::default();
            for step in steps.iter().filter(|step| owner(step) == Some(name)) {
                prospective.add(step);
            }
            let mut post_hoc = prospective.clone();
            for step in moved {
                post_hoc.add(step);
            }
            RuleUsage {
                name: name.to_string(),
                prospective,
                post_hoc,
            }
        })
        .collect();
    rows.sort_by_key(|row| {
        std::cmp::Reverse(row.post_hoc.input_tokens - row.prospective.input_tokens)
    });
    rows
}

fn report_text(report: &BindReport) -> String {
    let colors = Colors::default();
    let mut out = format!(
        "{}BIND ATTRIBUTION · {} of {} Sessions bound to a Task after starting{}\n",
        colors.bold,
        report.bound.len(),
        report.sessions,
        colors.reset
    );
    if report.bound.is_empty() {
        out.push_str("No bound Session: prospective and post-hoc totals are identical.\n");
        return out;
    }
    out.push_str(&format!(
        "\n{}{:<10}  {:<10}  {:<12}  {:>22}  {:>22}  {:>22}{}\n",
        colors.bold,
        "SESSION",
        "TASK",
        "BOUND",
        "BEFORE in/out",
        "DURING in/out",
        "AFTER in/out",
        colors.reset
    ));
    for session in &report.bound {
        out.push_str(&format!(
            "{:<10}  {:<10}  {:<12}  {:>22}  {:>22}  {:>22}\n",
            short_id(&session.session_id),
            truncate(&session.task, 10),
            session.bound_at.date().to_string(),
            tokens(&session.before),
            tokens(&session.during),
            tokens(&session.after),
        ));
    }
    for (title, rows) in [("TASK", &report.by_task), ("WAVE", &report.by_wave)] {
        if rows.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "\n{}{:<14}  {:>22}  {:>22}  {:>9}  {:>9}{}\n",
            colors.bold,
            title,
            "PROSPECTIVE in/out",
            "POST-HOC in/out",
            "COST",
            "POST-HOC",
            colors.reset
        ));
        for row in rows {
            out.push_str(&format!(
                "{:<14}  {:>22}  {:>22}  {:>9}  {:>9}\n",
                truncate(&row.name, 14),
                tokens(&row.prospective),
                tokens(&row.post_hoc),
                cost(&row.prospective),
                cost(&row.post_hoc),
            ));
        }
    }
    out.push_str("\nBEFORE and DURING move to the Task under post-hoc. DURING was running at the bind; prospective cannot split it. * marks steps with no reported usage.\n");
    out
}

fn tokens(usage: &Usage) -> String {
    let count = |value: i64| format_int(u64::try_from(value).unwrap_or(0));
    format!(
        "{}/{}{}",
        count(usage.input_tokens),
        count(usage.output_tokens),
        if usage.unmeasured_steps == 0 { "" } else { "*" }
    )
}

fn cost(usage: &Usage) -> String {
    usage
        .cost_usd
        .map(format_cost)
        .unwrap_or_else(|| "-".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(session: &str, task: Option<&str>, ended_at: Option<i64>, input: i64) -> Step {
        Step {
            session_id: session.to_string(),
            task: task.map(str::to_string),
            wave: task.map(|_| "intelligence".to_string()),
            ended_at,
            tokens: Some((input, 1)),
            cost_usd: None,
        }
    }

    fn bind(session: &str, at: i64, task: &str) -> SessionBind {
        SessionBind {
            session_id: session.to_string(),
            at,
            task: task.to_string(),
            wave: Some("intelligence".to_string()),
        }
    }

    #[test]
    fn earlier_usage_moves_to_the_task_only_under_post_hoc() {
        let steps = [
            step("late", None, Some(50), 1_000),
            step("late", None, Some(150), 200),
            step("late", None, None, 30),
            step("late", Some("LOO-1"), Some(300), 4),
            step("worker", Some("LOO-1"), Some(10), 10_000),
            step("taskless", None, Some(10), 7),
        ];
        let report = build_report(&steps, &[bind("late", 100, "LOO-1")]);

        assert_eq!(report.sessions, 3);
        let session = &report.bound[0];
        assert_eq!(session.before.input_tokens, 1_000);
        assert_eq!(
            (session.during.steps, session.during.input_tokens),
            (2, 230)
        );
        assert_eq!(session.after.input_tokens, 4);
        let task = &report.by_task[0];
        assert_eq!(task.name, "LOO-1");
        assert_eq!(task.prospective.input_tokens, 10_004);
        assert_eq!(task.post_hoc.input_tokens, 11_234);
        assert_eq!(report.by_wave[0].post_hoc.input_tokens, 11_234);
    }

    #[test]
    fn a_session_bound_before_any_work_changes_no_total() {
        let steps = [step("early", Some("LOO-2"), Some(300), 5)];
        let report = build_report(&steps, &[bind("early", 100, "LOO-2")]);

        assert_eq!(report.bound[0].after.input_tokens, 5);
        assert_eq!(report.bound[0].before.steps, 0);
        assert!(report.by_task.is_empty());
        assert!(report.by_wave.is_empty());
        assert!(report_text(&build_report(&steps, &[])).contains("identical"));
    }
}
