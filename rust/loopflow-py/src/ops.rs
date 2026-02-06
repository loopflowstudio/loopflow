use pyo3::prelude::*;

use loopflow_ops::{
    abandon_branch, commit_workflow, create_or_update_pr, land, next_branch, rebase_with_recovery,
    AbandonOptions, CommitOptions, LandOptions, NextOptions, PrOptions, RebaseOptions,
};

#[pyclass]
#[derive(Debug, Clone)]
pub struct PyPrResult {
    #[pyo3(get)]
    pub url: String,
    #[pyo3(get)]
    pub created: bool,
    #[pyo3(get)]
    pub updated: bool,
}

#[pyclass]
#[derive(Debug, Clone)]
pub struct PyLandResult {
    #[pyo3(get)]
    pub merged: bool,
}

#[pyclass]
#[derive(Debug, Clone)]
pub struct PyNextResult {
    #[pyo3(get)]
    pub new_branch: String,
}

#[pyclass]
#[derive(Debug, Clone)]
pub struct PyRebaseResult {
    #[pyo3(get)]
    pub success: bool,
}

#[pyfunction]
#[pyo3(
    signature = (
        repo,
        add=true,
        lint=true,
        push=false,
        create_draft_pr=true,
        task="commit",
        flow_parents=None,
        message=None
    )
)]
#[allow(clippy::too_many_arguments)]
fn py_commit_workflow(
    repo: &str,
    add: bool,
    lint: bool,
    push: bool,
    create_draft_pr: bool,
    task: &str,
    flow_parents: Option<Vec<String>>,
    message: Option<&str>,
) -> PyResult<bool> {
    let options = CommitOptions {
        add,
        lint,
        push,
        create_draft_pr,
        task: task.to_string(),
        flow_parents: flow_parents.unwrap_or_default(),
        message: message.map(str::to_string),
    };

    commit_workflow(
        std::path::Path::new(repo),
        &options,
        &loopflow_ops::NullProgress,
    )
    .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
#[pyo3(signature = (repo, refresh=false, lint=true))]
fn py_create_or_update_pr(repo: &str, refresh: bool, lint: bool) -> PyResult<PyPrResult> {
    let options = PrOptions { refresh, lint };
    create_or_update_pr(
        std::path::Path::new(repo),
        &options,
        &loopflow_ops::NullProgress,
    )
    .map(|result| PyPrResult {
        url: result.url,
        created: result.created,
        updated: result.updated,
    })
    .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
#[pyo3(
    signature = (repo, strict=false, local=false, create_pr=false, worktree=None, lint=true)
)]
fn py_land(
    repo: &str,
    strict: bool,
    local: bool,
    create_pr: bool,
    worktree: Option<&str>,
    lint: bool,
) -> PyResult<PyLandResult> {
    let options = LandOptions {
        strict,
        local,
        create_pr,
        worktree: worktree.map(str::to_string),
        lint,
    };

    land(
        std::path::Path::new(repo),
        &options,
        &loopflow_ops::NullProgress,
    )
    .map(|result| PyLandResult {
        merged: result.merged,
    })
    .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
#[pyo3(signature = (repo, block=false, create_pr=false, rebase=true, wave_name=None))]
fn py_next_branch(
    repo: &str,
    block: bool,
    create_pr: bool,
    rebase: bool,
    wave_name: Option<&str>,
) -> PyResult<PyNextResult> {
    let options = NextOptions {
        block,
        create_pr,
        rebase,
        wave_name: wave_name.map(str::to_string),
    };

    next_branch(
        std::path::Path::new(repo),
        &options,
        &loopflow_ops::NullProgress,
    )
    .map(|result| PyNextResult {
        new_branch: result.new_branch,
    })
    .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
#[pyo3(signature = (repo, onto, push=true))]
fn py_rebase_with_recovery(repo: &str, onto: &str, push: bool) -> PyResult<PyRebaseResult> {
    let options = RebaseOptions {
        onto: onto.to_string(),
        push,
    };
    rebase_with_recovery(
        std::path::Path::new(repo),
        &options,
        &loopflow_ops::NullProgress,
    )
    .map(|result| PyRebaseResult {
        success: result.success,
    })
    .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
#[pyo3(signature = (repo, branch=None, force=false))]
fn py_abandon_branch(repo: &str, branch: Option<&str>, force: bool) -> PyResult<()> {
    let options = AbandonOptions {
        branch: branch.map(str::to_string),
        force,
    };
    abandon_branch(
        std::path::Path::new(repo),
        &options,
        &loopflow_ops::NullProgress,
    )
    .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

pub fn register(parent: &Bound<PyModule>) -> PyResult<()> {
    let ops_mod = PyModule::new_bound(parent.py(), "ops")?;

    ops_mod.add_class::<PyPrResult>()?;
    ops_mod.add_class::<PyLandResult>()?;
    ops_mod.add_class::<PyNextResult>()?;
    ops_mod.add_class::<PyRebaseResult>()?;

    ops_mod.add_function(wrap_pyfunction!(py_commit_workflow, &ops_mod)?)?;
    ops_mod.add_function(wrap_pyfunction!(py_create_or_update_pr, &ops_mod)?)?;
    ops_mod.add_function(wrap_pyfunction!(py_land, &ops_mod)?)?;
    ops_mod.add_function(wrap_pyfunction!(py_next_branch, &ops_mod)?)?;
    ops_mod.add_function(wrap_pyfunction!(py_rebase_with_recovery, &ops_mod)?)?;
    ops_mod.add_function(wrap_pyfunction!(py_abandon_branch, &ops_mod)?)?;

    parent.add_submodule(&ops_mod)?;
    Ok(())
}
