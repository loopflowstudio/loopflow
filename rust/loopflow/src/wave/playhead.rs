use std::path::Path;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::engine::{expand_flow, load_flow, ConcreteStep};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Skill,
    Op,
    And,
    Xor,
    Or,
    Loop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepPlan {
    pub path: String,
    pub name: String,
    pub kind: StepKind,
}

impl StepPlan {
    fn from_concrete(index: usize, step: &ConcreteStep) -> Self {
        let (name, kind) = match step {
            ConcreteStep::Skill(skill) => (skill.skill.name.clone(), StepKind::Skill),
            ConcreteStep::Op(op) => (op.item.display_name(), StepKind::Op),
            ConcreteStep::And(_) => ("and".to_string(), StepKind::And),
            ConcreteStep::Xor(branch) => (
                branch
                    .router
                    .clone()
                    .unwrap_or_else(|| "xor-route".to_string()),
                StepKind::Xor,
            ),
            ConcreteStep::Or(branch) => (
                branch
                    .router
                    .clone()
                    .unwrap_or_else(|| "or-route".to_string()),
                StepKind::Or,
            ),
            ConcreteStep::Loop(_) => ("loop".to_string(), StepKind::Loop),
        };
        Self {
            path: index.to_string(),
            name,
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedInvocation {
    pub id: String,
    pub flow: String,
    pub steps: Vec<StepPlan>,
}

impl QueuedInvocation {
    pub fn load(repo: &Path, flow: &str) -> Result<Self> {
        let definition = load_flow(flow, repo)?;
        let steps = expand_flow(&definition, repo)?
            .iter()
            .enumerate()
            .map(|(index, step)| StepPlan::from_concrete(index, step))
            .collect::<Vec<_>>();
        if steps.is_empty() {
            return Err(anyhow!("flow '{flow}' has no steps"));
        }
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            flow: definition.name,
            steps,
        })
    }

    fn start(self) -> InvocationState {
        InvocationState {
            id: self.id,
            flow: self.flow,
            steps: self.steps,
            cursor: 0,
            iteration: 0,
            queue: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationState {
    pub id: String,
    pub flow: String,
    pub steps: Vec<StepPlan>,
    pub cursor: u32,
    pub iteration: u32,
    pub queue: Vec<QueuedInvocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRef {
    pub invocation_id: String,
    pub flow: String,
    pub step_path: String,
    pub step: String,
    pub kind: StepKind,
    pub index: u32,
    pub total: u32,
    pub iteration: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyProvenance {
    pub body_id: String,
    pub invocation_id: String,
    pub step_path: String,
    pub flow: String,
    pub step: String,
    pub iteration: u32,
    pub session_id: Option<String>,
    pub harness: Option<String>,
    pub model: Option<String>,
    pub host: String,
    pub worktree: String,
    pub run_id: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub termination_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveBody {
    pub step: StepRef,
    pub body: BodyProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Playhead {
    pub stack: Vec<InvocationState>,
    pub active: Option<ActiveBody>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayheadView {
    pub stack: Vec<InvocationState>,
    pub active: Option<ActiveBody>,
    pub now: Option<StepRef>,
    pub next: Option<StepRef>,
    pub queued: Vec<QueuedInvocation>,
    pub return_to: Option<StepRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepOutcome {
    Completed,
    Skipped,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlayheadEvent {
    Initialized {
        invocation_id: String,
        flow: String,
    },
    FlowEnqueued {
        parent_invocation_id: String,
        invocation_id: String,
        flow: String,
    },
    InvocationStarted {
        invocation_id: String,
        flow: String,
    },
    InvocationCompleted {
        invocation_id: String,
        flow: String,
    },
    StepStarted {
        step: StepRef,
        body_id: String,
    },
    StepCompleted {
        step: StepRef,
        body_id: String,
    },
    StepSkipped {
        step: StepRef,
        body_id: String,
        reason: String,
    },
    StepFailed {
        step: StepRef,
        body_id: String,
        reason: String,
    },
    StepInterrupted {
        step: StepRef,
        body_id: String,
        reason: String,
    },
}

impl Playhead {
    pub fn new(root: QueuedInvocation) -> (Self, PlayheadEvent) {
        let event = PlayheadEvent::Initialized {
            invocation_id: root.id.clone(),
            flow: root.flow.clone(),
        };
        (
            Self {
                stack: vec![root.start()],
                active: None,
            },
            event,
        )
    }

    pub fn current(&self) -> Option<StepRef> {
        let invocation = self.stack.last()?;
        let step = invocation.steps.get(invocation.cursor as usize)?;
        Some(StepRef {
            invocation_id: invocation.id.clone(),
            flow: invocation.flow.clone(),
            step_path: step.path.clone(),
            step: step.name.clone(),
            kind: step.kind,
            index: invocation.cursor,
            total: invocation.steps.len() as u32,
            iteration: invocation.iteration,
        })
    }

    pub fn view(&self) -> PlayheadView {
        let queued = self
            .stack
            .last()
            .map(|invocation| invocation.queue.clone())
            .unwrap_or_default();
        PlayheadView {
            stack: self.stack.clone(),
            active: self.active.clone(),
            now: self.current(),
            next: self.next_after_current(),
            queued,
            return_to: self.return_target(),
        }
    }

    pub fn enqueue(&mut self, invocation: QueuedInvocation) -> Result<PlayheadEvent> {
        let parent = self
            .stack
            .last_mut()
            .ok_or_else(|| anyhow!("playhead has no invocation"))?;
        let event = PlayheadEvent::FlowEnqueued {
            parent_invocation_id: parent.id.clone(),
            invocation_id: invocation.id.clone(),
            flow: invocation.flow.clone(),
        };
        parent.queue.push(invocation);
        Ok(event)
    }

    pub fn start_body(&mut self, body: BodyProvenance) -> Result<PlayheadEvent> {
        if self.active.is_some() {
            return Err(anyhow!("playhead already has an active body"));
        }
        let step = self
            .current()
            .ok_or_else(|| anyhow!("playhead has no current step"))?;
        if body.invocation_id != step.invocation_id || body.step_path != step.step_path {
            return Err(anyhow!("body does not match the current playhead step"));
        }
        let event = PlayheadEvent::StepStarted {
            step: step.clone(),
            body_id: body.body_id.clone(),
        };
        self.active = Some(ActiveBody { step, body });
        Ok(event)
    }

    pub fn finish_body(
        &mut self,
        body_id: &str,
        outcome: StepOutcome,
        reason: &str,
    ) -> Result<Vec<PlayheadEvent>> {
        let active = self
            .active
            .take()
            .ok_or_else(|| anyhow!("playhead has no active body"))?;
        if active.body.body_id != body_id {
            self.active = Some(active);
            return Err(anyhow!("body id does not match the active playhead body"));
        }

        let mut events = vec![match outcome {
            StepOutcome::Completed => PlayheadEvent::StepCompleted {
                step: active.step.clone(),
                body_id: body_id.to_string(),
            },
            StepOutcome::Skipped => PlayheadEvent::StepSkipped {
                step: active.step.clone(),
                body_id: body_id.to_string(),
                reason: reason.to_string(),
            },
            StepOutcome::Failed => PlayheadEvent::StepFailed {
                step: active.step.clone(),
                body_id: body_id.to_string(),
                reason: reason.to_string(),
            },
            StepOutcome::Interrupted => PlayheadEvent::StepInterrupted {
                step: active.step.clone(),
                body_id: body_id.to_string(),
                reason: reason.to_string(),
            },
        }];

        if matches!(outcome, StepOutcome::Completed | StepOutcome::Skipped) {
            let frame = self
                .stack
                .last_mut()
                .expect("an active body always belongs to an invocation");
            frame.cursor += 1;
            self.settle(&mut events);
        }
        Ok(events)
    }

    fn settle(&mut self, events: &mut Vec<PlayheadEvent>) {
        loop {
            let depth = self.stack.len();
            let Some(top) = self.stack.last_mut() else {
                return;
            };

            // The root is the default playlist: an explicit continuation cuts
            // in after the current step, then returns to the saved cursor.
            if depth == 1 && !top.queue.is_empty() {
                let queued = top.queue.remove(0);
                events.push(PlayheadEvent::InvocationStarted {
                    invocation_id: queued.id.clone(),
                    flow: queued.flow.clone(),
                });
                self.stack.push(queued.start());
                return;
            }

            if (top.cursor as usize) < top.steps.len() {
                return;
            }

            events.push(PlayheadEvent::InvocationCompleted {
                invocation_id: top.id.clone(),
                flow: top.flow.clone(),
            });

            if !top.queue.is_empty() {
                let queued = top.queue.remove(0);
                events.push(PlayheadEvent::InvocationStarted {
                    invocation_id: queued.id.clone(),
                    flow: queued.flow.clone(),
                });
                self.stack.push(queued.start());
                return;
            }

            if depth == 1 {
                top.cursor = 0;
                top.iteration += 1;
                events.push(PlayheadEvent::InvocationStarted {
                    invocation_id: top.id.clone(),
                    flow: top.flow.clone(),
                });
                return;
            }

            self.stack.pop();
        }
    }

    fn next_after_current(&self) -> Option<StepRef> {
        let mut next = self.clone();
        next.active = None;
        let frame = next.stack.last_mut()?;
        frame.cursor += 1;
        next.settle(&mut Vec::new());
        next.current()
    }

    fn return_target(&self) -> Option<StepRef> {
        if self.stack.len() < 2 {
            return None;
        }
        self.stack[..self.stack.len() - 1]
            .iter()
            .rev()
            .find_map(|invocation| {
                let step = invocation.steps.get(invocation.cursor as usize)?;
                Some(StepRef {
                    invocation_id: invocation.id.clone(),
                    flow: invocation.flow.clone(),
                    step_path: step.path.clone(),
                    step: step.name.clone(),
                    kind: step.kind,
                    index: invocation.cursor,
                    total: invocation.steps.len() as u32,
                    iteration: invocation.iteration,
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation(flow: &str, steps: &[&str]) -> QueuedInvocation {
        QueuedInvocation {
            id: format!("{flow}-{}", Uuid::new_v4()),
            flow: flow.to_string(),
            steps: steps
                .iter()
                .enumerate()
                .map(|(index, name)| StepPlan {
                    path: index.to_string(),
                    name: (*name).to_string(),
                    kind: StepKind::Skill,
                })
                .collect(),
        }
    }

    fn body(playhead: &Playhead, id: &str) -> BodyProvenance {
        let step = playhead.current().unwrap();
        BodyProvenance {
            body_id: id.to_string(),
            invocation_id: step.invocation_id,
            step_path: step.step_path,
            flow: step.flow,
            step: step.step,
            iteration: step.iteration,
            session_id: None,
            harness: None,
            model: None,
            host: "host".to_string(),
            worktree: "/repo".to_string(),
            run_id: None,
            started_at: "2026-07-09T00:00:00Z".to_string(),
            ended_at: None,
            termination_reason: None,
        }
    }

    fn complete(playhead: &mut Playhead, id: &str) -> Vec<PlayheadEvent> {
        playhead.start_body(body(playhead, id)).unwrap();
        playhead
            .finish_body(id, StepOutcome::Completed, "completed")
            .unwrap()
    }

    #[test]
    fn root_loops_and_preserves_iteration() {
        let (mut playhead, _) = Playhead::new(invocation("wave", &["clarify", "pursue"]));
        complete(&mut playhead, "body-1");
        assert_eq!(playhead.current().unwrap().step, "pursue");
        complete(&mut playhead, "body-2");
        let current = playhead.current().unwrap();
        assert_eq!(current.step, "clarify");
        assert_eq!(current.iteration, 1);
    }

    #[test]
    fn root_queue_cuts_in_then_returns_to_saved_step() {
        let (mut playhead, _) = Playhead::new(invocation("wave", &["clarify", "pursue", "mutate"]));
        complete(&mut playhead, "body-1");
        playhead
            .enqueue(invocation(
                "review-design",
                &["clarify", "pursue", "mutate"],
            ))
            .unwrap();
        complete(&mut playhead, "body-2");
        assert_eq!(playhead.current().unwrap().flow, "review-design");
        for id in ["body-3", "body-4", "body-5"] {
            complete(&mut playhead, id);
        }
        let current = playhead.current().unwrap();
        assert_eq!(current.flow, "wave");
        assert_eq!(current.step, "mutate");
    }

    #[test]
    fn nested_queue_drains_after_the_invocation_and_before_returning() {
        let (mut playhead, _) = Playhead::new(invocation("wave", &["pursue", "mutate"]));
        playhead
            .enqueue(invocation(
                "review-design",
                &["clarify", "pursue", "mutate"],
            ))
            .unwrap();
        complete(&mut playhead, "body-1");
        complete(&mut playhead, "body-2");
        assert_eq!(playhead.current().unwrap().step, "pursue");
        playhead
            .enqueue(invocation("research", &["research"]))
            .unwrap();
        complete(&mut playhead, "body-3");
        complete(&mut playhead, "body-4");
        assert_eq!(playhead.current().unwrap().flow, "research");
        complete(&mut playhead, "body-5");
        assert_eq!(playhead.current().unwrap().step, "mutate");
    }

    #[test]
    fn skip_advances_but_failure_retries_the_same_step() {
        let (mut playhead, _) = Playhead::new(invocation("wave", &["clarify", "pursue"]));
        playhead.start_body(body(&playhead, "body-1")).unwrap();
        playhead
            .finish_body("body-1", StepOutcome::Failed, "crashed")
            .unwrap();
        assert_eq!(playhead.current().unwrap().step, "clarify");

        playhead.start_body(body(&playhead, "body-2")).unwrap();
        playhead
            .finish_body("body-2", StepOutcome::Skipped, "skipped by user")
            .unwrap();
        assert_eq!(playhead.current().unwrap().step, "pursue");
    }
}
