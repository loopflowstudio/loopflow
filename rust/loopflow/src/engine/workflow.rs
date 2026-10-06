//! A Task workflow: the stages where a person takes part in the Task
//! conversation, joined by edges that are operational Flows. `start` and `end`
//! are implicit. A workflow executes nothing; `lf task run` traverses it by
//! running an edge's Flow like any other.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::error::LoadError;
use super::flow::{compile_flow, find_flow_source_path, load_flow, load_skill};

pub const START: &str = "start";
pub const END: &str = "end";

const BUILTIN_WORKFLOWS: [(&str, &str); 3] = [
    ("code", include_str!("builtins/task/workflow/code.yaml")),
    (
        "feature",
        include_str!("builtins/task/workflow/feature.yaml"),
    ),
    (
        "research",
        include_str!("builtins/task/workflow/research.yaml"),
    ),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStage {
    pub name: String,
    /// The skill the Task conversation uses at this stage.
    pub skill: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEdge {
    pub from: String,
    pub to: String,
    /// The Flow that carries the Task along this edge. Only an edge into
    /// `end` may name none.
    pub flow: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    pub stages: Vec<WorkflowStage>,
    pub edges: Vec<WorkflowEdge>,
}

impl WorkflowEdge {
    /// What `lf task run ISSUE <name>` calls this edge: its Flow, or the stage
    /// it enters when it runs none.
    pub fn name(&self) -> &str {
        self.flow.as_deref().unwrap_or(&self.to)
    }
}

impl Workflow {
    /// Edges leaving `node`, with their index in the authored order.
    pub fn outgoing<'a>(
        &'a self,
        node: &'a str,
    ) -> impl Iterator<Item = (u32, &'a WorkflowEdge)> + 'a {
        self.edges
            .iter()
            .enumerate()
            .filter(move |(_, edge)| edge.from == node)
            .map(|(index, edge)| (index as u32, edge))
    }

    pub fn stage(&self, name: &str) -> Option<&WorkflowStage> {
        self.stages.iter().find(|stage| stage.name == name)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredWorkflow {
    #[serde(default)]
    stages: serde_yaml_ng::Mapping,
    edges: Vec<WorkflowEdge>,
}

fn workflow_path(name: &str, repo: &Path) -> Option<PathBuf> {
    ["yaml", "yml"]
        .iter()
        .map(|extension| repo.join(format!(".lf/workflows/{name}.{extension}")))
        .find(|path| path.is_file())
}

/// The builtin workflow `name`, unless a repository Flow keeps that name.
fn builtin_workflow(name: &str, repo: &Path) -> Option<&'static str> {
    let (_, content) = BUILTIN_WORKFLOWS
        .iter()
        .find(|(builtin, _)| *builtin == name)?;
    find_flow_source_path(name, repo)
        .is_none()
        .then_some(*content)
}

pub fn names_workflow(name: &str, repo: &Path) -> bool {
    workflow_path(name, repo).is_some() || builtin_workflow(name, repo).is_some()
}

/// Load `name` when it names a workflow. A repository's own file wins; a
/// repository Flow of the same name keeps the name from a builtin workflow.
pub fn load_workflow(name: &str, repo: &Path) -> Result<Option<Workflow>, LoadError> {
    let content = match workflow_path(name, repo) {
        Some(path) => fs::read_to_string(path)?,
        None => match builtin_workflow(name, repo) {
            Some(content) => content.to_string(),
            None => return Ok(None),
        },
    };
    parse_workflow(name, &content, repo)
        .map(Some)
        .map_err(|error| LoadError::InvalidFlow(format!("workflow {name}: {error}")))
}

fn parse_workflow(name: &str, content: &str, repo: &Path) -> Result<Workflow, String> {
    let authored: AuthoredWorkflow =
        serde_yaml_ng::from_str(content).map_err(|error| error.to_string())?;
    let mut stages = Vec::new();
    for (stage, skill) in &authored.stages {
        let (Some(stage), Some(skill)) = (stage.as_str(), skill.as_str()) else {
            return Err("each stage is `name: skill`".into());
        };
        if stage == START || stage == END {
            return Err(format!("{stage} is implicit and cannot be a stage"));
        }
        load_skill(skill, repo).map_err(|error| format!("stage {stage}: {error}"))?;
        stages.push(WorkflowStage {
            name: stage.to_string(),
            skill: skill.to_string(),
        });
    }
    let workflow = Workflow {
        name: name.to_string(),
        stages,
        edges: authored.edges,
    };
    let known = |node: &str| workflow.stage(node).is_some();
    for edge in &workflow.edges {
        let label = format!("edge {} → {}", edge.from, edge.to);
        if edge.from != START && !known(&edge.from) {
            return Err(format!("{label} leaves an unknown stage"));
        }
        if edge.to != END && !known(&edge.to) {
            return Err(format!("{label} enters an unknown stage"));
        }
        match &edge.flow {
            Some(flow) => {
                let definition =
                    load_flow(flow, repo).map_err(|error| format!("{label}: {error}"))?;
                compile_flow(&definition, repo).map_err(|error| format!("{label}: {error}"))?;
            }
            None if edge.to != END => {
                return Err(format!("{label} names no flow; only an edge into end may"))
            }
            None => {}
        }
        if workflow
            .outgoing(&edge.from)
            .filter(|(_, other)| other.name() == edge.name())
            .count()
            > 1
        {
            return Err(format!(
                "{} has two outgoing edges named {}",
                edge.from,
                edge.name()
            ));
        }
    }
    if workflow.outgoing(START).next().is_none() {
        return Err("no edge leaves start".into());
    }
    let mut reached = BTreeSet::from([START]);
    let mut frontier = vec![START];
    while let Some(node) = frontier.pop() {
        for (_, edge) in workflow.outgoing(node) {
            if reached.insert(edge.to.as_str()) {
                frontier.push(edge.to.as_str());
            }
        }
    }
    if let Some(stage) = workflow
        .stages
        .iter()
        .find(|stage| !reached.contains(stage.name.as_str()))
    {
        return Err(format!("stage {} is unreachable from start", stage.name));
    }
    Ok(workflow)
}

#[cfg(test)]
mod tests {
    use super::{load_workflow, names_workflow, END, START};

    fn write(repo: &std::path::Path, path: &str, content: &str) {
        let path = repo.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn builtin_workflows_join_conversation_stages_with_operational_flows() {
        let repo = tempfile::tempdir().unwrap();
        let feature = load_workflow("feature", repo.path()).unwrap().unwrap();
        let stages: Vec<_> = feature.stages.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(stages, ["design", "demo"]);
        let from_demo: Vec<_> = feature
            .outgoing("demo")
            .map(|(_, edge)| (edge.to.as_str(), edge.flow.as_deref()))
            .collect();
        assert_eq!(from_demo, [("demo", Some("pursue")), (END, Some("ship"))]);
        // A Task with no PR: its last edge runs nothing.
        let research = load_workflow("research", repo.path()).unwrap().unwrap();
        assert!(research
            .edges
            .iter()
            .any(|edge| edge.to == END && edge.flow.is_none()));
        assert!(load_workflow("code", repo.path()).unwrap().is_some());
        assert!(load_workflow("pursue", repo.path()).unwrap().is_none());
        // A repository Flow keeps its name from the builtin workflow.
        write(repo.path(), ".lf/flows/feature.yaml", "- implement\n");
        assert!(load_workflow("feature", repo.path()).unwrap().is_none());
        assert!(!names_workflow("feature", repo.path()));
        assert!(names_workflow("code", repo.path()));
    }

    #[test]
    fn an_authored_workflow_must_be_a_connected_graph_of_known_flows() {
        let repo = tempfile::tempdir().unwrap();
        let load = |content: &str| {
            write(repo.path(), ".lf/workflows/proof.yaml", content);
            load_workflow("proof", repo.path()).map(|workflow| workflow.unwrap())
        };
        let valid = load(
            "stages:\n  review: demo\nedges:\n  - {from: start, to: review, flow: pursue}\n  - {from: review, to: end}\n",
        )
        .unwrap();
        assert_eq!(valid.outgoing(START).count(), 1);
        for (content, expected) in [
            (
                "stages:\n  review: demo\n  lost: demo\nedges:\n  - {from: start, to: review, flow: pursue}\n",
                "unreachable",
            ),
            (
                "edges:\n  - {from: start, to: end, flow: no-such-flow}\n",
                "no-such-flow",
            ),
            (
                "stages:\n  review: demo\nedges:\n  - {from: start, to: review}\n",
                "names no flow",
            ),
            (
                "edges:\n  - {from: start, to: missing, flow: pursue}\n",
                "unknown stage",
            ),
            (
                "stages:\n  review: no-such-skill\nedges:\n  - {from: start, to: review, flow: pursue}\n",
                "no-such-skill",
            ),
            (
                "stages:\n  review: demo\nedges:\n  - {from: start, to: review, flow: pursue}\n  - {from: start, to: end, flow: pursue}\n",
                "two outgoing edges",
            ),
        ] {
            let error = load(content).unwrap_err().to_string();
            assert!(error.contains(expected), "{expected}: {error}");
        }
    }
}
