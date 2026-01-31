use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

use loopflow_engine::runtime::{
    tick_flow_with_runner, Agent, AgentStatus, FlowRun, FlowRunStatus, ForkRun, ForkRunStatus,
    StepResult, TickResult,
};
use loopflow_engine::{load_flow, RunId, RunStore, Step};
use tempfile::TempDir;

struct MemoryStore {
    runs: Mutex<HashMap<RunId, FlowRun>>,
    agents: Mutex<Vec<Agent>>,
    fork_runs: Mutex<HashMap<(RunId, usize, usize), ForkRun>>,
}

impl MemoryStore {
    fn new(run: FlowRun) -> Self {
        let mut map = HashMap::new();
        map.insert(run.id.clone(), run);
        Self {
            runs: Mutex::new(map),
            agents: Mutex::new(Vec::new()),
            fork_runs: Mutex::new(HashMap::new()),
        }
    }

    fn get_run_copy(&self, id: &RunId) -> FlowRun {
        self.runs.lock().unwrap().get(id).unwrap().clone()
    }

    fn agents(&self) -> Vec<Agent> {
        self.agents.lock().unwrap().clone()
    }
}

impl RunStore for MemoryStore {
    fn get_run(&self, id: &RunId) -> Result<FlowRun, loopflow_engine::StoreError> {
        self.runs
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| loopflow_engine::StoreError::RunNotFound(id.to_string()))
    }

    fn update_run(&self, run: &FlowRun) -> Result<(), loopflow_engine::StoreError> {
        self.runs
            .lock()
            .unwrap()
            .insert(run.id.clone(), run.clone());
        Ok(())
    }

    fn create_agent(&self, agent: &Agent) -> Result<(), loopflow_engine::StoreError> {
        self.agents.lock().unwrap().push(agent.clone());
        Ok(())
    }

    fn list_fork_runs(
        &self,
        run_id: &RunId,
        step_index: usize,
    ) -> Result<Vec<ForkRun>, loopflow_engine::StoreError> {
        let runs = self
            .fork_runs
            .lock()
            .unwrap()
            .values()
            .filter(|fork| fork.run_id == *run_id && fork.step_index == step_index)
            .cloned()
            .collect();
        Ok(runs)
    }

    fn upsert_fork_run(&self, fork_run: &ForkRun) -> Result<(), loopflow_engine::StoreError> {
        self.fork_runs.lock().unwrap().insert(
            (
                fork_run.run_id.clone(),
                fork_run.step_index,
                fork_run.branch_index,
            ),
            fork_run.clone(),
        );
        Ok(())
    }

    fn delete_fork_runs(
        &self,
        run_id: &RunId,
        step_index: usize,
    ) -> Result<(), loopflow_engine::StoreError> {
        let mut fork_runs = self.fork_runs.lock().unwrap();
        fork_runs.retain(|(id, step, _), _| id != run_id || *step != step_index);
        Ok(())
    }
}

struct FakeRunner {
    exit_code: i32,
}

impl loopflow_engine::runtime::StepRunner for FakeRunner {
    fn run(
        &self,
        _step: &Step,
        _worktree: &std::path::Path,
        _directions: &[String],
    ) -> Result<StepResult, loopflow_engine::CoreError> {
        Ok(StepResult {
            exit_code: self.exit_code,
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

struct LoopRunner {
    repo: std::path::PathBuf,
    runs: Mutex<usize>,
}

impl loopflow_engine::runtime::StepRunner for LoopRunner {
    fn run(
        &self,
        _step: &Step,
        _worktree: &std::path::Path,
        _directions: &[String],
    ) -> Result<StepResult, loopflow_engine::CoreError> {
        let mut runs = self.runs.lock().unwrap();
        *runs += 1;
        if *runs == 1 {
            let wave_dir = self.repo.join("roadmap").join("testwave");
            let item = wave_dir.join("item.md");
            let _ = std::fs::remove_file(item);
        }
        Ok(StepResult {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

fn write_flow(repo: &std::path::Path, name: &str, content: &str) {
    let flows_dir = repo.join(".lf/flows");
    fs::create_dir_all(&flows_dir).unwrap();
    fs::write(flows_dir.join(format!("{name}.yaml")), content).unwrap();
    load_flow(name, repo).unwrap();
}

#[test]
fn tick_auto_flow_end_to_end() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(repo, "auto", "- implement\n- polish\n");

    let run_id = RunId::new("run-1");
    let run = FlowRun {
        id: run_id.clone(),
        flow: "auto".to_string(),
        direction: vec!["product-engineer".to_string()],
        area: vec![".".to_string()],
        repo: repo.to_path_buf(),
        status: FlowRunStatus::Running,
        step_index: 0,
        worktree: None,
        current_step: None,
        error: None,
    };
    let store = MemoryStore::new(run);
    let runner = FakeRunner { exit_code: 0 };

    let first = tick_flow_with_runner(&run_id, &store, &runner).unwrap();
    assert_eq!(first, TickResult::StepComplete);
    let second = tick_flow_with_runner(&run_id, &store, &runner).unwrap();
    assert_eq!(second, TickResult::StepComplete);
    let third = tick_flow_with_runner(&run_id, &store, &runner).unwrap();
    assert_eq!(third, TickResult::FlowComplete);
    let updated = store.get_run_copy(&run_id);
    assert_eq!(updated.status, FlowRunStatus::Completed);
}

#[test]
fn tick_interactive_pauses() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "interactive",
        "- step:\n    name: design\n    interactive: true\n",
    );

    let run_id = RunId::new("run-2");
    let run = FlowRun {
        id: run_id.clone(),
        flow: "interactive".to_string(),
        direction: vec!["designer".to_string()],
        area: vec![".".to_string()],
        repo: repo.to_path_buf(),
        status: FlowRunStatus::Running,
        step_index: 0,
        worktree: None,
        current_step: None,
        error: None,
    };
    let store = MemoryStore::new(run);
    let runner = FakeRunner { exit_code: 0 };

    let result = tick_flow_with_runner(&run_id, &store, &runner).unwrap();
    assert_eq!(result, TickResult::WaitingInteractive);
    let updated = store.get_run_copy(&run_id);
    assert_eq!(updated.status, FlowRunStatus::Waiting);
    assert_eq!(updated.step_index, 0);

    let agents = store.agents();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].status, AgentStatus::Waiting);
}

#[test]
fn tick_fork_advances_after_branches() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "forked",
        r#"
- fork:
    branches:
      - step: { name: implement }
      - step: { name: polish }
    synthesize: consolidate
"#,
    );

    let run_id = RunId::new("run-fork");
    let run = FlowRun {
        id: run_id.clone(),
        flow: "forked".to_string(),
        direction: vec!["product-engineer".to_string()],
        area: vec![".".to_string()],
        repo: repo.to_path_buf(),
        status: FlowRunStatus::Running,
        step_index: 0,
        worktree: None,
        current_step: None,
        error: None,
    };

    let fork0 = std::path::PathBuf::from(format!("{}-fork-0", repo.display()));
    let fork1 = std::path::PathBuf::from(format!("{}-fork-1", repo.display()));
    std::fs::create_dir_all(&fork0).unwrap();
    std::fs::create_dir_all(&fork1).unwrap();

    let store = MemoryStore::new(run);
    let runner = FakeRunner { exit_code: 0 };

    let result = tick_flow_with_runner(&run_id, &store, &runner).unwrap();
    assert_eq!(result, TickResult::StepComplete);
    let updated = store.get_run_copy(&run_id);
    assert_eq!(updated.step_index, 1);
    assert_eq!(updated.status, FlowRunStatus::Running);

    let fork_runs = store
        .fork_runs
        .lock()
        .unwrap()
        .values()
        .filter(|fork| fork.status != ForkRunStatus::Completed)
        .count();
    assert_eq!(fork_runs, 0);
}

#[test]
fn tick_choose_selects_branch() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "choose-flow",
        r#"
- choose:
    prompt: pick
    options:
      alpha:
        - step: { name: implement }
      beta:
        - step: { name: polish }
"#,
    );

    let run_id = RunId::new("run-choose");
    let run = FlowRun {
        id: run_id.clone(),
        flow: "choose-flow".to_string(),
        direction: vec!["product-engineer".to_string()],
        area: vec![".".to_string()],
        repo: repo.to_path_buf(),
        status: FlowRunStatus::Running,
        step_index: 0,
        worktree: None,
        current_step: None,
        error: None,
    };

    let store = MemoryStore::new(run);
    let runner = FakeRunner { exit_code: 0 };

    let first = tick_flow_with_runner(&run_id, &store, &runner).unwrap();
    assert_eq!(first, TickResult::StepComplete);
    let updated = store.get_run_copy(&run_id);
    assert_eq!(updated.step_index, 1);

    let steps = store.agents();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].step, "implement");

    let done = tick_flow_with_runner(&run_id, &store, &runner).unwrap();
    assert_eq!(done, TickResult::FlowComplete);
}

#[test]
fn tick_loop_until_empty_terminates() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "looped",
        r#"
- loop_until_empty:
    wave: testwave
    max_iterations: 3
    steps:
      - step: { name: implement }
"#,
    );

    let wave_dir = repo.join("roadmap").join("testwave");
    fs::create_dir_all(&wave_dir).unwrap();
    fs::write(wave_dir.join("item.md"), "todo").unwrap();

    let run_id = RunId::new("run-loop");
    let run = FlowRun {
        id: run_id.clone(),
        flow: "looped".to_string(),
        direction: vec!["product-engineer".to_string()],
        area: vec![".".to_string()],
        repo: repo.to_path_buf(),
        status: FlowRunStatus::Running,
        step_index: 0,
        worktree: None,
        current_step: None,
        error: None,
    };

    let store = MemoryStore::new(run);
    let runner = LoopRunner {
        repo: repo.to_path_buf(),
        runs: Mutex::new(0),
    };

    let result = tick_flow_with_runner(&run_id, &store, &runner).unwrap();
    assert_eq!(result, TickResult::StepComplete);
    let updated = store.get_run_copy(&run_id);
    assert_eq!(updated.step_index, 1);
}
