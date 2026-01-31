//! PyO3 bindings for lf-core.
//!
//! Exposes the Rust engine to Python via PyO3. This allows Python `lf` to
//! import lf-core directly without subprocess overhead.

use pyo3::prelude::*;
use std::path::PathBuf;

use crate::agent::{launch_agent, LaunchConfig, LaunchResult as RustLaunchResult};
use crate::config::{load_config_or_default, parse_model};
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

/// The lf_core Python module.
#[pymodule]
fn lf_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLaunchResult>()?;
    m.add_class::<PyPromptComponents>()?;
    m.add_function(wrap_pyfunction!(py_count_tokens, m)?)?;
    m.add_function(wrap_pyfunction!(py_parse_model, m)?)?;
    m.add_function(wrap_pyfunction!(py_gather_context, m)?)?;
    m.add_function(wrap_pyfunction!(py_launch_agent, m)?)?;
    m.add_function(wrap_pyfunction!(run_step, m)?)?;
    Ok(())
}
