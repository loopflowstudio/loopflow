//! Linear's half of the tracker queue. Each action, in store words, becomes
//! the GraphQL request that carries it, and a look reads its owner back. This
//! is the only place an action turns into Linear input (`teamId`, `stateId`,
//! `assigneeId`, sort keys).

use std::path::Path;

use serde_json::{json, Value};

use super::{LinearClient, OrderedIssue, PlanningResponse};
use crate::ops::tracker::{
    creation_id, ingest_task, message, now, observed_project, observed_task, opening_state,
    remote_project, remote_task, repository_scope, target_state, value, values, Sent,
};
use crate::ops::OpsResult;
use crate::store::Store;
use loopflow_model::durable::{ProjectId, TaskId};
use loopflow_model::id::WaveId;
use loopflow_storage::planning::rules::Observed;
use loopflow_storage::planning::{StateHistory, TrackerProject};
use loopflow_storage::sqlite::tracker_actions::{
    ActionKind, ActionOwner, ActionPhase, TaskLink, TrackerAction,
};
use loopflow_storage::work::edit::WorkValue;

impl LinearClient {
    /// A Project created under a Wave with no Initiative stays local until the
    /// Wave has one. A missing Project or Wave is not a block: the send reports it.
    pub(crate) async fn blocked_action(
        &self,
        store: &Store,
        repo: &Path,
        action: &TrackerAction,
    ) -> Option<String> {
        let (ActionOwner::Project(project), ActionKind::Create) = (&action.owner, action.kind)
        else {
            return None;
        };
        let (_, slug) = project_wave(store, project).await.ok()?;
        crate::ops::linear::read_initiative(repo, &slug)
            .is_none()
            .then(|| NO_INITIATIVE.to_string())
    }

    pub(crate) async fn send_action(
        &self,
        store: &Store,
        repo: &Path,
        action: &TrackerAction,
    ) -> Sent {
        let sent = match (&action.owner, action.kind) {
            (ActionOwner::Task(task), ActionKind::Create) => {
                self.create_task(store, repo, task, action).await
            }
            (ActionOwner::Project(project), ActionKind::Create) => {
                self.create_project(store, repo, project, action).await
            }
            (ActionOwner::Task(task), ActionKind::Field) => {
                self.task_field(store, repo, task, action).await
            }
            (ActionOwner::Project(project), ActionKind::Field) => {
                self.project_field(store, repo, project, action).await
            }
            (ActionOwner::Task(task), ActionKind::State) => self.state(store, task, action).await,
            (ActionOwner::Task(task), ActionKind::Comment) => {
                self.task_comment(store, task, action).await
            }
            (ActionOwner::Task(task), ActionKind::Delete) => self.delete(store, task).await,
            (ActionOwner::Project(project), ActionKind::Order) => {
                self.order(store, project, action).await
            }
            (ActionOwner::Project(project), ActionKind::Link) => {
                self.link(store, repo, project).await
            }
            (ActionOwner::Task(task), ActionKind::Link) => {
                self.task_link(store, task, action).await
            }
            (owner, kind) => Err(message(format!(
                "Linear carries no {} action for {owner:?}",
                kind.as_str()
            ))),
        };
        // An error before the mutating request left the tracker untouched.
        sent.unwrap_or_else(|error| Sent::Rejected(error.to_string()))
    }

    pub(crate) async fn look_action(
        &self,
        store: &Store,
        repo: &Path,
        action: &TrackerAction,
    ) -> Result<Option<Observed>, String> {
        let observed = match (&action.owner, action.kind) {
            (ActionOwner::Task(task), ActionKind::Create) => {
                self.look_created_task(store, repo, task).await
            }
            (ActionOwner::Project(project), ActionKind::Create) => {
                self.look_created_project(store, repo, project).await
            }
            (ActionOwner::Task(task), ActionKind::Delete) => {
                self.look_deletion(store, repo, task, action).await
            }
            (ActionOwner::Task(task), ActionKind::Comment) => self.look_comments(store, task).await,
            (ActionOwner::Task(task), _) => self.look_task(store, repo, task, action).await,
            (ActionOwner::Project(project), ActionKind::Order) => {
                self.look_order(store, repo, project).await
            }
            (ActionOwner::Project(project), _) => self.look_project(store, repo, project).await,
            (ActionOwner::Wave(_), _) => Err(message("Linear Initiatives are not queued")),
        };
        observed.map_err(|error| error.to_string())
    }

    // ---- Sends ----

    async fn create_task(
        &self,
        store: &Store,
        repo: &Path,
        task: &TaskId,
        action: &TrackerAction,
    ) -> OpsResult<Sent> {
        let team = repository_scope(repo)?;
        let mut input = json!({"id": creation_id(task.as_str())?, "teamId": team});
        for value in values(action)? {
            match value {
                WorkValue::TaskTitle(title) => input["title"] = json!(title),
                WorkValue::TaskDescription(description) => {
                    input["description"] = json!(description)
                }
                WorkValue::TaskAssignee(assignee) => input["assigneeId"] = json!(assignee),
                WorkValue::TaskDueDate(due) => input["dueDate"] = json!(due),
                WorkValue::TaskProject(project) => {
                    input["projectId"] = json!(project_id(store, project.as_deref()).await?)
                }
                _ => {}
            }
        }
        let state = opening_state(store, task).await?;
        input["stateId"] = json!(self.team_state_id(&team, &state).await.map_err(message)?);
        let response =
            match crate::linear::urgent(self.deliver_planning_creation(false, input)).await {
                Ok(response) => response,
                Err(error) => return Ok(Sent::failed(error)),
            };
        self.accepted(store, repo, response).await
    }

    async fn create_project(
        &self,
        store: &Store,
        repo: &Path,
        project: &ProjectId,
        action: &TrackerAction,
    ) -> OpsResult<Sent> {
        let (wave, initiative) = project_scope(store, repo, project).await?;
        let team = repository_scope(repo)?;
        let id = creation_id(project.as_str())?;
        let mut input = json!({"id": id, "teamIds": [team], "useDefaultTemplate": false});
        let (mut description, mut krs, mut targets) = (String::new(), Vec::new(), Vec::new());
        for value in values(action)? {
            match value {
                WorkValue::ProjectName(name) => input["name"] = json!(name),
                WorkValue::ProjectSummary(summary) => input["description"] = json!(summary),
                WorkValue::ProjectDescription(text) => description = text.unwrap_or_default(),
                WorkValue::ProjectStatus(status) => {
                    input["statusId"] = self
                        .project_field_input(&WorkValue::ProjectStatus(status), "")
                        .await
                        .map_err(message)?["statusId"]
                        .clone()
                }
                WorkValue::Krs(values) => krs = values,
                WorkValue::MetricTargets(values) => targets = values,
                _ => {}
            }
        }
        input["content"] = json!(format!(
            "{description}\n{}",
            loopflow_storage::planning::render_project_content(&krs, &targets)
        ));
        let project = match crate::linear::urgent(self.deliver_planning_creation(true, input)).await
        {
            Ok(PlanningResponse::Project { project, .. }) => *project,
            Ok(PlanningResponse::Task { .. }) => {
                return Ok(Sent::Unknown("Project creation returned an issue".into()))
            }
            Err(error) => return Ok(Sent::failed(error)),
        };
        let project = match self.attached(&initiative, project).await {
            Ok(project) => project,
            Err(error) => return Ok(Sent::Unknown(error.to_string())),
        };
        accept_project(store, &wave, &initiative, &project)?;
        Ok(Sent::Acknowledged {
            revision: project.revision,
            remote_id: Some(id),
        })
    }

    /// A created Project joins its Wave's Initiative. The attachment carries an
    /// id derived from both, so repeating it after a lost reply is safe.
    async fn attached(
        &self,
        initiative: &str,
        project: TrackerProject,
    ) -> OpsResult<TrackerProject> {
        if !project.initiative_ids.is_empty() {
            return Ok(project);
        }
        let link = uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            format!("loopflow/project/{}/initiative/{initiative}", project.id).as_bytes(),
        )
        .to_string();
        self.deliver_project_attachment(&link, &project.id, initiative)
            .await
            .map_err(message)
    }

    async fn task_field(
        &self,
        store: &Store,
        repo: &Path,
        task: &TaskId,
        action: &TrackerAction,
    ) -> OpsResult<Sent> {
        let external = remote_task(store, task).await?;
        let input = match value(action)? {
            WorkValue::TaskTitle(title) => json!({"title": title}),
            WorkValue::TaskDescription(description) => json!({"description": description}),
            WorkValue::TaskAssignee(assignee) => json!({"assigneeId": assignee}),
            WorkValue::TaskProject(project) => {
                json!({"projectId": project_id(store, project.as_deref()).await?})
            }
            other => {
                return Err(message(format!(
                    "Linear has no Task field {}",
                    other.field()
                )))
            }
        };
        let response = match self.deliver_planning_field(&external, false, input).await {
            Ok(response) => response,
            Err(error) => return Ok(Sent::failed(error)),
        };
        self.accepted(store, repo, response).await
    }

    async fn project_field(
        &self,
        store: &Store,
        repo: &Path,
        project: &ProjectId,
        action: &TrackerAction,
    ) -> OpsResult<Sent> {
        let external = remote_project(store, project).await?;
        // Prose, KRs and targets share the Project's one content document.
        let (_, content) = self
            .fetch_project_content(&external)
            .await
            .map_err(message)?;
        let input = self
            .project_field_input(&value(action)?, &content)
            .await
            .map_err(message)?;
        let observed = match self.deliver_planning_field(&external, true, input).await {
            Ok(PlanningResponse::Project { project, .. }) => *project,
            Ok(PlanningResponse::Task { .. }) => {
                return Ok(Sent::Unknown("Project update returned an issue".into()))
            }
            Err(error) => return Ok(Sent::failed(error)),
        };
        let (wave, initiative) = project_scope(store, repo, project).await?;
        accept_project(store, &wave, &initiative, &observed)?;
        Ok(Sent::Acknowledged {
            revision: observed.revision,
            remote_id: Some(external),
        })
    }

    // Linear has no expected-revision guard here: a reopening between the
    // look and this mutation can be overwritten.
    async fn state(&self, store: &Store, task: &TaskId, action: &TrackerAction) -> OpsResult<Sent> {
        let external = remote_task(store, task).await?;
        let target = target_state(action)?;
        let state = self
            .item_state_id(&external, &target)
            .await
            .map_err(message)?;
        match self.set_item_state(&external, &state).await {
            Ok(item) => Ok(Sent::Acknowledged {
                revision: item.revision,
                remote_id: Some(item.id),
            }),
            Err(error) => Ok(Sent::failed(error)),
        }
    }

    /// Linear keeps the comment id we give it, so repeating a send is safe.
    async fn task_comment(
        &self,
        store: &Store,
        task: &TaskId,
        action: &TrackerAction,
    ) -> OpsResult<Sent> {
        let external = remote_task(store, task).await?;
        let id = action
            .subject
            .as_deref()
            .ok_or_else(|| message("comment action names no comment"))?;
        let body = action
            .comment_body()
            .ok_or_else(|| message("comment action holds no body"))?;
        match self.sync_comment(id, &external, body).await {
            Ok(()) => Ok(Sent::Acknowledged {
                revision: None,
                remote_id: Some(id.to_owned()),
            }),
            Err(error) => Ok(Sent::failed(error)),
        }
    }

    async fn delete(&self, store: &Store, task: &TaskId) -> OpsResult<Sent> {
        let external = remote_task(store, task).await?;
        match self.delete_issue(&external).await {
            Ok(()) => Ok(Sent::Acknowledged {
                revision: None,
                remote_id: Some(external),
            }),
            Err(error) => Ok(Sent::failed(error)),
        }
    }

    /// Move Tasks one at a time toward the saved order, each move against the
    /// list it read. A confirmed move becomes the action's base, so a reply
    /// lost later in the sequence is judged against the order it was sent from.
    async fn order(
        &self,
        store: &Store,
        project: &ProjectId,
        action: &TrackerAction,
    ) -> OpsResult<Sent> {
        let external = remote_project(store, project).await?;
        let desired: Vec<String> = serde_json::from_value(action.value.clone()).map_err(message)?;
        let mut issues = self.ordered_items(&external).await.map_err(message)?;
        loop {
            let mut observed = Vec::new();
            for issue in &issues {
                let task = store
                    .get_task_by_issue(&issue.item.id)
                    .await
                    .map_err(message)?
                    .ok_or_else(|| message("Linear order has an unmapped Task"))?;
                observed.push(task.id.to_string());
            }
            if desired.iter().any(|id| !observed.contains(id)) {
                return Err(message(
                    "Order awaits Task creation or confirmed membership; saved order retained",
                ));
            }
            let desired =
                loopflow_storage::sqlite::planning_order::include_members(&desired, &observed);
            let Some((issue, input, after)) = next_move(&issues, &observed, &desired)? else {
                return Ok(Sent::Acknowledged {
                    revision: None,
                    remote_id: Some(external),
                });
            };
            let response = match self.deliver_planning_field(&issue, false, input).await {
                Ok(response) => response,
                Err(error) => return Ok(Sent::failed(error)),
            };
            let PlanningResponse::Task { issue, .. } = response else {
                return Ok(Sent::Unknown("Order move returned a Project".into()));
            };
            store
                .sqlite
                .rebase_tracker_action(&action.id, &json!({"value": after}))
                .map_err(message)?;
            let Some(moved) = issues
                .iter_mut()
                .find(|candidate| candidate.item.id == issue.item.id)
            else {
                return Ok(Sent::Unknown("Order move returned an unknown issue".into()));
            };
            *moved = *issue;
            issues.sort_by(|a, b| {
                a.priority_sort_order
                    .total_cmp(&b.priority_sort_order)
                    .then(a.sort_order.total_cmp(&b.sort_order))
            });
        }
    }

    async fn link(&self, store: &Store, repo: &Path, project: &ProjectId) -> OpsResult<Sent> {
        let (wave, initiative) = project_scope(store, repo, project).await?;
        let external = remote_project(store, project).await?;
        let observed = self.project_ownership(&external).await.map_err(message)?;
        let observed = match self.attached(&initiative, observed).await {
            Ok(observed) => observed,
            Err(error) => return Ok(Sent::Unknown(error.to_string())),
        };
        accept_project(store, &wave, &initiative, &observed)?;
        Ok(Sent::Acknowledged {
            revision: observed.revision,
            remote_id: Some(external),
        })
    }

    async fn task_link(
        &self,
        store: &Store,
        task: &TaskId,
        action: &TrackerAction,
    ) -> OpsResult<Sent> {
        let issue = remote_task(store, task).await?;
        match action.task_link() {
            Some(TaskLink::Relation { id, target }) => {
                let related = remote_task(store, &target).await?;
                match self.ensure_follow_up_relation(&issue, &related, &id).await {
                    Ok(()) => Ok(Sent::Acknowledged {
                        revision: None,
                        remote_id: Some(id),
                    }),
                    Err(error) => Ok(Sent::failed(error)),
                }
            }
            // PR links spend the reserved budget so they never wait behind sync.
            Some(TaskLink::Pr { url }) => {
                let posted = store
                    .sqlite
                    .tracker_actions_for(&action.owner)
                    .map_err(message)?
                    .iter()
                    .any(|earlier| {
                        earlier.subject == action.subject
                            && matches!(
                                earlier.phase,
                                ActionPhase::Confirmed | ActionPhase::Conflicted
                            )
                    });
                Ok(super::urgent(self.pr_link(task, &issue, &url, action, posted)).await)
            }
            None => Err(message("link action names nothing to link")),
        }
    }

    /// The PR as an attachment, whose subtitle is its state, and one managed
    /// comment. Linear keeps one attachment per URL and the comment keeps one
    /// id per PR, so sending again refreshes both instead of adding more.
    async fn pr_link(
        &self,
        task: &TaskId,
        issue: &str,
        url: &str,
        action: &TrackerAction,
        posted: bool,
    ) -> Sent {
        let (number, state) = (&action.value["number"], &action.value["state"]);
        let state = state.as_str().unwrap_or("Open");
        let title = format!("GitHub PR #{number}");
        let body = format!("[GitHub PR #{number}]({url}) — {state}");
        // An earlier lf recorded the id Linear gave its comment.
        let recorded = action.value["comment_id"].as_str();
        let comment = match recorded {
            Some(id) => id.to_owned(),
            None => match pr_comment_id(task, url) {
                Ok(id) => id,
                Err(error) => return Sent::Rejected(error.to_string()),
            },
        };
        let attachment = match self.link_attachment(issue, url, &title).await {
            Ok(id) => id,
            Err(error) => return Sent::failed(error),
        };
        if let Err(error) = self.update_attachment(&attachment, &title, state).await {
            return Sent::failed(error);
        }
        let commented = if posted || recorded.is_some() {
            self.update_comment(&comment, &body).await
        } else {
            self.sync_comment(&comment, issue, &body).await
        };
        match commented {
            Ok(()) => Sent::Acknowledged {
                revision: None,
                remote_id: Some(attachment),
            },
            Err(error) => Sent::failed(error),
        }
    }

    /// A Task response is accepted into Work, which confirms a creation by its id.
    async fn accepted(
        &self,
        store: &Store,
        repo: &Path,
        response: PlanningResponse,
    ) -> OpsResult<Sent> {
        let PlanningResponse::Task { issue, project } = response else {
            return Ok(Sent::Unknown("Task request returned a Project".into()));
        };
        let item = issue.item;
        ingest_task(store, repo, &item, project.as_deref(), now()).await?;
        Ok(Sent::Acknowledged {
            revision: item.revision,
            remote_id: Some(item.id),
        })
    }

    // ---- Looks ----

    async fn look_created_task(
        &self,
        store: &Store,
        repo: &Path,
        task: &TaskId,
    ) -> OpsResult<Option<Observed>> {
        let Some((item, project)) = self
            .find_export_issue(&creation_id(task.as_str())?)
            .await
            .map_err(message)?
        else {
            return Ok(None);
        };
        ingest_task(store, repo, &item, project.as_ref(), now()).await?;
        Ok(Some(observed_task(store, &item).await))
    }

    async fn look_created_project(
        &self,
        store: &Store,
        repo: &Path,
        project: &ProjectId,
    ) -> OpsResult<Option<Observed>> {
        let Some(found) = self
            .find_project(&creation_id(project.as_str())?)
            .await
            .map_err(message)?
        else {
            return Ok(None);
        };
        let (wave, initiative) = project_scope(store, repo, project).await?;
        let found = self.attached(&initiative, found).await?;
        accept_project(store, &wave, &initiative, &found)?;
        Ok(Some(observed_project(&found)))
    }

    async fn look_task(
        &self,
        store: &Store,
        repo: &Path,
        task: &TaskId,
        action: &TrackerAction,
    ) -> OpsResult<Option<Observed>> {
        let external = remote_task(store, task).await?;
        let Some((item, project)) = self.issue_ownership(&external).await.map_err(message)? else {
            return Ok(None);
        };
        ingest_task(store, repo, &item, project.as_ref(), now()).await?;
        let mut observed = observed_task(store, &item).await;
        if action.kind == ActionKind::State {
            observed.state_history = Some(self.state_history(&external, action, &item).await);
        }
        Ok(Some(observed))
    }

    /// Whether the state moved since the action's base, when it reads as its
    /// base now: a write that landed and was undone looks the same as none.
    async fn state_history(
        &self,
        external: &str,
        action: &TrackerAction,
        item: &loopflow_storage::planning::TrackerItem,
    ) -> StateHistory {
        let base = action.base.as_ref();
        let revision = base.and_then(|base| base["revision"].as_str());
        let state = base.and_then(|base| base["value"]["value"]["planning_state"].as_str());
        let (Some(revision), Some(state)) = (revision, state) else {
            return StateHistory::Unknown;
        };
        if item.state.as_deref() != Some(state) {
            return StateHistory::Unknown;
        }
        if item.revision.as_deref() == Some(revision) {
            return StateHistory::Unchanged;
        }
        self.state_history_since(external, revision, state)
            .await
            .unwrap_or_else(|error| {
                tracing::debug!(%error, "Task state history unavailable");
                StateHistory::Unknown
            })
    }

    async fn look_deletion(
        &self,
        store: &Store,
        repo: &Path,
        task: &TaskId,
        action: &TrackerAction,
    ) -> OpsResult<Option<Observed>> {
        let external = remote_task(store, task).await?;
        match self.issue_trash(&external).await.map_err(message)? {
            None => Ok(None),
            Some((true, revision)) => Ok(Some(Observed {
                revision: Some(revision),
                deleted: true,
                ..Observed::default()
            })),
            Some((false, _)) => self.look_task(store, repo, task, action).await,
        }
    }

    async fn look_comments(&self, store: &Store, task: &TaskId) -> OpsResult<Option<Observed>> {
        let external = remote_task(store, task).await?;
        let event = self.fetch_issue(&external).await.map_err(message)?;
        Ok(Some(Observed {
            revision: Some(event.revision),
            comments: event
                .comments
                .into_iter()
                .map(|comment| comment.id)
                .collect(),
            ..Observed::default()
        }))
    }

    async fn look_project(
        &self,
        store: &Store,
        repo: &Path,
        project: &ProjectId,
    ) -> OpsResult<Option<Observed>> {
        let external = remote_project(store, project).await?;
        let (observed, _) = self
            .fetch_project_content(&external)
            .await
            .map_err(message)?;
        let (wave, initiative) = project_scope(store, repo, project).await?;
        accept_project(store, &wave, &initiative, &observed)?;
        Ok(Some(observed_project(&observed)))
    }

    /// The Project's complete order, as local Task ids; `order` stays unset
    /// when a listed issue has no Task here yet.
    async fn look_order(
        &self,
        store: &Store,
        repo: &Path,
        project: &ProjectId,
    ) -> OpsResult<Option<Observed>> {
        let external = remote_project(store, project).await?;
        let issues = self.ordered_items(&external).await.map_err(message)?;
        let items: Vec<_> = issues.iter().map(|issue| issue.item.clone()).collect();
        let complete = loopflow_storage::sqlite::linear_events::accept_linear_project_order_in(
            &store.sqlite,
            &repo.to_string_lossy(),
            &external,
            &items,
        )
        .map_err(message)?;
        let mut order = Vec::new();
        for item in &items {
            match store.get_task_by_issue(&item.id).await.map_err(message)? {
                Some(task) => order.push(task.id.to_string()),
                None => {
                    order.clear();
                    break;
                }
            }
        }
        Ok(Some(Observed {
            remote_id: Some(external),
            order: (complete && order.len() == items.len()).then_some(order),
            ..Observed::default()
        }))
    }
}

/// The id of a PR's managed comment: one per Task and PR URL.
fn pr_comment_id(task: &TaskId, url: &str) -> OpsResult<String> {
    let id = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_URL,
        format!("loopflow/task/{task}/pr/{url}").as_bytes(),
    );
    super::comment_delivery_id(&id.to_string()).map_err(message)
}

/// A Task's Project as Linear knows it.
async fn project_id(store: &Store, project: Option<&str>) -> OpsResult<Option<String>> {
    let Some(project) = project else {
        return Ok(None);
    };
    let project = ProjectId::parse(project).map_err(message)?;
    Ok(Some(remote_project(store, &project).await?))
}

const NO_INITIATIVE: &str = "Wave has no Linear Initiative; its Projects stay local";

/// The Wave a Project belongs to and that Wave's Initiative.
async fn project_scope(
    store: &Store,
    repo: &Path,
    project: &ProjectId,
) -> OpsResult<(WaveId, String)> {
    let (wave, slug) = project_wave(store, project).await?;
    let initiative =
        crate::ops::linear::read_initiative(repo, &slug).ok_or_else(|| message(NO_INITIATIVE))?;
    Ok((wave, initiative))
}

/// The Wave a Project belongs to, with its slug.
async fn project_wave(store: &Store, project: &ProjectId) -> OpsResult<(WaveId, String)> {
    let project = store
        .get_project(project)
        .await
        .map_err(message)?
        .ok_or_else(|| message("Project is missing"))?;
    let wave = store
        .get_wave(&project.wave_id)
        .await
        .map_err(message)?
        .ok_or_else(|| message("Project Wave is missing"))?;
    Ok((project.wave_id, wave.slug().to_owned()))
}

fn accept_project(
    store: &Store,
    wave: &WaveId,
    initiative: &str,
    project: &TrackerProject,
) -> OpsResult<()> {
    loopflow_storage::sqlite::linear_events::accept_linear_project_in(
        &store.sqlite,
        wave,
        initiative,
        project,
        now(),
        loopflow_storage::planning_sync::TrackerKind::Linear,
    )
    .map_err(message)
}

fn between(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    let value = match (left, right) {
        (Some(left), Some(right)) => left / 2.0 + right / 2.0,
        (Some(left), None) => {
            if left + 1.0 > left {
                left + 1.0
            } else {
                left.next_up()
            }
        }
        (None, Some(right)) => {
            if right - 1.0 < right {
                right - 1.0
            } else {
                right.next_down()
            }
        }
        (None, None) => 0.0,
    };
    (value.is_finite() && left.is_none_or(|l| l < value) && right.is_none_or(|r| value < r))
        .then_some(value)
}

fn move_input(issues: &[&OrderedIssue], position: usize) -> Option<Value> {
    let left = position.checked_sub(1).map(|p| issues[p]);
    let right = issues.get(position).copied();
    // Preserve the primary group when both neighbours share it; otherwise
    // choose a primary interval and retain a secondary position within it.
    let primary = match (left, right) {
        (Some(l), Some(r)) if l.priority_sort_order != r.priority_sort_order => {
            return between(Some(l.priority_sort_order), Some(r.priority_sort_order))
                .map(|p| json!({"prioritySortOrder":p}))
                .or_else(|| {
                    between(Some(l.sort_order), None)
                        .map(|s| json!({"prioritySortOrder":l.priority_sort_order,"sortOrder":s}))
                });
        }
        (Some(l), _) => l.priority_sort_order,
        (_, Some(r)) => r.priority_sort_order,
        _ => return None,
    };
    between(left.map(|i| i.sort_order), right.map(|i| i.sort_order))
        .map(|s| json!({"prioritySortOrder":primary,"sortOrder":s}))
}

/// The next single move toward `desired`: the issue, its sort-key input and
/// the order it leaves.
fn next_move(
    issues: &[OrderedIssue],
    observed: &[String],
    desired: &[String],
) -> OpsResult<Option<(String, Value, Vec<String>)>> {
    let Some(position) = observed.iter().zip(desired).position(|(a, b)| a != b) else {
        return Ok(None);
    };
    let source = observed
        .iter()
        .position(|id| id == &desired[position])
        .expect("desired members were matched to the observed Project");
    let mut remaining = issues.iter().collect::<Vec<_>>();
    remaining.remove(source);
    let (source, position, input) = if let Some(input) = move_input(&remaining, position) {
        (source, position, input)
    } else {
        // Equal keys have no insertion interval. Move the blocking neighbour
        // past this tied run, then retry the saved target on the next read.
        let blocking = &issues[position];
        if position == 0
            || issues[position - 1].priority_sort_order != blocking.priority_sort_order
            || issues[position - 1].sort_order != blocking.sort_order
        {
            return Err(message(
                "Linear sort keys have no representable interval; order remains pending",
            ));
        }
        let end = issues
            .iter()
            .rposition(|i| {
                i.priority_sort_order == blocking.priority_sort_order
                    && i.sort_order == blocking.sort_order
            })
            .expect("blocking issue is in its own tied run");
        remaining = issues.iter().collect();
        remaining.remove(position);
        let input = move_input(&remaining, end).ok_or_else(|| {
            message("Linear sort keys have no representable interval; order remains pending")
        })?;
        (position, end, input)
    };
    let mut after = observed.to_vec();
    let moved = after.remove(source);
    after.insert(position, moved);
    Ok(Some((issues[source].item.id.clone(), input, after)))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{json, Value};

    use super::pr_comment_id;
    use crate::linear::test_server::{self, json_response};
    use crate::linear::LinearClient;
    use crate::ops::tracker::Sent;
    use crate::store::Store;
    use loopflow_model::durable::TaskId;
    use loopflow_storage::sqlite::tracker_actions::Resolution;

    const URL: &str = "https://github.com/acme/repo/pull/7";

    fn reply(data: Value) -> test_server::QueuedResponse {
        json_response(StatusCode::OK, json!({ "data": data }))
    }

    /// Queue the PR link in `state` and send it through a Linear stand-in that
    /// answers `comment`; returns what the tracker said and the request bodies.
    async fn send(
        store: &Store,
        task: &TaskId,
        state: &str,
        comment: Value,
    ) -> (Sent, Vec<String>) {
        let id = store
            .sqlite
            .enqueue_pr_link(task, URL, 7, state, None)
            .unwrap()
            .unwrap();
        let action = store.sqlite.tracker_action(&id).unwrap().unwrap();
        let (base, requests) = test_server::spawn(vec![
            reply(json!({"attachmentLinkURL": {"attachment": {"id": "att-1"}}})),
            reply(json!({"attachmentUpdate": {"attachment": {"id": "att-1"}}})),
            reply(comment),
        ])
        .await;
        let client = LinearClient::with_base_url("secret".into(), None, base);
        let sent = client
            .send_action(store, std::path::Path::new("/repo"), &action)
            .await;
        if let Sent::Acknowledged {
            revision,
            remote_id,
        } = &sent
        {
            store
                .sqlite
                .resolve_tracker_action(
                    &id,
                    &Resolution::Confirmed {
                        revision: revision.clone(),
                        remote_id: remote_id.clone(),
                    },
                )
                .unwrap();
        }
        let bodies = requests
            .lock()
            .await
            .iter()
            .map(|r| r.body.clone())
            .collect();
        (sent, bodies)
    }

    #[tokio::test]
    async fn a_pr_link_posts_one_comment_and_refreshes_it_on_merge() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = Store {
            sqlite: crate::store::opening::open_ephemeral(&path).unwrap(),
        };
        let task = TaskId::new();
        store
            .sqlite
            .create_task(&loopflow_storage::work::task::NewTask {
                repository_id: loopflow_storage::test_fixture::repository(&store.sqlite, "/repo"),
                id: task.clone(),
                project_id: None,
                title: "Title".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE tasks SET external_issue_id=?2 WHERE id=?1",
                [task.as_str(), &uuid::Uuid::new_v4().to_string()],
            )
            .unwrap();
        let comment = pr_comment_id(&task, URL).unwrap();

        let (sent, opened) = send(
            &store,
            &task,
            "Open",
            json!({"commentCreate": {"comment": {"id": comment}}}),
        )
        .await;
        assert_eq!(
            sent,
            Sent::Acknowledged {
                revision: None,
                remote_id: Some("att-1".into())
            }
        );
        // `subtitle` on attachmentLinkURL is the 400 that shipped in #1010.
        let link: Value = serde_json::from_str(&opened[0]).unwrap();
        assert!(link["variables"].get("subtitle").is_none());
        assert!(opened[2].contains("commentCreate") && opened[2].contains(&comment));

        let (_, merged) = send(
            &store,
            &task,
            "Merged",
            json!({"commentUpdate": {"comment": {"id": comment}}}),
        )
        .await;
        assert!(merged[1].contains("Merged"));
        assert!(merged[2].contains("commentUpdate") && merged[2].contains(&comment));
        assert!(!merged.iter().any(|body| body.contains("commentCreate")));
    }
}
