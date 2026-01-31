//! Context gathering and prompt assembly for LLM sessions.
//!
//! This module handles gathering all context components (docs, diff, clipboard, etc.)
//! and assembling them into a formatted prompt.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{load_config_or_default, Config};
use crate::error::CoreError;
use crate::flow::{load_direction, load_step, Direction, Step};

/// A document included in context.
#[derive(Debug, Clone)]
pub struct Document {
    pub path: String,
    pub content: String,
    pub category: String,
}

/// Options for gathering context.
#[derive(Debug, Clone, Default)]
pub struct GatherContextOpts {
    pub repo_root: PathBuf,
    pub step: Option<String>,
    pub inline: Option<String>,
    pub step_args: Vec<String>,
    pub run_mode: Option<String>,
    pub directions: Vec<String>,
    /// Include lfdocs (roadmap/, scratch/, root .md files)
    pub lfdocs: bool,
    /// Include files changed on branch
    pub diff_files: bool,
    /// Include raw diff
    pub diff: bool,
    /// Include clipboard
    pub clipboard: bool,
    /// Area path for scoped context
    pub area: Option<String>,
    /// Wave name for roadmap scoping
    pub wave: Option<String>,
}

/// All components of a prompt before assembly.
#[derive(Debug, Clone, Default)]
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
    pub wave: Option<String>,
    pub loopflow_doc: Option<String>,
}

/// Count tokens using tiktoken (cl100k_base encoding).
/// Falls back to byte length / 3 if tiktoken fails.
pub fn count_tokens(text: &str) -> usize {
    // Try tiktoken first
    if let Ok(bpe) = tiktoken_rs::cl100k_base() {
        return std::cmp::max(bpe.encode_ordinary(text).len(), 1);
    }
    // Fallback: rough estimate
    std::cmp::max(text.len() / 3, 1)
}

/// Analyze total token count of components.
pub fn analyze_tokens(components: &PromptComponents) -> usize {
    let mut total = 0;

    if let Some(ref doc) = components.loopflow_doc {
        total += count_tokens(doc);
    }

    for doc in &components.docs {
        total += count_tokens(&doc.content);
    }

    if let Some(ref diff) = components.diff {
        total += count_tokens(diff);
    }

    for doc in &components.diff_files {
        total += count_tokens(&doc.content);
    }

    if let Some(ref step) = components.step {
        if let Some(ref content) = step.content {
            total += count_tokens(content);
        }
    }

    for dir in &components.directions {
        total += count_tokens(&dir.content);
    }

    for summary in &components.summaries {
        total += count_tokens(&summary.content);
    }

    if let Some(ref clipboard) = components.clipboard {
        total += count_tokens(clipboard);
    }

    total
}

/// Trim context to fit within token budget.
/// Priority: keep step, directions, diff_files; drop docs and summaries first.
pub fn trim_context(mut components: PromptComponents, max_tokens: usize) -> PromptComponents {
    let mut total = analyze_tokens(&components);
    if total <= max_tokens {
        return components;
    }

    // Drop summaries first
    while total > max_tokens && !components.summaries.is_empty() {
        components.summaries.pop();
        total = analyze_tokens(&components);
    }

    // Drop docs next
    while total > max_tokens && !components.docs.is_empty() {
        components.docs.pop();
        total = analyze_tokens(&components);
    }

    // Drop diff
    if total > max_tokens && components.diff.is_some() {
        components.diff = None;
        total = analyze_tokens(&components);
    }

    // Last resort: drop diff_files
    while total > max_tokens && !components.diff_files.is_empty() {
        components.diff_files.pop();
        total = analyze_tokens(&components);
    }

    components
}

/// Gather all prompt components.
pub fn gather_context(opts: &GatherContextOpts) -> Result<PromptComponents, CoreError> {
    let repo_root = &opts.repo_root;

    // Load step
    let step = match &opts.step {
        Some(step_name) => Some(load_step(step_name, repo_root)?),
        None => None,
    };

    // Load directions
    let mut directions = Vec::new();
    for name in &opts.directions {
        directions.push(load_direction(name, repo_root)?);
    }

    // Gather docs
    let docs = if opts.lfdocs {
        gather_docs(repo_root)?
    } else {
        Vec::new()
    };

    // Gather diff files
    let diff_files = if opts.diff_files {
        gather_diff_files(repo_root)?
    } else {
        Vec::new()
    };

    // Gather raw diff
    let diff = if opts.diff {
        gather_diff(repo_root)?
    } else {
        None
    };

    // Gather clipboard
    let clipboard = if opts.clipboard {
        read_clipboard()
    } else {
        None
    };

    // Load bundled LOOPFLOW.md (placeholder - would need to embed this)
    let loopflow_doc = None; // TODO: embed LOOPFLOW.md in binary

    Ok(PromptComponents {
        run_mode: opts.run_mode.clone(),
        docs,
        diff,
        diff_files,
        step,
        repo_root: repo_root.to_string_lossy().to_string(),
        clipboard,
        directions,
        summaries: Vec::new(), // TODO: implement summary loading
        wave: opts.wave.clone(),
        loopflow_doc,
    })
}

/// Gather docs from repo (roadmap/, scratch/, root .md files).
fn gather_docs(repo_root: &Path) -> Result<Vec<Document>, CoreError> {
    let mut docs = Vec::new();

    // Root .md files
    for entry in fs::read_dir(repo_root)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Ok(content) = fs::read_to_string(&path) {
                docs.push(Document {
                    path: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    content,
                    category: "docs".to_string(),
                });
            }
        }
    }

    // scratch/
    let scratch_dir = repo_root.join("scratch");
    if scratch_dir.is_dir() {
        gather_md_files(&scratch_dir, &mut docs, "scratch")?;
    }

    // roadmap/
    let roadmap_dir = repo_root.join("roadmap");
    if roadmap_dir.is_dir() {
        gather_md_files(&roadmap_dir, &mut docs, "roadmap")?;
    }

    // reports/
    let reports_dir = repo_root.join("reports");
    if reports_dir.is_dir() {
        gather_md_files(&reports_dir, &mut docs, "reports")?;
    }

    Ok(docs)
}

/// Recursively gather .md files from a directory.
fn gather_md_files(dir: &Path, docs: &mut Vec<Document>, category: &str) -> Result<(), CoreError> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            gather_md_files(&path, docs, category)?;
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Ok(content) = fs::read_to_string(&path) {
                docs.push(Document {
                    path: path
                        .strip_prefix(dir.parent().unwrap_or(dir))
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string(),
                    content,
                    category: category.to_string(),
                });
            }
        }
    }

    Ok(())
}

/// Get files changed on this branch vs main.
fn gather_diff_files(repo_root: &Path) -> Result<Vec<Document>, CoreError> {
    // Get current branch
    let branch_output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(repo_root)
        .output()?;

    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();
    if branch.is_empty() || branch == "main" {
        return Ok(Vec::new());
    }

    // Get base ref
    let base_ref = get_default_base_ref(repo_root);

    // Get list of changed files
    let diff_output = Command::new("git")
        .args(["diff", "--name-only", &format!("{}...HEAD", base_ref)])
        .current_dir(repo_root)
        .output()?;

    if !diff_output.status.success() {
        return Ok(Vec::new());
    }

    let mut docs = Vec::new();
    let files = String::from_utf8_lossy(&diff_output.stdout);

    for line in files.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let file_path = repo_root.join(line);
        if file_path.exists() {
            if let Ok(content) = fs::read_to_string(&file_path) {
                docs.push(Document {
                    path: line.to_string(),
                    content,
                    category: "diff_files".to_string(),
                });
            }
        }
    }

    Ok(docs)
}

/// Get raw diff against base branch.
fn gather_diff(repo_root: &Path) -> Result<Option<String>, CoreError> {
    // Get current branch
    let branch_output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(repo_root)
        .output()?;

    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();
    if branch.is_empty() || branch == "main" {
        return Ok(None);
    }

    let base_ref = get_default_base_ref(repo_root);

    let diff_output = Command::new("git")
        .args(["diff", &format!("{}...HEAD", base_ref)])
        .current_dir(repo_root)
        .output()?;

    if !diff_output.status.success() {
        return Ok(None);
    }

    let diff = String::from_utf8_lossy(&diff_output.stdout).to_string();
    if diff.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(diff))
}

/// Get default base ref for diffs.
fn get_default_base_ref(repo_root: &Path) -> String {
    // Try origin/main first
    let check = Command::new("git")
        .args(["rev-parse", "--verify", "origin/main"])
        .current_dir(repo_root)
        .output();

    if check.map(|o| o.status.success()).unwrap_or(false) {
        return "origin/main".to_string();
    }

    "main".to_string()
}

/// Read clipboard content (macOS only).
fn read_clipboard() -> Option<String> {
    let output = Command::new("pbpaste").output().ok()?;
    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    None
}

/// Format all components into the final prompt string.
pub fn format_prompt(components: &PromptComponents) -> String {
    let mut parts = Vec::new();

    // 1. System docs (loopflow)
    if let Some(ref doc) = components.loopflow_doc {
        parts.push(format!("<lf:loopflow>\n{}\n</lf:loopflow>", doc));
    }

    // 2. Run mode
    if components.run_mode.as_deref() == Some("auto") {
        parts.push(
            "Run mode is auto (headless). Proceed without pausing for questions. \
             If you need clarification, make the best assumption you can and append \
             any open questions to `scratch/questions.md`."
                .to_string(),
        );
    }

    // 2.5. Wave context
    if let Some(ref wave) = components.wave {
        parts.push(format!(
            "<lf:wave name=\"{}\">\n\
             You are building toward the {} program of work.\n\
             Roadmap context is included in docs below.\n\
             </lf:wave>",
            wave, wave
        ));
    }

    // 3. Reference material (docs, summaries)
    if !components.docs.is_empty() {
        let doc_parts: Vec<String> = components
            .docs
            .iter()
            .map(|doc| {
                let name = Path::new(&doc.path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| doc.path.clone());
                format!("<lf:{}>\n{}\n</lf:{}>", name, doc.content, name)
            })
            .collect();

        let docs_body = doc_parts.join("\n\n");
        parts.push(format!(
            "Repository documentation. Follow STYLE carefully. \
             May include design artifacts (scratch/) and internal docs (reports/).\n\n\
             <lf:docs>\n{}\n</lf:docs>",
            docs_body
        ));
    }

    if !components.summaries.is_empty() {
        let summary_parts: Vec<String> = components
            .summaries
            .iter()
            .map(|s| {
                format!(
                    "<lf:summary path=\"{}\">\n{}\n</lf:summary>",
                    s.path, s.content
                )
            })
            .collect();
        let summaries_body = summary_parts.join("\n\n");
        parts.push(format!(
            "Pre-generated codebase summaries.\n\n\
             <lf:summaries>\n{}\n</lf:summaries>",
            summaries_body
        ));
    }

    // 4. Instructions (direction, then step)
    if !components.directions.is_empty() {
        if components.directions.len() == 1 {
            let d = &components.directions[0];
            parts.push(format!(
                "Direction for this work.\n\n\
                 <lf:direction:{}>\n{}\n</lf:direction:{}>",
                d.name, d.content, d.name
            ));
        } else {
            let direction_parts: Vec<String> = components
                .directions
                .iter()
                .map(|d| {
                    format!(
                        "<lf:direction:{}>\n{}\n</lf:direction:{}>",
                        d.name, d.content, d.name
                    )
                })
                .collect();
            parts.push(format!(
                "Directions for this work.\n\n\
                 <lf:directions>\n{}\n</lf:directions>",
                direction_parts.join("\n")
            ));
        }
    }

    if let Some(ref step) = components.step {
        let step_tag = if let Some(ref content) = step.content {
            format!(
                "<lf:step:{}>\n{}\n</lf:step:{}>",
                step.name, content, step.name
            )
        } else {
            format!("<lf:step:{}>\n</lf:step:{}>", step.name, step.name)
        };
        parts.push(format!("The step.\n\n{}", step_tag));
    }

    // 5. Working context (diff, diff_files, clipboard)
    if components.diff.is_some() || !components.diff_files.is_empty() {
        let mut diff_parts = Vec::new();

        if let Some(ref diff) = components.diff {
            diff_parts.push(format!("<lf:diff>\n{}\n</lf:diff>", diff));
        }

        if !components.diff_files.is_empty() {
            let files_content = format_files(&components.diff_files);
            diff_parts.push(files_content);
        }

        parts.push(format!(
            "Changes on this branch (diff against main).\n\n{}",
            diff_parts.join("\n\n")
        ));
    }

    if let Some(ref clipboard) = components.clipboard {
        parts.push(format!(
            "Content from clipboard.\n\n\
             <lf:clipboard>\n{}\n</lf:clipboard>",
            clipboard
        ));
    }

    parts.join("\n\n")
}

/// Format file documents for inclusion in prompt.
fn format_files(docs: &[Document]) -> String {
    let mut parts = Vec::new();
    parts.push(
        "Reference files for this task. Includes parent documentation for context.".to_string(),
    );
    parts.push("<lf:files>".to_string());

    for doc in docs {
        parts.push(format!(
            "<lf:file path=\"{}\">\n{}\n</lf:file>",
            doc.path, doc.content
        ));
    }

    parts.push("</lf:files>".to_string());
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_tokens_basic() {
        // tiktoken should give roughly 1 token per 4 chars for English text
        let text = "Hello, world! This is a test.";
        let tokens = count_tokens(text);
        assert!(tokens > 0);
        assert!(tokens < text.len()); // Should be less than byte length
    }

    #[test]
    fn analyze_tokens_empty() {
        let components = PromptComponents::default();
        assert_eq!(analyze_tokens(&components), 0);
    }

    #[test]
    fn trim_context_under_budget() {
        let mut components = PromptComponents::default();
        components.docs.push(Document {
            path: "test.md".to_string(),
            content: "Short content".to_string(),
            category: "docs".to_string(),
        });

        let trimmed = trim_context(components.clone(), 1000000);
        assert_eq!(trimmed.docs.len(), 1);
    }

    #[test]
    fn format_prompt_basic() {
        let mut components = PromptComponents::default();
        components.run_mode = Some("auto".to_string());

        let prompt = format_prompt(&components);
        assert!(prompt.contains("Run mode is auto"));
    }

    #[test]
    fn format_prompt_with_wave() {
        let mut components = PromptComponents::default();
        components.wave = Some("rust".to_string());

        let prompt = format_prompt(&components);
        assert!(prompt.contains("lf:wave"));
        assert!(prompt.contains("rust"));
    }
}
