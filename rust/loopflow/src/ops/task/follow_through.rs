use super::{block_on_task, owning_wave, task_error, task_store};
use crate::ops::OpsResult;
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
            let (issue_id, title, existing) = if let Some(selector) = &options.existing {
                let saved = match store
                    .get_task_by_issue(selector)
                    .await
                    .map_err(task_error)?
                {
                    Some(saved) => saved,
                    None => {
                        let acquired = crate::ops::task_pm::resolve_task_async(
                            repo,
                            selector,
                            crate::ops::pm::PmRefresh::Force,
                        )
                        .await?;
                        store
                            .get_task_by_issue(&acquired.item.id)
                            .await
                            .map_err(task_error)?
                            .ok_or_else(|| task_error("follow-up Task was not retained"))?
                    }
                };
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
            let candidate = FollowThroughIntent {
                key: key.into(),
                issue_id,
                relation_id: uuid::Uuid::new_v4().to_string(),
                project_id: project.id,
                team_id: String::new(),
                state_id: None,
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
    let store = task_store().await?;
    let saved = if let Some(saved) = store
        .get_task_by_issue(&intent.issue_id)
        .await
        .map_err(task_error)?
    {
        saved
    } else if intent.issue_id.starts_with("task_") && !intent.existing {
        let project = store
            .get_project_by_project(&intent.project_id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("follow-up destination Project is unavailable"))?;
        let wave = store
            .get_wave(&project.wave_id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("follow-up destination Wave is unavailable"))?;
        store
            .create_task(
                &crate::planning::NewTask {
                    id: crate::durable::TaskId::parse(&intent.issue_id).map_err(task_error)?,
                    project_id: project.id,
                    title: intent.title.clone(),
                    description: intent.notes.clone(),
                    due_date: intent.due.clone(),
                },
                crate::ops::pm::lock_wave_planning(&wave).await?,
            )
            .await
            .map_err(task_error)?
    } else {
        // Historical provider receipts retain their UUID and payload. Acquire an
        // already-created issue through the common owner; never issue another create.
        let acquired = crate::ops::task_pm::resolve_task_async(
            repo,
            &intent.issue_id,
            crate::ops::pm::PmRefresh::Force,
        )
        .await?;
        store
            .get_task_by_issue(&acquired.item.id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error("historical follow-up filing remains unconfirmed"))?
    };
    let item = super::task_planning_item(&store, &saved)?;
    if let (Some(source), Some(target)) = (&task.plan.linear_id, &saved.plan.linear_id) {
        let wave = owning_wave(&store, task).await?;
        if crate::ops::linear_observe::connected(wave.repo()) {
            // Local linkage is durable even while its optional provider relation is pending.
            let result = async {
                crate::ops::pm::issue_client(repo)
                    .await?
                    .ensure_follow_up_relation(
                        source.as_str(),
                        target.as_str(),
                        &intent.relation_id,
                    )
                    .await
                    .map_err(task_error)
            }
            .await;
            if let Err(error) = result {
                tracing::warn!(%error, "follow-up relation synchronization pending");
            }
        }
    }
    Ok(FollowThroughLink {
        key: intent.key.clone(),
        issue_id: intent.issue_id.clone(),
        identifier: saved.plan.identifier,
        url: item.url,
        due: item.due_date,
    })
}

#[cfg(test)]
mod tests;
