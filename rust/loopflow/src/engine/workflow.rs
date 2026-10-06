//! A workflow definition: the nodes where a person takes part in the Task
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
pub struct WorkflowNode {
    pub name: String,
    /// The skill the Task conversation uses at this node.
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
pub struct WorkflowDefinition {
    pub name: String,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
}

impl WorkflowEdge {
    /// What `lf task run ISSUE <name>` calls this edge: its Flow, or the node
    /// it enters when it runs none.
    pub fn name(&self) -> &str {
        self.flow.as_deref().unwrap_or(&self.to)
    }
}

impl WorkflowDefinition {
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

    pub fn node(&self, name: &str) -> Option<&WorkflowNode> {
        self.nodes.iter().find(|node| node.name == name)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredWorkflow {
    #[serde(default)]
    nodes: serde_yaml_ng::Mapping,
    edges: Vec<WorkflowEdge>,
}

/// The repository file that defines workflow `name`.
pub fn workflow_path(name: &str, repo: &Path) -> Option<PathBuf> {
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

/// Every workflow `repo` can name: its own files, then builtins it has not
/// given to a Flow.
pub fn available_workflow_names(repo: &Path) -> Vec<String> {
    let mut names: BTreeSet<String> = BUILTIN_WORKFLOWS
        .iter()
        .filter(|(name, _)| builtin_workflow(name, repo).is_some())
        .map(|(name, _)| name.to_string())
        .collect();
    for entry in fs::read_dir(repo.join(".lf/workflows"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let path = entry.path();
        let authored = matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("yaml" | "yml")
        );
        if let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) {
            if authored && path.is_file() {
                names.insert(name.to_string());
            }
        }
    }
    names.into_iter().collect()
}

/// The repository file to edit for `name`, written from the builtin when the
/// repository has none. A workflow wins over a Flow of the same name, as it
/// does for `lf task run`.
pub fn customize(name: &str, repo: &Path) -> Result<PathBuf, LoadError> {
    if let Some(path) = workflow_path(name, repo) {
        return Ok(path);
    }
    let (path, content) = if let Some(content) = builtin_workflow(name, repo) {
        (format!(".lf/workflows/{name}.yaml"), content)
    } else if let Some(path) = find_flow_source_path(name, repo) {
        return Ok(path);
    } else if let Some(content) = super::builtins::get_builtin_flow(name) {
        (format!(".lf/flows/{name}.yaml"), content)
    } else {
        return Err(LoadError::FlowNotFound(name.to_string()));
    };
    let path = repo.join(path);
    fs::create_dir_all(path.parent().expect("a definition sits in a directory"))?;
    fs::write(&path, content)?;
    Ok(path)
}

pub fn names_workflow(name: &str, repo: &Path) -> bool {
    workflow_path(name, repo).is_some() || builtin_workflow(name, repo).is_some()
}

/// Load `name` when it names a workflow. A repository's own file wins; a
/// repository Flow of the same name keeps the name from a builtin workflow.
pub fn load_workflow(name: &str, repo: &Path) -> Result<Option<WorkflowDefinition>, LoadError> {
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

fn parse_workflow(name: &str, content: &str, repo: &Path) -> Result<WorkflowDefinition, String> {
    let authored: AuthoredWorkflow =
        serde_yaml_ng::from_str(content).map_err(|error| error.to_string())?;
    let mut nodes = Vec::new();
    for (node, skill) in &authored.nodes {
        let (Some(node), Some(skill)) = (node.as_str(), skill.as_str()) else {
            return Err("each node is `name: skill`".into());
        };
        if node == START || node == END {
            return Err(format!("{node} is implicit and cannot be a node"));
        }
        load_skill(skill, repo).map_err(|error| format!("node {node}: {error}"))?;
        nodes.push(WorkflowNode {
            name: node.to_string(),
            skill: skill.to_string(),
        });
    }
    let workflow = WorkflowDefinition {
        name: name.to_string(),
        nodes,
        edges: authored.edges,
    };
    let known = |node: &str| workflow.node(node).is_some();
    for edge in &workflow.edges {
        let label = format!("edge {} → {}", edge.from, edge.to);
        if edge.from != START && !known(&edge.from) {
            return Err(format!("{label} leaves an unknown node"));
        }
        if edge.to != END && !known(&edge.to) {
            return Err(format!("{label} enters an unknown node"));
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
    if let Some(node) = workflow
        .nodes
        .iter()
        .find(|node| !reached.contains(node.name.as_str()))
    {
        return Err(format!("node {} is unreachable from start", node.name));
    }
    Ok(workflow)
}

#[cfg(test)]
mod tests {
    use super::{customize, load_workflow, names_workflow, END, START};
    use crate::engine::flow_graph::{flow_catalog, CatalogKind};

    fn write(repo: &std::path::Path, path: &str, content: &str) {
        let path = repo.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn builtin_workflows_join_conversation_nodes_with_operational_flows() {
        let repo = tempfile::tempdir().unwrap();
        let feature = load_workflow("feature", repo.path()).unwrap().unwrap();
        let nodes: Vec<_> = feature.nodes.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(nodes, ["design", "demo"]);
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
            "nodes:\n  review: demo\nedges:\n  - {from: start, to: review, flow: pursue}\n  - {from: review, to: end}\n",
        )
        .unwrap();
        assert_eq!(valid.outgoing(START).count(), 1);
        for (content, expected) in [
            (
                "nodes:\n  review: demo\n  lost: demo\nedges:\n  - {from: start, to: review, flow: pursue}\n",
                "unreachable",
            ),
            (
                "edges:\n  - {from: start, to: end, flow: no-such-flow}\n",
                "no-such-flow",
            ),
            (
                "nodes:\n  review: demo\nedges:\n  - {from: start, to: review}\n",
                "names no flow",
            ),
            (
                "edges:\n  - {from: start, to: missing, flow: pursue}\n",
                "unknown node",
            ),
            (
                "nodes:\n  review: no-such-skill\nedges:\n  - {from: start, to: review, flow: pursue}\n",
                "no-such-skill",
            ),
            (
                "nodes:\n  review: demo\nedges:\n  - {from: start, to: review, flow: pursue}\n  - {from: start, to: end, flow: pursue}\n",
                "two outgoing edges",
            ),
        ] {
            let error = load(content).unwrap_err().to_string();
            assert!(error.contains(expected), "{expected}: {error}");
        }
    }

    #[test]
    fn a_customized_builtin_is_a_repository_file_that_stays_listed_when_invalid() {
        let repo = tempfile::tempdir().unwrap();
        let entry = |name: &str, kind: CatalogKind| {
            flow_catalog(repo.path())
                .into_iter()
                .find(|entry| entry.name == name && entry.kind == kind)
        };
        let builtin = entry("feature", CatalogKind::Workflow).unwrap();
        assert_eq!(builtin.source, None);
        assert_eq!(builtin.workflow.unwrap().nodes.len(), 2);
        // Listing creates nothing; customizing writes the builtin once.
        assert!(!repo.path().join(".lf").exists());
        let path = customize("feature", repo.path()).unwrap();
        assert_eq!(path, repo.path().join(".lf/workflows/feature.yaml"));
        std::fs::write(
            &path,
            "edges:\n  - {from: start, to: end, flow: no-such-flow}\n",
        )
        .unwrap();
        assert_eq!(customize("feature", repo.path()).unwrap(), path);
        let invalid = entry("feature", CatalogKind::Workflow).unwrap();
        assert_eq!(
            invalid.source.as_deref(),
            Some(".lf/workflows/feature.yaml")
        );
        assert!(invalid.workflow.is_none());
        assert!(invalid.unavailable.unwrap().contains("no-such-flow"));
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("no-such-flow"));

        let flow = customize("pursue", repo.path()).unwrap();
        assert_eq!(flow, repo.path().join(".lf/flows/pursue.yaml"));
        let pursue = entry("pursue", CatalogKind::Flow).unwrap();
        assert_eq!(pursue.source.as_deref(), Some(".lf/flows/pursue.yaml"));
        assert!(pursue.graph.is_some());
        assert!(customize("no-such-definition", repo.path()).is_err());
    }
}
