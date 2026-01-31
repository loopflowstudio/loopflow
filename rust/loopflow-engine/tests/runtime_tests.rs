use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

use lf_core::runtime::{
    tick_flow_with_runner, FlowRun, FlowRunStatus, ForkRun, ForkRunStatus, StepResult, StepRun,
    StepRunStatus, TickResult,
};
use lf_core::{load_flow, RunId, RunStore, Step};
use tempfile::TempDir;

struct MemoryStore {
    runs: Mutex<HashMap<RunId, FlowRun>>,
    step_runs: Mutex<Vec<StepRun>>,
    fork_runs: Mutex<HashMap<(RunId, usize, usize), ForkRun>>,
}

impl MemoryStore {
    fn new(run: FlowRun) -> Self {
        let mut map = HashMap::new();
        map.insert(run.id.clone(), run);
        Self {
            runs: Mutex::new(map),
            step_runs: Mutex::new(Vec::new()),
            fork_runs: Mutex::new(HashMap::new()),
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

    fn list_fork_runs(
        &self,
        run_id: &RunId,
        step_index: usize,
    ) -> Result<Vec<ForkRun>, lf_core::StoreError> {
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

    fn upsert_fork_run(&self, fork_run: &ForkRun) -> Result<(), lf_core::StoreError> {
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
    ) -> Result<(), lf_core::StoreError> {
        let mut fork_runs = self.fork_runs.lock().unwrap();
        fork_runs.retain(|(id, step, _), _| id != run_id || *step != step_index);
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
