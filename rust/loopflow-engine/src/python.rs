//! PyO3 bindings for loopflow-engine.
//!
//! Exposes the Rust engine to Python via PyO3. This allows Python `lf` to
//! import loopflow_engine directly without subprocess overhead.

use pyo3::prelude::*;
use std::path::PathBuf;

use crate::agent::{launch_agent, LaunchConfig, LaunchResult as RustLaunchResult};
use crate::config::{load_config_or_default, parse_model};
use crate::git::{
    commit as git_commit, create_branch as git_create_branch, delete_local_branch,
    delete_remote_branch, get_default_branch, is_clean as git_is_clean, land as git_land,
    pr_create_draft, pr_exists, pr_merge_squash_auto, push as git_push,
    push_with_upstream as git_push_with_upstream, rebase as git_rebase, rev_parse, stage_all,
    sync_main, worktree_add, worktree_move, worktree_remove, BranchInfo as RustBranchInfo,
    LandResult as RustLandResult, LandStrategy as RustLandStrategy,
    RebaseResult as RustRebaseResult,
};
use crate::prompt::{
    count_tokens, format_prompt, gather_context, GatherContextOpts,
    PromptComponents as RustPromptComponents,
};

/// Python-exposed launch result.
#[pyclass]
#[derive(Debug, Clone)]
pub struct PyLaunchResult {
    #[pyo3(get)]
    pub exit_code: i32,
    #[pyo3(get)]
    pub stdout: String,
    #[pyo3(get)]
    pub stderr: String,
}

impl From<RustLaunchResult> for PyLaunchResult {
    fn from(r: RustLaunchResult) -> Self {
        Self {
            exit_code: r.exit_code,
            stdout: r.stdout,
            stderr: r.stderr,
        }
    }
}

/// Python-exposed prompt components.
#[pyclass]
#[derive(Debug, Clone)]
pub struct PyPromptComponents {
    inner: RustPromptComponents,
}

/// Python-exposed git rebase result.
#[pyclass]
#[derive(Debug, Clone)]
pub struct PyRebaseResult {
    #[pyo3(get)]
    pub success: bool,
    #[pyo3(get)]
    pub conflicts: Option<Vec<PathBuf>>,
    #[pyo3(get)]
    pub new_head: Option<String>,
}

impl From<RustRebaseResult> for PyRebaseResult {
    fn from(result: RustRebaseResult) -> Self {
        Self {
            success: result.success,
            conflicts: result.conflicts,
            new_head: result.new_head,
        }
    }
}

/// Python-exposed branch info.
#[pyclass]
#[derive(Debug, Clone)]
pub struct PyBranchInfo {
    #[pyo3(get)]
    pub old_branch: String,
    #[pyo3(get)]
    pub old_head: String,
    #[pyo3(get)]
    pub new_branch: String,
}

impl From<RustBranchInfo> for PyBranchInfo {
    fn from(info: RustBranchInfo) -> Self {
        Self {
            old_branch: info.old_branch,
            old_head: info.old_head,
            new_branch: info.new_branch,
        }
    }
}

/// Python-exposed land result.
#[pyclass]
#[derive(Debug, Clone)]
pub struct PyLandResult {
    #[pyo3(get)]
    pub merged_commit: String,
    #[pyo3(get)]
    pub branch_deleted: bool,
}

impl From<RustLandResult> for PyLandResult {
    fn from(result: RustLandResult) -> Self {
        Self {
            merged_commit: result.merged_commit,
            branch_deleted: result.branch_deleted,
        }
    }
}

#[pymethods]
impl PyPromptComponents {
    #[getter]
    fn run_mode(&self) -> Option<String> {
        self.inner.run_mode.clone()
    }

    #[getter]
    fn repo_root(&self) -> String {
        self.inner.repo_root.clone()
    }

    #[getter]
    fn clipboard(&self) -> Option<String> {
        self.inner.clipboard.clone()
    }

    #[getter]
    fn wave(&self) -> Option<String> {
        self.inner.wave.clone()
    }

    /// Get token count.
    fn token_count(&self) -> usize {
        crate::prompt::analyze_tokens(&self.inner)
    }

    /// Format into final prompt string.
    fn format(&self) -> String {
        format_prompt(&self.inner)
    }
}

/// Count tokens in text using tiktoken (cl100k_base).
#[pyfunction]
fn py_count_tokens(text: &str) -> usize {
    count_tokens(text)
}

/// Parse model string into (backend, variant).
#[pyfunction]
fn py_parse_model(model: &str) -> (String, Option<String>) {
    parse_model(model)
}

/// Gather prompt context.
#[pyfunction]
#[pyo3(signature = (repo_root, step=None, directions=None, run_mode=None, lfdocs=true, diff_files=true, diff=false, clipboard=false, area=None, wave=None))]
fn py_gather_context(
    repo_root: &str,
    step: Option<&str>,
    directions: Option<Vec<String>>,
    run_mode: Option<&str>,
    lfdocs: bool,
    diff_files: bool,
    diff: bool,
    clipboard: bool,
    area: Option<&str>,
    wave: Option<&str>,
) -> PyResult<PyPromptComponents> {
    let opts = GatherContextOpts {
        repo_root: PathBuf::from(repo_root),
        step: step.map(String::from),
        inline: None,
        step_args: Vec::new(),
        run_mode: run_mode.map(String::from),
        directions: directions.unwrap_or_default(),
        lfdocs,
        diff_files,
        diff,
        clipboard,
        area: area.map(String::from),
        wave: wave.map(String::from),
    };

    match gather_context(&opts) {
        Ok(components) => Ok(PyPromptComponents { inner: components }),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Launch an agent with the given prompt.
#[pyfunction]
#[pyo3(signature = (model, prompt, auto=true, stream=false, skip_permissions=false, model_variant=None, chrome=false, cwd=None))]
fn py_launch_agent(
    model: &str,
    prompt: &str,
    auto: bool,
    stream: bool,
    skip_permissions: bool,
    model_variant: Option<String>,
    chrome: bool,
    cwd: Option<&str>,
) -> PyResult<PyLaunchResult> {
    let config = LaunchConfig {
        auto,
        stream,
        skip_permissions,
        model_variant,
        chrome,
        cwd: cwd.map(PathBuf::from),
    };

    match launch_agent(model, prompt, &config) {
        Ok(result) => Ok(result.into()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: rebase worktree.
#[pyfunction]
#[pyo3(signature = (worktree, onto, base_commit=None))]
fn py_git_rebase(
    worktree: &str,
    onto: &str,
    base_commit: Option<&str>,
) -> PyResult<PyRebaseResult> {
    match git_rebase(PathBuf::from(worktree).as_path(), onto, base_commit) {
        Ok(result) => Ok(result.into()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: push worktree.
#[pyfunction]
#[pyo3(signature = (worktree, force_with_lease=false))]
fn py_git_push(worktree: &str, force_with_lease: bool) -> PyResult<()> {
    match git_push(PathBuf::from(worktree).as_path(), force_with_lease) {
        Ok(()) => Ok(()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: push and set upstream.
#[pyfunction]
fn py_git_push_with_upstream(worktree: &str, remote: &str, branch: &str) -> PyResult<()> {
    match git_push_with_upstream(PathBuf::from(worktree).as_path(), remote, branch) {
        Ok(()) => Ok(()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: create branch.
#[pyfunction]
fn py_git_create_branch(worktree: &str, name: &str) -> PyResult<PyBranchInfo> {
    match git_create_branch(PathBuf::from(worktree).as_path(), name) {
        Ok(info) => Ok(info.into()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: land branch.
#[pyfunction]
#[pyo3(signature = (worktree, strategy, main_branch="main"))]
fn py_git_land(worktree: &str, strategy: &str, main_branch: &str) -> PyResult<PyLandResult> {
    let parsed = match strategy {
        "squash" | "squash_merge" => RustLandStrategy::SquashMerge,
        "local" | "local_merge" => RustLandStrategy::LocalMerge,
        _ => {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Invalid land strategy",
            ))
        }
    };
    match git_land(PathBuf::from(worktree).as_path(), parsed, main_branch) {
        Ok(result) => Ok(result.into()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: get default branch.
#[pyfunction]
fn py_git_get_default_branch(repo: &str) -> PyResult<String> {
    match get_default_branch(PathBuf::from(repo).as_path()) {
        Ok(branch) => Ok(branch),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: check if repo is clean.
#[pyfunction]
fn py_git_is_clean(repo: &str) -> PyResult<bool> {
    match git_is_clean(PathBuf::from(repo).as_path()) {
        Ok(clean) => Ok(clean),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: stage all.
#[pyfunction]
fn py_git_stage_all(repo: &str) -> PyResult<()> {
    match stage_all(PathBuf::from(repo).as_path()) {
        Ok(()) => Ok(()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: commit.
#[pyfunction]
fn py_git_commit(repo: &str, message: &str) -> PyResult<()> {
    match git_commit(PathBuf::from(repo).as_path(), message) {
        Ok(()) => Ok(()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: delete remote branch.
#[pyfunction]
fn py_git_delete_remote_branch(repo: &str, remote: &str, branch: &str) -> PyResult<()> {
    match delete_remote_branch(PathBuf::from(repo).as_path(), remote, branch) {
        Ok(()) => Ok(()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: delete local branch.
#[pyfunction]
fn py_git_delete_local_branch(repo: &str, branch: &str) -> PyResult<()> {
    match delete_local_branch(PathBuf::from(repo).as_path(), branch) {
        Ok(()) => Ok(()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: check if PR exists.
#[pyfunction]
fn py_git_pr_exists(repo: &str) -> PyResult<bool> {
    match pr_exists(PathBuf::from(repo).as_path()) {
        Ok(exists) => Ok(exists),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: create draft PR.
#[pyfunction]
fn py_git_pr_create_draft(repo: &str) -> PyResult<String> {
    match pr_create_draft(PathBuf::from(repo).as_path()) {
        Ok(url) => Ok(url),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: enable squash auto-merge.
#[pyfunction]
fn py_git_pr_merge_squash_auto(repo: &str) -> PyResult<()> {
    match pr_merge_squash_auto(PathBuf::from(repo).as_path()) {
        Ok(()) => Ok(()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: sync main branch.
#[pyfunction]
#[pyo3(signature = (repo, main_branch="main"))]
fn py_git_sync_main(repo: &str, main_branch: &str) -> PyResult<bool> {
    match sync_main(PathBuf::from(repo).as_path(), main_branch) {
        Ok(result) => Ok(result),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: remove worktree.
#[pyfunction]
fn py_git_worktree_remove(repo: &str, path: &str) -> PyResult<()> {
    match worktree_remove(PathBuf::from(repo).as_path(), PathBuf::from(path).as_path()) {
        Ok(()) => Ok(()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: move worktree.
#[pyfunction]
fn py_git_worktree_move(repo: &str, old_path: &str, new_path: &str) -> PyResult<()> {
    match worktree_move(
        PathBuf::from(repo).as_path(),
        PathBuf::from(old_path).as_path(),
        PathBuf::from(new_path).as_path(),
    ) {
        Ok(()) => Ok(()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: create worktree with new branch.
#[pyfunction]
fn py_git_worktree_add(repo: &str, path: &str, branch: &str, start_point: &str) -> PyResult<()> {
    match worktree_add(
        PathBuf::from(repo).as_path(),
        PathBuf::from(path).as_path(),
        branch,
        start_point,
    ) {
        Ok(()) => Ok(()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Git: get SHA for ref.
#[pyfunction]
fn py_git_rev_parse(repo: &str, refspec: &str) -> PyResult<String> {
    match rev_parse(PathBuf::from(repo).as_path(), refspec) {
        Ok(sha) => Ok(sha),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// Run a step with assembled context.
///
/// This is the main entry point for Python lf to execute steps via Rust.
#[pyfunction]
#[pyo3(signature = (step, repo_root=None, directions=None, auto=true, clipboard=false, model=None))]
fn run_step(
    step: &str,
    repo_root: Option<&str>,
    directions: Option<Vec<String>>,
    auto: bool,
    clipboard: bool,
    model: Option<&str>,
) -> PyResult<PyLaunchResult> {
    let repo = repo_root
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let config = load_config_or_default(Some(&repo));
    let model_str = model.unwrap_or(&config.agent_model);
    let run_mode = if auto { Some("auto") } else { None };

    // Gather context
    let opts = GatherContextOpts {
        repo_root: repo.clone(),
        step: Some(step.to_string()),
        inline: None,
        step_args: Vec::new(),
        run_mode: run_mode.map(String::from),
        directions: directions.unwrap_or_default(),
        lfdocs: config.lfdocs,
        diff_files: config.diff_files,
        diff: config.diff,
        clipboard,
        area: config.area.clone(),
        wave: None,
    };

    let components = gather_context(&opts)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

    let prompt = format_prompt(&components);

    // Launch agent
    let launch_config = LaunchConfig {
        auto,
        stream: false,
        skip_permissions: config.yolo,
        model_variant: None,
        chrome: config.chrome,
        cwd: Some(repo),
    };

    match launch_agent(model_str, &prompt, &launch_config) {
        Ok(result) => Ok(result.into()),
        Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
    }
}

/// The loopflow_engine Python module.
#[pymodule]
fn loopflow_engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLaunchResult>()?;
    m.add_class::<PyPromptComponents>()?;
    m.add_class::<PyRebaseResult>()?;
    m.add_class::<PyBranchInfo>()?;
    m.add_class::<PyLandResult>()?;
    m.add_function(wrap_pyfunction!(py_count_tokens, m)?)?;
    m.add_function(wrap_pyfunction!(py_parse_model, m)?)?;
    m.add_function(wrap_pyfunction!(py_gather_context, m)?)?;
    m.add_function(wrap_pyfunction!(py_launch_agent, m)?)?;
    m.add_function(wrap_pyfunction!(run_step, m)?)?;
    let git_mod = PyModule::new_bound(m.py(), "git")?;
    git_mod.add_function(wrap_pyfunction!(py_git_rebase, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_push, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_push_with_upstream, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_create_branch, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_land, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_get_default_branch, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_is_clean, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_stage_all, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_commit, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_delete_remote_branch, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_delete_local_branch, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_pr_exists, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_pr_create_draft, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_pr_merge_squash_auto, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_sync_main, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_worktree_remove, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_worktree_move, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_worktree_add, &git_mod)?)?;
    git_mod.add_function(wrap_pyfunction!(py_git_rev_parse, &git_mod)?)?;
    m.add_submodule(&git_mod)?;
    Ok(())
}
