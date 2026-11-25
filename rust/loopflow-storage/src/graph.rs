use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CompiledGraph {
    pub name: String,
    pub steps: Vec<GraphNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GraphNodeKind {
    Breakpoint,
    Skill,
    Op,
    Xor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CompiledGraphPath {
    pub name: String,
    pub description: String,
    pub steps: Vec<GraphNode>,
}

/// Execution evidence for one backward edge in the pinned invocation. The edge
/// itself is the deciding node's `returns_to`; this carries only its counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GraphReturn {
    /// Key of the deciding occurrence that owns the edge.
    pub decider: u32,
    pub traversals: u32,
}

impl CompiledGraph {
    /// Look up a captured node, including nodes inside unselected alternatives.
    pub fn node_at(&self, id: u32) -> Option<&GraphNode> {
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
