//! Project-wide ordering delivery; individual moves retain both Linear sort keys.

use std::path::Path;

use serde_json::json;

use crate::durable::ProjectId;
use crate::pm::linear::{LinearClient, OrderedIssue};
use crate::store::sqlite::planning_order::{include_members, OrderEffect};
use crate::store::Store;

use super::{OpsError, OpsResult};

fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

pub(super) async fn sync_order(
    store: &Store,
    repo: &Path,
    project: &ProjectId,
    client: &LinearClient,
) -> OpsResult<()> {
    let local = store
        .get_project(project)
        .await
        .map_err(message)?
        .ok_or_else(|| message("Project is missing"))?;
    let external = local.plan.linear_id()?.as_str();
    // The caller bounds the entire operation. Each confirmed move is durable;
    // a timeout preserves the exact last attempt for the next foreground pass.
    loop {
        let issues = client.ordered_items(external).await.map_err(message)?;
        let items = issues.iter().map(|i| i.item.clone()).collect::<Vec<_>>();
        if !store
            .sqlite
            .put_pm_project_order(&repo.to_string_lossy(), external, &items)
            .map_err(message)?
        {
            return Err(message(
                "Incomplete or older Project order; saved reorder retained",
            ));
        }
        let Some(delivery) = store
            .sqlite
            .project_order_delivery(project)
            .map_err(message)?
        else {
            return Ok(());
        };
        if delivery.effects.last().is_some_and(|e| !e.settled) {
            return Err(message(
                "Order move remains uncertain; no repeated mutation issued",
            ));
        }
        let mut observed = Vec::new();
        for issue in &issues {
            let task = store
                .get_task_by_issue(&issue.item.id)
                .await
                .map_err(message)?
                .ok_or_else(|| message("Linear order has an unmapped Task"))?;
            observed.push(task.id.to_string());
        }
        let desired = delivery.desired;
        if desired.iter().any(|id| !observed.contains(id)) {
            return Err(message(
                "Order awaits Task export or confirmed membership; saved reorder retained",
            ));
        }
        let desired = include_members(&desired, &observed);
        let Some(effect) = next_move(&issues, &observed, &desired)? else {
            return Ok(());
        };
        if !store
            .sqlite
            .attempt_project_order(project, &delivery.change.id, &effect)
            .map_err(message)?
        {
            return Ok(());
        }
        client
            .deliver_planning_field(&effect.issue, false, effect.input)
            .await
            .map_err(message)?;
    }
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

fn move_input(issues: &[&OrderedIssue], position: usize) -> Option<serde_json::Value> {
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

fn next_move(
    issues: &[OrderedIssue],
    observed: &[String],
    desired: &[String],
) -> OpsResult<Option<OrderEffect>> {
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
        // past this tied run, then retry the saved target on the next read. This intermediate order is captured too.
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
    Ok(Some(OrderEffect {
        before: observed.to_vec(),
        after,
        issue: issues[source].item.id.clone(),
        input,
        settled: false,
    }))
}
