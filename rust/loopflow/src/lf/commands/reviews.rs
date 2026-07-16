use anyhow::{anyhow, Context, Result};

use crate::interaction_review::{InteractionReview, InteractionReviewer};
use crate::lf::ReviewsCommand;
use crate::store::{open_store, storage_config_from_env};
use crate::task::TaskSession;

#[derive(Debug)]
pub struct CatchUpPlan {
    pub skill: String,
    pub prompt: String,
    pub review_count: usize,
    pub preview: bool,
}

#[derive(Debug)]
struct CatchUpItem {
    review: InteractionReview,
    task: Option<TaskSession>,
}

pub fn prepare(command: &ReviewsCommand, wave: Option<&str>) -> Result<CatchUpPlan> {
    let ReviewsCommand::CatchUp {
        skill,
        plan: preview,
    } = command;
    let wave = crate::ops::resolve_wave_name(wave)
        .ok_or_else(|| anyhow!("cannot determine Wave; pass --wave <name>"))?;
    let runtime = tokio::runtime::Runtime::new().context("start review catch-up runtime")?;
    runtime.block_on(_prepare(&wave, skill, *preview))
}

async fn _prepare(wave_name: &str, skill: &str, preview: bool) -> Result<CatchUpPlan> {
    let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
    let store = open_store(&config)
        .await
        .context("open the shared Loopflow store")?;
    let wave = store
        .get_wave_by_name(wave_name)
        .await?
        .ok_or_else(|| anyhow!("Wave {wave_name} is not registered"))?;
    let reviews = store.list_interaction_reviews(Some(wave.id())).await?;
    let mut items = Vec::new();
    for review in reviews {
        if matches!(&review.reviewer, InteractionReviewer::Human) {
            continue;
        }
        let task = store.get_task_session(&review.task_session_id).await?;
        items.push(CatchUpItem { review, task });
    }
    if items.is_empty() {
        anyhow::bail!("Wave {wave_name} has no parent-reviewed interaction evidence to catch up");
    }
    Ok(CatchUpPlan {
        skill: skill.to_string(),
        prompt: format_catch_up_prompt(wave_name, skill, &items),
        review_count: items.len(),
        preview,
    })
}

fn format_catch_up_prompt(wave: &str, skill: &str, items: &[CatchUpItem]) -> String {
    let completed = items
        .iter()
        .filter(|item| item.review.status.is_terminal())
        .count();
    let open = items.len() - completed;
    let exercise = match skill {
        "demo" => {
            "For every relevant `Done When` in the design docs, build a proof matrix and show how it holds through the product, code, admin state, logs, stats, or metrics. Exercise real sign-in/login when authentication is in scope; do not bypass it."
        }
        "code-review" => {
            "Review the combined code trajectory and integration seams, not each diff in isolation. Identify structural debt, conflicting decisions, missing operational evidence, and the smallest concrete follow-up work."
        }
        _ => "Conduct the named review exercise over the combined Wave evidence.",
    };
    let mut prompt = format!(
        "Reviewer Mode: Human\n\nConduct one `{skill}` catch-up for Wave `{wave}` over {completed} completed parent reviews and {open} still-open parent reviews. This is a manual integration and feedback pass, not Task lifecycle authority. Do not rewrite source InteractionReview dispositions. Record new work as follow-up Tasks under the appropriate Project.\n\n{exercise}\n\nTreat each recorded outcome as a claim to verify against the current integrated product. Group the walkthrough by user-visible capability and shared seam rather than replaying Tasks chronologically. End with: proven now, regressed or contradictory, still unproven, and concrete follow-up Tasks.\n\n# Durable review evidence\n"
    );
    for item in items {
        prompt.push_str(&format_catch_up_item(item));
    }
    prompt
}

fn format_catch_up_item(item: &CatchUpItem) -> String {
    let review = &item.review;
    let task = item.task.as_ref();
    let task_label = task
        .map(|task| {
            format!(
                "{} — {}",
                task.launch.issue.identifier, task.launch.issue.title
            )
        })
        .unwrap_or_else(|| review.task_session_id.to_string());
    let project = task
        .map(|task| task.launch.project.name.as_str())
        .unwrap_or("unknown project");
    let reviewer = review
        .reviewer
        .id()
        .map(|id| format!("{} {id}", review.reviewer.kind()))
        .unwrap_or_else(|| review.reviewer.kind().to_string());
    let disposition = review
        .disposition
        .map(|value| value.as_str())
        .unwrap_or("pending");
    let outcome = review
        .outcome
        .as_deref()
        .unwrap_or("No outcome recorded yet.");
    let pull_request = review
        .evidence
        .pr
        .as_ref()
        .map(|pr| format!("#{} {}", pr.number, pr.url))
        .unwrap_or_else(|| "none".to_string());
    format!(
        "\n## {} · {} / {}\n\n- Task: {}\n- Project: {}\n- Reviewer: {}\n- Status: {} / {}\n- Reason: {}\n- Outcome: {}\n- Evidence: worktree `{}`, branch `{}`, base `{}`, head `{}`, PR {}\n- Requested at: {}\n",
        review.id,
        review.phase.as_str(),
        review.step,
        task_label,
        project,
        reviewer,
        review.status.as_str(),
        disposition,
        review.reason,
        outcome,
        review.evidence.worktree.display(),
        review.evidence.branch,
        review.evidence.base_commit,
        review.evidence.head_commit,
        pull_request,
        review.requested_at,
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use time::OffsetDateTime;

    use super::{format_catch_up_prompt, CatchUpItem};
    use crate::engine::InteractionPolicy;
    use crate::id::WaveId;
    use crate::interaction_review::{
        InteractionReview, InteractionReviewDisposition, InteractionReviewEvidence,
        InteractionReviewId, InteractionReviewStatus, InteractionReviewer,
    };
    use crate::project_session::ProjectSessionId;
    use crate::task::{TaskLifecyclePhase, TaskSessionId};

    fn review(status: InteractionReviewStatus) -> InteractionReview {
        let completed = status.is_terminal();
        let project = ProjectSessionId::new();
        InteractionReview {
            id: InteractionReviewId::new(),
            wave_id: WaveId::new(),
            project_session_id: project.clone(),
            task_session_id: TaskSessionId::new(),
            phase: TaskLifecyclePhase::Gate,
            phase_epoch: 3,
            flow: "task-gate".to_string(),
            step: "demo".to_string(),
            step_index: 0,
            phase_iteration: 0,
            policy: InteractionPolicy::Defer,
            reviewer: InteractionReviewer::Project(project),
            status,
            reason: "Prove login works".to_string(),
            prompt: "Conduct a demo".to_string(),
            evidence: InteractionReviewEvidence {
                worktree: PathBuf::from("/repo.login"),
                branch: "task/login".to_string(),
                base_commit: "base".to_string(),
                head_commit: "head".to_string(),
                worktree_fingerprint: "fingerprint".to_string(),
                pr: None,
            },
            requested_by_generation: 1,
            reviewer_generation: Some(1),
            disposition: completed.then_some(InteractionReviewDisposition::Approved),
            outcome: completed.then(|| "Login proven through the product".to_string()),
            requested_at: OffsetDateTime::UNIX_EPOCH,
            completed_at: completed.then_some(OffsetDateTime::UNIX_EPOCH),
        }
    }

    #[test]
    fn demo_catch_up_turns_parent_reviews_into_human_proof_work() {
        let items = [
            CatchUpItem {
                review: review(InteractionReviewStatus::Completed),
                task: None,
            },
            CatchUpItem {
                review: review(InteractionReviewStatus::Requested),
                task: None,
            },
        ];

        let prompt = format_catch_up_prompt("product", "demo", &items);

        assert!(prompt.contains("Reviewer Mode: Human"));
        assert!(prompt.contains("1 completed parent reviews and 1 still-open"));
        assert!(prompt.contains("For every relevant `Done When`"));
        assert!(prompt.contains("product, code, admin state, logs, stats, or metrics"));
        assert!(prompt.contains("real sign-in/login"));
        assert!(prompt.contains("Do not rewrite source InteractionReview dispositions"));
        assert!(prompt.contains("Login proven through the product"));
    }
}
