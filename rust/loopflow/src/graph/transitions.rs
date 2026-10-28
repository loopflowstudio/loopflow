use std::collections::BTreeMap;

use anyhow::{bail, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::graph::{return_target, ConcreteStep};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum GraphDecision {
    #[serde(alias = "next", alias = "complete")]
    Advance,
    #[serde(alias = "repeat", alias = "continue")]
    Iterate,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphVerdict {
    pub decision: GraphDecision,
    pub summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphProgress {
    /// Backward traversals by the deciding step's index in its body, retained
    /// for the invocation.
    pub repeats: BTreeMap<String, u32>,
    pub direction: Option<String>,
    pub verdict: Option<GraphVerdict>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphTransition {
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
    progress: &mut GraphProgress,
) -> Result<GraphTransition> {
    if index >= steps.len() {
        bail!("graph cursor {index} is outside {} steps", steps.len());
    }
    let edge = return_target(steps, index);

    let decision = match &progress.verdict {
        Some(verdict) => {
            if verdict.summary.trim().is_empty() {
                return Ok(GraphTransition::Blocked(format!(
                    "graph decision at step {index} requires evidence or direction"
                )));
            }
            verdict.decision
        }
        None if edge.is_some() => {
            return Ok(GraphTransition::Blocked(format!(
                "loop at step {index} requires a decision"
            )));
        }
        None => GraphDecision::Advance,
    };

    match decision {
        GraphDecision::Blocked => Ok(GraphTransition::Blocked(
            progress
                .verdict
                .as_ref()
                .expect("blocked decision has a verdict")
                .summary
                .clone(),
        )),
        GraphDecision::Iterate => {
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
            Ok(GraphTransition::Repeat(target))
        }
        GraphDecision::Advance => {
            if progress.verdict.take().is_some() {
                progress.direction = None;
            }
            if index + 1 == steps.len() {
                Ok(GraphTransition::Finished)
            } else {
                Ok(GraphTransition::Next(index + 1))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::graph::transitions::{
        finish_step, GraphDecision, GraphProgress, GraphTransition, GraphVerdict,
    };
    use crate::graph::{Command, ConcreteCommand, ConcreteSkill, ConcreteStep, Skill};

    fn step(id: &str, returns: Option<usize>) -> ConcreteStep {
        ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named(id),
            id: Some(id.to_owned()),
            interactive: false,
            returns,
            sources: vec![],
        })
    }

    fn decide(progress: &mut GraphProgress, decision: GraphDecision, summary: &str) {
        progress.verdict = Some(GraphVerdict {
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
        let mut progress = GraphProgress {
            direction: Some("previous direction".to_owned()),
            ..GraphProgress::default()
        };
        assert_eq!(
            finish_step(&steps, 0, &mut progress).unwrap(),
            GraphTransition::Next(1)
        );
        assert_eq!(progress.direction.as_deref(), Some("previous direction"));
        assert_eq!(
            finish_step(&steps, 1, &mut progress).unwrap(),
            GraphTransition::Finished
        );
    }

    #[test]
    fn iterate_requires_an_explicit_edge_even_for_interactive_steps() {
        for interactive in [false, true] {
            let mut review = step("review", None);
            if let ConcreteStep::Skill(skill) = &mut review {
                skill.interactive = interactive;
            }
            let steps = [step("work", None), review];
            let mut progress = GraphProgress::default();
            decide(&mut progress, GraphDecision::Iterate, "revise the design");
            let mut recovered: GraphProgress =
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
        let mut progress = GraphProgress::default();
        for (count, direction) in [(1, "repair behavior"), (2, "prove recovery")] {
            decide(&mut progress, GraphDecision::Iterate, direction);
            assert_eq!(
                finish_step(&steps, 1, &mut progress).unwrap(),
                GraphTransition::Repeat(0)
            );
            assert_eq!(progress.repeats["1"], count);
            assert_eq!(progress.direction.as_deref(), Some(direction));
            assert_eq!(progress.verdict, None);
            assert_eq!(
                finish_step(&steps, 0, &mut progress).unwrap(),
                GraphTransition::Next(1)
            );
            assert_eq!(progress.direction.as_deref(), Some(direction));
        }
        decide(&mut progress, GraphDecision::Advance, "all claims verified");
        assert_eq!(
            finish_step(&steps, 1, &mut progress).unwrap(),
            GraphTransition::Finished
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
        let mut progress = GraphProgress::default();
        for (review, target) in [(1, 0), (3, 2)] {
            decide(&mut progress, GraphDecision::Iterate, "another pass");
            assert_eq!(
                finish_step(&steps, review, &mut progress).unwrap(),
                GraphTransition::Repeat(target)
            );
            finish_step(&steps, target, &mut progress).unwrap();
            decide(&mut progress, GraphDecision::Advance, "verified");
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
        let mut progress = GraphProgress::default();
        decide(&mut progress, GraphDecision::Iterate, "first edge");
        assert_eq!(
            finish_step(&steps, 2, &mut progress).unwrap(),
            GraphTransition::Repeat(0)
        );
        finish_step(&steps, 0, &mut progress).unwrap();
        finish_step(&steps, 1, &mut progress).unwrap();
        decide(&mut progress, GraphDecision::Advance, "first edge complete");
        assert_eq!(
            finish_step(&steps, 2, &mut progress).unwrap(),
            GraphTransition::Next(3)
        );
        decide(&mut progress, GraphDecision::Iterate, "overlapping pass");
        assert_eq!(
            finish_step(&steps, 3, &mut progress).unwrap(),
            GraphTransition::Repeat(1)
        );
        finish_step(&steps, 1, &mut progress).unwrap();
        decide(&mut progress, GraphDecision::Iterate, "revisit first edge");
        assert_eq!(
            finish_step(&steps, 2, &mut progress).unwrap(),
            GraphTransition::Repeat(0)
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
            Some((GraphDecision::Advance, " \n")),
            Some((GraphDecision::Iterate, "")),
        ];
        for verdict in verdicts {
            let mut progress = GraphProgress {
                repeats: [("1".to_owned(), 1)].into(),
                direction: Some("preserved direction".to_owned()),
                verdict: verdict.map(|(decision, summary)| GraphVerdict {
                    decision,
                    summary: summary.to_owned(),
                }),
            };
            let before = progress.clone();
            let outcome = finish_step(&steps, 1, &mut progress).unwrap();
            assert!(matches!(&outcome, GraphTransition::Blocked(reason) if !reason.is_empty()));
            assert_eq!(progress, before);
        }
    }

    #[test]
    fn observed_pass_count_never_prevents_iteration() {
        for traversals in [0, 7, 100, u32::MAX] {
            let steps = [step("work", None), step("review", Some(1))];
            let mut progress = GraphProgress {
                repeats: [("1".to_owned(), traversals)].into(),
                direction: Some("retained".to_owned()),
                verdict: None,
            };
            decide(&mut progress, GraphDecision::Iterate, "one more pass");
            assert_eq!(
                finish_step(&steps, 1, &mut progress).unwrap(),
                GraphTransition::Repeat(0)
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
            let mut progress = GraphProgress::default();
            decide(&mut progress, GraphDecision::Iterate, "another pass");
            let before = progress.clone();
            assert!(finish_step(&steps, index, &mut progress).is_err());
            assert_eq!(progress, before);
        }
    }

    #[test]
    fn decisions_read_old_names_and_write_current_values() {
        for (saved, decision, canonical) in [
            ("continue", GraphDecision::Iterate, "iterate"),
            ("repeat", GraphDecision::Iterate, "iterate"),
            ("complete", GraphDecision::Advance, "advance"),
            ("next", GraphDecision::Advance, "advance"),
            ("blocked", GraphDecision::Blocked, "blocked"),
        ] {
            let decoded: GraphDecision = serde_json::from_value(saved.into()).unwrap();
            assert_eq!(decoded, decision);
            assert_eq!(serde_json::to_value(decoded).unwrap(), canonical);
        }
    }

    #[test]
    fn persisted_pending_decision_recovers_the_same_transition() {
        let steps = [step("work", None), step("review", Some(1))];
        let mut original = GraphProgress {
            repeats: [("1".to_owned(), 1)].into(),
            ..GraphProgress::default()
        };
        decide(
            &mut original,
            GraphDecision::Iterate,
            "recover this direction",
        );
        let encoded = serde_json::to_value(&original).unwrap();
        assert_eq!(encoded["verdict"]["decision"], "iterate");
        let mut recovered: GraphProgress = serde_json::from_value(encoded).unwrap();
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
