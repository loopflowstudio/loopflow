//! Read retained Session inputs and observe provider conversations.

use anyhow::{anyhow, Result};

use crate::lf::commands::WorkFilter;
pub use loopflow_execution::session_record::SessionHistory;

const WINDOW_DAYS: i64 = 7;
const MAX_SESSIONS: usize = 50;

/// Shared Session history, newest input first. SQL selects the recent budget
/// before decoding; exact Task and caller-input drills remain complete.
pub(crate) fn collect_recent_history(
    store: &loopflow_storage::sqlite::SqliteStore,
    filter: WorkFilter,
) -> Result<(Vec<SessionHistory>, bool)> {
    let since = chrono::Utc::now().timestamp() - WINDOW_DAYS * 24 * 3600;
    Ok(
        loopflow_execution::store::sessions::recent_conversation_history(
            store,
            filter.wave,
            filter.project,
            filter.task,
            since,
            MAX_SESSIONS,
        )?,
    )
}

/// Select conversations in SQL before reading their subordinate history.
pub(crate) fn collect_history(
    filter: WorkFilter,
    parent: Option<&str>,
    since: i64,
) -> Result<Vec<SessionHistory>> {
    let database = loopflow_execution::store::database_path_from_env()?;
    if !database.exists() {
        return Ok(Vec::new());
    }
    let store = loopflow_storage::sqlite::SqliteStore::open_read_only(
        &database,
        loopflow_execution::store::token_key::encryption_key,
    )?;
    Ok(loopflow_execution::store::sessions::conversation_history(
        &store,
        filter.wave,
        filter.project,
        filter.task,
        parent,
        since,
        false,
    )?)
}

/// The part of a Session's evidence `lf history show` prints. Its flags
/// conflict pairwise, so a command asks for one at most.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionView {
    Summary,
    Final,
    Context,
    Instructions,
    Transcript,
}

pub fn inspect(selector: &str, view: SessionView, json: bool) -> Result<()> {
    print!("{}", render(selector, view, json)?);
    Ok(())
}

pub(crate) fn render(selector: &str, view: SessionView, json: bool) -> Result<String> {
    use std::fmt::Write;
    let mut output = String::new();
    let home = loopflow_execution::store::lf_home_dir();
    let database = loopflow_execution::store::database_path_from_env()?;
    let store = loopflow_storage::sqlite::SqliteStore::open_read_only(
        &database,
        loopflow_execution::store::token_key::encryption_key,
    )?;
    let snapshot = loopflow_execution::store::sessions::input_history(&store, selector)
        .map_err(|error| anyhow!("Session capture unavailable: {error}"))?;
    if view == SessionView::Transcript {
        let conversation = super::history_transcript::session(&store, &snapshot.session_id)?;
        if json {
            writeln!(&mut output, "{}", serde_json::to_string(&conversation)?)?;
        } else {
            write!(
                &mut output,
                "{}",
                super::history_transcript::render(&conversation)
            )?;
        }
        return Ok(output);
    }
    let input = loopflow_execution::session_record::parse_artifact_key(snapshot.selector())?;
    let dir = loopflow_execution::session_record::history_record_dir(&home, &snapshot)
        .ok_or_else(|| anyhow!("Input {} has no artifact path", snapshot.selector()))?;
    if view == SessionView::Final {
        let answer =
            loopflow_execution::transcript::answer::input_final_answer_for_people(&store, &input)
                .map_err(|error| anyhow!("Capture final answer unavailable: {error:#}"))?;
        return match answer {
            Some(answer) => {
                writeln!(&mut output, "{answer}")?;
                Ok(output)
            }
            None if snapshot.recorded_outcome.is_none() => Err(anyhow!(
                "Capture {} is not settled and has no final answer",
                snapshot.selector()
            )),
            None => Err(anyhow!(
                "Capture {} settled as {} without a final answer",
                snapshot.selector(),
                snapshot.status()
            )),
        };
    }
    if view == SessionView::Instructions {
        let decisions = loopflow_execution::context_usage::read_instructions(&dir)
            .map_err(|error| anyhow!("Instruction evidence unavailable: {error}"))?;
        if json {
            writeln!(&mut output, "{}", serde_json::to_string(&decisions)?)?;
        } else {
            writeln!(
                &mut output,
                "{}",
                loopflow_execution::context_usage::render_instructions(&decisions)
            )?;
        }
        return Ok(output);
    }
    if view == SessionView::Context {
        let step = loopflow_execution::context_usage::step_context(&home, &store, &snapshot);
        if json {
            writeln!(&mut output, "{}", serde_json::to_string(&step)?)?;
        } else {
            writeln!(
                &mut output,
                "{}",
                loopflow_execution::context_usage::render_step(&step)
            )?;
        }
        return Ok(output);
    }
    if json {
        writeln!(&mut output, "{}", serde_json::to_string(&snapshot)?)?;
        return Ok(output);
    }
    writeln!(
        &mut output,
        "Session {} · input {}",
        snapshot.session_id,
        snapshot.selector()
    )?;
    if let Some(parent) = &snapshot.caller_artifact_key {
        writeln!(&mut output, "Parent: {parent}")?;
    }
    writeln!(&mut output, "Status: {}", snapshot.status())?;
    writeln!(
        &mut output,
        "Agent: {}",
        snapshot
            .model
            .as_ref()
            .map(|model| format!("{}:{model}", snapshot.harness))
            .unwrap_or_else(|| snapshot.harness.clone())
    )?;
    writeln!(
        &mut output,
        "Working directory: {}",
        snapshot.worktree.as_deref().unwrap_or("unknown")
    )?;
    let manifest = loopflow_execution::session_record::read_manifest(&dir).ok();
    writeln!(
        &mut output,
        "Replay: {}",
        match manifest
            .as_ref()
            .and_then(|manifest| manifest.process.as_ref())
        {
            Some(_) => "available",
            None => "unavailable",
        }
    )?;
    writeln!(&mut output, "Evidence gaps: {}", snapshot.evidence_gaps)?;
    writeln!(
        &mut output,
        "{}",
        loopflow_execution::context_usage::render_step(
            &loopflow_execution::context_usage::step_context(&home, &store, &snapshot)
        )
    )?;
    for transition in store.provider_transitions(&snapshot.session_id)? {
        writeln!(
            &mut output,
            "Provider transition: {}",
            serde_json::to_string(&transition)?
        )?;
    }
    let conversation = super::history_transcript::session(&store, &snapshot.session_id)?;
    write!(
        &mut output,
        "\n{}",
        super::history_transcript::render(&conversation)
    )?;
    Ok(output)
}

pub(crate) fn format_tokens(value: i64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}k", value as f64 / 1_000.0)
    } else if value > 0 {
        value.to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::format_tokens;

    #[test]
    fn tokens_keep_the_compact_human_format() {
        assert_eq!(format_tokens(184_000), "184.0k");
        assert_eq!(format_tokens(0), "");
    }
}
