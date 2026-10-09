use super::{acquire_task, block_on_task, owning_wave, task_error, task_store};
use crate::ops::OpsResult;
use crate::store::Store;
use crate::work::task::follow_through::{FollowThroughIntent, FollowThroughLink};
use crate::work::task::{PrPhase, TaskId};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct FollowUpOptions {
    pub title: Option<String>,
    pub notes: Option<String>,
    pub due: Option<String>,
    pub wave: Option<String>,
    pub existing: Option<String>,
    pub none: Option<String>,
    pub finish: Option<String>,
    pub key: Option<String>,
}

pub fn task_follow_up(repo: &Path, issue: &str, options: &FollowUpOptions) -> OpsResult<String> {
    block_on_task(async {
        let store = task_store().await?;
        let task = store
            .get_task_by_issue(issue)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("follow-through needs a saved Task"))?;
        let pr = store.active_task_pr(&task.id).await.map_err(task_error)?;
        if let Some(pr) = &pr {
            if pr.phase() != PrPhase::Merged {
                return Err(task_error(
                    "File follow-through after the pull request merges",
                ));
            }
        }
        let prior = store
            .sqlite
            .task_follow_through(&task.id)
            .map_err(task_error)?;
        if let Some(reason) = options.none.as_ref().or(options.finish.as_ref()) {
            if options.finish.is_some() {
                for intent in &prior.intents {
                    link_intent(&store, repo, &task.id, intent).await?;
                }
            }
            store
                .sqlite
                .finish_follow_through(&task.id, reason, options.none.is_some())
                .map_err(task_error)?;
            return Ok(format!(
                "Follow-through recorded; complete with `lf task complete {issue}`"
            ));
        }
        let key = options.key.as_deref().unwrap_or("follow-up");
        if key.trim().is_empty() {
            return Err(task_error("follow-up key cannot be empty"));
        }
        let intent = if let Some(intent) = prior.intents.iter().find(|intent| intent.key == key) {
            intent.clone()
        } else {
            let source_wave = owning_wave(&store, &task).await?;
            let wave_name = options.wave.as_deref().unwrap_or(source_wave.slug());
            let locator =
                crate::work::wave::WaveLocator::discover(repo, wave_name).map_err(task_error)?;
            let wave = store
                .get_wave_at(&locator)
                .await
                .map_err(task_error)?
                .ok_or_else(|| task_error("destination Wave is not initialized"))?;
            let selected = crate::ops::project::current_project(&store, &wave)?;
            let project = store
                .get_project_by_project(&selected.id)
                .await
                .map_err(task_error)?
                .ok_or_else(|| task_error("follow-up destination Project is unavailable"))?;
            let (issue_id, title, existing) = if let Some(selector) = &options.existing {
                let saved = acquire_task(&store, repo, selector).await?;
                if saved.id == task.id {
                    return Err(task_error("a Task cannot follow up itself"));
                }
                (saved.id.to_string(), saved.plan.title, true)
            } else {
                let title = options
                    .title
                    .clone()
                    .filter(|title| !title.trim().is_empty())
                    .ok_or_else(|| {
                        task_error("provide --title and --notes, --existing, --none or --finish")
                    })?;
                if options
                    .notes
                    .as_deref()
                    .is_none_or(|notes| notes.trim().is_empty())
                {
                    return Err(task_error(
                        "a new follow-up needs --notes with the evidence condition",
                    ));
                }
                (crate::durable::TaskId::new().to_string(), title, false)
            };
            if let Some(due) = &options.due {
                let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
                    .map_err(task_error)?;
                time::Date::parse(due, &format).map_err(task_error)?;
            }
            let source = pr
                .as_ref()
                .and_then(|pr| pr.github())
                .map(|pr| pr.url.as_str())
                .unwrap_or("");
            let notes = format!(
                "Follow-up to {} {}\n\n{}",
                task.plan.identifier,
                source,
                options
                    .notes
                    .as_deref()
                    .unwrap_or("Linked accepted follow-through")
            );
            FollowThroughIntent {
                key: key.into(),
                issue_id,
                relation_id: uuid::Uuid::new_v4().to_string(),
                project_id: project.id.to_string(),
                team_id: String::new(),
                state_id: None,
                wave: wave_name.into(),
                title,
                notes,
                due: options.due.clone(),
                existing,
            }
        };
        let link = link_intent(&store, repo, &task.id, &intent).await?;
        Ok(format!("Follow-up {} linked. Record the disposition with --finish REASON. Due checks return on the next Wave pass; filing schedules no automatic check.", link.identifier))
    })
}

async fn link_intent(
    store: &Store,
    repo: &Path,
    task: &TaskId,
    intent: &FollowThroughIntent,
) -> OpsResult<FollowThroughLink> {
    let intent = store
        .sqlite
        .reserve_follow_through(task, intent)
        .map_err(task_error)?;
    // Filing reservations already retain their child locally. Acquisition remains
    // necessary only for historical links to an existing, not-yet-retained issue.
    let saved = acquire_task(store, repo, &intent.issue_id).await?;
    let item = super::task_planning_item(store, &saved)?;
    let link = FollowThroughLink {
        key: intent.key.clone(),
        issue_id: intent.issue_id.clone(),
        identifier: saved.plan.identifier,
        url: item.url,
        due: item.due_date,
    };
    store
        .sqlite
        .link_follow_through(task, &link)
        .map_err(task_error)?;
    Ok(link)
}

#[cfg(test)]
mod tests;
