//! Asana's half of the tracker queue. A Task is an Asana task addressed by its
//! Loopflow uuid (`external:<uuid>`); a Wave Project is mirrored once, when it
//! is created. Project edits, order and Initiative links have no Asana form
//! and retire with a reason.

use std::path::Path;

use reqwest::Method;
use serde_json::{json, Value};

use super::{completed, rich_text, AsanaClient, Task, IDENTITY_LABEL};
use crate::ops::tracker::{
    creation_id, ingest_task, message, now, observed_task, remote_project, remote_task,
    repository_scope, visible_text, Sent,
};
use crate::ops::OpsResult;
use crate::store::Store;
use loopflow_model::durable::{ProjectId, TaskId};
use loopflow_storage::planning::rules::Observed;
use loopflow_storage::planning::TrackerProject;
use loopflow_storage::sqlite::tracker_effects::{EffectKind, EffectOwner, TaskLink, TrackerEffect};
use loopflow_storage::work::edit::WorkValue;

const PROJECT_EDITS: &str =
    "Asana mirrors a Project when it is created; later Project edits stay local";

impl AsanaClient {
    pub(crate) async fn send_effect(
        &self,
        store: &Store,
        repo: &Path,
        effect: &TrackerEffect,
    ) -> Sent {
        let sent = match (&effect.owner, effect.kind) {
            (EffectOwner::Task(task), EffectKind::Create) => {
                self.create_task(store, repo, task, effect).await
            }
            (EffectOwner::Project(project), EffectKind::Create) => {
                self.create_project(store, repo, project, effect).await
            }
            (EffectOwner::Task(task), EffectKind::Field) => {
                self.task_field(store, repo, task, effect).await
            }
            (EffectOwner::Task(task), EffectKind::State) => self.state(store, task, effect).await,
            (EffectOwner::Task(task), EffectKind::Comment) => {
                self.task_comment(store, task, effect).await
            }
            (EffectOwner::Task(task), EffectKind::Delete) => self.delete(store, task).await,
            (EffectOwner::Task(task), EffectKind::Link) => match effect.task_link() {
                Some(TaskLink::Pr { url }) => self.pr_link(store, task, &url, effect).await,
                _ => Ok(Sent::Unsupported(
                    "Asana has no follow-up relations; the follow-up stays a comment".into(),
                )),
            },
            (EffectOwner::Project(_), _) => Ok(Sent::Unsupported(PROJECT_EDITS.into())),
            (owner, kind) => Ok(Sent::Unsupported(format!(
                "Asana carries no {} effect for {owner:?}",
                kind.as_str()
            ))),
        };
        sent.unwrap_or_else(|error| Sent::Rejected(error.to_string()))
    }

    pub(crate) async fn look_effect(
        &self,
        store: &Store,
        repo: &Path,
        effect: &TrackerEffect,
    ) -> Result<Option<Observed>, String> {
        let observed = match (&effect.owner, effect.kind) {
            (EffectOwner::Task(task), EffectKind::Create) => {
                self.look_task(
                    store,
                    repo,
                    &creation_id(task.as_str()).map_err(|e| e.to_string())?,
                )
                .await
            }
            (EffectOwner::Task(task), EffectKind::Comment) => {
                self.look_comments(store, task, effect).await
            }
            (EffectOwner::Task(task), _) => match remote_task(store, task).await {
                Ok(external) => self.look_task(store, repo, &external).await,
                Err(error) => Err(error),
            },
            (EffectOwner::Project(project), EffectKind::Create) => {
                self.look_created_project(store, repo, project, effect)
                    .await
            }
            _ => Err(message(PROJECT_EDITS)),
        };
        observed.map_err(|error| error.to_string())
    }

    // ---- Sends ----

    async fn create_task(
        &self,
        store: &Store,
        repo: &Path,
        task: &TaskId,
        effect: &TrackerEffect,
    ) -> OpsResult<Sent> {
        let id = creation_id(task.as_str())?;
        let mut data = json!({"external": {"gid": id}});
        let mut projects = vec![self.project().await.map_err(message)?.to_string()];
        for value in values(effect)? {
            match value {
                WorkValue::TaskProject(Some(project)) => {
                    projects.push(self.mirrored(store, &project).await?)
                }
                value => fields(&mut data, &value),
            }
        }
        let state = store
            .get_task(task)
            .await
            .map_err(message)?
            .and_then(|task| task.state)
            .unwrap_or_else(|| "unstarted".into());
        data["completed"] = json!(completed(&state).map_err(message)?);
        data["projects"] = json!(projects);
        let created: Task = match self.send(Method::POST, "/tasks", Some(data)).await {
            Ok(created) => created,
            Err(error) => return Ok(Sent::failed(error.into())),
        };
        self.accepted(store, repo, &id, created).await
    }

    async fn create_project(
        &self,
        store: &Store,
        repo: &Path,
        project: &ProjectId,
        effect: &TrackerEffect,
    ) -> OpsResult<Sent> {
        let id = creation_id(project.as_str())?;
        let mirrored = mirrored_project(&id, &repository_scope(repo)?, None, effect)?;
        let footer = format!("{IDENTITY_LABEL} {id}");
        let summary = mirrored.summary.trim();
        let notes = if summary.is_empty() {
            footer
        } else {
            format!("{summary}\n\n{footer}")
        };
        let created: super::Project = match self
            .send(
                Method::POST,
                &format!("/teams/{}/projects", self.team),
                Some(json!({"name": mirrored.name, "html_notes": rich_text::to_html(&notes)})),
            )
            .await
        {
            Ok(created) => created,
            Err(error) => return Ok(Sent::failed(error.into())),
        };
        let mirrored = TrackerProject {
            revision: created.modified_at,
            ..mirrored
        };
        accept_mirror(store, project, &mirrored).await?;
        Ok(Sent::Acknowledged {
            revision: mirrored.revision,
            remote_id: Some(id),
        })
    }

    async fn task_field(
        &self,
        store: &Store,
        repo: &Path,
        task: &TaskId,
        effect: &TrackerEffect,
    ) -> OpsResult<Sent> {
        let external = remote_task(store, task).await?;
        let path = format!("/tasks/external:{external}");
        let updated: Result<Task, super::AsanaFailure> = match value(effect)? {
            WorkValue::TaskProject(project) => {
                let target = match project {
                    Some(project) => Some(self.mirrored(store, &project).await?),
                    None => None,
                };
                let current: Task = match self.send(Method::GET, &path, None).await {
                    Ok(current) => current,
                    Err(error) => {
                        return Ok(Sent::Rejected(super::LinearError::from(error).to_string()))
                    }
                };
                match self.set_project(&current, target).await {
                    Ok(()) => self.send(Method::GET, &path, None).await,
                    Err(error) => return Ok(Sent::failed(error)),
                }
            }
            value => {
                let mut data = json!({});
                fields(&mut data, &value);
                self.send(Method::PUT, &path, Some(data)).await
            }
        };
        match updated {
            Ok(updated) => self.accepted(store, repo, &external, updated).await,
            Err(error) => Ok(Sent::failed(error.into())),
        }
    }

    /// Asana has open and completed tasks; a canceled Task is closed there too.
    async fn state(&self, store: &Store, task: &TaskId, effect: &TrackerEffect) -> OpsResult<Sent> {
        let external = remote_task(store, task).await?;
        let WorkValue::Disposition(disposition) = value(effect)? else {
            return Err(message("state effect holds no Disposition"));
        };
        let target = disposition
            .planning_state
            .ok_or_else(|| message("state effect names no state"))?;
        match self.set_state(&external, &target).await {
            Ok(item) => Ok(Sent::Acknowledged {
                revision: item.revision,
                remote_id: Some(item.id),
            }),
            Err(error) => Ok(Sent::failed(error)),
        }
    }

    /// Asana stories carry no caller identity: a comment showing ours is ours.
    async fn task_comment(
        &self,
        store: &Store,
        task: &TaskId,
        effect: &TrackerEffect,
    ) -> OpsResult<Sent> {
        let external = remote_task(store, task).await?;
        let body = effect.value["value"]["body"]
            .as_str()
            .ok_or_else(|| message("comment effect holds no body"))?;
        match self
            .sync_comment(&external, &super::comment_markdown(&visible_text(body)))
            .await
        {
            Ok(()) => Ok(Sent::Acknowledged {
                revision: None,
                remote_id: effect.subject.clone(),
            }),
            Err(error) => Ok(Sent::failed(error)),
        }
    }

    /// One comment per PR state: Asana keeps no id of ours, so a comment
    /// with the same text already there is this one.
    async fn pr_link(
        &self,
        store: &Store,
        task: &TaskId,
        url: &str,
        effect: &TrackerEffect,
    ) -> OpsResult<Sent> {
        let external = remote_task(store, task).await?;
        let number = &effect.value["number"];
        let state = effect.value["state"].as_str().unwrap_or("Open");
        let body = format!("[GitHub PR #{number}]({url}) — {state}");
        match self
            .sync_comment(&external, &super::comment_markdown(&body))
            .await
        {
            Ok(()) => Ok(Sent::Acknowledged {
                revision: None,
                remote_id: None,
            }),
            Err(error) => Ok(Sent::failed(error)),
        }
    }

    async fn delete(&self, store: &Store, task: &TaskId) -> OpsResult<Sent> {
        let external = remote_task(store, task).await?;
        match self.delete_task(&external).await {
            Ok(()) => Ok(Sent::Acknowledged {
                revision: None,
                remote_id: Some(external),
            }),
            Err(error) => Ok(Sent::failed(error)),
        }
    }

    async fn accepted(&self, store: &Store, repo: &Path, id: &str, task: Task) -> OpsResult<Sent> {
        let item = self.issue_unchecked(id, task).await.map_err(message)?;
        ingest_task(store, repo, &item, None, now()).await?;
        Ok(Sent::Acknowledged {
            revision: item.revision,
            remote_id: Some(item.id),
        })
    }

    /// The Asana project gid mirroring Loopflow Project `project`.
    async fn mirrored(&self, store: &Store, project: &str) -> OpsResult<String> {
        let project = ProjectId::parse(project).map_err(message)?;
        let id = remote_project(store, &project).await?;
        self.wave_project(&id)
            .await
            .map_err(message)?
            .map(|project| project.gid)
            .ok_or_else(|| {
                message(format!(
                    "Asana team {} has no project for Loopflow Project {id}",
                    self.team
                ))
            })
    }

    // ---- Looks ----

    async fn look_task(
        &self,
        store: &Store,
        repo: &Path,
        external: &str,
    ) -> OpsResult<Option<Observed>> {
        let Some(item) = self.task(external).await.map_err(message)? else {
            return Ok(None);
        };
        ingest_task(store, repo, &item, None, now()).await?;
        Ok(Some(observed_task(store, &item).await))
    }

    async fn look_comments(
        &self,
        store: &Store,
        task: &TaskId,
        effect: &TrackerEffect,
    ) -> OpsResult<Option<Observed>> {
        let external = remote_task(store, task).await?;
        let claimed = effect.value["value"]["body"]
            .as_str()
            .map(|body| super::comment_markdown(&visible_text(body)));
        let observation = self.observe(&external).await.map_err(message)?;
        let mut comments = Vec::new();
        for comment in observation.comments {
            if Some(&comment.body) == claimed.as_ref() {
                comments.extend(effect.subject.clone());
            }
            comments.push(comment.body);
        }
        Ok(Some(Observed {
            revision: Some(observation.revision),
            comments,
            ..Observed::default()
        }))
    }

    async fn look_created_project(
        &self,
        store: &Store,
        repo: &Path,
        project: &ProjectId,
        effect: &TrackerEffect,
    ) -> OpsResult<Option<Observed>> {
        let id = creation_id(project.as_str())?;
        let Some(revision) = self.project_revision(&id).await.map_err(message)? else {
            return Ok(None);
        };
        let mirrored = mirrored_project(&id, &repository_scope(repo)?, revision, effect)?;
        accept_mirror(store, project, &mirrored).await?;
        Ok(Some(Observed {
            remote_id: Some(id),
            revision: mirrored.revision,
            ..Observed::default()
        }))
    }
}

fn value(effect: &TrackerEffect) -> OpsResult<WorkValue> {
    serde_json::from_value(effect.value.clone()).map_err(message)
}

fn values(effect: &TrackerEffect) -> OpsResult<Vec<WorkValue>> {
    serde_json::from_value(effect.value.clone()).map_err(message)
}

/// The Asana task field a Task value sets; Project membership is separate.
fn fields(data: &mut Value, value: &WorkValue) {
    match value {
        WorkValue::TaskTitle(title) => data["name"] = json!(title),
        WorkValue::TaskDescription(description) => {
            data["html_notes"] = json!(rich_text::to_html(
                description.as_deref().unwrap_or_default()
            ))
        }
        WorkValue::TaskAssignee(assignee) => data["assignee"] = json!(assignee),
        WorkValue::TaskDueDate(due) => data["due_on"] = json!(due),
        _ => {}
    }
}

/// A created Project as the Asana mirror holds it. Asana shows its name and
/// summary; status, KRs and targets stay the creation values Loopflow owns.
fn mirrored_project(
    id: &str,
    team: &str,
    revision: Option<String>,
    effect: &TrackerEffect,
) -> OpsResult<TrackerProject> {
    let mut project = TrackerProject {
        id: id.to_owned(),
        revision,
        slug: String::new(),
        name: String::new(),
        summary: String::new(),
        description: None,
        metric_targets: Vec::new(),
        status: loopflow_storage::work::project::ProjectStatus::Backlog,
        krs: Vec::new(),
        initiative_ids: vec![team.to_owned()],
        team_ids: vec![team.to_owned()],
    };
    for value in values(effect)? {
        match value {
            WorkValue::ProjectName(Some(name)) => {
                project.slug = loopflow_storage::planning::project_slug(&name);
                project.name = name;
            }
            WorkValue::ProjectSummary(summary) => project.summary = summary,
            WorkValue::ProjectDescription(description) => project.description = description,
            WorkValue::ProjectStatus(status) => project.status = status,
            WorkValue::Krs(krs) => project.krs = krs,
            WorkValue::MetricTargets(targets) => project.metric_targets = targets,
            _ => {}
        }
    }
    Ok(project)
}

/// Asana has no Initiative: a Wave's Projects belong to the repository's team.
async fn accept_mirror(
    store: &Store,
    project: &ProjectId,
    mirrored: &TrackerProject,
) -> OpsResult<()> {
    let wave = store
        .get_project(project)
        .await
        .map_err(message)?
        .ok_or_else(|| message("Project is missing"))?
        .wave_id;
    loopflow_storage::sqlite::linear_acceptance::accept_linear_project_in(
        &store.sqlite,
        &wave,
        &mirrored.initiative_ids[0],
        mirrored,
        now(),
        loopflow_storage::provider::Provider::Asana,
    )
    .map_err(message)
}
