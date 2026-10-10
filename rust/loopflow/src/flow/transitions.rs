use std::collections::BTreeMap;

use anyhow::{bail, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::flow::{return_target, ConcreteStep};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum FlowDecision {
    #[serde(alias = "next", alias = "complete")]
    Advance,
    #[serde(alias = "repeat", alias = "continue")]
    Iterate,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowVerdict {
    pub decision: FlowDecision,
    pub summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowProgress {
    /// Backward traversals by the deciding step's index in its body, retained
    /// for the invocation.
    pub repeats: BTreeMap<String, u32>,
    pub direction: Option<String>,
    pub verdict: Option<FlowVerdict>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowTransition {
    Next(usize),
    Repeat(usize),
    Finished,
    Blocked(String),
}

/// Compute a transition; the caller must settle it under its execution authority.
///
/// # Errors
/// Returns an error for an invalid cursor, or an iterate decision at a step
/// without a backward edge. Errors and blocked
/// outcomes leave progress unchanged.
pub fn finish_step(
    steps: &[ConcreteStep],
    index: usize,
    progress: &mut FlowProgress,
) -> Result<FlowTransition> {
    if index >= steps.len() {
        bail!("flow cursor {index} is outside {} steps", steps.len());
    }
    let edge = return_target(steps, index);

    let decision = match &progress.verdict {
        Some(verdict) => {
            if verdict.summary.trim().is_empty() {
                return Ok(FlowTransition::Blocked(format!(
                    "flow decision at step {index} requires evidence or direction"
                )));
            }
            verdict.decision
        }
        None if edge.is_some() => {
            return Ok(FlowTransition::Blocked(format!(
                "loop at step {index} requires a decision"
            )));
        }
        None => FlowDecision::Advance,
    };

    match decision {
        FlowDecision::Blocked => Ok(FlowTransition::Blocked(
            progress
                .verdict
                .as_ref()
                .expect("blocked decision has a verdict")
                .summary
                .clone(),
        )),
        FlowDecision::Iterate => {
            let Some(target) = edge else {
                bail!("iterate decision at step {index} has no declared backward edge");
            };
            let traversals = progress.repeats.entry(index.to_string()).or_default();
            *traversals = traversals.saturating_add(1);
            progress.direction = Some(
                progress
                    .verdict
                    .take()
                    .expect("a repeat decision has a verdict")
                    .summary,
            );
            Ok(FlowTransition::Repeat(target))
        }
        FlowDecision::Advance => {
            if progress.verdict.take().is_some() {
                progress.direction = None;
            }
            if index + 1 == steps.len() {
                Ok(FlowTransition::Finished)
            } else {
                Ok(FlowTransition::Next(index + 1))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::flow::transitions::{
        finish_step, FlowDecision, FlowProgress, FlowTransition, FlowVerdict,
    };
    use crate::flow::{Command, ConcreteCommand, ConcreteSkill, ConcreteStep, Skill};

    fn step(id: &str, returns: Option<usize>) -> ConcreteStep {
        ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named(id),
            id: Some(id.to_owned()),
            human: false,
            returns,
            sources: vec![],
        })
    }

    fn decide(progress: &mut FlowProgress, decision: FlowDecision, summary: &str) {
        progress.verdict = Some(FlowVerdict {
            decision,
            summary: summary.to_owned(),
        });
    }

    #[test]
    fn ordinary_steps_advance_and_finish_without_decisions() {
        let steps = [
            step("start", None),
            ConcreteStep::Command(ConcreteCommand {
                item: Command {
                    command: "finish".to_owned(),
                    args: vec![],
                },
                sources: vec![],
            }),
        ];
        let mut progress = FlowProgress {
            direction: Some("previous direction".to_owned()),
            ..FlowProgress::default()
        };
        assert_eq!(
            finish_step(&steps, 0, &mut progress).unwrap(),
            FlowTransition::Next(1)
        );
        assert_eq!(progress.direction.as_deref(), Some("previous direction"));
        assert_eq!(
            finish_step(&steps, 1, &mut progress).unwrap(),
            FlowTransition::Finished
        );
    }

    #[test]
    fn iterate_requires_an_explicit_edge_even_for_saved_human_steps() {
        for human in [false, true] {
            let mut review = step("review", None);
            if let ConcreteStep::Skill(skill) = &mut review {
                skill.human = human;
            }
            let steps = [step("work", None), review];
            let mut progress = FlowProgress::default();
            decide(&mut progress, FlowDecision::Iterate, "revise the design");
            let mut recovered: FlowProgress =
                serde_json::from_value(serde_json::to_value(&progress).unwrap()).unwrap();
            assert!(finish_step(&steps, 1, &mut recovered)
                .unwrap_err()
                .to_string()
                .contains("no declared backward edge"));
            assert_eq!(recovered, progress);
        }
    }

    #[test]
    fn repeated_visits_carry_direction_then_finish_forward() {
        let steps = [step("work", None), step("review", Some(1))];
        let mut progress = FlowProgress::default();
        for (count, direction) in [(1, "repair behavior"), (2, "prove recovery")] {
            decide(&mut progress, FlowDecision::Iterate, direction);
            assert_eq!(
                finish_step(&steps, 1, &mut progress).unwrap(),
                FlowTransition::Repeat(0)
            );
            assert_eq!(progress.repeats["1"], count);
            assert_eq!(progress.direction.as_deref(), Some(direction));
            assert_eq!(progress.verdict, None);
            assert_eq!(
                finish_step(&steps, 0, &mut progress).unwrap(),
                FlowTransition::Next(1)
            );
            assert_eq!(progress.direction.as_deref(), Some(direction));
        }
        decide(&mut progress, FlowDecision::Advance, "all claims verified");
        assert_eq!(
            finish_step(&steps, 1, &mut progress).unwrap(),
            FlowTransition::Finished
        );
        assert_eq!(progress.repeats["1"], 2);
        assert_eq!(progress.verdict, None);
        assert_eq!(progress.direction, None);
    }

    #[test]
    fn independent_edges_keep_separate_counts() {
        let steps = [
            step("first", None),
            step("first-review", Some(1)),
            step("second", None),
            step("second-review", Some(1)),
        ];
        let mut progress = FlowProgress::default();
        for (review, target) in [(1, 0), (3, 2)] {
            decide(&mut progress, FlowDecision::Iterate, "another pass");
            assert_eq!(
                finish_step(&steps, review, &mut progress).unwrap(),
                FlowTransition::Repeat(target)
            );
            finish_step(&steps, target, &mut progress).unwrap();
            decide(&mut progress, FlowDecision::Advance, "verified");
            finish_step(&steps, review, &mut progress).unwrap();
        }
        assert_eq!(progress.repeats.len(), 2);
        assert_eq!(progress.repeats["1"], 1);
        assert_eq!(progress.repeats["3"], 1);
    }

    #[test]
    fn overlapping_edges_can_revisit_a_completed_section() {
        let steps = [
            step("start", None),
            step("middle", None),
            step("inner-review", Some(2)),
            step("outer-review", Some(2)),
        ];
        let mut progress = FlowProgress::default();
        decide(&mut progress, FlowDecision::Iterate, "first edge");
        assert_eq!(
            finish_step(&steps, 2, &mut progress).unwrap(),
            FlowTransition::Repeat(0)
        );
        finish_step(&steps, 0, &mut progress).unwrap();
        finish_step(&steps, 1, &mut progress).unwrap();
        decide(&mut progress, FlowDecision::Advance, "first edge complete");
        assert_eq!(
            finish_step(&steps, 2, &mut progress).unwrap(),
            FlowTransition::Next(3)
        );
        decide(&mut progress, FlowDecision::Iterate, "overlapping pass");
        assert_eq!(
            finish_step(&steps, 3, &mut progress).unwrap(),
            FlowTransition::Repeat(1)
        );
        finish_step(&steps, 1, &mut progress).unwrap();
        decide(&mut progress, FlowDecision::Iterate, "revisit first edge");
        assert_eq!(
            finish_step(&steps, 2, &mut progress).unwrap(),
            FlowTransition::Repeat(0)
        );
        assert_eq!(progress.direction.as_deref(), Some("revisit first edge"));
        assert_eq!(progress.repeats["2"], 2);
        assert_eq!(progress.repeats["3"], 1);
    }

    #[test]
    fn missing_and_empty_decisions_preserve_all_progress() {
        let steps = [step("work", None), step("review", Some(1))];
        let verdicts = [
            None,
            Some((FlowDecision::Advance, " \n")),
            Some((FlowDecision::Iterate, "")),
        ];
        for verdict in verdicts {
            let mut progress = FlowProgress {
                repeats: [("1".to_owned(), 1)].into(),
                direction: Some("preserved direction".to_owned()),
                verdict: verdict.map(|(decision, summary)| FlowVerdict {
                    decision,
                    summary: summary.to_owned(),
                }),
            };
            let before = progress.clone();
            let outcome = finish_step(&steps, 1, &mut progress).unwrap();
            assert!(matches!(&outcome, FlowTransition::Blocked(reason) if !reason.is_empty()));
            assert_eq!(progress, before);
        }
    }

    #[test]
    fn observed_pass_count_never_prevents_iteration() {
        for traversals in [0, 7, 100, u32::MAX] {
            let steps = [step("work", None), step("review", Some(1))];
            let mut progress = FlowProgress {
                repeats: [("1".to_owned(), traversals)].into(),
                direction: Some("retained".to_owned()),
                verdict: None,
            };
            decide(&mut progress, FlowDecision::Iterate, "one more pass");
            assert_eq!(
                finish_step(&steps, 1, &mut progress).unwrap(),
                FlowTransition::Repeat(0)
            );
            assert_eq!(progress.direction.as_deref(), Some("one more pass"));
            assert!(progress.verdict.is_none());
        }
    }

    #[test]
    fn invalid_cursor_and_edges_are_errors_without_mutation() {
        let invalid = [
            (vec![], 0),
            (vec![step("work", None)], 1),
            (vec![step("review", Some(0))], 0),
            (vec![step("review", Some(1)), step("later", None)], 0),
            (vec![step("work", None), step("review", Some(2))], 1),
            (vec![step("work", None)], 0),
        ];
        for (steps, index) in invalid {
            let mut progress = FlowProgress::default();
            decide(&mut progress, FlowDecision::Iterate, "another pass");
            let before = progress.clone();
            assert!(finish_step(&steps, index, &mut progress).is_err());
            assert_eq!(progress, before);
        }
    }

    #[test]
    fn decisions_read_old_names_and_write_current_values() {
        for (saved, decision, canonical) in [
            ("continue", FlowDecision::Iterate, "iterate"),
            ("repeat", FlowDecision::Iterate, "iterate"),
            ("complete", FlowDecision::Advance, "advance"),
            ("next", FlowDecision::Advance, "advance"),
            ("blocked", FlowDecision::Blocked, "blocked"),
        ] {
            let decoded: FlowDecision = serde_json::from_value(saved.into()).unwrap();
            assert_eq!(decoded, decision);
            assert_eq!(serde_json::to_value(decoded).unwrap(), canonical);
        }
    }

    #[test]
    fn persisted_pending_decision_recovers_the_same_transition() {
        let steps = [step("work", None), step("review", Some(1))];
        let mut original = FlowProgress {
            repeats: [("1".to_owned(), 1)].into(),
            ..FlowProgress::default()
        };
        decide(
            &mut original,
            FlowDecision::Iterate,
            "recover this direction",
        );
        let encoded = serde_json::to_value(&original).unwrap();
        assert_eq!(encoded["verdict"]["decision"], "iterate");
        let mut recovered: FlowProgress = serde_json::from_value(encoded).unwrap();
        assert_eq!(
            finish_step(&steps, 1, &mut original).unwrap(),
            finish_step(&steps, 1, &mut recovered).unwrap()
        );
        assert_eq!(original, recovered);
        assert_eq!(recovered.repeats["1"], 2);
        assert_eq!(
            recovered.direction.as_deref(),
            Some("recover this direction")
        );
    }
}
