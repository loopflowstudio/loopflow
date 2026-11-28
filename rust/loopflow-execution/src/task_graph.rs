//! A Task runs its captured workers and reviews their results at Graph breakpoints.

mod legacy;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::LoadError;
use crate::graph::runner::{ExecutionCursor, NestedCursor};
use crate::graph::{compile_graph, load_authored_graph, return_target, ConcreteStep};
use loopflow_storage::sqlite::task_work::PauseStage;
use loopflow_storage::sqlite::{Auto, AutoState};

pub use loopflow_storage::sqlite::task_work::{END, START};

/// The graph a Task took up, compiled as it was then.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedGraph {
    pub name: String,
    pub steps: Vec<ConcreteStep>,
}

/// What lifting a pause hold does at the Task's saved position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UnpauseEffect {
    Resume,
    /// Stay parked; settle a dead run cursor to `At { step }` when `Some`.
    Park(Option<String>),
}

/// A place in the captured steps: the top level, or inside XOR paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Place {
    /// Each XOR step descended through, with the path taken.
    path: Vec<(usize, String)>,
    index: usize,
}

impl Place {
    pub fn cursor(&self) -> ExecutionCursor {
        let mut cursor = ExecutionCursor {
            index: self.index,
            ..ExecutionCursor::default()
        };
        for (index, selected) in self.path.iter().rev() {
            cursor = ExecutionCursor {
                index: *index,
                child: Some(Box::new(NestedCursor::Xor {
                    selected: selected.clone(),
                    cursor,
                })),
                ..ExecutionCursor::default()
            };
        }
        cursor
    }
}

/// The steps of the body `path` descends to.
fn body<'a>(steps: &'a [ConcreteStep], path: &[(usize, String)]) -> &'a [ConcreteStep] {
    path.iter()
        .fold(steps, |body, (index, selected)| match &body[*index] {
            ConcreteStep::Xor(branch) => &branch.paths[selected].steps,
            _ => unreachable!("a place descends only through XOR steps"),
        })
}

/// Every name a step answers to for `--from`: its id, its skill or command,
/// and the graph it was composed from.
fn names(step: &ConcreteStep) -> Vec<&str> {
    let (sources, own, id) = match step {
        ConcreteStep::Skill(skill) => (
            &skill.sources,
            skill.skill.name.as_str(),
            skill.id.as_deref(),
        ),
        ConcreteStep::Breakpoint(stop) => (&stop.sources, stop.name.as_str(), None),
        ConcreteStep::Command(command) => (&command.sources, command.item.command.as_str(), None),
        ConcreteStep::Xor(branch) => (&branch.sources, branch.router.name.as_str(), None),
    };
    let interactive = match step {
        ConcreteStep::Skill(skill) => skill.interactive_name.as_deref(),
        _ => None,
    };
    interactive
        .into_iter()
        .chain(id)
        .chain(std::iter::once(own))
        .chain(sources.get(1).map(String::as_str))
        .collect()
}

/// The name a Task waits at before a step that merges: `lf land` or `lf pr
/// arm` however the graph spells it, or the `pr-land` skill.
pub fn landing_name(
    step: &ConcreteStep,
    mut merges: impl FnMut(&[String]) -> bool,
) -> Option<String> {
    match step {
        ConcreteStep::Command(command) => {
            merges(&command.item.argv()).then(|| command.item.display_name())
        }
        ConcreteStep::Skill(skill) if skill.skill.name == "pr-land" => {
            Some(names(step)[0].to_owned())
        }
        _ => None,
    }
}

/// Visit each occurrence in execution order, including alternatives in name order.
fn visit_steps<'a>(steps: &'a [ConcreteStep], visit: &mut impl FnMut(&'a ConcreteStep)) {
    for step in steps {
        visit(step);
        if let ConcreteStep::Xor(branch) = step {
            for path in branch.paths.values() {
                visit_steps(&path.steps, visit);
            }
        }
    }
}

fn find_place(
    steps: &[ConcreteStep],
    matches: &mut impl FnMut(&ConcreteStep) -> bool,
) -> Option<Place> {
    for (index, step) in steps.iter().enumerate() {
        if matches(step) {
            return Some(Place {
                index,
                path: Vec::new(),
            });
        }
        if let ConcreteStep::Xor(branch) = step {
            for (name, path) in &branch.paths {
                if let Some(mut place) = find_place(&path.steps, matches) {
                    place.path.insert(0, (index, name.clone()));
                    return Some(place);
                }
            }
        }
    }
    None
}

impl CapturedGraph {
    /// Compile the graph named `name` as the repository defines it now.
    pub fn load(name: &str, repo: &Path) -> Result<Option<Self>, LoadError> {
        let graph = match load_authored_graph(name, repo) {
            Ok(graph) => graph,
            Err(LoadError::GraphNotFound(_)) => return Ok(None),
            Err(error) => return Err(error),
        };
        let steps = compile_graph(&graph, repo)?;
        Ok(Some(Self {
            name: graph.name,
            steps,
        }))
    }

    /// The first step answering to `name`; `start` is the first step.
    pub fn find(&self, name: &str) -> Option<Place> {
        if name == START {
            return (!self.steps.is_empty()).then(Place::default);
        }
        find_place(&self.steps, &mut |step| {
            // A Graph records an operation under its display name.
            matches!(step, ConcreteStep::Command(command) if command.item.display_name() == name)
                || names(step).contains(&name)
        })
    }

    /// LOO-1100: the unconverted interactive step named `step` on a released capture.
    pub(crate) fn interactive_step(&self, step: &str) -> Option<Place> {
        find_place(
            &self.steps,
            &mut |node| matches!(node, ConcreteStep::Skill(skill) if skill.interactive_step() == Some(step)),
        )
    }

    /// The step that merges named `step`, where a Task someone turned auto off waits.
    pub fn landing_step(&self, step: &str, merges: fn(&[String]) -> bool) -> Option<Place> {
        find_place(&self.steps, &mut |node| {
            landing_name(node, merges).as_deref() == Some(step)
        })
    }

    /// The place after `place` in its body, or in the body that XOR belongs
    /// to; `None` after the last step.
    pub fn after(&self, place: &Place) -> Option<Place> {
        let mut path = place.path.clone();
        let mut index = place.index + 1;
        loop {
            if index < body(&self.steps, &path).len() {
                return Some(Place { path, index });
            }
            let (xor, _) = path.pop()?;
            index = xor + 1;
        }
    }

    /// The first breakpoint a run from `place` reaches, looking into
    /// every XOR path in name order; `None` when it runs to the end.
    pub fn next_breakpoint(&self, place: &Place) -> Option<&str> {
        fn scan(body: &[ConcreteStep], from: usize) -> Option<&str> {
            body[from..].iter().find_map(|step| match step {
                ConcreteStep::Xor(branch) => {
                    branch.paths.values().find_map(|path| scan(&path.steps, 0))
                }
                step => stop_name(step),
            })
        }
        let mut path = place.path.clone();
        let mut from = place.index;
        loop {
            if let Some(step) = scan(body(&self.steps, &path), from) {
                return Some(step);
            }
            let (index, _) = path.pop()?;
            from = index + 1;
        }
    }

    fn has_landing(steps: &[ConcreteStep], merges: &mut dyn FnMut(&[String]) -> bool) -> bool {
        steps.iter().any(|step| match step {
            ConcreteStep::Xor(branch) => branch
                .paths
                .values()
                .any(|path| Self::has_landing(&path.steps, merges)),
            _ => landing_name(step, &mut *merges).is_some(),
        })
    }

    /// Could any remaining path need a person before or at its next landing?
    fn needs_review(
        &self,
        place: &Place,
        stop_at_landing: bool,
        merges: &mut dyn FnMut(&[String]) -> bool,
    ) -> bool {
        fn scan(
            steps: &[ConcreteStep],
            after: bool,
            stop_at_landing: bool,
            merges: &mut dyn FnMut(&[String]) -> bool,
        ) -> bool {
            steps.iter().rev().fold(after, |later, step| match step {
                step if stop_name(step).is_some() => true,
                ConcreteStep::Xor(branch) => branch
                    .paths
                    .values()
                    .any(|path| scan(&path.steps, later, stop_at_landing, merges)),
                _ if landing_name(step, &mut *merges).is_some() => stop_at_landing,
                _ => later,
            })
        }
        let mut steps = self.steps.as_slice();
        let mut later = false;
        for (index, selected) in &place.path {
            later = scan(&steps[index + 1..], later, stop_at_landing, merges);
            let ConcreteStep::Xor(branch) = &steps[*index] else {
                unreachable!("validated XOR path")
            };
            steps = &branch.paths[selected].steps;
        }
        scan(&steps[place.index..], later, stop_at_landing, merges)
    }

    /// Whether no review remains before landing from the current cursor or stop.
    pub fn ships_before_review(
        &self,
        step: &str,
        cursor: Option<&ExecutionCursor>,
        auto: Option<AutoState>,
        mut merges: impl FnMut(&[String]) -> bool,
    ) -> bool {
        if !Self::has_landing(&self.steps, &mut merges) {
            return false;
        }
        let place = if let Some(cursor) = cursor {
            if cursor.index == self.steps.len() {
                return true;
            }
            self.place_of(cursor)
        } else if step == END {
            return true;
        } else {
            self.interactive_step(step).or_else(|| self.find(step))
        };
        let stop_at_landing = auto == Some(AutoState::Off);
        place.is_some_and(|place| !self.needs_review(&place, stop_at_landing, &mut merges))
    }

    /// What lifting a pause hold does at `(step, cursor)` under `auto`:
    /// `Resume` restarts the run; `Park(step)` leaves it parked, settling a
    /// dead run cursor to `At { step }` when `Some`.
    pub(crate) fn unpause_effect(
        &self,
        step: &str,
        cursor: Option<&ExecutionCursor>,
        at_breakpoint: bool,
        auto: Option<AutoState>,
        merges: impl FnMut(&[String]) -> bool,
    ) -> UnpauseEffect {
        match cursor {
            Some(cursor) => match self.place_of(cursor) {
                Some(place) => match (auto == Some(AutoState::Off))
                    .then(|| self.landing_at(&place, merges))
                    .flatten()
                {
                    Some(step) => UnpauseEffect::Park(Some(step)),
                    None => UnpauseEffect::Resume,
                },
                None => UnpauseEffect::Park(None),
            },
            None if step == START => UnpauseEffect::Resume,
            None if step != END
                && !at_breakpoint
                && auto != Some(AutoState::Off)
                && self.interactive_step(step).is_none() =>
            {
                UnpauseEffect::Resume
            }
            None => UnpauseEffect::Park(None),
        }
    }

    /// A run restarts at its saved cursor unless `auto` now parks at that step.
    /// A Task already waiting for a person stays there.
    pub(crate) fn resumes_on_unpause(
        &self,
        step: &str,
        cursor: Option<&ExecutionCursor>,
        at_breakpoint: bool,
        auto: Option<AutoState>,
        merges: impl FnMut(&[String]) -> bool,
    ) -> bool {
        matches!(
            self.unpause_effect(step, cursor, at_breakpoint, auto, merges),
            UnpauseEffect::Resume
        )
    }

    /// Whether a run can reach a merging step. With `through`, the run
    /// follows paths past their breakpoints.
    pub fn reaches_landing(
        &self,
        place: &Place,
        through: bool,
        merges: fn(&[String]) -> bool,
    ) -> bool {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Reach {
            Landing,
            Stopped,
            Continue,
        }
        fn scan(
            body: &[ConcreteStep],
            from: usize,
            through: bool,
            merges: fn(&[String]) -> bool,
        ) -> Reach {
            for (offset, step) in body[from..].iter().enumerate() {
                let index = from + offset;
                match step {
                    ConcreteStep::Xor(branch) => {
                        let mut all_stopped = !branch.paths.is_empty();
                        for alternative in branch.paths.values() {
                            match scan(&alternative.steps, 0, through, merges) {
                                Reach::Landing => return Reach::Landing,
                                Reach::Stopped => {}
                                Reach::Continue => all_stopped = false,
                            }
                        }
                        if all_stopped {
                            return Reach::Stopped;
                        }
                    }
                    step => {
                        if !through && stop_name(step).is_some() {
                            return Reach::Stopped;
                        }
                        if landing_name(step, merges).is_some() {
                            return Reach::Landing;
                        }
                        if let Some(target) = return_target(body, index) {
                            if target < from
                                && scan(&body[..from], target, through, merges) == Reach::Landing
                            {
                                return Reach::Landing;
                            }
                        }
                    }
                }
            }
            Reach::Continue
        }
        let mut path = place.path.clone();
        let mut from = place.index;
        loop {
            match scan(body(&self.steps, &path), from, through, merges) {
                Reach::Landing => return true,
                Reach::Stopped => return false,
                Reach::Continue => {}
            }
            let Some((index, _)) = path.pop() else {
                return false;
            };
            from = index + 1;
        }
    }

    /// The place `cursor` stands at, if the graph has it.
    pub fn place_of(&self, cursor: &ExecutionCursor) -> Option<Place> {
        let mut path = Vec::new();
        let mut body = self.steps.as_slice();
        let mut cursor = cursor;
        while let Some(NestedCursor::Xor {
            selected,
            cursor: child,
        }) = cursor.child.as_deref()
        {
            let ConcreteStep::Xor(branch) = body.get(cursor.index)? else {
                return None;
            };
            body = &branch.paths.get(selected)?.steps;
            path.push((cursor.index, selected.clone()));
            cursor = child;
        }
        (cursor.index < body.len()).then_some(Place {
            path,
            index: cursor.index,
        })
    }

    pub fn landing_at(
        &self,
        place: &Place,
        merges: impl FnMut(&[String]) -> bool,
    ) -> Option<String> {
        landing_name(self.step_at(place), merges)
    }

    fn step_at(&self, place: &Place) -> &ConcreteStep {
        &body(&self.steps, &place.path)[place.index]
    }

    /// What a person calls the step at `place`; an operation by the display
    /// name its Graph records it under.
    pub fn step_name(&self, place: &Place) -> String {
        step_name(self.step_at(place))
    }

    /// What a person calls the step `cursor` stands at, if the graph has it.
    pub fn step_name_of(&self, cursor: &ExecutionCursor) -> Option<String> {
        let (body, leaf) = cursor.current_body(&self.steps);
        body.get(leaf.index).map(step_name)
    }

    /// Steps a person would name for `--from`, for messages.
    pub fn describe_steps(&self) -> String {
        let mut seen: Vec<&str> = Vec::new();
        visit_steps(&self.steps, &mut |step| {
            let name = names(step)[0];
            if !seen.contains(&name) {
                seen.push(name);
            }
        });
        if seen.is_empty() {
            "none".into()
        } else {
            seen.join(", ")
        }
    }
}

/// Breakpoint name for `step`, or the unconverted released interactive stop name (LOO-1100).
fn stop_name(step: &ConcreteStep) -> Option<&str> {
    match step {
        ConcreteStep::Breakpoint(stop) => Some(stop.name.as_str()),
        ConcreteStep::Skill(skill) => skill.interactive_step(),
        _ => None,
    }
}

/// The one sentence every surface prints when a Task is parked at a breakpoint.
pub fn breakpoint_sentence(
    step: &str,
    stop: &crate::store::task_work::BreakpointArrival,
) -> String {
    match stop.blocked_reason() {
        Some(reason) => format!("At breakpoint {step}. Blocked: {reason}"),
        None => format!("At breakpoint {step}."),
    }
}

/// The top-level steps as a person reads them, consecutive repeats collapsed,
/// with the index in them of each top-level place. A composed step reads as
/// the graph it came from.
struct WorkflowTrack {
    steps: Vec<WorkflowStep>,
    indices: Vec<u32>,
    breakpoints: Vec<String>,
}

fn workflow(steps: &[ConcreteStep]) -> WorkflowTrack {
    let mut breakpoints = Vec::new();
    let mut line: Vec<WorkflowStep> = Vec::new();
    let mut indices = Vec::with_capacity(steps.len());
    for step in steps {
        let sources = match step {
            ConcreteStep::Skill(skill) => &skill.sources,
            ConcreteStep::Breakpoint(stop) => &stop.sources,
            ConcreteStep::Command(command) => &command.sources,
            ConcreteStep::Xor(branch) => &branch.sources,
        };
        let name = sources.get(1).cloned().unwrap_or_else(|| step_name(step));
        let mut breakpoint = false;
        visit_steps(std::slice::from_ref(step), &mut |node| {
            if let Some(name) = stop_name(node) {
                breakpoint = true;
                if !breakpoints.iter().any(|seen| seen == name) {
                    breakpoints.push(name.to_owned());
                }
            }
        });
        let kind = if sources.get(1).is_none() && matches!(step, ConcreteStep::Breakpoint(_)) {
            WorkflowStepKind::Breakpoint
        } else if breakpoint {
            WorkflowStepKind::StageWithBreakpoint
        } else {
            WorkflowStepKind::Stage
        };
        match line.last_mut() {
            Some(last)
                if last.name == name
                    && last.kind != WorkflowStepKind::Breakpoint
                    && kind != WorkflowStepKind::Breakpoint =>
            {
                if kind == WorkflowStepKind::StageWithBreakpoint {
                    last.kind = kind;
                }
            }
            _ => line.push(WorkflowStep { name, kind }),
        }
        indices.push(u32::try_from(line.len() - 1).expect("a graph has few steps"));
    }
    WorkflowTrack {
        steps: line,
        indices,
        breakpoints,
    }
}

/// What a person calls `step`; an operation by the display name its Graph
/// records it under.
pub fn step_name(step: &ConcreteStep) -> String {
    match step {
        ConcreteStep::Command(command) => command.item.display_name(),
        step => names(step)[0].to_owned(),
    }
}

/// The workflow track's primary action and current-step chip label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkflowControl {
    pub action: WorkflowAction,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowAction {
    Run,
    Continue,
    Pause,
    Pausing,
    Unpause,
    Resume,
    Wait,
    Done,
}

fn chip_step(top: Option<&str>, at: &str) -> String {
    match top {
        Some(top) if top != at => format!("{top} · {at}"),
        _ => at.to_owned(),
    }
}

impl WorkflowControl {
    fn run(step: &str) -> Self {
        Self {
            action: WorkflowAction::Run,
            label: format!("Run: {step}"),
        }
    }

    fn paused(step: &str, resume: bool, stopping: bool) -> Self {
        let (action, prefix) = match (stopping, resume) {
            (true, _) => (WorkflowAction::Pausing, "Pausing"),
            (false, true) => (WorkflowAction::Resume, "Paused"),
            (false, false) => (WorkflowAction::Unpause, "Paused"),
        };
        Self {
            action,
            label: format!("{prefix}: {step}"),
        }
    }

    fn start(steps: &[WorkflowStep]) -> Self {
        Self::run(steps.first().map_or(END, |step| step.name.as_str()))
    }
}

/// A Graph as the workflow line shows it before any Task stands on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkflowOutline {
    pub start: WorkflowControl,
    /// The Graph's top-level steps in order, as `TaskGraph.steps` carries them.
    pub steps: Vec<WorkflowStep>,
    /// Every breakpoint in order, including composed Graphs and XOR paths.
    pub breakpoints: Vec<String>,
    /// "You take part at after-explore and after-ship-review." or "Runs on its own."
    pub sentence: String,
}

impl WorkflowOutline {
    pub fn of(steps: &[ConcreteStep]) -> Self {
        let WorkflowTrack {
            steps, breakpoints, ..
        } = workflow(steps);
        let sentence = match breakpoints.as_slice() {
            [] => "Runs on its own.".to_owned(),
            [only] => format!("You take part at {only}."),
            [rest @ .., last] => format!("You take part at {} and {last}.", rest.join(", ")),
        };
        Self {
            start: WorkflowControl::start(&steps),
            steps,
            breakpoints,
            sentence,
        }
    }
}

/// One Task's Graph as `lf task status --json` carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskGraph {
    pub control: WorkflowControl,
    pub name: String,
    /// Every breakpoint in order, including composed Graphs and XOR paths.
    pub breakpoints: Vec<String>,
    pub position: TaskGraphPosition,
    /// The Graph's top-level steps in order, as the workflow line shows them.
    pub steps: Vec<WorkflowStep>,
    /// The index in `steps` of the step the Task stands in; null at `start`
    /// and at `end`.
    #[serde(deserialize_with = "Option::deserialize")]
    pub current_step: Option<u32>,
    /// Where the Task stands, in one sentence: "build is running implement.
    /// You are next needed at after-ship-review."
    pub sentence: String,
}

/// One top-level step of a Task's Graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkflowStep {
    pub name: String,
    pub kind: WorkflowStepKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepKind {
    Stage,
    /// Runnable stage with a breakpoint, including composed Graphs and XOR paths.
    StageWithBreakpoint,
    Breakpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskGraphPosition {
    /// The Task stands at `start`, at `end`, at a landing wait (`auto: off`),
    /// or at an unconverted released wait (LOO-1100).
    At { step: String },
    /// The Task is parked at breakpoint `step` for visit `arrival`.
    Breakpoint {
        step: String,
        arrival: loopflow_model::id::LfProcessId,
        #[serde(deserialize_with = "Option::deserialize")]
        blocker: Option<String>,
    },
    /// `lf task run` (`lf_process_id`) carries the Task on from `step`, and
    /// runs `at`. `running` is read from that Process: a run that ended
    /// without arriving stopped, and the next `lf task run` starts it again
    /// at `at`; the steps before it stay done.
    Run {
        step: String,
        at: String,
        lf_process_id: String,
        running: bool,
    },
}

impl TaskGraphPosition {
    /// Where the Task waits, or the step its run left.
    pub fn step(&self) -> &str {
        match self {
            Self::Breakpoint { step, .. } | Self::At { step } | Self::Run { step, .. } => step,
        }
    }
}

/// A Task's run as its Graph row holds it.
#[derive(Debug, Clone)]
pub struct TaskGraphRun<'a> {
    pub cursor: &'a ExecutionCursor,
    pub lf_process_id: String,
    pub running: bool,
}

impl TaskGraph {
    /// Where the Task stands on `graph`: waiting at `step`, parked at a
    /// breakpoint, or carried on from it by `run`. `auto` on names auto, not a
    /// person, as the next to take part.
    pub fn read(
        graph: &CapturedGraph,
        step: String,
        breakpoint: Option<&crate::store::task_work::BreakpointArrival>,
        run: Option<TaskGraphRun<'_>>,
        auto: Option<AutoState>,
        pause: Option<PauseStage>,
        merges: impl FnMut(&[String]) -> bool,
    ) -> Self {
        let WorkflowTrack {
            steps,
            indices,
            breakpoints,
        } = workflow(&graph.steps);
        let top = |place: &Place| {
            let index = place.path.first().map_or(place.index, |(index, _)| *index);
            indices[index]
        };
        let top_name = |place: &Place| steps[top(place) as usize].name.as_str();
        let resumes = pause.is_some()
            && graph.resumes_on_unpause(
                &step,
                run.as_ref().map(|run| run.cursor),
                breakpoint.is_some(),
                auto,
                merges,
            );
        let pausing = pause == Some(PauseStage::Stopping)
            || (pause.is_some() && run.as_ref().is_some_and(|run| run.running));
        let unpause = if resumes {
            "`lf task unpause --run` resumes it."
        } else {
            "`lf task unpause` lifts the hold."
        };
        let (position, current_step, sentence, control, chip) = match (breakpoint, run) {
            (Some(stop), None) => {
                let place = graph.place_of(&stop.cursor);
                let blocker = stop.blocked_reason();
                let sentence = match pause {
                    Some(PauseStage::Stopping) => format!("Pausing at {step}."),
                    Some(PauseStage::Held) => format!("Paused at {step}. {unpause}"),
                    None => breakpoint_sentence(&step, stop),
                };
                let chip = place.as_ref().map_or(step.as_str(), top_name).to_owned();
                let control = if blocker.is_some() {
                    WorkflowControl {
                        action: WorkflowAction::Wait,
                        label: format!("Blocked: {chip}"),
                    }
                } else {
                    WorkflowControl {
                        action: WorkflowAction::Continue,
                        label: "Continue".into(),
                    }
                };
                (
                    TaskGraphPosition::Breakpoint {
                        step,
                        arrival: stop.id.clone(),
                        blocker: blocker.map(str::to_owned),
                    },
                    place.as_ref().map(top),
                    sentence,
                    control,
                    chip,
                )
            }
            (None, None) => {
                let (place, sentence, control, chip) = match step.as_str() {
                    START => {
                        let first = steps.first().map_or(END, |step| step.name.as_str());
                        (
                            None,
                            "Not started.".to_owned(),
                            WorkflowControl::run(first),
                            first.to_owned(),
                        )
                    }
                    END => (
                        None,
                        "At its end.".to_owned(),
                        WorkflowControl {
                            action: WorkflowAction::Done,
                            label: "Done".into(),
                        },
                        END.to_owned(),
                    ),
                    step => match graph.interactive_step(step) {
                        Some(place) => {
                            let next = graph.after(&place).map(|next| {
                                if top(&place) == top(&next) {
                                    graph.step_name(&next)
                                } else {
                                    top_name(&next).to_owned()
                                }
                            });
                            let chip = chip_step(Some(top_name(&place)), step);
                            (
                                Some(place),
                                format!("Waiting for you at {step}."),
                                WorkflowControl {
                                    action: WorkflowAction::Run,
                                    label: format!(
                                        "{step} · next: {}",
                                        next.as_deref().unwrap_or(END)
                                    ),
                                },
                                chip,
                            )
                        }
                        None => {
                            let place = graph.find(step);
                            let chip = chip_step(place.as_ref().map(top_name), step);
                            let (sentence, control) = if auto == Some(AutoState::Off) {
                                (
                                    format!("Waiting at {step}: auto is off."),
                                    WorkflowControl {
                                        action: WorkflowAction::Wait,
                                        label: format!("Waiting: {chip}"),
                                    },
                                )
                            } else {
                                (format!("Waiting at {step}."), WorkflowControl::run(&chip))
                            };
                            (place, sentence, control, chip)
                        }
                    },
                };
                let sentence = match pause {
                    Some(PauseStage::Stopping) => format!("Pausing at {step}."),
                    Some(PauseStage::Held) => {
                        format!("Paused at {step}. {unpause}")
                    }
                    None => sentence,
                };
                (
                    TaskGraphPosition::At { step },
                    place.as_ref().map(top),
                    sentence,
                    control,
                    chip,
                )
            }
            (_, Some(run)) => {
                let place = graph.place_of(run.cursor);
                let at = place
                    .as_ref()
                    .map_or_else(|| step.clone(), |place| graph.step_name(place));
                let current = place.as_ref().map(top);
                let top_name = place.as_ref().map(top_name);
                let label = top_name.unwrap_or(at.as_str());
                let chip = chip_step(top_name, &at);
                let location = || {
                    if label == at {
                        format!("at {at}")
                    } else {
                        format!("in {label} at {at}")
                    }
                };
                let sentence = if pause.is_some() {
                    if pausing {
                        let verb = if run.running
                            && place.as_ref().is_some_and(|place| {
                                matches!(graph.step_at(place), ConcreteStep::Command(_))
                            }) {
                            "finishing"
                        } else {
                            "stopping"
                        };
                        format!("Pausing at {label} ({at} is {verb}).")
                    } else {
                        format!("Paused {}. {unpause}", location())
                    }
                } else if !run.running {
                    format!("Stopped {}. `lf task run` starts it again.", location())
                } else {
                    let running = if label == at {
                        format!("{label} is running.")
                    } else {
                        format!("{label} is running {at}.")
                    };
                    let next = place
                        .as_ref()
                        .and_then(|place| graph.after(place))
                        .and_then(|place| graph.next_breakpoint(&place));
                    match (next, auto) {
                        (None, _) => running,
                        (Some(next), Some(AutoState::On)) => {
                            format!("{running} Auto continues through {next}.")
                        }
                        (Some(next), _) => format!("{running} You are next needed at {next}."),
                    }
                };
                let control = if run.running {
                    WorkflowControl {
                        action: WorkflowAction::Pause,
                        label: chip.clone(),
                    }
                } else {
                    WorkflowControl::run(&chip)
                };
                (
                    TaskGraphPosition::Run {
                        step,
                        at,
                        lf_process_id: run.lf_process_id,
                        running: run.running,
                    },
                    current,
                    sentence,
                    control,
                    chip,
                )
            }
        };
        let control = if pause.is_some() {
            WorkflowControl::paused(&chip, resumes, pausing)
        } else {
            control
        };
        Self {
            control,
            name: graph.name.clone(),
            breakpoints,
            position,
            steps,
            current_step,
            sentence,
        }
    }

    /// Where the Task waits, or the step its run left.
    pub fn step(&self) -> &str {
        self.position.step()
    }

    /// The breakpoint where a person takes part now (or a released capture's
    /// interactive wait before conversion).
    pub fn waiting_step(&self) -> Option<&str> {
        match &self.position {
            TaskGraphPosition::Breakpoint { step, .. } => Some(step.as_str()),
            TaskGraphPosition::At { step } => self
                .breakpoints
                .iter()
                .find(|name| name.as_str() == step.as_str())
                .map(String::as_str),
            TaskGraphPosition::Run { .. } => None,
        }
    }

    /// The merging step the Task waits at and the off that stopped it there,
    /// when it stands at a step that is neither `start`, `end` nor
    /// interactive: it stopped before merging, which only an off does.
    pub fn landing_stop<'a>(&'a self, auto: Option<&'a Auto>) -> Option<(&'a str, &'a Auto)> {
        let off = auto.filter(|mark| mark.state == AutoState::Off)?;
        let TaskGraphPosition::At { step } = &self.position else {
            return None;
        };
        let ends = step == START || step == END;
        (!ends && self.waiting_step().is_none()).then_some((step.as_str(), off))
    }
}

/// Tasks in a list often share a Graph. Parse each command's argv once per reading.
pub(crate) fn cached_merges(merges: fn(&[String]) -> bool) -> impl FnMut(&[String]) -> bool {
    let mut commands = std::collections::HashMap::<Vec<String>, bool>::new();
    move |argv| {
        if let Some(&result) = commands.get(argv) {
            return result;
        }
        let result = merges(argv);
        commands.insert(argv.to_vec(), result);
        result
    }
}

/// Name the committed file a teammate's graph selection needs.
pub fn missing_graph(name: &str) -> String {
    format!(
        "graph {name:?} is unavailable: add .lf/graphs/{name}.yaml \
         (a teammate's graph arrives when its commit is merged or pulled)"
    )
}

pub(crate) fn validate_graph_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.chars().any(char::is_whitespace) || name.contains([':', '\\']) {
        return Err("invalid graph name".into());
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    pub(crate) fn test_merges(argv: &[String]) -> bool {
        matches!(
            argv,
            [_, cmd, ..] if cmd == "land"
        ) || matches!(
            argv,
            [_, cmd, sub, ..] if cmd == "pr" && matches!(sub.as_str(), "land" | "arm")
        )
    }

    use super::{CapturedGraph, TaskGraph, TaskGraphRun, WorkflowStepKind};
    use loopflow_storage::sqlite::{Auto, AutoState};

    fn write(repo: &std::path::Path, path: &str, content: &str) {
        let path = repo.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn builtin_graphs_name_their_breakpoints() {
        let repo = tempfile::tempdir().unwrap();
        for (name, stops) in [
            ("feature", &["after-explore", "after-ship-review"][..]),
            ("code", &["after-ship-review"]),
            ("research", &["after-research"]),
            ("redesign", &["after-design"]),
            ("task-design", &[]),
            ("pursue", &[]),
            ("quick", &[]),
            ("deliver", &[]),
        ] {
            let graph = CapturedGraph::load(name, repo.path()).unwrap().unwrap();
            assert_eq!(
                super::WorkflowOutline::of(&graph.steps).breakpoints,
                stops,
                "{name}"
            );
            assert_eq!(
                graph.next_breakpoint(&super::Place::default()),
                stops.first().copied(),
                "{name}"
            );
        }
        assert!(CapturedGraph::load("no-such-graph", repo.path())
            .unwrap()
            .is_none());
    }

    #[test]
    fn a_run_continues_after_the_step_it_stopped_at_or_from_any_named_step() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        let explore = graph.find("after-explore").unwrap();
        let next = graph.after(&explore).unwrap();
        assert_eq!(graph.next_breakpoint(&next), Some("after-ship-review"));
        // A composed graph's name finds its first step.
        let plan = graph.find("plan").unwrap();
        assert_eq!(plan.cursor().index, next.cursor().index);
        assert_eq!(
            graph.find("kickoff").unwrap().cursor().index,
            next.cursor().index
        );
        assert_eq!(
            graph.find("write-docs").unwrap().cursor().index,
            next.cursor().index + 3
        );
        assert_eq!(graph.find("start").unwrap().cursor().index, 0);
        assert!(graph.find("no-such-step").is_none());
        let review = graph.find("after-ship-review").unwrap();
        assert!(graph
            .after(&review)
            .is_none_or(|place| graph.next_breakpoint(&place).is_none()));
    }

    #[test]
    fn an_interactive_step_inside_an_xor_path_is_found_there() {
        let repo = tempfile::tempdir().unwrap();
        write(
            repo.path(),
            ".lf/graphs/conditional.yaml",
            "- implement\n- xor:\n    paths:\n      review:\n        description: Review it\n        steps:\n          - interactive: demo\n      skip:\n        description: Nothing to review\n- qa\n",
        );
        let graph = CapturedGraph::load("conditional", repo.path())
            .unwrap()
            .unwrap();
        let start = graph.find("start").unwrap();
        assert_eq!(graph.next_breakpoint(&start), Some("after-demo"));
        let demo = graph.find("after-demo").unwrap();
        let after = graph.after(&demo).unwrap();
        assert_eq!(after.cursor().index, 2);
        assert_eq!(graph.next_breakpoint(&after), None);
    }

    #[test]
    fn only_an_off_makes_a_merging_step_a_landing_wait() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        let at =
            |step: &str| TaskGraph::read(&graph, step.into(), None, None, None, None, test_merges);
        let mark = |state| Auto {
            state,
            at: 0,
            by: crate::process::Starter::Person {
                name: Some("Maya".into()),
            },
        };
        let off = mark(AutoState::Off);
        assert_eq!(at("land").landing_stop(Some(&off)), Some(("land", &off)));
        assert_eq!(at("land").landing_stop(None), None);
        assert_eq!(at("end").landing_stop(Some(&off)), None);
    }

    #[test]
    fn the_workflow_line_names_top_level_steps_and_where_the_task_stands() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("code", repo.path()).unwrap().unwrap();
        let read = |step: &str, run: Option<(&str, bool)>, auto| {
            let cursor = run.map(|(at, _)| graph.find(at).unwrap().cursor());
            TaskGraph::read(
                &graph,
                step.into(),
                None,
                cursor
                    .as_ref()
                    .zip(run)
                    .map(|(cursor, (_, running))| TaskGraphRun {
                        cursor,
                        lf_process_id: "lfp_1".into(),
                        running,
                    }),
                auto,
                None,
                test_merges,
            )
        };
        let start = read("start", None, None);
        assert_eq!(start.control.label, "Run: plan");
        let steps: Vec<_> = start
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
        assert_eq!(start.breakpoints, ["after-ship-review"]);
        assert_eq!(
            (start.current_step, start.sentence.as_str()),
            (None, "Not started.")
        );
        let end = read("end", None, None);
        assert_eq!(
            (end.current_step, end.sentence.as_str()),
            (None, "At its end.")
        );

        let implement = read("plan", Some(("implement", true)), None);
        assert_eq!(implement.current_step, Some(1));
        assert_eq!(
            implement.sentence,
            "build is running implement. You are next needed at after-ship-review."
        );
        let auto = read("plan", Some(("implement", true)), Some(AutoState::On));
        assert_eq!(
            auto.sentence,
            "build is running implement. Auto continues through after-ship-review."
        );
        let stopped = read("plan", Some(("implement", false)), None);
        assert_eq!(
            stopped.sentence,
            "Stopped in build at implement. `lf task run` starts it again."
        );
        let merge_wait = read("land --wait-and-fix", None, None);
        assert_eq!(merge_wait.current_step, Some(4));
        assert_eq!(merge_wait.sentence, "Waiting at land --wait-and-fix.");
        assert_eq!(merge_wait.waiting_step(), None);

        let arrival = crate::store::task_work::BreakpointArrival {
            id: loopflow_model::id::LfProcessId::new(),
            cursor: graph.find("after-ship-review").unwrap().cursor(),
        };
        let parked = TaskGraph::read(
            &graph,
            "after-ship-review".into(),
            Some(&arrival),
            None,
            None,
            None,
            test_merges,
        );
        assert_eq!(parked.current_step, Some(3));
        assert_eq!(parked.control.action, super::WorkflowAction::Continue);
        assert_eq!(parked.sentence, "At breakpoint after-ship-review.");
        assert_eq!(parked.waiting_step(), Some("after-ship-review"));
        let mut blocked_arrival = arrival;
        blocked_arrival.cursor.leaf_mut().progress.verdict =
            Some(crate::graph::transitions::GraphVerdict {
                decision: crate::graph::transitions::GraphDecision::Blocked,
                summary: "needs a layout decision".into(),
            });
        let blocked = TaskGraph::read(
            &graph,
            "after-ship-review".into(),
            Some(&blocked_arrival),
            None,
            None,
            None,
            test_merges,
        );
        assert_eq!(
            blocked.sentence,
            "At breakpoint after-ship-review. Blocked: needs a layout decision"
        );
        assert_eq!(blocked.control.label, "Blocked: after-ship-review");
    }

    #[test]
    fn a_blocked_breakpoint_names_its_composed_stage_in_the_workflow() {
        let repo = tempfile::tempdir().unwrap();
        write(
            repo.path(),
            ".lf/graphs/parent.yaml",
            "- nested-review\n- break: outer-review\n",
        );
        write(
            repo.path(),
            ".lf/graphs/nested-review.yaml",
            "- implement\n- break: inside-review\n",
        );
        let graph = CapturedGraph::load("parent", repo.path()).unwrap().unwrap();
        let stage = graph.find("nested-review").unwrap();
        assert_eq!(graph.step_name(&stage), "implement");
        let stop = graph.after(&stage).unwrap();
        let mut cursor = stop.cursor();
        cursor.leaf_mut().progress.verdict = Some(crate::graph::transitions::GraphVerdict {
            decision: crate::graph::transitions::GraphDecision::Blocked,
            summary: "Choose a layout".into(),
        });
        let arrival = crate::store::task_work::BreakpointArrival {
            id: loopflow_model::id::LfProcessId::new(),
            cursor,
        };
        let parked = TaskGraph::read(
            &graph,
            "inside-review".into(),
            Some(&arrival),
            None,
            None,
            None,
            test_merges,
        );
        assert_eq!(parked.current_step, Some(0));
        assert_eq!(parked.breakpoints, ["inside-review", "outer-review"]);
        assert_eq!(parked.steps[0].name, "nested-review");
        assert_eq!(parked.steps[0].kind, WorkflowStepKind::StageWithBreakpoint);
        assert_eq!(parked.steps[1].kind, WorkflowStepKind::Breakpoint);
        assert_eq!(parked.control.label, "Blocked: nested-review");
        assert!(matches!(
            parked.position,
            super::TaskGraphPosition::Breakpoint { blocker: Some(reason), .. }
                if reason == "Choose a layout"
        ));
    }

    #[test]
    fn a_paused_workflow_names_the_stopping_step_until_cleanup_finishes() {
        use super::WorkflowAction::{Pausing, Resume};
        use loopflow_storage::sqlite::task_work::PauseStage;
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("code", repo.path()).unwrap().unwrap();
        for (step, pause, running, sentence, action, label) in [
            (
                "implement",
                PauseStage::Stopping,
                true,
                "Pausing at build (implement is stopping).",
                Pausing,
                "Pausing: build · implement",
            ),
            (
                "implement",
                PauseStage::Stopping,
                false,
                "Pausing at build (implement is stopping).",
                Pausing,
                "Pausing: build · implement",
            ),
            (
                "implement",
                PauseStage::Held,
                false,
                "Paused in build at implement. `lf task unpause --run` resumes it.",
                Resume,
                "Paused: build · implement",
            ),
            (
                "land --wait-and-fix",
                PauseStage::Held,
                true,
                "Pausing at deliver (land --wait-and-fix is finishing).",
                Pausing,
                "Pausing: deliver · land --wait-and-fix",
            ),
        ] {
            let cursor = graph.find(step).unwrap().cursor();

            let run = TaskGraphRun {
                cursor: &cursor,
                lf_process_id: "carrier".into(),
                running,
            };
            let reading = TaskGraph::read(
                &graph,
                "start".into(),
                None,
                Some(run),
                None,
                Some(pause),
                test_merges,
            );
            assert_eq!(reading.sentence, sentence);
            assert_eq!(
                (reading.control.action, reading.control.label.as_str()),
                (action, label)
            );
        }
        assert_eq!(
            TaskGraph::read(
                &graph,
                "after-ship-review".into(),
                Some(&crate::store::task_work::BreakpointArrival {
                    id: loopflow_model::id::LfProcessId::new(),
                    cursor: graph.find("after-ship-review").unwrap().cursor(),
                }),
                None,
                Some(AutoState::On),
                Some(PauseStage::Held),
                test_merges,
            )
            .sentence,
            "Paused at after-ship-review. `lf task unpause` lifts the hold."
        );
    }

    #[test]
    fn paused_stop_cursors_honour_auto_and_do_not_promise_resume() {
        use super::WorkflowAction::{Resume, Unpause};
        use loopflow_storage::sqlite::task_work::PauseStage;

        let repo = tempfile::tempdir().unwrap();
        let code = CapturedGraph::load("code", repo.path()).unwrap().unwrap();
        let land = code.find("land --wait-and-fix").unwrap().cursor();
        let implement = code.find("implement").unwrap().cursor();
        let read = |cursor: &crate::graph::runner::ExecutionCursor, auto| {
            TaskGraph::read(
                &code,
                "start".into(),
                None,
                Some(TaskGraphRun {
                    cursor,
                    lf_process_id: "carrier".into(),
                    running: false,
                }),
                auto,
                Some(PauseStage::Held),
                test_merges,
            )
        };

        for (cursor, auto, action, sentence) in [
            (
                &land,
                Some(AutoState::On),
                Resume,
                "Paused in deliver at land --wait-and-fix. `lf task unpause --run` resumes it.",
            ),
            (
                &land,
                None,
                Resume,
                "Paused in deliver at land --wait-and-fix. `lf task unpause --run` resumes it.",
            ),
            (
                &land,
                Some(AutoState::Off),
                Unpause,
                "Paused in deliver at land --wait-and-fix. `lf task unpause` lifts the hold.",
            ),
            (
                &implement,
                Some(AutoState::Off),
                Resume,
                "Paused in build at implement. `lf task unpause --run` resumes it.",
            ),
        ] {
            let reading = read(cursor, auto);
            assert_eq!(reading.control.action, action, "{sentence}");
            assert_eq!(reading.sentence, sentence);
        }

        // A parked review keeps the unpause action regardless of a prior worker cursor.
        write(
            repo.path(),
            ".lf/graphs/demo.yaml",
            "- cmd: help graph\n- break: after-demo\n",
        );
        let demo = CapturedGraph::load("demo", repo.path()).unwrap().unwrap();
        let parked = TaskGraph::read(
            &demo,
            "after-demo".into(),
            Some(&crate::store::task_work::BreakpointArrival {
                id: loopflow_model::id::LfProcessId::new(),
                cursor: demo.find("after-demo").unwrap().cursor(),
            }),
            None,
            None,
            Some(PauseStage::Held),
            test_merges,
        );
        assert_eq!(parked.control.action, Unpause);
        assert_eq!(parked.control.label, "Paused: after-demo");
        assert_eq!(
            parked.sentence,
            "Paused at after-demo. `lf task unpause` lifts the hold."
        );
    }

    #[test]
    fn a_step_two_parts_share_is_placed_by_the_cursor_not_its_name() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("code", repo.path()).unwrap().unwrap();
        let labels: Vec<_> = graph
            .steps
            .iter()
            .enumerate()
            .filter(|(_, step)| super::step_name(step) == "concept-review")
            .map(|(index, _)| {
                let cursor = super::Place {
                    index,
                    path: Vec::new(),
                }
                .cursor();
                let line = TaskGraph::read(
                    &graph,
                    "plan".into(),
                    None,
                    Some(TaskGraphRun {
                        cursor: &cursor,
                        lf_process_id: "lfp_1".into(),
                        running: true,
                    }),
                    None,
                    None,
                    test_merges,
                );
                line.steps[line.current_step.unwrap() as usize].name.clone()
            })
            .collect();
        assert_eq!(labels, ["plan", "build"]);
    }

    #[test]
    fn reaches_landing_follows_landing_steps_through_stops_xors_and_loops() {
        let repo = tempfile::tempdir().unwrap();
        let load = |name| CapturedGraph::load(name, repo.path()).unwrap().unwrap();

        // Builtin code: stops at ship-review before deliver; reaches land after it or with through.
        let code = load("code");
        let start = code.find("start").unwrap();
        assert!(!code.reaches_landing(&start, false, test_merges));
        assert!(code.reaches_landing(&start, true, test_merges));
        let after_review = code
            .after(&code.find("after-ship-review").unwrap())
            .unwrap();
        assert!(code.reaches_landing(&after_review, false, test_merges));

        // Builtin feature: stops at explore, then ship-review, then lands in deliver.
        let feature = load("feature");
        let start = feature.find("start").unwrap();
        assert!(!feature.reaches_landing(&start, false, test_merges));
        assert!(feature.reaches_landing(&start, true, test_merges));
        let after_explore = feature
            .after(&feature.find("after-explore").unwrap())
            .unwrap();
        assert!(!feature.reaches_landing(&after_explore, false, test_merges));
        assert!(feature.reaches_landing(&after_explore, true, test_merges));
        let after_review = feature
            .after(&feature.find("after-ship-review").unwrap())
            .unwrap();
        assert!(feature.reaches_landing(&after_review, false, test_merges));

        // Quick reaches landing without stopping.
        let quick = load("quick");
        assert!(quick.reaches_landing(&quick.find("start").unwrap(), false, test_merges));

        // Non-merging builtins never report reaching a landing step.
        for name in ["pursue", "research", "deep-clean", "incident"] {
            let graph = load(name);
            let start = graph.find("start").unwrap();
            assert!(!graph.reaches_landing(&start, false, test_merges), "{name}");
            assert!(!graph.reaches_landing(&start, true, test_merges), "{name}");
        }

        // Custom post-merge-stop Graph warns when crossing land, not before review or at post-merge stop.
        write(
            repo.path(),
            ".lf/graphs/post-merge.yaml",
            "- implement\n- interactive: demo\n- cmd: land\n- interactive: research\n",
        );
        let post_merge = load("post-merge");
        let start = post_merge.find("start").unwrap();
        assert!(!post_merge.reaches_landing(&start, false, test_merges));
        assert!(post_merge.reaches_landing(&start, true, test_merges));
        let after_demo = post_merge
            .after(&post_merge.find("after-demo").unwrap())
            .unwrap();
        assert!(post_merge.reaches_landing(&after_demo, false, test_merges));
        let post_stop = post_merge.find("after-research").unwrap();
        assert!(!post_merge.reaches_landing(&post_stop, false, test_merges));
        assert!(!post_merge.reaches_landing(&post_stop, true, test_merges));

        // A leading breakpoint also gates a merging skill.
        write(
            repo.path(),
            ".lf/graphs/review-before-land.yaml",
            "- break: review\n- pr-land\n",
        );
        let review_before_land = load("review-before-land");
        let start = review_before_land.find("start").unwrap();
        assert!(!review_before_land.reaches_landing(&start, false, test_merges));
        assert!(review_before_land.reaches_landing(&start, true, test_merges));

        // XOR: any branch reaching land returns true; a trailing land is only blocked when every branch stops.
        write(
            repo.path(),
            ".lf/graphs/xor-merge.yaml",
            "- xor:\n    paths:\n      ship:\n        description: Land it\n        steps:\n          - pr-land\n      hold:\n        description: Wait\n        steps:\n          - interactive: demo\n",
        );
        let xor_merge = load("xor-merge");
        assert!(xor_merge.reaches_landing(&xor_merge.find("start").unwrap(), false, test_merges));

        write(
            repo.path(),
            ".lf/graphs/xor-gated.yaml",
            "- xor:\n    paths:\n      one:\n        description: First\n        steps:\n          - interactive: demo\n          - implement\n      two:\n        description: Second\n        steps:\n          - interactive: research\n- cmd: land\n",
        );
        let xor_gated = load("xor-gated");
        let start = xor_gated.find("start").unwrap();
        assert!(!xor_gated.reaches_landing(&start, false, test_merges));
        assert!(xor_gated.reaches_landing(&start, true, test_merges));
        // Resuming inside an XOR branch ascends to the trailing `cmd: land`.
        let inside_xor = xor_gated
            .after(&xor_gated.find("after-demo").unwrap())
            .unwrap();
        assert!(xor_gated.reaches_landing(&inside_xor, false, test_merges));

        write(
            repo.path(),
            ".lf/graphs/xor-partial.yaml",
            "- xor:\n    paths:\n      review:\n        description: Review\n        steps:\n          - interactive: demo\n      skip:\n        description: Skip\n- cmd: land\n",
        );
        let xor_partial = load("xor-partial");
        assert!(xor_partial.reaches_landing(
            &xor_partial.find("start").unwrap(),
            false,
            test_merges
        ));

        // A loop breakpoint can return to a landing step.
        write(
            repo.path(),
            ".lf/graphs/loop-back.yaml",
            "- loop:\n    - cmd: land\n    - interactive: demo\n",
        );
        let loop_back = load("loop-back");
        let demo = loop_back.find("after-demo").unwrap();
        assert!(!loop_back.reaches_landing(&demo, false, test_merges));
        assert!(loop_back.reaches_landing(&demo, true, test_merges));
    }
}
