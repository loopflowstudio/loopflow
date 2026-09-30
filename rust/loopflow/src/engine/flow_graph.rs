//! Display projection of one captured Flow definition.
//!
//! Surfaces draw the Flow from this shape instead of parsing YAML or inferring
//! topology from skill names. Node keys are captured preorder IDs, including
//! every sorted XOR alternative. Authored names and runtime cursor paths remain
//! separate; repeated skills keep distinct IDs within their captured Flow.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use std::path::Path;

use crate::engine::execution::{ExecutionCursor, NestedCursor};
use crate::engine::flow::ConcreteStep;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowGraph {
    pub name: String,
    pub steps: Vec<FlowNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowNode {
    pub key: u32,
    /// Authored occurrence id, when the definition names one.
    pub id: Option<String>,
    /// Literal skill name, operation, or XOR router.
    pub label: String,
    pub kind: FlowNodeKind,
    pub human: bool,
    /// Key of the earlier node this deciding occurrence can return to.
    pub returns_to: Option<u32>,
    /// Composed Flows this occurrence was compiled from, outermost first.
    pub sources: Vec<String>,
    /// XOR alternatives, sorted by name; empty for other kinds.
    pub paths: Vec<FlowGraphPath>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowNodeKind {
    Skill,
    Op,
    Xor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowGraphPath {
    pub name: String,
    pub description: String,
    pub steps: Vec<FlowNode>,
}

/// Execution evidence for one backward edge in the pinned invocation. The edge
/// itself is the deciding node's `returns_to`; this carries only its counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowReturn {
    /// Key of the deciding occurrence that owns the edge.
    pub decider: u32,
    pub traversals: u32,
}

/// One selectable Flow and the topology it would pin if started now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowCatalogEntry {
    pub name: String,
    /// `None` when the definition cannot be loaded or compiled.
    pub graph: Option<FlowGraph>,
    /// Why the definition is unusable; set exactly when `graph` is `None`.
    pub unavailable: Option<String>,
}

/// Every Flow available in `repo`, each compiled through the shared loader.
pub fn flow_catalog(repo: &Path) -> Vec<FlowCatalogEntry> {
    crate::engine::available_flow_names(repo)
        .into_iter()
        .map(|name| {
            let compiled = crate::engine::load_flow(&name, repo)
                .map_err(|error| error.to_string())
                .and_then(|flow| {
                    crate::engine::compile_flow(&flow, repo)
                        .map(|steps| FlowGraph::new(&flow.name, &steps))
                        .map_err(|error| error.to_string())
                });
            match compiled {
                Ok(graph) => FlowCatalogEntry {
                    name,
                    graph: Some(graph),
                    unavailable: None,
                },
                Err(reason) => FlowCatalogEntry {
                    name,
                    graph: None,
                    unavailable: Some(reason),
                },
            }
        })
        .collect()
}

impl FlowGraph {
    pub fn new(name: impl Into<String>, steps: &[ConcreteStep]) -> Self {
        Self {
            name: name.into(),
            steps: nodes(steps, &mut 0),
        }
    }

    /// Look up a captured node, including nodes inside unselected alternatives.
    pub(crate) fn node_at(&self, id: u32) -> Option<&FlowNode> {
        fn find(nodes: &[FlowNode], id: u32) -> Option<&FlowNode> {
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

fn nodes(steps: &[ConcreteStep], next: &mut u32) -> Vec<FlowNode> {
    let mut result: Vec<FlowNode> = Vec::with_capacity(steps.len());
    for (index, step) in steps.iter().enumerate() {
        let key = *next;
        *next = next
            .checked_add(1)
            .expect("captured Flow fits u32 node IDs");
        let node = match step {
            ConcreteStep::Skill(skill) => FlowNode {
                returns_to: return_target(steps, index).map(|target| result[target].key),
                key,
                id: skill.id.clone(),
                label: skill.skill.name.clone(),
                kind: FlowNodeKind::Skill,
                human: skill.human,
                sources: skill.sources.clone(),
                paths: Vec::new(),
            },
            ConcreteStep::Command(op) => FlowNode {
                key,
                id: None,
                label: op.item.display_name(),
                kind: FlowNodeKind::Op,
                human: false,
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
                        FlowGraphPath {
                            name: name.clone(),
                            description: path.description.clone(),
                            steps: nodes(&path.steps, next),
                        }
                    })
                    .collect();
                FlowNode {
                    key,
                    id: None,
                    label: branch.router.name.clone(),
                    kind: FlowNodeKind::Xor,
                    human: false,
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

/// Index of the earlier occurrence the deciding skill at `index` returns to.
fn return_target(steps: &[ConcreteStep], index: usize) -> Option<usize> {
    let ConcreteStep::Skill(skill) = &steps[index] else {
        return None;
    };
    let from = skill.repeat.as_ref()?.from.as_str();
    steps[..index].iter().position(|target| {
        matches!(target, ConcreteStep::Skill(target) if target.id.as_deref() == Some(from))
    })
}

/// Ordered backward-edge traversal counts at each active nesting level, outermost
/// first. Empty levels are retained so nested counts never masquerade as root
/// counts. Visit tokens used for human-boundary identity are deliberately absent.
pub fn flow_iterations(steps: &[ConcreteStep], cursor: &ExecutionCursor) -> Vec<Vec<u32>> {
    let counts = steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| {
            return_target(steps, index)?;
            let ConcreteStep::Skill(skill) = step else {
                return None;
            };
            let id = skill.id.as_ref()?;
            Some(cursor.progress.repeats.get(id).copied().unwrap_or(0))
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
            levels.extend(flow_iterations(&path.steps, child));
        }
    }
    levels
}

/// Where a saved cursor stands inside its captured definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorProjection {
    /// Key of the current occurrence; `None` when the cursor selects no node.
    pub current: Option<u32>,
    /// Occurrences already finished in the current pass, including the router of
    /// a selected XOR. Earlier passes' completions are not carried forward.
    pub completed: Vec<u32>,
    pub returns: Vec<FlowReturn>,
}

pub fn project_cursor(graph: &FlowGraph, cursor: &ExecutionCursor) -> CursorProjection {
    let mut projection = CursorProjection {
        current: None,
        completed: Vec::new(),
        returns: Vec::new(),
    };
    walk_cursor(&graph.steps, Some(cursor), &mut projection);
    collect_returns(
        &graph.steps,
        Some(cursor),
        &cursor.progress.repeats,
        &mut projection.returns,
    );
    projection
}

fn walk_cursor(
    nodes: &[FlowNode],
    cursor: Option<&ExecutionCursor>,
    projection: &mut CursorProjection,
) {
    let Some(cursor) = cursor else { return };
    projection
        .completed
        .extend(nodes.iter().take(cursor.index).map(|node| node.key));
    let Some(node) = nodes.get(cursor.index) else {
        return;
    };
    if let Some(NestedCursor::Xor {
        selected,
        cursor: child,
    }) = cursor.child.as_deref()
    {
        if let Some(path) = node.paths.iter().find(|path| &path.name == selected) {
            if child.index < path.steps.len() {
                projection.completed.push(node.key);
                walk_cursor(&path.steps, Some(child), projection);
                return;
            }
        }
    }
    // A completed or unrecorded child still waits on its XOR.
    projection.current = Some(node.key);
}

fn collect_returns(
    nodes: &[FlowNode],
    cursor: Option<&ExecutionCursor>,
    repeats: &BTreeMap<String, u32>,
    out: &mut Vec<FlowReturn>,
) {
    for (index, node) in nodes.iter().enumerate() {
        if let (Some(id), Some(_)) = (&node.id, node.returns_to) {
            out.push(FlowReturn {
                decider: node.key,
                traversals: repeats.get(id).copied().unwrap_or(0),
            });
        }
        for path in &node.paths {
            let child =
                cursor
                    .filter(|cursor| cursor.index == index)
                    .and_then(|cursor| match cursor.child.as_deref() {
                        Some(NestedCursor::Xor { selected, cursor }) if selected == &path.name => {
                            Some(cursor)
                        }
                        _ => None,
                    });
            // Runtime repeat storage keeps its own path convention. Only the
            // public node reference changes; active counts still win.
            let settled_prefix = format!("xor:{index}:{}/", path.name);
            let settled: BTreeMap<String, u32> = repeats
                .iter()
                .filter_map(|(key, count)| {
                    key.strip_prefix(&settled_prefix)
                        .map(|key| (key.to_owned(), *count))
                })
                .collect();
            collect_returns(
                &path.steps,
                child,
                child.map_or(&settled, |child| &child.progress.repeats),
                out,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use crate::engine::execution::{ExecutionCursor, NestedCursor};
    use crate::engine::flow::{
        ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor, RepeatPolicy, Skill,
    };
    use crate::engine::flow_graph::{flow_iterations, project_cursor, FlowGraph, FlowNodeKind};
    use crate::engine::{compile_flow, load_flow};

    fn skill(name: &str, id: Option<&str>, human: bool, from: Option<&str>) -> ConcreteStep {
        ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named(name),
            id: id.map(str::to_string),
            human,
            repeat: from.map(|from| RepeatPolicy {
                from: from.to_string(),
            }),
            sources: Vec::new(),
        })
    }

    #[test]
    fn retained_flows_keep_their_delivery_and_review_boundaries() {
        let repo = tempfile::tempdir().unwrap();
        let cases: &[(&str, &[&str], &[usize], usize)] = &[
            ("code", &["implement", "compress"], &[], 0),
            (
                "queue",
                &["compress", "task rebase", "realign", "gate"],
                &[],
                0,
            ),
            ("refresh", &["task rebase", "realign"], &[], 0),
            ("task-design", &["kickoff", "review-design"], &[1], 0),
            ("incident", &["unbreak", "5whys", "launch-plan"], &[], 0),
            (
                "pursue",
                &[
                    "implement",
                    "compress",
                    "task rebase",
                    "realign",
                    "loop-decide",
                    "pr-publish",
                    "demo",
                    "loop-decide",
                ],
                &[6],
                2,
            ),
            ("deploy", &["gate", "task pr land"], &[], 0),
            ("ship", &["gate", "task pr land -c"], &[], 0),
            ("ship-demo", &["gate", "demo", "task pr land -c"], &[1], 0),
            ("vsm-operate", &["s1", "s2", "s3", "s4", "s5"], &[], 0),
        ];
        for (name, labels, humans, returns) in cases {
            let flow = load_flow(name, repo.path()).unwrap();
            let graph = FlowGraph::new(*name, &compile_flow(&flow, repo.path()).unwrap());
            assert_eq!(
                graph
                    .steps
                    .iter()
                    .map(|s| s.label.as_str())
                    .collect::<Vec<_>>(),
                *labels,
                "{name}"
            );
            assert_eq!(
                graph
                    .steps
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| s.human.then_some(i))
                    .collect::<Vec<_>>(),
                *humans,
                "{name}"
            );
            assert_eq!(
                graph
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
        let flow = load_flow("refresh", repo.path()).unwrap();
        let graph = FlowGraph::new(&flow.name, &compile_flow(&flow, repo.path()).unwrap());
        let labels: Vec<_> = graph.steps.iter().map(|node| node.label.as_str()).collect();
        assert_eq!(labels, ["task rebase", "realign"]);
        assert_eq!(graph.steps[0].kind, FlowNodeKind::Op);
        assert_eq!(graph.steps[1].kind, FlowNodeKind::Skill);
    }

    #[test]
    fn feature_draws_both_returns_to_implement_with_forward_delivery() {
        let repo = tempfile::tempdir().unwrap();
        let flow = load_flow("feature", repo.path()).unwrap();
        let graph = FlowGraph::new(&flow.name, &compile_flow(&flow, repo.path()).unwrap());
        let labels: Vec<_> = graph.steps.iter().map(|node| node.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "kickoff",
                "review-design",
                "implement",
                "compress",
                "task rebase",
                "realign",
                "loop-decide",
                "pr-publish",
                "demo",
                "loop-decide",
                "compress",
                "task rebase",
                "realign",
                "gate",
                "task pr land -c"
            ]
        );
        let implement = graph.steps[2].key;
        let returns: Vec<_> = graph
            .steps
            .iter()
            .filter_map(|node| node.returns_to.as_ref().map(|to| (node.id.clone(), to)))
            .collect();
        assert_eq!(
            returns,
            [
                (Some("decide".to_string()), &implement),
                (Some("decide_delivery".to_string()), &implement)
            ]
        );
        assert!(graph.steps[1].human && graph.steps[8].human);
        assert_eq!(graph.steps[14].kind, FlowNodeKind::Op);
        assert_eq!(graph.steps[5].sources, ["feature", "pursue", "refresh"]);
    }

    #[test]
    fn repeated_decisions_keep_independent_counts_and_a_pass_scoped_completion() {
        let steps = vec![
            skill("design", Some("design"), false, None),
            skill("implement", Some("implement"), false, None),
            skill("loop-decide", Some("decide"), false, Some("implement")),
            skill("demo", Some("demo"), true, None),
            skill(
                "loop-decide",
                Some("decide_delivery"),
                false,
                Some("implement"),
            ),
            skill("land", None, false, None),
        ];
        let cursor = ExecutionCursor {
            index: 1,
            iteration: 3,
            progress: crate::engine::transitions::FlowProgress {
                repeats: BTreeMap::from([("decide".into(), 2), ("decide_delivery".into(), 1)]),
                ..Default::default()
            },
            ..Default::default()
        };
        let projection = project_cursor(&FlowGraph::new("", &steps), &cursor);
        assert_eq!(projection.current, Some(1));
        // Only the opening step is complete; earlier passes' decisions are not.
        assert_eq!(projection.completed, [0]);
        let counts: Vec<_> = projection
            .returns
            .iter()
            .map(|edge| (edge.decider, edge.traversals))
            .collect();
        assert_eq!(counts, [(2, 2), (4, 1)]);
        assert_eq!(flow_iterations(&steps, &cursor), [vec![2, 1]]);
        // Moving beyond one span does not collapse its independent count.
        let later = ExecutionCursor {
            index: 3,
            iteration: 99,
            ..cursor
        };
        assert_eq!(flow_iterations(&steps, &later), [vec![2, 1]]);
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
                            skill("check", Some("check"), false, Some("patch")),
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
        let graph = FlowGraph::new("routed", &steps);
        let xor = &graph.steps[1];
        assert_eq!(xor.kind, FlowNodeKind::Xor);
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
            progress: crate::engine::transitions::FlowProgress {
                repeats: BTreeMap::from([("xor:1:fix/check".into(), 4)]),
                ..Default::default()
            },
            child: Some(Box::new(NestedCursor::Xor {
                selected: "fix".into(),
                cursor: ExecutionCursor {
                    index: 1,
                    iteration: 2,
                    progress: crate::engine::transitions::FlowProgress {
                        repeats: BTreeMap::from([("check".into(), 2)]),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            })),
            ..Default::default()
        };
        let projection = project_cursor(&FlowGraph::new("", &steps), &cursor);
        assert_eq!(projection.current, Some(3));
        assert_eq!(projection.completed, [0, 1, 2]);
        // The active child's own count wins over an older settled visit.
        assert_eq!(projection.returns[0].traversals, 2);
        assert_eq!(flow_iterations(&steps, &cursor), [vec![], vec![2]]);
    }

    #[test]
    fn numeric_wire_matches_captured_ids_across_nested_alternatives() {
        use crate::engine::invocation::QueuedInvocation;
        use crate::ops::task_flow::{TaskFlowRecord, TaskFlowSnapshot};

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
        fn check() -> ConcreteStep {
            skill("check", Some("check"), false, Some("begin"))
        }
        let steps = vec![
            begin(),
            branch(vec![
                ("zeta", vec![begin(), check()]),
                (
                    "alpha",
                    vec![
                        begin(),
                        branch(vec![("fix", vec![begin(), check()])]),
                        check(),
                    ],
                ),
            ]),
            check(),
        ];
        let invocation = QueuedInvocation::new("nested", steps).unwrap();
        let fixture: TaskFlowSnapshot = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/flow_numeric_nested.json"
        ))
        .unwrap();
        let TaskFlowRecord::Pinned(pinned) = fixture.record else {
            panic!("pinned fixture")
        };
        let graph = FlowGraph::new("nested", &invocation.steps);
        assert_eq!(graph, pinned.graph);
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
        for cursor in cursors(&invocation.steps) {
            let id = invocation.node_id(&cursor).unwrap();
            ids.push(id);
            assert_eq!(project_cursor(&graph, &cursor).current, Some(id));
            assert_eq!(graph.node_at(id).unwrap().key, id);
        }
        ids.sort();
        assert_eq!(ids, (0..10).collect::<Vec<_>>());
        let cursor = ExecutionCursor {
            index: 1,
            progress: crate::engine::transitions::FlowProgress {
                repeats: BTreeMap::from([("check".into(), 3)]),
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
                            progress: crate::engine::transitions::FlowProgress {
                                repeats: BTreeMap::from([("check".into(), 2)]),
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
        let projection = project_cursor(&graph, &cursor);
        assert_eq!(projection.current, pinned.current);
        assert_eq!(projection.completed, pinned.completed);
        assert_eq!(projection.returns, pinned.returns);
        assert_eq!(
            flow_iterations(&invocation.steps, &cursor),
            pinned.iterations
        );
        // Settled nested counts retain their runtime keys, including inactive paths.
        let settled = ExecutionCursor {
            index: 2,
            progress: crate::engine::transitions::FlowProgress {
                repeats: BTreeMap::from([
                    ("xor:1:alpha/xor:1:fix/check".into(), 7),
                    ("xor:1:zeta/check".into(), 4),
                ]),
                ..Default::default()
            },
            ..Default::default()
        };
        let projection = project_cursor(&graph, &settled);
        assert_eq!(projection.current, Some(9));
        assert_eq!(projection.completed, [0, 1]);
        assert_eq!(
            projection
                .returns
                .iter()
                .map(|r| (r.decider, r.traversals))
                .collect::<Vec<_>>(),
            [(5, 7), (6, 0), (8, 4), (9, 0)]
        );
        let finished = ExecutionCursor {
            index: 3,
            ..Default::default()
        };
        assert_eq!(project_cursor(&graph, &finished).current, None);
        assert_eq!(project_cursor(&graph, &finished).completed, [0, 1, 9]);
    }
}
