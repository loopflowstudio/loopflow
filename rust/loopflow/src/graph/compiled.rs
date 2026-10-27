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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledGraph {
    pub name: String,
    pub steps: Vec<GraphNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNode {
    pub key: u32,
    /// Authored occurrence id, when the definition names one.
    pub id: Option<String>,
    /// Literal skill name, operation, or XOR router.
    pub label: String,
    pub kind: GraphNodeKind,
    // Retained Graph records predate the public field rename.
    #[serde(alias = "human")]
    pub interactive: bool,
    /// Key of the earlier node this deciding occurrence can return to.
    pub returns_to: Option<u32>,
    /// Composed Graphs this occurrence was compiled from, outermost first.
    pub sources: Vec<String>,
    /// XOR alternatives, sorted by name; empty for other kinds.
    pub paths: Vec<CompiledGraphPath>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphNodeKind {
    Skill,
    Op,
    Xor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledGraphPath {
    pub name: String,
    pub description: String,
    pub steps: Vec<GraphNode>,
}

/// Execution evidence for one backward edge in the pinned invocation. The edge
/// itself is the deciding node's `returns_to`; this carries only its counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphReturn {
    /// Key of the deciding occurrence that owns the edge.
    pub decider: u32,
    pub traversals: u32,
}

/// Disclosure structure from the same resolution that supplies the execution graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphComposition {
    /// Digest of resolved content and composition; unrelated to invocation identity.
    pub revision: String,
    pub items: Vec<GraphCompositionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphCatalogEntry {
    pub name: String,
    /// The repository file that defines it; `None` for a builtin.
    pub source: Option<String>,
    /// Topology; `None` for an unusable definition.
    pub compiled: Option<CompiledGraph>,
    pub composition: Option<GraphComposition>,
    /// Why the definition is unusable; set exactly when it has no topology.
    pub unavailable: Option<String>,
}

/// Every Graph available in `repo`, each read through the shared
/// loader. A file that does not load stays listed with the reason.
pub fn graph_catalog(repo: &Path) -> Result<Vec<GraphCatalogEntry>, crate::error::LoadError> {
    crate::graph::available_graph_names(repo)?
        .into_iter()
        .map(|name| graph_catalog_entry(&name, repo))
        .collect()
}

/// Read only the requested definition; invalid sources retain their diagnostic.
pub fn graph_catalog_entry(
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
            Ok((
                CompiledGraph::new(&graph.name, &flatten_resolved(&resolved)),
                composition,
            ))
        }),
    };
    let (compiled, composition, unavailable) = match compiled {
        Ok((compiled, composition)) => (Some(compiled), Some(composition), None),
        Err(error) => (None, None, Some(error.to_string())),
    };
    Ok(GraphCatalogEntry {
        name: compiled
            .as_ref()
            .map_or_else(|| name.to_string(), |compiled| compiled.name.clone()),
        source: crate::graph::find_graph_source_path(name, repo)?.map(|path| {
            path.strip_prefix(repo)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned()
        }),
        compiled,
        composition,
        unavailable,
    })
}

impl CompiledGraph {
    pub fn new(name: impl Into<String>, steps: &[ConcreteStep]) -> Self {
        Self {
            name: name.into(),
            steps: nodes(steps, &mut 0),
        }
    }

    /// Look up a captured node, including nodes inside unselected alternatives.
    pub(crate) fn node_at(&self, id: u32) -> Option<&GraphNode> {
        fn find(nodes: &[GraphNode], id: u32) -> Option<&GraphNode> {
            nodes.iter().find_map(|node| {
                if node.key == id {
                    Some(node)
                } else {
                    node.paths.iter().find_map(|path| find(&path.steps, id))
                }
            })
        }
        find(&self.steps, id)
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
                interactive: skill.interactive,
                sources: skill.sources.clone(),
                paths: Vec::new(),
            },
            ConcreteStep::Command(op) => GraphNode {
                key,
                id: None,
                label: op.item.display_name(),
                kind: GraphNodeKind::Op,
                interactive: false,
                returns_to: None,
                sources: op.sources.clone(),
                paths: Vec::new(),
            },
            ConcreteStep::Xor(branch) => {
                let mut names: Vec<_> = branch.paths.keys().collect();
                names.sort();
                let paths = names
                    .into_iter()
                    .map(|name| {
                        let path = &branch.paths[name];
                        CompiledGraphPath {
                            name: name.clone(),
                            description: path.description.clone(),
                            steps: nodes(&path.steps, next),
                        }
                    })
                    .collect();
                GraphNode {
                    key,
                    id: None,
                    label: branch.router.name.clone(),
                    kind: GraphNodeKind::Xor,
                    interactive: false,
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
pub(crate) fn location(
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
            let mut paths: Vec<_> = branch.paths.iter().collect();
            paths.sort_by_key(|(name, _)| *name);
            index += 1;
            for (name, path) in paths {
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
pub fn loop_passes(iterations: &[Vec<u32>]) -> Option<String> {
    let counts: Vec<u32> = iterations.iter().flatten().copied().collect();
    let passes: Vec<String> = counts
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 0)
        .map(|(index, count)| {
            let pass = u64::from(*count) + 1;
            if counts.len() == 1 {
                format!("pass {pass}")
            } else {
                format!("loop {} pass {pass}", index + 1)
            }
        })
        .collect();
    (!passes.is_empty()).then(|| passes.join(", "))
}

/// Where a recorded step stands inside its compiled Graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionProjection {
    /// Key of the current occurrence; `None` once the Graph finished.
    pub current: Option<u32>,
    /// Occurrences already finished in the current pass, including the router of
    /// a selected XOR. Earlier passes' completions are not carried forward.
    pub completed: Vec<u32>,
    pub returns: Vec<GraphReturn>,
}

/// Project the step at `key`, which recorded `iterations` (see
/// `graph_iterations`), onto `graph`. Deciders outside the step's own path
/// report no traversals.
pub fn project_position(
    compiled: &CompiledGraph,
    key: u32,
    iterations: &[Vec<u32>],
    finished: bool,
) -> PositionProjection {
    fn contains(node: &GraphNode, key: u32) -> bool {
        node.key == key
            || node
                .paths
                .iter()
                .any(|path| path.steps.iter().any(|node| contains(node, key)))
    }
    fn walk(
        nodes: &[GraphNode],
        key: u32,
        levels: &[Vec<u32>],
        active: bool,
        projection: &mut PositionProjection,
    ) {
        let counts = levels.first().filter(|_| active);
        let on_path = nodes.iter().position(|node| active && contains(node, key));
        let mut decider = 0;
        for (index, node) in nodes.iter().enumerate() {
            if on_path.is_some_and(|current| index < current) {
                projection.completed.push(node.key);
            }
            if node.returns_to.is_some() {
                projection.returns.push(GraphReturn {
                    decider: node.key,
                    traversals: counts
                        .and_then(|counts| counts.get(decider))
                        .copied()
                        .unwrap_or(0),
                });
                decider += 1;
            }
            let here = on_path == Some(index);
            if here && node.key == key {
                projection.current = Some(key);
            } else if here {
                projection.completed.push(node.key);
            }
            for path in &node.paths {
                let selected = here && path.steps.iter().any(|node| contains(node, key));
                walk(
                    &path.steps,
                    key,
                    levels.get(1..).unwrap_or_default(),
                    selected,
                    projection,
                );
            }
        }
    }
    let mut projection = PositionProjection {
        current: None,
        completed: Vec::new(),
        returns: Vec::new(),
    };
    walk(&compiled.steps, key, iterations, true, &mut projection);
    if finished {
        projection.current = None;
        projection.completed = compiled.steps.iter().map(|node| node.key).collect();
    }
    projection
}

#[cfg(test)]
mod tests {
    #[test]
    fn template_resolution_preserves_composition_and_execution() {
        use super::{graph_catalog, CompiledGraph, GraphCompositionItem};
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
- step: {name: sample, id: start}
- loop: start
- demo
- loop: start
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
        assert_eq!(compiled, CompiledGraph::new("study", &expanded));
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

    use std::collections::{BTreeMap, HashMap};

    use crate::graph::compile_graph;
    use crate::graph::compiled::{
        graph_iterations, project_position, CompiledGraph, GraphNodeKind, PositionProjection,
    };
    use crate::graph::load_graph;
    use crate::graph::runner::{ExecutionCursor, NestedCursor};
    use crate::graph::{ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor, Skill};

    /// The projection of the step `cursor` selects, as its Graph process records it.
    fn project(steps: &[ConcreteStep], cursor: &ExecutionCursor) -> PositionProjection {
        let (key, iterations) = super::location(steps, cursor).unwrap();
        project_position(&CompiledGraph::new("", steps), key, &iterations, false)
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
            returns,
            sources: Vec::new(),
        })
    }

    #[test]
    fn retained_graph_nodes_preserve_review_stages_after_field_rename() {
        let node: super::GraphNode = serde_json::from_str(
            r#"{"key":0,"id":"review","label":"demo","kind":"skill","human":true,
                "returns_to":null,"sources":["code"],"paths":[]}"#,
        )
        .unwrap();
        assert!(node.interactive);
        let current = serde_json::to_value(&node).unwrap();
        assert_eq!(current["interactive"], true);
        assert!(current.get("human").is_none());
        assert_eq!(
            serde_json::from_value::<super::GraphNode>(current).unwrap(),
            node
        );

        let missing = r#"{"key":0,"id":null,"label":"demo","kind":"skill",
            "returns_to":null,"sources":[],"paths":[]}"#;
        assert!(serde_json::from_str::<super::GraphNode>(missing).is_err());
    }

    #[test]
    fn builtin_graphs_keep_their_delivery_boundaries_without_review_steps() {
        let repo = tempfile::tempdir().unwrap();
        let cases: &[(&str, &[&str], &[usize], usize)] = &[
            ("queue", &["compress", "sync", "realign", "gate"], &[], 0),
            ("refresh", &["sync", "realign"], &[], 0),
            ("task-design", &["kickoff"], &[], 0),
            ("incident", &["unbreak", "5whys", "launch-plan"], &[], 0),
            (
                "pursue",
                &[
                    "implement",
                    "compress",
                    "sync",
                    "realign",
                    "loop-or-next",
                    "concept-review",
                    "loop-or-next",
                    "pr-publish",
                ],
                &[],
                2,
            ),
            ("deliver", &["gate", "land --wait-and-fix"], &[], 0),
            ("vsm-operate", &["s1", "s2", "s3", "s4", "s5"], &[], 0),
        ];
        for (name, labels, interactive, returns) in cases {
            let graph = load_graph(name, repo.path()).unwrap();
            let compiled = CompiledGraph::new(*name, &compile_graph(&graph, repo.path()).unwrap());
            assert_eq!(
                compiled
                    .steps
                    .iter()
                    .map(|s| s.label.as_str())
                    .collect::<Vec<_>>(),
                *labels,
                "{name}"
            );
            assert_eq!(
                compiled
                    .steps
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| s.interactive.then_some(i))
                    .collect::<Vec<_>>(),
                *interactive,
                "{name}"
            );
            assert_eq!(
                compiled
                    .steps
                    .iter()
                    .filter(|s| s.returns_to.is_some())
                    .count(),
                *returns,
                "{name}"
            );
        }
    }

    #[test]
    fn refresh_integrates_upstream_before_realigning() {
        let repo = tempfile::tempdir().unwrap();
        let graph = load_graph("refresh", repo.path()).unwrap();
        let compiled =
            CompiledGraph::new(&graph.name, &compile_graph(&graph, repo.path()).unwrap());
        let labels: Vec<_> = compiled
            .steps
            .iter()
            .map(|node| node.label.as_str())
            .collect();
        assert_eq!(labels, ["sync", "realign"]);
        assert_eq!(compiled.steps[0].kind, GraphNodeKind::Op);
        assert_eq!(compiled.steps[1].kind, GraphNodeKind::Skill);
    }

    #[test]
    fn pursue_loops_to_a_published_pr() {
        let repo = tempfile::tempdir().unwrap();
        let graph = load_graph("pursue", repo.path()).unwrap();
        let compiled =
            CompiledGraph::new(&graph.name, &compile_graph(&graph, repo.path()).unwrap());
        let labels: Vec<_> = compiled
            .steps
            .iter()
            .map(|node| node.label.as_str())
            .collect();
        assert_eq!(
            labels,
            [
                "implement",
                "compress",
                "sync",
                "realign",
                "loop-or-next",
                "concept-review",
                "loop-or-next",
                "pr-publish",
            ]
        );
        let implement = compiled.steps[0].key;
        let returns: Vec<_> = compiled
            .steps
            .iter()
            .filter_map(|node| node.returns_to.as_ref().map(|to| (node.key, to)))
            .collect();
        assert_eq!(
            returns,
            [
                (compiled.steps[4].key, &implement),
                (compiled.steps[6].key, &implement)
            ]
        );
        // Review and landing are the conversation's to launch, not this Graph's.
        assert!(compiled.steps.iter().all(|node| !node.interactive));
        assert!(compiled
            .steps
            .iter()
            .all(|node| node.kind != GraphNodeKind::Op || node.label == "sync"));
        assert_eq!(compiled.steps[3].sources, ["pursue", "refresh"]);
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
            paths: HashMap::from([
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
        let compiled = CompiledGraph::new("routed", &steps);
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
        let compiled = CompiledGraph::new("nested", &steps);
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
