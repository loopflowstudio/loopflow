use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

use lf_core::runtime::{
    tick_flow_with_runner, FlowRun, FlowRunStatus, StepResult, StepRun, StepRunStatus, TickResult,
};
use lf_core::{load_flow, RunId, RunStore, Step};
use tempfile::TempDir;

struct MemoryStore {
    runs: Mutex<HashMap<RunId, FlowRun>>,
    step_runs: Mutex<Vec<StepRun>>,
}

impl MemoryStore {
    fn new(run: FlowRun) -> Self {
        let mut map = HashMap::new();
        map.insert(run.id.clone(), run);
        Self {
            runs: Mutex::new(map),
            step_runs: Mutex::new(Vec::new()),
        }
    }

    fn get_run_copy(&self, id: &RunId) -> FlowRun {
        self.runs.lock().unwrap().get(id).unwrap().clone()
    }

    fn step_runs(&self) -> Vec<StepRun> {
        self.step_runs.lock().unwrap().clone()
    }
}

impl RunStore for MemoryStore {
    fn get_run(&self, id: &RunId) -> Result<FlowRun, lf_core::StoreError> {
        self.runs
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| lf_core::StoreError::RunNotFound(id.to_string()))
    }

    fn update_run(&self, run: &FlowRun) -> Result<(), lf_core::StoreError> {
        self.runs
            .lock()
            .unwrap()
            .insert(run.id.clone(), run.clone());
        Ok(())
    }

    fn create_step_run(&self, step_run: &StepRun) -> Result<(), lf_core::StoreError> {
        self.step_runs.lock().unwrap().push(step_run.clone());
        Ok(())
    }
}

struct FakeRunner {
    exit_code: i32,
}

impl lf_core::runtime::StepRunner for FakeRunner {
    fn run(
        &self,
        _step: &Step,
        _worktree: &std::path::Path,
        _directions: &[String],
    ) -> Result<StepResult, lf_core::CoreError> {
        Ok(StepResult {
            exit_code: self.exit_code,
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

    let step_runs = store.step_runs();
    assert_eq!(step_runs.len(), 1);
    assert_eq!(step_runs[0].status, StepRunStatus::Waiting);
}
