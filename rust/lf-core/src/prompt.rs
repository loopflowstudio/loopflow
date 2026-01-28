use std::path::PathBuf;

use crate::error::CoreError;
use crate::flow::{load_direction, load_step, Direction, Step};

#[derive(Debug, Clone)]
pub struct GatherContextOpts {
    pub repo_root: PathBuf,
    pub step: Option<String>,
    pub inline: Option<String>,
    pub step_args: Vec<String>,
    pub run_mode: Option<String>,
    pub directions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub path: String,
    pub content: String,
    pub category: String,
}

#[derive(Debug, Clone)]
pub struct PromptComponents {
    pub run_mode: Option<String>,
    pub docs: Vec<Document>,
    pub diff: Option<String>,
    pub diff_files: Vec<Document>,
    pub step: Option<Step>,
    pub repo_root: String,
    pub clipboard: Option<String>,
    pub directions: Vec<Direction>,
    pub summaries: Vec<Document>,
}

pub fn gather_context(opts: &GatherContextOpts) -> Result<PromptComponents, CoreError> {
    let repo_root = opts.repo_root.clone();
    let step = match &opts.step {
        Some(step_name) => Some(load_step(step_name, &repo_root)?),
        None => None,
    };
    let mut directions = Vec::new();
    for name in &opts.directions {
        directions.push(load_direction(name, &repo_root)?);
    }
    Ok(PromptComponents {
        run_mode: opts.run_mode.clone(),
        docs: Vec::new(),
        diff: None,
        diff_files: Vec::new(),
        step,
        repo_root: repo_root.to_string_lossy().to_string(),
        clipboard: None,
        directions,
        summaries: Vec::new(),
    })
}

/// Estimate token count from text length.
/// Uses byte length / 3 as a rough approximation until tiktoken integration.
pub fn count_tokens(text: &str) -> usize {
    // str::len() returns byte length, which is what we want for this estimate
    std::cmp::max(text.len() / 3, 1)
}

pub fn format_prompt(_components: &PromptComponents) -> String {
    String::new()
}

pub fn analyze_tokens(components: &PromptComponents) -> usize {
    let mut total = 0;
    for doc in &components.docs {
        total += count_tokens(&doc.content);
    }
    if let Some(diff) = &components.diff {
        total += count_tokens(diff);
    }
    for doc in &components.diff_files {
        total += count_tokens(&doc.content);
    }
    if let Some(step) = &components.step {
        if let Some(content) = &step.content {
            total += count_tokens(content);
        }
    }
    total
}

pub fn trim_context(mut components: PromptComponents, max_tokens: usize) -> PromptComponents {
    let mut total = analyze_tokens(&components);
    if total <= max_tokens {
        return components;
    }

    while total > max_tokens && !components.diff_files.is_empty() {
        components.diff_files.pop();
        total = analyze_tokens(&components);
    }
    while total > max_tokens && components.diff.is_some() {
        components.diff = None;
        total = analyze_tokens(&components);
    }
    components
}
