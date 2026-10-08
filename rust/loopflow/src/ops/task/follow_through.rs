use super::{block_on_task, owning_wave, task_error, task_store};
use crate::ops::{OpsError, OpsResult};
use crate::work::task::follow_through::{FollowThroughIntent, FollowThroughLink};
use crate::work::task::{PrPhase, Task};
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
            .ok_or_else(|| task_error("follow-through needs a placed Task"))?;
        if let Some(pr) = store.active_task_pr(&task.id).await.map_err(task_error)? {
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
                    let link = confirm_intent(repo, &task, intent).await?;
                    store
                        .sqlite
                        .link_follow_through(&task.id, &link)
                        .map_err(task_error)?;
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
            let project = crate::ops::project::current_project(&store, &wave)?;
            let (issue_id, title, existing) = if let Some(existing) = &options.existing {
                let client = crate::ops::pm::issue_client(repo).await?;
                let (item, _) = client
                    .issue_ownership(existing)
                    .await
                    .map_err(task_error)?
                    .ok_or_else(|| task_error("follow-up Task not found"))?;
                if item.id == task.plan.id.as_str() {
                    return Err(task_error("a Task cannot follow up itself"));
                }
                (item.id, item.name, true)
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
                (uuid::Uuid::new_v4().to_string(), title, false)
            };
            if let Some(due) = &options.due {
                let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
                    .map_err(task_error)?;
                time::Date::parse(due, &format).map_err(task_error)?;
            }
            let pr = store.active_task_pr(&task.id).await.map_err(task_error)?;
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
            let context = crate::ops::pm::resolve_context(repo, wave_name).await?;
            let (team_id, state_id) = context
                .client
                .follow_up_creation_state()
                .await
                .map_err(task_error)?;
            let candidate = FollowThroughIntent {
                key: key.into(),
                issue_id,
                relation_id: uuid::Uuid::new_v4().to_string(),
                project_id: project.id,
                team_id,
                state_id,
                wave: wave_name.into(),
                title,
                notes,
                due: options.due.clone(),
                existing,
            };
            store
                .sqlite
                .reserve_follow_through(&task.id, &candidate)
                .map_err(task_error)?
        };
        let link = confirm_intent(repo, &task, &intent).await?;
        store
            .sqlite
            .link_follow_through(&task.id, &link)
            .map_err(task_error)?;
        Ok(format!("Follow-up {} linked. Record the disposition with --finish REASON. Due checks return on the next Wave pass; filing schedules no automatic check.", link.identifier))
    })
}

async fn confirm_intent(
    repo: &Path,
    task: &Task,
    intent: &FollowThroughIntent,
) -> OpsResult<FollowThroughLink> {
    let ctx = crate::ops::pm::resolve_context(repo, &intent.wave).await?;
    let item = ctx
        .client
        .confirm_follow_up(intent)
        .await
        .map_err(|error| {
            OpsError::Message(format!(
                "Follow-up {} remains pending: {error}; retry the same --key {}",
                intent.issue_id, intent.key
            ))
        })?;
    ctx.client
        .ensure_follow_up_relation(task.plan.id.as_str(), &intent.issue_id, &intent.relation_id)
        .await
        .map_err(task_error)?;
    Ok(FollowThroughLink {
        key: intent.key.clone(),
        issue_id: item.id,
        identifier: item.identifier,
        url: item.url,
        due: item.due_date,
    })
}

#[cfg(test)]
mod tests;
