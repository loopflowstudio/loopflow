//! Display projection of one compiled Graph definition.
//!
//! Surfaces draw the Graph from this shape instead of parsing YAML or inferring
//! topology from skill names. Node keys are preorder IDs, including every
//! sorted XOR alternative. Authored names and runtime cursor paths remain
//! separate; repeated skills keep distinct IDs within their Graph.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::graph::{flatten_resolved, resolve_graph, return_target, ResolvedGraphItem};

use std::path::Path;

use crate::graph::runner::{ExecutionCursor, NestedCursor};
use crate::graph::ConcreteStep;
use crate::task_graph::WorkflowOutline;

pub use loopflow_storage::graph::{
    project_position, CompiledGraph, CompiledGraphPath, GraphNode, GraphNodeKind, GraphReturn,
    PositionProjection,
};

/// Disclosure structure from the same resolution that supplies the execution graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GraphComposition {
    /// Digest of resolved content and composition; unrelated to invocation identity.
    pub revision: String,
    pub items: Vec<GraphCompositionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GraphCompositionItem {
    Node {
        key: u32,
        paths: BTreeMap<String, Vec<GraphCompositionItem>>,
    },
    Group {
        id: String,
        name: String,
        items: Vec<GraphCompositionItem>,
    },
}

fn composition_items(
    items: &[ResolvedGraphItem],
    next: &mut u32,
    group: &mut usize,
) -> Vec<GraphCompositionItem> {
    items
        .iter()
        .map(|item| {
            if let ResolvedGraphItem::Group { name, items } = item {
                let id = format!("group-{}", *group);
                *group += 1;
                return GraphCompositionItem::Group {
                    id,
                    name: name.clone(),
                    items: composition_items(items, next, group),
                };
            }
            let key = *next;
            *next += 1;
            let paths = if let ResolvedGraphItem::Xor { paths, .. } = item {
                paths
                    .iter()
                    .map(|(name, path)| (name.clone(), composition_items(&path.items, next, group)))
                    .collect()
            } else {
                BTreeMap::new()
            };
            GraphCompositionItem::Node { key, paths }
        })
        .collect()
}

/// One autonomous Graph definition, as it would be captured if started now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GraphCatalogEntry {
    pub name: String,
    /// Its `description:` line; `None` when the file has none or does not load.
    pub description: Option<String>,
    /// The repository file that defines it; `None` for a builtin.
    pub source: Option<String>,
    /// Topology; `None` for an unusable definition.
    pub compiled: Option<CompiledGraph>,
    pub composition: Option<GraphComposition>,
    /// The Graph as a Task's workflow line draws it; null exactly when
    /// `compiled` is.
    #[serde(deserialize_with = "Option::deserialize")]
    pub workflow: Option<WorkflowOutline>,
    /// Why the definition is unusable; set exactly when it has no topology.
    pub unavailable: Option<String>,
    /// The Graphs that include this one with `graph:`, by name. Empty for a
    /// Graph people start with; a piece of other Graphs names them here.
    pub included_by: Vec<String>,
    /// What it runs for: a Task, or a Wave's own operation.
    pub runs: GraphSubject,
}

/// What a Graph is started on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum GraphSubject {
    Task,
    Wave,
}

/// Every Graph available in `repo`, each read through the shared
/// loader. A file that does not load stays listed with the reason.
pub fn graph_catalog(repo: &Path) -> Result<Vec<GraphCatalogEntry>, crate::error::LoadError> {
    let mut catalog = crate::graph::available_graph_names(repo)?
        .into_iter()
        .map(|name| load_catalog_entry(&name, repo))
        .collect::<Result<Vec<_>, _>>()?;
    // A compiled node's sources name the Graphs it came through, outermost first.
    let mut included = BTreeMap::<String, std::collections::BTreeSet<String>>::new();
    for entry in &catalog {
        let mut pending: Vec<&GraphNode> = entry
            .compiled
            .iter()
            .flat_map(|compiled| &compiled.steps)
            .collect();
        while let Some(node) = pending.pop() {
            for source in node.sources.iter().filter(|source| **source != entry.name) {
                included
                    .entry(source.clone())
                    .or_default()
                    .insert(entry.name.clone());
            }
            pending.extend(node.paths.iter().flat_map(|path| &path.steps));
        }
    }
    for entry in &mut catalog {
        entry.included_by = included
            .remove(&entry.name)
            .map(|names| names.into_iter().collect())
            .unwrap_or_default();
    }
    Ok(catalog)
}

/// The requested definition as the catalog lists it; an invalid source keeps its diagnostic.
pub fn graph_catalog_entry(
    name: &str,
    repo: &Path,
) -> Result<GraphCatalogEntry, crate::error::LoadError> {
    let entry = load_catalog_entry(name, repo)?;
    Ok(graph_catalog(repo)?
        .into_iter()
        .find(|listed| listed.name == entry.name)
        .unwrap_or(entry))
}

/// One definition alone; who includes it is known only across the catalog.
fn load_catalog_entry(
    name: &str,
    repo: &Path,
) -> Result<GraphCatalogEntry, crate::error::LoadError> {
    let compiled = match crate::graph::load_authored_graph(name, repo) {
        Err(error @ crate::error::LoadError::GraphNotFound(_)) => return Err(error),
        loaded => loaded.and_then(|graph| {
            let resolved = resolve_graph(&graph, repo)?;
            let bytes = serde_json::to_vec(&(&graph.name, &resolved))
                .expect("resolved Graph composition is serializable");
            let composition = GraphComposition {
                revision: hex::encode(Sha256::digest(bytes)),
                items: composition_items(&resolved, &mut 0, &mut 0),
            };
            let steps = flatten_resolved(&resolved);
            Ok((
                crate::graph::compiled::compile(&graph.name, &steps),
                composition,
                WorkflowOutline::of(&steps),
                graph.description,
            ))
        }),
    };
    let (compiled, composition, workflow, description, unavailable) = match compiled {
        Ok((compiled, composition, workflow, description)) => (
            Some(compiled),
            Some(composition),
            Some(workflow),
            description,
            None,
        ),
        Err(error) => (None, None, None, None, Some(error.to_string())),
    };
    let name = compiled
        .as_ref()
        .map_or_else(|| name.to_string(), |compiled| compiled.name.clone());
    let runs = match loopflow_definitions::builtins::builtin_graph_category(&name) {
        Some("Wave") => GraphSubject::Wave,
        _ => GraphSubject::Task,
    };
    Ok(GraphCatalogEntry {
        description,
        source: crate::graph::find_graph_source_path(&name, repo)?.map(|path| {
            path.strip_prefix(repo)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned()
        }),
        compiled,
        composition,
        workflow,
        unavailable,
        included_by: Vec::new(),
        runs,
        name,
    })
}

pub fn compile(name: impl Into<String>, steps: &[ConcreteStep]) -> CompiledGraph {
    CompiledGraph {
        name: name.into(),
        steps: nodes(steps, &mut 0),
    }
}

fn nodes(steps: &[ConcreteStep], next: &mut u32) -> Vec<GraphNode> {
    let mut result: Vec<GraphNode> = Vec::with_capacity(steps.len());
    for (index, step) in steps.iter().enumerate() {
        let key = *next;
        *next = next
            .checked_add(1)
            .expect("captured Graph fits u32 node IDs");
        let node = match step {
            ConcreteStep::Skill(skill) => GraphNode {
                returns_to: return_target(steps, index).map(|target| result[target].key),
                key,
                id: skill.id.clone(),
                label: skill.skill.name.clone(),
                kind: GraphNodeKind::Skill,
                sources: skill.sources.clone(),
                paths: Vec::new(),
            },
            ConcreteStep::Breakpoint(stop) => GraphNode {
                key,
                id: Some(stop.name.clone()),
                label: stop.name.clone(),
                kind: GraphNodeKind::Breakpoint,
                returns_to: return_target(steps, index).map(|target| result[target].key),
                sources: stop.sources.clone(),
                paths: Vec::new(),
            },
            ConcreteStep::Command(op) => GraphNode {
                key,
                id: None,
                label: op.item.display_name(),
                kind: GraphNodeKind::Op,
                returns_to: None,
                sources: op.sources.clone(),
                paths: Vec::new(),
            },
            ConcreteStep::Xor(branch) => {
                let paths = branch
                    .paths
                    .iter()
                    .map(|(name, path)| CompiledGraphPath {
                        name: name.clone(),
                        description: path.description.clone(),
                        steps: nodes(&path.steps, next),
                    })
                    .collect();
                GraphNode {
                    key,
                    id: None,
                    label: branch.router.name.clone(),
                    kind: GraphNodeKind::Xor,
                    returns_to: None,
                    sources: branch.sources.clone(),
                    paths,
                }
            }
        };
        result.push(node);
    }
    result
}

/// Exact occurrence in this compiled graph: its preorder node key, counting
/// every XOR alternative, and the returns taken at each nesting level.
pub fn location(
    steps: &[ConcreteStep],
    cursor: &ExecutionCursor,
) -> anyhow::Result<(u32, Vec<Vec<u32>>)> {
    fn count(steps: &[ConcreteStep]) -> usize {
        steps
            .iter()
            .map(|step| {
                1 + match step {
                    ConcreteStep::Xor(branch) => {
                        branch.paths.values().map(|path| count(&path.steps)).sum()
                    }
                    _ => 0,
                }
            })
            .sum()
    }
    fn locate(steps: &[ConcreteStep], cursor: &ExecutionCursor) -> anyhow::Result<usize> {
        let step = steps
            .get(cursor.index)
            .ok_or_else(|| anyhow::anyhow!("cursor has no captured node"))?;
        let mut index = count(&steps[..cursor.index]);
        if let Some(child) = cursor.child.as_deref() {
            let (ConcreteStep::Xor(branch), NestedCursor::Xor { selected, cursor }) = (step, child)
            else {
                anyhow::bail!("cursor child does not belong to a captured XOR");
            };
            index += 1;
            for (name, path) in &branch.paths {
                if name == selected {
                    return Ok(index + locate(&path.steps, cursor)?);
                }
                index += count(&path.steps);
            }
            anyhow::bail!("cursor selects an uncaptured XOR alternative");
        }
        Ok(index)
    }
    Ok((
        u32::try_from(locate(steps, cursor)?)?,
        graph_iterations(steps, cursor),
    ))
}

/// Ordered backward-edge traversal counts at each active nesting level, outermost
/// first. Empty levels are retained so nested counts never masquerade as root
/// counts. Visit tokens used for step identity are deliberately absent.
pub fn graph_iterations(steps: &[ConcreteStep], cursor: &ExecutionCursor) -> Vec<Vec<u32>> {
    let counts = (0..steps.len())
        .filter(|index| return_target(steps, *index).is_some())
        .map(|index| {
            let traversals = cursor.progress.repeats.get(&index.to_string());
            traversals.copied().unwrap_or(0)
        })
        .collect();
    let mut levels = vec![counts];
    if let (
        Some(ConcreteStep::Xor(branch)),
        Some(NestedCursor::Xor {
            selected,
            cursor: child,
        }),
    ) = (steps.get(cursor.index), cursor.child.as_deref())
    {
        if let Some(path) = branch.paths.get(selected) {
            levels.extend(graph_iterations(&path.steps, child));
        }
    }
    levels
}

/// Recorded return counts (see `graph_iterations`) in words: the pass each
/// returned loop is on, loops numbered in authored order. `None` on a first pass.
pub use loopflow_storage::graph::loop_passes;

#[cfg(test)]
mod tests {
    #[test]
    fn a_catalog_entry_outlines_the_workflow_a_task_would_show() {
        use super::graph_catalog;
        use crate::task_graph::{CapturedGraph, TaskGraph, WorkflowStepKind};
        let repo = tempfile::tempdir().unwrap();
        let graphs = repo.path().join(".lf/graphs");
        std::fs::create_dir_all(&graphs).unwrap();
        // Its only stop is inside an XOR path.
        std::fs::write(
            graphs.join("conditional.yaml"),
            "- implement\n- xor:\n    paths:\n      review:\n        description: Review it\n        steps:\n          - interactive: demo\n      skip:\n        description: Nothing to review\n- qa\n",
        )
        .unwrap();
        std::fs::write(graphs.join("alone.yaml"), "- implement\n- qa\n").unwrap();
        std::fs::write(graphs.join("broken.yaml"), "- graph: no-such-graph\n").unwrap();
        let catalog = graph_catalog(repo.path()).unwrap();
        let outline = |name: &str| {
            let entry = catalog.iter().find(|entry| entry.name == name).unwrap();
            assert_eq!(entry.workflow.is_some(), entry.compiled.is_some());
            entry.workflow.clone()
        };

        // A composed Graph reads as the Graphs it includes.
        let code = outline("code").unwrap();
        let steps: Vec<_> = code
            .steps
            .iter()
            .map(|step| (step.name.as_str(), step.kind))
            .collect();
        assert_eq!(
            steps,
            [
                ("plan", WorkflowStepKind::Stage),
                ("build", WorkflowStepKind::Stage),
                ("ship-review", WorkflowStepKind::Stage),
                ("after-ship-review", WorkflowStepKind::Breakpoint),
                ("deliver", WorkflowStepKind::Stage),
            ]
        );
        assert_eq!(code.breakpoints, ["after-ship-review"]);
        assert_eq!(code.sentence, "You take part at after-ship-review.");
        assert_eq!(
            outline("feature").unwrap().breakpoints,
            ["after-explore", "after-ship-review"]
        );
        assert_eq!(
            outline("feature").unwrap().sentence,
            "You take part at after-explore and after-ship-review."
        );

        let conditional = outline("conditional").unwrap();
        let stops: Vec<_> = conditional
            .steps
            .iter()
            .filter(|step| step.kind == WorkflowStepKind::StageWithBreakpoint)
            .map(|step| step.name.as_str())
            .collect();
        assert_eq!(stops, ["xor-route"]);
        assert_eq!(conditional.breakpoints, ["after-demo"]);
        assert_eq!(conditional.sentence, "You take part at after-demo.");

        assert_eq!(outline("alone").unwrap().sentence, "Runs on its own.");
        assert_eq!(outline("broken"), None);

        // A Task that takes the Graph up reads the same steps and stops.
        for name in ["code", "conditional"] {
            let captured = CapturedGraph::load(name, repo.path()).unwrap().unwrap();
            let task =
                TaskGraph::read(&captured, "start".into(), None, None, None, None, |_| false);
            let outline = outline(name).unwrap();
            assert_eq!(task.steps, outline.steps, "{name}");
            assert_eq!(task.breakpoints, outline.breakpoints, "{name}");
        }
    }

    #[test]
    fn template_resolution_preserves_composition_and_execution() {
        use super::{graph_catalog, GraphCompositionItem};
        use crate::graph::{compile_graph, load_graph};
        let repo = tempfile::tempdir().unwrap();
        let graphs = repo.path().join(".lf/graphs");
        let skills = repo.path().join(".lf/skills");
        std::fs::create_dir_all(&graphs).unwrap();
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::write(skills.join("sample.md"), "First content").unwrap();
        std::fs::write(graphs.join("piece.yaml"), "- sample\n").unwrap();
        std::fs::write(graphs.join("empty.yaml"), "[]\n").unwrap();
        std::fs::write(
            graphs.join("study.yaml"),
            r#"
- graph: piece
- graph: piece
- graph: empty
- xor:
    paths:
      fix:
        description: Fix it
        graph: piece
      pass:
        description: Continue
        steps: []
- loop:
    - loop:
        - step: {name: sample, id: start}
        - loop-or-next
    - demo
    - loop-or-next
"#,
        )
        .unwrap();
        let entry = graph_catalog(repo.path())
            .unwrap()
            .into_iter()
            .find(|e| e.name == "study")
            .unwrap();
        assert!(entry.unavailable.is_none(), "{:?}", entry.unavailable);
        let composition = entry.composition.unwrap();
        let expanded =
            compile_graph(&load_graph("study", repo.path()).unwrap(), repo.path()).unwrap();
        let compiled = entry.compiled.unwrap();
        assert_eq!(
            compiled,
            crate::graph::compiled::compile("study", &expanded)
        );
        assert_eq!(
            compiled
                .steps
                .iter()
                .filter(|n| n.returns_to.is_some())
                .count(),
            2
        );
        let [GraphCompositionItem::Group { id: first, .. }, GraphCompositionItem::Group { id: second, .. }, GraphCompositionItem::Group { items: empty, .. }, GraphCompositionItem::Node { paths, .. }, ..] =
            composition.items.as_slice()
        else {
            panic!("expected distinct composition uses and XOR");
        };
        assert_ne!(first, second);
        assert!(empty.is_empty());
        assert_eq!(paths.len(), 2);
        let GraphCompositionItem::Group { items, .. } = &paths["fix"][0] else {
            panic!("expected nested group");
        };
        assert!(matches!(&items[0], GraphCompositionItem::Node { key, .. } if *key == 3));
        assert!(paths["pass"].is_empty());
        let reread = graph_catalog(repo.path())
            .unwrap()
            .into_iter()
            .find(|e| e.name == "study")
            .unwrap();
        assert_eq!(
            reread.composition.as_ref().unwrap().revision,
            composition.revision
        );
        std::fs::write(skills.join("sample.md"), "Changed content").unwrap();
        let changed = graph_catalog(repo.path())
            .unwrap()
            .into_iter()
            .find(|e| e.name == "study")
            .unwrap();
        assert_ne!(changed.composition.unwrap().revision, composition.revision);
        std::fs::write(graphs.join("piece.yaml"), "- graph: study\n").unwrap();
        let cyclic = graph_catalog(repo.path())
            .unwrap()
            .into_iter()
            .find(|e| e.name == "study")
            .unwrap();
        assert!(cyclic.compiled.is_none() && cyclic.composition.is_none());
        assert!(cyclic.unavailable.unwrap().contains("cycle"));
        std::fs::remove_file(graphs.join("piece.yaml")).unwrap();
        let missing = graph_catalog(repo.path())
            .unwrap()
            .into_iter()
            .find(|e| e.name == "study")
            .unwrap();
        assert!(missing.compiled.is_none() && missing.composition.is_none());
        assert!(missing.unavailable.is_some());
    }

    use std::collections::BTreeMap;

    use crate::graph::compile_graph;
    use crate::graph::compiled::{
        graph_iterations, project_position, GraphNodeKind, PositionProjection,
    };
    use crate::graph::load_graph;
    use crate::graph::runner::{ExecutionCursor, NestedCursor};
    use crate::graph::{ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor, Skill};

    /// The projection of the step `cursor` selects, as its Graph process records it.
    fn project(steps: &[ConcreteStep], cursor: &ExecutionCursor) -> PositionProjection {
        let (key, iterations) = super::location(steps, cursor).unwrap();
        project_position(
            &crate::graph::compiled::compile("", steps),
            key,
            &iterations,
            false,
        )
    }

    fn skill(
        name: &str,
        id: Option<&str>,
        interactive: bool,
        returns: Option<usize>,
    ) -> ConcreteStep {
        ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named(name),
            id: id.map(str::to_string),
            interactive,
            interactive_name: None,
            returns,
            sources: Vec::new(),
        })
    }

    #[test]
    fn retained_graph_nodes_ignore_retired_per_skill_flags_and_project_breakpoints() {
        for legacy in [
            r#"{"key":0,"id":"review","label":"demo","kind":"skill","human":true,
                "returns_to":null,"sources":["code"],"paths":[]}"#,
            r#"{"key":0,"id":"review","label":"demo","kind":"skill","interactive":true,
                "returns_to":null,"sources":["code"],"paths":[]}"#,
        ] {
            let node: super::GraphNode = serde_json::from_str(legacy).unwrap();
            assert_eq!(node.kind, GraphNodeKind::Skill);
            let current = serde_json::to_value(&node).unwrap();
            assert!(current.get("interactive").is_none());
            assert!(current.get("human").is_none());
            assert_eq!(
                serde_json::from_value::<super::GraphNode>(current).unwrap(),
                node
            );
        }

        let repo = tempfile::tempdir().unwrap();
        let graph = load_graph("research", repo.path()).unwrap();
        let compiled = crate::graph::compiled::compile(
            &graph.name,
            &compile_graph(&graph, repo.path()).unwrap(),
        );
        assert_eq!(compiled.steps.len(), 2);
        assert_eq!(compiled.steps[0].kind, GraphNodeKind::Skill);
        assert_eq!(compiled.steps[1].kind, GraphNodeKind::Breakpoint);
        assert_eq!(compiled.steps[1].label, "after-research");
        assert_eq!(compiled.steps[1].returns_to, Some(compiled.steps[0].key));
    }

    fn compile_builtin(repo: &std::path::Path, name: &str) -> super::CompiledGraph {
        let graph = load_graph(name, repo).unwrap();
        crate::graph::compiled::compile(&graph.name, &compile_graph(&graph, repo).unwrap())
    }

    #[test]
    fn builtin_graphs_keep_their_delivery_boundaries_without_breakpoints() {
        let repo = tempfile::tempdir().unwrap();
        type Case<'a> = (&'a str, &'a [&'a str], &'a [(usize, usize)]);
        let cases: &[Case<'_>] = &[
            ("refresh", &["sync", "realign"], &[]),
            ("task-design", &["kickoff"], &[]),
            (
                "incident",
                &["unbreak", "debug", "5whys", "launch-plan"],
                &[],
            ),
            (
                "pursue",
                &[
                    "kickoff",
                    "concept-review",
                    "realign",
                    "write-docs",
                    "sync",
                    "implement",
                    "concept-review",
                    "realign",
                    "compress",
                    "qa",
                    "pr-publish",
                ],
                &[(5, 4), (7, 4)],
            ),
            (
                "quick",
                &["sync", "implement", "qa", "land --wait-and-fix"],
                &[(1, 0)],
            ),
            ("deliver", &["reflect", "land --wait-and-fix"], &[]),
            ("vsm-operate", &["s1", "s2", "s3", "s4", "s5"], &[]),
        ];
        for (name, labels, returns) in cases {
            let compiled = compile_builtin(repo.path(), name);
            assert_eq!(
                compiled
                    .steps
                    .iter()
                    .map(|s| s.label.as_str())
                    .collect::<Vec<_>>(),
                *labels,
                "{name}"
            );
            assert!(
                compiled
                    .steps
                    .iter()
                    .all(|s| s.kind != GraphNodeKind::Breakpoint),
                "{name} should have no interactive stops"
            );
            let actual_returns: Vec<_> = compiled
                .steps
                .iter()
                .filter_map(|s| s.returns_to.map(|to| (s.key, to)))
                .collect();
            let expected_returns: Vec<_> = returns
                .iter()
                .map(|&(from, to)| (compiled.steps[from].key, compiled.steps[to].key))
                .collect();
            assert_eq!(actual_returns, expected_returns, "{name}");
        }
    }

    #[test]
    fn refresh_integrates_upstream_before_realigning() {
        let repo = tempfile::tempdir().unwrap();
        let compiled = compile_builtin(repo.path(), "refresh");
        assert_eq!(compiled.steps[0].kind, GraphNodeKind::Op);
        assert_eq!(compiled.steps[1].kind, GraphNodeKind::Skill);
    }

    #[test]
    fn pursue_runs_no_landing_ops_and_tracks_included_sources() {
        let repo = tempfile::tempdir().unwrap();
        let compiled = compile_builtin(repo.path(), "pursue");
        // Review and landing are the conversation's to launch, not this Graph's.
        assert!(compiled
            .steps
            .iter()
            .all(|node| node.kind != GraphNodeKind::Op || node.label == "sync"));
        assert_eq!(compiled.steps[4].sources, ["pursue", "build"]);
    }

    #[test]
    fn the_catalog_names_who_includes_a_graph_and_what_it_runs_for() {
        let repo = tempfile::tempdir().unwrap();
        let catalog = super::graph_catalog(repo.path()).unwrap();
        let entry = |name: &str| catalog.iter().find(|entry| entry.name == name).unwrap();
        // People start a Task on code; build is a piece of the Graphs that include it.
        assert!(entry("code").included_by.is_empty());
        let includes_build = &entry("build").included_by;
        assert!(["code", "feature", "pursue"]
            .iter()
            .all(|name| includes_build.contains(&name.to_string())));
        assert!(!includes_build.contains(&"build".to_string()));
        assert_eq!(entry("code").runs, super::GraphSubject::Task);
        assert_eq!(entry("vsm-operate").runs, super::GraphSubject::Wave);
        assert_eq!(
            super::graph_catalog_entry("build", repo.path()).unwrap(),
            *entry("build")
        );
    }

    #[test]
    fn repeated_decisions_keep_independent_counts_and_a_pass_scoped_completion() {
        let steps = vec![
            skill("design", Some("design"), false, None),
            skill("implement", Some("implement"), false, None),
            skill("loop-or-next", None, false, Some(1)),
            skill("demo", Some("demo"), true, None),
            skill("loop-or-next", None, false, Some(3)),
            skill("land", None, false, None),
        ];
        let cursor = ExecutionCursor {
            index: 1,
            iteration: 3,
            progress: crate::graph::transitions::GraphProgress {
                repeats: BTreeMap::from([("2".into(), 2), ("4".into(), 1)]),
                ..Default::default()
            },
            ..Default::default()
        };
        let projection = project(&steps, &cursor);
        assert_eq!(projection.current, Some(1));
        // Only the opening step is complete; earlier passes' decisions are not.
        assert_eq!(projection.completed, [0]);
        let counts: Vec<_> = projection
            .returns
            .iter()
            .map(|edge| (edge.decider, edge.traversals))
            .collect();
        assert_eq!(counts, [(2, 2), (4, 1)]);
        assert_eq!(graph_iterations(&steps, &cursor), [vec![2, 1]]);
        // Moving beyond one span does not collapse its independent count.
        let later = ExecutionCursor {
            index: 3,
            iteration: 99,
            ..cursor
        };
        assert_eq!(graph_iterations(&steps, &later), [vec![2, 1]]);
    }

    #[test]
    fn a_selected_xor_path_is_drawn_honestly_with_captured_ids() {
        let branch = ConcreteXor {
            router: Skill::named("xor-route"),
            paths: BTreeMap::from([
                (
                    "fix".to_string(),
                    ConcretePath {
                        description: "Fix it".into(),
                        steps: vec![
                            skill("patch", Some("patch"), false, None),
                            skill("check", Some("check"), false, Some(1)),
                        ],
                    },
                ),
                (
                    "skip".to_string(),
                    ConcretePath {
                        description: "Nothing to do".into(),
                        steps: Vec::new(),
                    },
                ),
            ]),
            sources: Vec::new(),
        };
        let steps = vec![
            skill("kickoff", None, false, None),
            ConcreteStep::Xor(branch),
        ];
        let compiled = crate::graph::compiled::compile("routed", &steps);
        let xor = &compiled.steps[1];
        assert_eq!(xor.kind, GraphNodeKind::Xor);
        assert_eq!(
            xor.paths
                .iter()
                .map(|path| path.name.as_str())
                .collect::<Vec<_>>(),
            ["fix", "skip"]
        );
        assert_eq!(xor.paths[0].steps[1].key, 3);
        assert_eq!(xor.paths[0].steps[1].returns_to, Some(2));

        let cursor = ExecutionCursor {
            index: 1,
            progress: crate::graph::transitions::GraphProgress {
                repeats: BTreeMap::from([("xor:1:fix/1".into(), 4)]),
                ..Default::default()
            },
            child: Some(Box::new(NestedCursor::Xor {
                selected: "fix".into(),
                cursor: ExecutionCursor {
                    index: 1,
                    iteration: 2,
                    progress: crate::graph::transitions::GraphProgress {
                        repeats: BTreeMap::from([("1".into(), 2)]),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            })),
            ..Default::default()
        };
        let projection = project(&steps, &cursor);
        assert_eq!(projection.current, Some(3));
        assert_eq!(projection.completed, [0, 1, 2]);
        // The active child's own count wins over an older settled visit.
        assert_eq!(projection.returns[0].traversals, 2);
        assert_eq!(graph_iterations(&steps, &cursor), [vec![], vec![2]]);
        assert_eq!(
            super::loop_passes(&[vec![], vec![2]]).as_deref(),
            Some("pass 3")
        );
        assert_eq!(
            super::loop_passes(&[vec![2, 0], vec![1]]).as_deref(),
            Some("loop 1 pass 3, loop 3 pass 2")
        );
        assert_eq!(super::loop_passes(&[vec![0, 0]]), None);
    }

    #[test]
    fn numeric_wire_matches_captured_ids_across_nested_alternatives() {
        use crate::durable::GraphProcessDetail;

        fn branch(paths: Vec<(&str, Vec<ConcreteStep>)>) -> ConcreteStep {
            ConcreteStep::Xor(ConcreteXor {
                router: Skill::named("route"),
                paths: paths
                    .into_iter()
                    .map(|(name, steps)| {
                        (
                            name.into(),
                            ConcretePath {
                                description: name.into(),
                                steps,
                            },
                        )
                    })
                    .collect(),
                sources: Vec::new(),
            })
        }
        fn begin() -> ConcreteStep {
            skill("patch", Some("begin"), false, None)
        }
        fn check(returns: usize) -> ConcreteStep {
            skill("check", Some("check"), false, Some(returns))
        }
        let steps = vec![
            begin(),
            branch(vec![
                ("zeta", vec![begin(), check(1)]),
                (
                    "alpha",
                    vec![
                        begin(),
                        branch(vec![("fix", vec![begin(), check(1)])]),
                        check(2),
                    ],
                ),
            ]),
            check(2),
        ];
        let pinned: GraphProcessDetail = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/graph_numeric_nested.json"
        ))
        .unwrap();
        let compiled = crate::graph::compiled::compile("nested", &steps);
        assert_eq!(compiled, pinned.compiled);
        // Root 0, XOR 1, alpha 2/3/(fix 4/5)/6, zeta 7/8, root 9.
        // Compare every cursor with the existing storage identity algorithm.
        fn cursors(steps: &[ConcreteStep]) -> Vec<ExecutionCursor> {
            let mut result = Vec::new();
            for (index, step) in steps.iter().enumerate() {
                result.push(ExecutionCursor {
                    index,
                    ..Default::default()
                });
                if let ConcreteStep::Xor(branch) = step {
                    for (selected, path) in &branch.paths {
                        for child in cursors(&path.steps) {
                            result.push(ExecutionCursor {
                                index,
                                child: Some(Box::new(NestedCursor::Xor {
                                    selected: selected.clone(),
                                    cursor: child,
                                })),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
            result
        }
        let mut ids = Vec::new();
        for cursor in cursors(&steps) {
            let id = super::location(&steps, &cursor).unwrap().0;
            ids.push(id);
            assert_eq!(project(&steps, &cursor).current, Some(id));
            assert_eq!(compiled.node_at(id).unwrap().key, id);
        }
        ids.sort();
        assert_eq!(ids, (0..10).collect::<Vec<_>>());
        let cursor = ExecutionCursor {
            index: 1,
            progress: crate::graph::transitions::GraphProgress {
                repeats: BTreeMap::from([("2".into(), 3)]),
                ..Default::default()
            },
            child: Some(Box::new(NestedCursor::Xor {
                selected: "alpha".into(),
                cursor: ExecutionCursor {
                    index: 1,
                    child: Some(Box::new(NestedCursor::Xor {
                        selected: "fix".into(),
                        cursor: ExecutionCursor {
                            index: 1,
                            progress: crate::graph::transitions::GraphProgress {
                                repeats: BTreeMap::from([("1".into(), 2)]),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    })),
                    ..Default::default()
                },
            })),
            ..Default::default()
        };
        let projection = project(&steps, &cursor);
        assert_eq!(projection.current, pinned.current);
        assert_eq!(projection.completed, pinned.completed);
        assert_eq!(projection.returns, pinned.returns);
        assert_eq!(graph_iterations(&steps, &cursor), pinned.iterations);
        // A step outside the alternatives reports their deciders untraversed.
        let last = ExecutionCursor {
            index: 2,
            ..Default::default()
        };
        let projection = project(&steps, &last);
        assert_eq!(projection.current, Some(9));
        assert_eq!(projection.completed, [0, 1]);
        assert_eq!(
            projection
                .returns
                .iter()
                .map(|r| (r.decider, r.traversals))
                .collect::<Vec<_>>(),
            [(5, 0), (6, 0), (8, 0), (9, 0)]
        );
        let finished = project_position(&compiled, 9, &[vec![0]], true);
        assert_eq!(finished.current, None);
        assert_eq!(finished.completed, [0, 1, 9]);
    }
}
