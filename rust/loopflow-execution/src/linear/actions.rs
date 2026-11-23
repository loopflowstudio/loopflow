//! Linear's half of the tracker queue. Each action, in store words, becomes
//! the GraphQL request that carries it, and a look reads its owner back. This
//! is the only place an action turns into Linear input (`teamId`, `stateId`,
//! `assigneeId`, sort keys).

use std::path::Path;

use serde_json::{json, Value};

use super::{
    parse_project_document, LinearClient, LinearError, LinearResult, OrderedIssue, PlanningResponse,
};
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
                self.project_field(store, project, action).await
            }
            (ActionOwner::Task(task), ActionKind::State) => self.state(store, task, action).await,
            (ActionOwner::Task(task), ActionKind::Comment) => {
                self.task_comment(store, task, action).await
            }
            (ActionOwner::Task(task), ActionKind::Delete) => self.delete(store, task).await,
            (ActionOwner::Project(project), ActionKind::Order) => {
                self.order(store, project, action).await
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
                self.look_created_project(store, project).await
            }
            (ActionOwner::Task(task), ActionKind::Delete) => {
                self.look_deletion(store, repo, task, action).await
            }
            (ActionOwner::Task(task), ActionKind::Comment) => self.look_comments(store, task).await,
            (ActionOwner::Task(task), _) => self.look_task(store, repo, task, action).await,
            (ActionOwner::Project(project), ActionKind::Order) => {
                self.look_order(store, repo, project).await
            }
            (ActionOwner::Project(project), _) => self.look_project(store, project).await,
            (ActionOwner::Wave(_), _) => Err(message("Linear carries no Wave actions")),
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
        let wave = project_wave(store, project).await?;
        let team = repository_scope(repo)?;
        let id = creation_id(project.as_str())?;
        let mut input =
            json!({"id": id, "teamIds": [team], "useDefaultTemplate": false, "content": ""});

        for value in values(action)? {
            match value {
                WorkValue::ProjectName(name) => input["name"] = json!(name),
                WorkValue::ProjectSummary(summary) => input["description"] = json!(summary),
                WorkValue::ProjectDescription(text) => {
                    input["content"] = json!(text.unwrap_or_default())
                }
                WorkValue::ProjectStatus(status) => {
                    input["statusId"] =
                        json!(self.project_status_id(status).await.map_err(message)?)
                }
                _ => {}
            }
        }
        let project = match crate::linear::urgent(self.deliver_planning_creation(true, input)).await
        {
            Ok(PlanningResponse::Project { project, .. }) => *project,
            Ok(PlanningResponse::Task { .. }) => {
                return Ok(Sent::Unknown("Project creation returned an issue".into()))
            }
            Err(error) => return Ok(Sent::failed(error)),
        };
        accept_project(store, &wave, &project)?;
        Ok(Sent::Acknowledged {
            revision: project.revision,
            remote_id: Some(id),
        })
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
        project: &ProjectId,
        action: &TrackerAction,
    ) -> OpsResult<Sent> {
        let external = remote_project(store, project).await?;
        let input = self
            .project_update_input(&external, value(action)?)
            .await
            .map_err(message)?;
        let observed = match self.deliver_planning_field(&external, true, input).await {
            Ok(PlanningResponse::Project { project, .. }) => *project,
            Ok(PlanningResponse::Task { .. }) => {
                return Ok(Sent::Unknown("Project update returned an issue".into()))
            }
            Err(error) => return Ok(Sent::failed(error)),
        };
        accept_project(store, &project_wave(store, project).await?, &observed)?;
        Ok(Sent::Acknowledged {
            revision: observed.revision,
            remote_id: Some(external),
        })
    }

    async fn project_update_input(&self, external: &str, value: WorkValue) -> LinearResult<Value> {
        Ok(match value {
            WorkValue::ProjectName(name) => json!({"name": name}),
            WorkValue::ProjectSummary(summary) => json!({"description": summary}),
            WorkValue::ProjectStatus(status) => {
                json!({"statusId": self.project_status_id(status).await?})
            }
            WorkValue::ProjectDescription(Some(description)) => {
                // Released sections share Linear's content field with the prose.
                let node = self.project_node(external).await?;
                let document = parse_project_document(node.content.as_deref().unwrap_or_default())?;
                json!({"content": document.replace_description(&description)?})
            }
            other => {
                return Err(LinearError::Message(format!(
                    "unsupported Project field {}",
                    other.field()
                )))
            }
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
                let earlier = store
                    .sqlite
                    .tracker_actions_for(&action.owner)
                    .map_err(message)?;
                let earlier = earlier.iter().filter(|e| e.subject == action.subject);
                let posted = earlier.clone().any(|earlier| {
                    matches!(
                        earlier.phase,
                        ActionPhase::Confirmed | ActionPhase::Conflicted
                    )
                });
                // A confirmed link's remote id is the PR's attachment.
                let attachment = earlier
                    .filter(|earlier| earlier.phase == ActionPhase::Confirmed)
                    .filter_map(|earlier| earlier.remote_id.clone())
                    .next_back();
                let link = self.pr_link(task, &issue, &url, action, posted, attachment.as_deref());
                Ok(super::urgent(link).await)
            }
            None => Err(message("link action names nothing to link")),
        }
    }

    /// The PR as one attachment, whose subtitle is its state, and one managed
    /// comment. Both keep their ids, so a state change updates them in place.
    /// `posted`: an earlier send posted the comment; `attachment`: the one an
    /// earlier send confirmed.
    async fn pr_link(
        &self,
        task: &TaskId,
        issue: &str,
        url: &str,
        action: &TrackerAction,
        posted: bool,
        attachment: Option<&str>,
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
        let attachment = match self.attach(issue, url, &title, state, attachment).await {
            Ok(id) => id,
            Err(sent) => return sent,
        };
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

    /// The PR's attachment on `issue`, refreshed to `state`. A recorded
    /// attachment is updated in place; only a PR without one, or whose
    /// attachment is gone, is linked.
    async fn attach(
        &self,
        issue: &str,
        url: &str,
        title: &str,
        state: &str,
        recorded: Option<&str>,
    ) -> Result<String, Sent> {
        let attachment = match recorded {
            Some(id) => match self.update_attachment(id, title, state).await {
                Ok(()) => return Ok(id.to_owned()),
                Err(error @ LinearError::RateLimited { .. }) => return Err(Sent::failed(error)),
                // Whether the attachment is still there decides, not the error text.
                Err(error) => match self.issue_attachment(issue, url).await? {
                    Some(found) if found == id => return Err(Sent::failed(error)),
                    Some(found) => found,
                    None => self.link_pr(issue, url, title).await?,
                },
            },
            None => self.link_pr(issue, url, title).await?,
        };
        self.update_attachment(&attachment, title, state)
            .await
            .map_err(Sent::failed)?;
        Ok(attachment)
    }

    /// Link the PR. Linear refuses a URL already attached, so that answer
    /// finds the existing attachment on this issue instead.
    async fn link_pr(&self, issue: &str, url: &str, title: &str) -> Result<String, Sent> {
        match self.link_attachment(issue, url, title).await {
            Ok(id) => Ok(id),
            Err(LinearError::Message(text)) if text.starts_with(ALREADY_LINKED) => {
                // Linked to another issue: moving a PR between issues is not a link.
                self.issue_attachment(issue, url)
                    .await?
                    .ok_or(Sent::Unsupported(text))
            }
            Err(error) => Err(Sent::failed(error)),
        }
    }

    /// The id of `issue`'s attachment for `url`.
    async fn issue_attachment(&self, issue: &str, url: &str) -> Result<Option<String>, Sent> {
        Ok(self
            .item_attachments(issue)
            .await
            .map_err(Sent::failed)?
            .into_iter()
            .find(|attachment| attachment.url == url)
            .map(|attachment| attachment.id))
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
        project: &ProjectId,
    ) -> OpsResult<Option<Observed>> {
        let Some(found) = self
            .find_project(&creation_id(project.as_str())?)
            .await
            .map_err(message)?
        else {
            return Ok(None);
        };
        accept_project(store, &project_wave(store, project).await?, &found)?;
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
        project: &ProjectId,
    ) -> OpsResult<Option<Observed>> {
        let external = remote_project(store, project).await?;
        let observed = self.project_ownership(&external).await.map_err(message)?;
        accept_project(store, &project_wave(store, project).await?, &observed)?;
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

/// What Linear answers when `attachmentLinkURL` names a URL some issue
/// already has: `This URL has already been linked with LOO-966.`
const ALREADY_LINKED: &str = "This URL has already been linked with";

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

/// The Wave a Project belongs to.
async fn project_wave(store: &Store, project: &ProjectId) -> OpsResult<WaveId> {
    Ok(store
        .get_project(project)
        .await
        .map_err(message)?
        .ok_or_else(|| message("Project is missing"))?
        .wave_id)
}

fn accept_project(store: &Store, wave: &WaveId, project: &TrackerProject) -> OpsResult<()> {
    loopflow_storage::sqlite::linear_events::accept_linear_project_in(
        &store.sqlite,
        wave,
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
    use crate::linear::{request_log, LinearClient};
    use crate::ops::tracker::Sent;
    use crate::store::Store;
    use loopflow_model::durable::TaskId;
    use loopflow_storage::sqlite::tracker_actions::Resolution;

    const URL: &str = "https://github.com/acme/repo/pull/7";

    fn reply(data: Value) -> test_server::QueuedResponse {
        json_response(StatusCode::OK, json!({ "data": data }))
    }

    fn refusal(text: &str) -> test_server::QueuedResponse {
        json_response(StatusCode::OK, json!({ "errors": [{ "message": text }] }))
    }

    fn linked(id: &str) -> test_server::QueuedResponse {
        reply(json!({"attachmentLinkURL": {"attachment": {"id": id}}}))
    }

    fn updated(id: &str) -> test_server::QueuedResponse {
        reply(json!({"attachmentUpdate": {"attachment": {"id": id}}}))
    }

    fn attachments(nodes: Value) -> test_server::QueuedResponse {
        reply(json!({"issue": {"attachments": {
            "nodes": nodes,
            "pageInfo": {"hasNextPage": false, "endCursor": null},
        }}}))
    }

    const ALREADY: &str = "This URL has already been linked with LOO-1.";

    /// One Task with a Linear issue, in a fresh store.
    fn task_store(home: &tempfile::TempDir) -> (Store, TaskId) {
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
        (store, task)
    }

    /// Queue the PR link in `state` and send it through a Linear stand-in that
    /// answers `responses` in order; an acknowledged send is confirmed. Returns
    /// what the tracker said and the GraphQL operation of each request.
    async fn send(
        store: &Store,
        task: &TaskId,
        state: &str,
        responses: Vec<test_server::QueuedResponse>,
    ) -> (Sent, Vec<String>) {
        let id = store
            .sqlite
            .enqueue_pr_link(task, URL, 7, state, None)
            .unwrap()
            .unwrap();
        let action = store.sqlite.tracker_action(&id).unwrap().unwrap();
        let (base, requests) = test_server::spawn(responses).await;
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
    async fn project_updates_read_content_only_to_replace_observed_prose() {
        use loopflow_storage::work::edit::WorkValue;
        let id = uuid::Uuid::new_v4().to_string();
        let (base, _) = test_server::spawn(vec![reply(json!({"project": {
            "id": id, "name": "Plan", "status": {"type": "started"}, "description": null,
            "content": "Old prose.\nworkflow: code\n", "initiatives": {"nodes":[]}, "teams": {"nodes":[]}
        }}))]).await;
        let client = LinearClient::with_base_url("fixture".into(), None, base);
        assert!(client
            .project_update_input(&id, WorkValue::ProjectDescription(None))
            .await
            .is_err());
        assert_eq!(
            client
                .project_update_input(&id, WorkValue::ProjectSummary("Summary".into()))
                .await
                .unwrap(),
            json!({"description": "Summary"})
        );
        assert_eq!(
            client
                .project_update_input(
                    &id,
                    WorkValue::ProjectDescription(Some("New prose.".into()))
                )
                .await
                .unwrap(),
            json!({"content": "New prose.\n\nworkflow: code\n"})
        );
    }

    /// Each body's operation name and the attachment id it names, if any.
    fn operations(bodies: &[String]) -> Vec<String> {
        bodies
            .iter()
            .map(|body| {
                let body: Value = serde_json::from_str(body).unwrap();
                let (_, name) = request_log::operation(body["query"].as_str().unwrap());
                match body["variables"]["id"].as_str() {
                    Some(id) if name.contains("Attachment") && id.starts_with("att") => {
                        format!("{name} {id}")
                    }
                    _ => name,
                }
            })
            .collect()
    }

    fn acknowledged(attachment: &str) -> Sent {
        Sent::Acknowledged {
            revision: None,
            remote_id: Some(attachment.into()),
        }
    }

    #[tokio::test]
    async fn a_pr_links_once_and_updates_its_attachment_and_comment_on_merge() {
        let home = tempfile::tempdir().unwrap();
        let (store, task) = task_store(&home);
        let comment = pr_comment_id(&task, URL).unwrap();

        let (sent, opened) = send(
            &store,
            &task,
            "Open",
            vec![
                linked("att-1"),
                updated("att-1"),
                reply(json!({"commentCreate": {"comment": {"id": comment}}})),
            ],
        )
        .await;
        assert_eq!(sent, acknowledged("att-1"));
        // `subtitle` on attachmentLinkURL is the 400 that shipped in #1010.
        let link: Value = serde_json::from_str(&opened[0]).unwrap();
        assert!(link["variables"].get("subtitle").is_none());
        assert!(opened[2].contains("commentCreate") && opened[2].contains(&comment));

        let (sent, merged) = send(
            &store,
            &task,
            "Merged",
            vec![
                updated("att-1"),
                reply(json!({"commentUpdate": {"comment": {"id": comment}}})),
            ],
        )
        .await;
        assert_eq!(sent, acknowledged("att-1"));
        assert_eq!(
            operations(&merged),
            ["UpdateAttachment att-1", "UpdateComment"]
        );
        assert!(merged[0].contains("Merged"));
        assert!(merged[1].contains(&comment));
    }

    #[tokio::test]
    async fn a_pr_already_linked_to_its_issue_confirms_with_that_attachment() {
        let home = tempfile::tempdir().unwrap();
        let (store, task) = task_store(&home);
        let comment = pr_comment_id(&task, URL).unwrap();

        let (sent, bodies) = send(
            &store,
            &task,
            "Open",
            vec![
                refusal(ALREADY),
                attachments(json!([
                    {"id": "att-other", "url": "https://example.com"},
                    {"id": "att-9", "url": URL},
                ])),
                updated("att-9"),
                reply(json!({"commentCreate": {"comment": {"id": comment}}})),
            ],
        )
        .await;
        assert_eq!(sent, acknowledged("att-9"));
        assert_eq!(
            operations(&bodies),
            [
                "LinkAttachment",
                "IssueAttachments",
                "UpdateAttachment att-9",
                "SyncComment"
            ]
        );
    }

    #[tokio::test]
    async fn a_pr_linked_to_another_issue_retires_with_linears_answer() {
        let home = tempfile::tempdir().unwrap();
        let (store, task) = task_store(&home);

        let (sent, bodies) = send(
            &store,
            &task,
            "Open",
            vec![refusal(ALREADY), attachments(json!([]))],
        )
        .await;
        assert_eq!(sent, Sent::Unsupported(ALREADY.into()));
        assert_eq!(operations(&bodies), ["LinkAttachment", "IssueAttachments"]);
    }

    #[tokio::test]
    async fn a_removed_attachment_is_linked_again() {
        let home = tempfile::tempdir().unwrap();
        let (store, task) = task_store(&home);
        let comment = pr_comment_id(&task, URL).unwrap();
        send(
            &store,
            &task,
            "Open",
            vec![
                linked("att-1"),
                updated("att-1"),
                reply(json!({"commentCreate": {"comment": {"id": comment}}})),
            ],
        )
        .await;

        let (sent, bodies) = send(
            &store,
            &task,
            "Merged",
            vec![
                refusal("Entity not found: Attachment"),
                attachments(json!([])),
                linked("att-2"),
                updated("att-2"),
                reply(json!({"commentUpdate": {"comment": {"id": comment}}})),
            ],
        )
        .await;
        assert_eq!(sent, acknowledged("att-2"));
        assert_eq!(
            operations(&bodies),
            [
                "UpdateAttachment att-1",
                "IssueAttachments",
                "LinkAttachment",
                "UpdateAttachment att-2",
                "UpdateComment"
            ]
        );
    }
}
