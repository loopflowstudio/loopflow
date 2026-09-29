//! A captured boundary's output contract, shared by provider launch and settlement.

use serde_json::{json, Value};

use crate::engine::transitions::{FlowDecision, FlowVerdict};
use crate::engine::{ConcreteStep, SkillOutcome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowOutput {
    Decision,
    Route(Vec<String>),
}

impl FlowOutput {
    pub fn instructions(&self) -> String {
        format!("\n\nReturn the final answer as the declared JSON value: {}. This output contract supersedes saved instructions to run Flow decision or router commands. The selected successful completion supplies the decision. If blocked, `lf flow blocked REASON` requests feedback; reassess after it returns.", self.schema())
    }

    pub fn decode_receipt(&self, payload: &Value) -> Result<SkillOutcome, String> {
        if let Some(value) = payload.get("value") {
            return self.decode(value);
        }
        let text = payload["text"].as_str().ok_or("final output is absent")?;
        let value = serde_json::from_str(text).map_err(|_| "final output must be JSON")?;
        self.decode(&value)
    }

    pub fn for_step(step: &ConcreteStep) -> Option<Self> {
        match step {
            ConcreteStep::Skill(skill) if !skill.policy.human && skill.policy.repeat.is_some() => {
                Some(Self::Decision)
            }
            ConcreteStep::Xor(branch) => {
                let mut paths: Vec<_> = branch.paths.keys().cloned().collect();
                paths.sort();
                Some(Self::Route(paths))
            }
            _ => None,
        }
    }

    pub fn for_step_instructions(step: &ConcreteStep) -> Option<String> {
        let output = Self::for_step(step)?;
        let mut instructions = output.instructions();
        if let ConcreteStep::Xor(branch) = step {
            instructions.push_str("\n\n");
            instructions.push_str(&crate::engine::flow::build_xor_routing_suffix(branch));
        }
        Some(instructions)
    }

    pub fn schema(&self) -> Value {
        match self {
            Self::Decision => json!({
                "type": "object",
                "properties": {
                    "decision": {"type": "string", "enum": ["advance", "iterate"]},
                    "summary": {"type": "string", "pattern": "\\S"}
                },
                "required": ["decision", "summary"],
                "additionalProperties": false
            }),
            Self::Route(paths) => json!({
                "type": "object",
                "properties": {"path": {"type": "string", "enum": paths}},
                "required": ["path"],
                "additionalProperties": false
            }),
        }
    }

    /// Decode only this boundary's value; ordinary prose is never a verdict.
    pub fn decode(&self, value: &Value) -> Result<SkillOutcome, String> {
        let object = value
            .as_object()
            .ok_or("expected a structured output object")?;
        match self {
            Self::Decision => {
                if object.len() != 2 {
                    return Err("expected exactly decision and summary".into());
                }
                let decision = match value["decision"].as_str() {
                    Some("advance") => FlowDecision::Advance,
                    Some("iterate") => FlowDecision::Iterate,
                    _ => return Err("decision must be advance or iterate".into()),
                };
                let summary = value["summary"]
                    .as_str()
                    .filter(|text| !text.trim().is_empty())
                    .ok_or("summary must contain evidence or direction")?;
                Ok(SkillOutcome::Decided(FlowVerdict {
                    decision,
                    summary: summary.to_owned(),
                }))
            }
            Self::Route(paths) => {
                if object.len() != 1 {
                    return Err("expected exactly path".into());
                }
                let path = value["path"].as_str().ok_or("path must be a string")?;
                if !paths.iter().any(|allowed| allowed == path) {
                    return Err(format!("path must be one of {}", paths.join(", ")));
                }
                Ok(SkillOutcome::Routed(path.to_owned()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FlowOutput;
    use crate::engine::transitions::FlowDecision;
    use crate::engine::SkillOutcome;
    use serde_json::json;

    #[test]
    fn decisions_require_the_declared_value_and_evidence() {
        let output = FlowOutput::Decision;
        for invalid in [
            json!({"decision":"next","summary":"legacy alias"}),
            json!({"decision":"advance","summary":"  "}),
            json!({"decision":"advance","summary":"proven","confidence":0.9}),
            json!({"decision":"advance"}),
            json!("advance"),
        ] {
            assert!(output.decode(&invalid).is_err());
        }
        let SkillOutcome::Decided(verdict) = output
            .decode(&json!({"decision":"iterate","summary":"repair the failed proof"}))
            .unwrap()
        else {
            panic!("typed decision")
        };
        assert_eq!(verdict.decision, FlowDecision::Iterate);
        assert_eq!(verdict.summary, "repair the failed proof");
    }

    #[test]
    fn routes_cannot_select_a_path_outside_the_captured_graph() {
        let output = FlowOutput::Route(vec!["work".into(), "silence".into()]);
        assert!(output.decode(&json!({"path":"new-catalog-path"})).is_err());
        assert!(output.decode(&json!({"path":"work","extra":true})).is_err());
        assert!(matches!(output.decode(&json!({"path":"silence"})).unwrap(),
            SkillOutcome::Routed(path) if path == "silence"));
    }
}
