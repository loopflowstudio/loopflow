//! Preserve the nested settings layout in captured Flow plans and journals.

use serde::{Deserialize, Serialize};

use super::{ConcreteSkill, RepeatPolicy, Skill};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct SavedSkill {
    skill: Skill,
    policy: Settings,
    flow_parents: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Settings {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    human: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    repeat: Option<RepeatPolicy>,
}

impl From<ConcreteSkill> for SavedSkill {
    fn from(step: ConcreteSkill) -> Self {
        Self {
            skill: step.skill,
            policy: Settings {
                id: step.id,
                human: step.human,
                repeat: step.repeat,
            },
            flow_parents: step.flow_parents,
        }
    }
}

impl From<SavedSkill> for ConcreteSkill {
    fn from(step: SavedSkill) -> Self {
        Self {
            skill: step.skill,
            id: step.policy.id,
            human: step.policy.human,
            repeat: step.policy.repeat,
            flow_parents: step.flow_parents,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::ConcreteStep;

    #[test]
    fn saved_reviews_and_repeat_edges_keep_their_wire_format() {
        for settings in [
            serde_json::json!({"id": "review", "human": true}),
            serde_json::json!({"id": "decide", "repeat": {"from": "work"}}),
        ] {
            let saved = serde_json::json!({"Skill": {
                "skill": {"name": "fixture", "content": "Captured instructions"},
                "policy": settings,
                "flow_parents": ["feature"]
            }});
            let restored: ConcreteStep = serde_json::from_value(saved.clone()).unwrap();
            let ConcreteStep::Skill(step) = &restored else {
                panic!("saved skill")
            };
            assert_eq!(step.id.as_deref(), settings["id"].as_str());
            assert_eq!(step.human, settings["human"] == true);
            assert_eq!(
                step.repeat.as_ref().map(|edge| edge.from.as_str()),
                settings["repeat"]["from"].as_str()
            );
            assert_eq!(serde_json::to_value(restored).unwrap(), saved);
        }
    }
}
