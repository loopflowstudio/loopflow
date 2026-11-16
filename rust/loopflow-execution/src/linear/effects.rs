//! Linear's half of the tracker queue. Each effect, in store words, becomes
//! the GraphQL request that carries it, and a look reads its owner back. This
//! is the only place an effect turns into Linear input (`teamId`, `stateId`,
//! `assigneeId`, sort keys).

use std::path::Path;

use serde_json::{json, Value};

use super::{LinearClient, OrderedIssue, PlanningResponse};
use crate::ops::tracker::{
    creation_id, ingest_task, message, now, observed_project, observed_task, remote_project,
    remote_task, repository_scope, Sent,
};
use crate::ops::OpsResult;
use crate::store::Store;
use loopflow_model::durable::{ProjectId, TaskId};
use loopflow_model::id::WaveId;
use loopflow_storage::planning::rules::Observed;
use loopflow_storage::planning::{StateHistory, TrackerProject};
use loopflow_storage::sqlite::tracker_effects::{EffectKind, EffectOwner, TrackerEffect};
use loopflow_storage::work::edit::WorkValue;

impl LinearClient {
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
            (EffectOwner::Project(project), EffectKind::Field) => {
                self.project_field(store, repo, project, effect).await
            }
            (EffectOwner::Task(task), EffectKind::State) => self.state(store, task, effect).await,
            (EffectOwner::Task(task), EffectKind::Comment) => {
                self.task_comment(store, task, effect).await
            }
            (EffectOwner::Task(task), EffectKind::Delete) => self.delete(store, task).await,
            (EffectOwner::Project(project), EffectKind::Order) => {
                self.order(store, project, effect).await
            }
            (EffectOwner::Project(project), EffectKind::Link) => {
                self.link(store, repo, project).await
            }
            (owner, kind) => Err(message(format!(
                "Linear carries no {} effect for {owner:?}",
                kind.as_str()
            ))),
        };
        // An error before the mutating request left the provider untouched.
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
                self.look_created_task(store, repo, task).await
            }
            (EffectOwner::Project(project), EffectKind::Create) => {
                self.look_created_project(store, repo, project).await
            }
            (EffectOwner::Task(task), EffectKind::Delete) => {
                self.look_deletion(store, repo, task).await
            }
            (EffectOwner::Task(task), EffectKind::Comment) => self.look_comments(store, task).await,
            (EffectOwner::Task(task), _) => self.look_task(store, repo, task, effect).await,
            (EffectOwner::Project(project), EffectKind::Order) => {
                self.look_order(store, repo, project).await
            }
            (EffectOwner::Project(project), _) => self.look_project(store, repo, project).await,
            (EffectOwner::Wave(_), _) => Err(message("Linear Initiatives are not queued")),
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
        let team = repository_scope(repo)?;
        let mut input = json!({"id": creation_id(task.as_str())?, "teamId": team});
        for value in creation_values(effect)? {
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
        let state = store
            .get_task(task)
            .await
            .map_err(message)?
            .and_then(|task| task.state)
            .unwrap_or_else(|| "unstarted".into());
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
        effect: &TrackerEffect,
    ) -> OpsResult<Sent> {
        let (wave, initiative) = project_scope(store, repo, project).await?;
        let team = repository_scope(repo)?;
        let id = creation_id(project.as_str())?;
        let mut input = json!({"id": id, "teamIds": [team], "useDefaultTemplate": false});
        let (mut description, mut krs, mut targets) = (String::new(), Vec::new(), Vec::new());
        for value in creation_values(effect)? {
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
        let project = match self.attached(&wave, &initiative, project).await {
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
        _wave: &WaveId,
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
        effect: &TrackerEffect,
    ) -> OpsResult<Sent> {
        let external = remote_task(store, task).await?;
        let input = match value(effect)? {
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
        effect: &TrackerEffect,
    ) -> OpsResult<Sent> {
        let external = remote_project(store, project).await?;
        // Prose, KRs and targets share the Project's one content document.
        let (_, content) = self
            .project_content_observation(&external)
            .await
            .map_err(message)?;
        let input = self
            .project_field_input(&value(effect)?, &content)
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
    async fn state(&self, store: &Store, task: &TaskId, effect: &TrackerEffect) -> OpsResult<Sent> {
        let external = remote_task(store, task).await?;
        let WorkValue::Disposition(disposition) = value(effect)? else {
            return Err(message("state effect holds no Disposition"));
        };
        let target = disposition
            .planning_state
            .ok_or_else(|| message("state effect names no state"))?;
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
        effect: &TrackerEffect,
    ) -> OpsResult<Sent> {
        let external = remote_task(store, task).await?;
        let id = effect
            .subject
            .as_deref()
            .ok_or_else(|| message("comment effect names no comment"))?;
        let body = effect.value["value"]["body"]
            .as_str()
            .ok_or_else(|| message("comment effect holds no body"))?;
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
    /// list it read. A confirmed move becomes the effect's base, so a reply
    /// lost later in the sequence is judged against the order it was sent from.
    async fn order(
        &self,
        store: &Store,
        project: &ProjectId,
        effect: &TrackerEffect,
    ) -> OpsResult<Sent> {
        let external = remote_project(store, project).await?;
        let desired: Vec<String> = serde_json::from_value(effect.value.clone()).map_err(message)?;
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
                .rebase_tracker_effect(&effect.id, &json!({"value": after}))
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
        let observed = match self.attached(&wave, &initiative, observed).await {
            Ok(observed) => observed,
            Err(error) => return Ok(Sent::Unknown(error.to_string())),
        };
        accept_project(store, &wave, &initiative, &observed)?;
        Ok(Sent::Acknowledged {
            revision: observed.revision,
            remote_id: Some(external),
        })
    }

    /// A Task response is accepted into Work, which settles a creation by its id.
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
        let found = self.attached(&wave, &initiative, found).await?;
        accept_project(store, &wave, &initiative, &found)?;
        Ok(Some(observed_project(&found)))
    }

    async fn look_task(
        &self,
        store: &Store,
        repo: &Path,
        task: &TaskId,
        effect: &TrackerEffect,
    ) -> OpsResult<Option<Observed>> {
        let external = remote_task(store, task).await?;
        let Some((item, project)) = self.issue_ownership(&external).await.map_err(message)? else {
            return Ok(None);
        };
        ingest_task(store, repo, &item, project.as_ref(), now()).await?;
        let mut observed = observed_task(store, &item).await;
        if effect.kind == EffectKind::State {
            observed.state_history = Some(self.state_history(&external, effect, &item).await);
        }
        Ok(Some(observed))
    }

    /// Whether the state moved since the effect's base, when it reads as its
    /// base now: a write that landed and was undone looks the same as none.
    async fn state_history(
        &self,
        external: &str,
        effect: &TrackerEffect,
        item: &loopflow_storage::planning::TrackerItem,
    ) -> StateHistory {
        let base = effect.base.as_ref();
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
    ) -> OpsResult<Option<Observed>> {
        let external = remote_task(store, task).await?;
        match self.issue_trash(&external).await.map_err(message)? {
            None => Ok(None),
            Some((true, revision)) => Ok(Some(Observed {
                revision: Some(revision),
                deleted: true,
                ..Observed::default()
            })),
            Some((false, _)) => self.look_task_plain(store, repo, &external).await,
        }
    }

    async fn look_task_plain(
        &self,
        store: &Store,
        repo: &Path,
        external: &str,
    ) -> OpsResult<Option<Observed>> {
        let Some((item, project)) = self.issue_ownership(external).await.map_err(message)? else {
            return Ok(None);
        };
        ingest_task(store, repo, &item, project.as_ref(), now()).await?;
        Ok(Some(observed_task(store, &item).await))
    }

    async fn look_comments(&self, store: &Store, task: &TaskId) -> OpsResult<Option<Observed>> {
        let external = remote_task(store, task).await?;
        let observation = self.observe_issue(&external).await.map_err(message)?;
        Ok(Some(Observed {
            revision: Some(observation.revision),
            comments: observation
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
            .project_content_observation(&external)
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
        let complete = loopflow_storage::sqlite::linear_acceptance::accept_linear_project_order_in(
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

fn value(effect: &TrackerEffect) -> OpsResult<WorkValue> {
    serde_json::from_value(effect.value.clone()).map_err(message)
}

fn creation_values(effect: &TrackerEffect) -> OpsResult<Vec<WorkValue>> {
    serde_json::from_value(effect.value.clone()).map_err(message)
}

/// A Task's Project as Linear knows it.
async fn project_id(store: &Store, project: Option<&str>) -> OpsResult<Option<String>> {
    let Some(project) = project else {
        return Ok(None);
    };
    let project = ProjectId::parse(project).map_err(message)?;
    Ok(Some(remote_project(store, &project).await?))
}

/// The Wave a Project belongs to and that Wave's Initiative.
async fn project_scope(
    store: &Store,
    repo: &Path,
    project: &ProjectId,
) -> OpsResult<(WaveId, String)> {
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
    let initiative = crate::ops::linear::read_initiative(repo, wave.slug())
        .ok_or_else(|| message("Wave has no Linear Initiative; its Projects stay local"))?;
    Ok((project.wave_id, initiative))
}

fn accept_project(
    store: &Store,
    wave: &WaveId,
    initiative: &str,
    project: &TrackerProject,
) -> OpsResult<()> {
    loopflow_storage::sqlite::linear_acceptance::accept_linear_project_in(
        &store.sqlite,
        wave,
        initiative,
        project,
        now(),
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
