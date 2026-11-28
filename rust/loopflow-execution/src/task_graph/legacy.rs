//! LOO-1100: translate released captures at carrier acquisition. Historical
//! process graphs and a live carrier's checkpoints stay in their own coordinates.
use std::collections::{BTreeMap, HashSet};

use crate::graph::runner::{ExecutionCursor, NestedCursor};
use crate::graph::{breakpoint_name, Breakpoint, ConcreteStep};

use super::{find_place, names, visit_steps, CapturedGraph};

/// Entry and exit boundaries of each old step in one body, including its end.
/// A worker keeps its entry, and its loop decision moves to its final node.
struct Body {
    starts: Vec<usize>,
    paths: BTreeMap<(usize, String), Body>,
}

impl Body {
    fn expand(steps: &mut Vec<ConcreteStep>, used: &mut HashSet<String>) -> Self {
        let mut starts = Vec::with_capacity(steps.len() + 1);
        let mut paths = BTreeMap::new();
        for (i, mut step) in std::mem::take(steps).into_iter().enumerate() {
            let entry = steps.len();
            starts.push(entry);
            let distance = |back: usize| i.checked_sub(back).map(|target| entry - starts[target]);
            match &mut step {
                ConcreteStep::Skill(skill) if skill.interactive => {
                    let name =
                        breakpoint_name(Some(skill.id.as_ref().unwrap_or(&skill.skill.name)), used);
                    let stop = Breakpoint {
                        name,
                        // The return now leaves the marker one step after the worker.
                        returns: skill.returns.take().and_then(distance).map(|back| back + 1),
                        sources: skill.sources.clone(),
                    };
                    skill.interactive = false;
                    // The saved alias still addresses this worker with --from.
                    steps.push(step);
                    steps.push(ConcreteStep::Breakpoint(stop));
                    continue;
                }
                ConcreteStep::Skill(skill) => skill.returns = skill.returns.and_then(distance),
                ConcreteStep::Breakpoint(stop) => stop.returns = stop.returns.and_then(distance),
                ConcreteStep::Xor(branch) => {
                    for (name, path) in &mut branch.paths {
                        paths.insert((i, name.clone()), Self::expand(&mut path.steps, used));
                    }
                }
                ConcreteStep::Command(_) => {}
            }
            steps.push(step);
        }
        starts.push(steps.len());
        Self { starts, paths }
    }

    fn translate(&self, cursor: &mut ExecutionCursor) {
        let old = cursor.index;
        if let Some(NestedCursor::Xor { selected, cursor }) = cursor.child.as_deref_mut() {
            if let Some(path) = self.paths.get(&(old, selected.clone())) {
                path.translate(cursor);
            }
        }
        if let Some(start) = self.starts.get(old) {
            cursor.index = *start;
        }
        // A finished decider checkpoint already owns its verdict. At a new
        // worker the runner settles this to the review without launching it.
        if !cursor.progress.repeats.is_empty() {
            let mut counts = std::mem::take(&mut cursor.progress.repeats);
            self.translate_counts("", "", &mut counts, &mut cursor.progress.repeats);
            cursor.progress.repeats.extend(counts);
        }
    }

    fn translate_counts(
        &self,
        old: &str,
        new: &str,
        counts: &mut BTreeMap<String, u32>,
        translated: &mut BTreeMap<String, u32>,
    ) {
        for (i, bounds) in self.starts.windows(2).enumerate() {
            if let Some(count) = counts.remove(&format!("{old}{i}")) {
                translated.insert(format!("{new}{}", bounds[1] - 1), count);
            }
        }
        for ((i, name), path) in &self.paths {
            path.translate_counts(
                &format!("{old}xor:{i}:{name}/"),
                &format!("{new}xor:{}:{name}/", self.starts[*i]),
                counts,
                translated,
            );
        }
    }
}

impl CapturedGraph {
    /// Translate every position the Task carries alongside this definition.
    pub(crate) fn expand_interactive<'a>(
        &mut self,
        cursors: impl IntoIterator<Item = &'a mut ExecutionCursor>,
    ) -> bool {
        if find_place(
            &self.steps,
            &mut |step| matches!(step, ConcreteStep::Skill(skill) if skill.interactive),
        )
        .is_none()
        {
            return false;
        }
        let mut used = HashSet::from(["start".into(), "end".into()]);
        visit_steps(&self.steps, &mut |step| {
            used.extend(names(step).into_iter().map(str::to_owned));
            if let ConcreteStep::Command(command) = step {
                used.insert(command.item.display_name());
            }
        });
        let body = Body::expand(&mut self.steps, &mut used);
        for cursor in cursors {
            body.translate(cursor);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::CapturedGraph;
    use crate::graph::runner::ExecutionCursor;
    use crate::graph::{return_target, ConcreteStep};
    use serde_json::json;

    #[test]
    fn released_nested_checkpoint_keeps_its_route_counts_and_finished_decision() {
        let worker = |name, interactive, returns: Option<usize>| {
            json!({"Skill":{
                "skill":{"name":name}, "id":null, "human":interactive,
                "returns":returns, "sources":["saved"]
            }})
        };
        let mut graph: CapturedGraph = serde_json::from_value(json!({"name":"saved","steps":[
            worker("explore",true,None),
            {"Xor":{"router":{"name":"xor-route"},"sources":["saved"],"paths":{
                "a/b":{"description":"work","steps":[worker("build",true,None),worker("review",true,Some(1))]}
            }}}, worker("verify",false,Some(2))
        ]})).unwrap();
        let mut cursor: ExecutionCursor = serde_json::from_value(json!({
            "index":1, "progress":{"direction":null,"verdict":null,"repeats":{"2":2,"xor:1:a/b/1":3}},
            "child":{"type":"xor","selected":"a/b","cursor":{
                "index":1,"iteration":3,"progress":{"direction":"check again","repeats":{"1":3},
                    "verdict":{"decision":"iterate","summary":"compare keyboard use"}}
            }}
        })).unwrap();
        let mut end = ExecutionCursor {
            index: graph.steps.len(),
            ..Default::default()
        };
        assert!(graph.expand_interactive([&mut cursor, &mut end]));
        assert_eq!(end.index, graph.steps.len());
        assert_eq!(cursor.index, 2);
        assert_eq!(cursor.progress.repeats["xor:2:a/b/3"], 3);
        assert_eq!(cursor.progress.repeats["3"], 2);
        assert_eq!(return_target(&graph.steps, 3), Some(0));
        assert_eq!(graph.step_name_of(&cursor).as_deref(), Some("review"));
        cursor.finish(&graph.steps).unwrap();
        assert_eq!(graph.step_name_of(&cursor).as_deref(), Some("after-review"));
        let (body, leaf) = cursor.current_body(&graph.steps);
        assert!(matches!(body[leaf.index], ConcreteStep::Breakpoint(_)));
        assert_eq!(
            leaf.progress.verdict.as_ref().unwrap().summary,
            "compare keyboard use"
        );
        cursor.finish(&graph.steps).unwrap();
        assert_eq!(graph.step_name_of(&cursor).as_deref(), Some("build"));
        assert_eq!(cursor.leaf().progress.repeats["3"], 4);
        assert_eq!(cursor.leaf().iteration, 4);
        assert_eq!(
            cursor.leaf().progress.direction.as_deref(),
            Some("compare keyboard use")
        );
        assert!(!graph.expand_interactive([&mut cursor]));
    }
}
