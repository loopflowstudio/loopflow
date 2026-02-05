//! Context gathering and prompt assembly for LLM sessions.
//!
//! This module handles gathering all context components (docs, diff, clipboard, etc.)
//! and assembling them into a formatted prompt.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    /// User message (positional args after step/flow name)
    pub message: Option<String>,
    pub run_mode: Option<String>,
    pub directions: Vec<String>,
    /// Specific files to include in context
    pub files: Vec<String>,
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
    /// User message (positional args after step/flow name)
    pub message: Option<String>,
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
    if let Some(ref step) = step {
        for name in &step.directions {
            directions.push(load_direction(name, repo_root)?);
        }
    }
    for name in &opts.directions {
        directions.push(load_direction(name, repo_root)?);
    }

    // Gather docs
    let docs = if opts.lfdocs {
        gather_docs(repo_root, opts.wave.as_deref())?
    } else {
        Vec::new()
    };

    // Gather diff files
    let mut diff_files = Vec::new();
    if opts.diff_files {
        diff_files.extend(gather_diff_files(repo_root)?);
    }
    if !opts.files.is_empty() {
        diff_files.extend(gather_files(repo_root, &opts.files)?);
    }
    dedup_documents(&mut diff_files);

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

    // Load bundled LOOPFLOW.md (system instructions, always included)
    let loopflow_doc = Some(crate::builtins::LOOPFLOW_DOC.to_string());

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
        message: opts.message.clone(),
    })
}

/// Gather docs from repo (scratch/, roadmap/<wave>/, root .md files).
///
/// Matches Python's gather_lfdocs behavior:
/// 1. scratch/ (design docs, ephemeral per-PR)
/// 2. roadmap/<wave>/ (only if wave is set)
/// 3. Root .md files
fn gather_docs(repo_root: &Path, wave: Option<&str>) -> Result<Vec<Document>, CoreError> {
    let mut docs = Vec::new();

    // 1. scratch/ (design docs, ephemeral per-PR)
    let scratch_dir = repo_root.join("scratch");
    if scratch_dir.is_dir() {
        gather_md_files(&scratch_dir, &mut docs, "scratch")?;
    }

    // 2. roadmap/<wave>/ (only if wave is set)
    if let Some(wave_name) = wave {
        let wave_dir = repo_root.join("roadmap").join(wave_name);
        if wave_dir.is_dir() {
            // README first
            let readme = wave_dir.join("README.md");
            if readme.is_file() {
                if let Ok(content) = fs::read_to_string(&readme) {
                    docs.push(Document {
                        path: format!("roadmap/{}/README.md", wave_name),
                        content,
                        category: "roadmap".to_string(),
                    });
                }
            }
            // Then other .md files (sorted)
            let mut entries: Vec<_> = fs::read_dir(&wave_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let path = e.path();
                    path.is_file()
                        && path.extension().map(|ext| ext == "md").unwrap_or(false)
                        && path.file_name().map(|n| n != "README.md").unwrap_or(false)
                })
                .collect();
            entries.sort_by_key(|e| e.path());
            for entry in entries {
                let path = entry.path();
                if let Ok(content) = fs::read_to_string(&path) {
                    docs.push(Document {
                        path: format!(
                            "roadmap/{}/{}",
                            wave_name,
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ),
                        content,
                        category: "roadmap".to_string(),
                    });
                }
            }
        }
    }

    // 3. Root .md files (sorted)
    let mut entries: Vec<_> = fs::read_dir(repo_root)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.is_file() && path.extension().map(|ext| ext == "md").unwrap_or(false)
        })
        .collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
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

fn gather_files(repo_root: &Path, files: &[String]) -> Result<Vec<Document>, CoreError> {
    if files.is_empty() {
        return Ok(Vec::new());
    }

    let gitignore = build_gitignore(repo_root);
    let mut seen = HashSet::new();
    let mut docs = Vec::new();

    for file in files {
        let path = match resolve_path(repo_root, file) {
            Some(path) => path,
            None => continue,
        };

        if !path.is_file() {
            continue;
        }

        if should_exclude(repo_root, &path, &gitignore) {
            continue;
        }

        let canonical = path.canonicalize().unwrap_or(path.clone());
        if !seen.insert(canonical) {
            continue;
        }

        let content = match read_text_file(&path) {
            Some(content) => content,
            None => continue,
        };

        let rel_path = path
            .strip_prefix(repo_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        docs.push(Document {
            path: rel_path,
            content,
            category: "diff_files".to_string(),
        });
    }

    Ok(docs)
}

#[cfg(test)]
fn gather_all_text_files(repo_root: &Path) -> Result<Vec<Document>, CoreError> {
    let gitignore = build_gitignore(repo_root);
    let mut docs = Vec::new();

    let walker = ignore::WalkBuilder::new(repo_root)
        .hidden(false)
        .standard_filters(true)
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if should_exclude(repo_root, path, &gitignore) {
            continue;
        }
        if let Some(content) = read_text_file(path) {
            let rel_path = path
                .strip_prefix(repo_root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            docs.push(Document {
                path: rel_path,
                content,
                category: "diff_files".to_string(),
            });
        }
    }

    Ok(docs)
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

fn build_gitignore(repo_root: &Path) -> ignore::gitignore::Gitignore {
    let mut builder = ignore::gitignore::GitignoreBuilder::new(repo_root);
    let _ = builder.add(repo_root.join(".gitignore"));
    match builder.build() {
        Ok(gitignore) => gitignore,
        Err(_) => ignore::gitignore::Gitignore::empty(),
    }
}

fn resolve_path(repo_root: &Path, file: &str) -> Option<PathBuf> {
    let path = Path::new(file);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    };
    if joined.exists() {
        Some(joined)
    } else if file.starts_with("./") {
        let trimmed = file.trim_start_matches("./");
        let joined = repo_root.join(trimmed);
        if joined.exists() {
            return Some(joined);
        }
        None
    } else {
        None
    }
}

fn should_exclude(repo_root: &Path, path: &Path, gitignore: &ignore::gitignore::Gitignore) -> bool {
    if path
        .components()
        .any(|component| component.as_os_str() == ".lf")
    {
        return true;
    }

    let relative = path.strip_prefix(repo_root).unwrap_or(path);
    gitignore
        .matched_path_or_any_parents(relative, path.is_dir())
        .is_ignore()
}

fn read_text_file(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    if bytes.is_empty() {
        return Some(String::new());
    }
    if bytes.contains(&0) {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn dedup_documents(docs: &mut Vec<Document>) {
    let mut seen = HashSet::new();
    docs.retain(|doc| seen.insert(doc.path.clone()));
}

/// Ensure an entry exists in the repo's root .gitignore.
///
/// Adds the entry on a new line if not already present.
fn ensure_gitignore_entry(repo_root: &Path, entry: &str) -> Result<(), CoreError> {
    let gitignore_path = repo_root.join(".gitignore");
    let content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    // Check if entry already exists (as a line)
    let entry_line = entry.trim();
    let already_present = content.lines().any(|line| line.trim() == entry_line);

    if !already_present {
        let mut new_content = content;
        // Ensure file ends with newline before adding entry
        if !new_content.is_empty() && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push_str(entry_line);
        new_content.push('\n');
        fs::write(&gitignore_path, new_content)?;
    }

    Ok(())
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
    } else if components.run_mode.as_deref() == Some("interactive") {
        parts.push(
            "Run mode is interactive. This is a conversation—ask questions, \
             propose approaches, and wait for feedback before taking major actions."
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

    // 6. User message (additional instructions)
    if let Some(ref message) = components.message {
        parts.push(format!(
            "Additional instructions from user.\n\n\
             <lf:message>\n{}\n</lf:message>",
            message
        ));
    }

    parts.join("\n\n")
}

/// Format context components for system prompt (everything except step).
///
/// This is used with `--append-system-prompt-file` to load context into the
/// system prompt without a tool call, keeping input history clean.
pub fn format_context_prompt(components: &PromptComponents) -> String {
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
    } else if components.run_mode.as_deref() == Some("interactive") {
        parts.push(
            "Run mode is interactive. This is a conversation—ask questions, \
             propose approaches, and wait for feedback before taking major actions."
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

    // 4. Directions (but NOT step - step goes in task prompt)
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

    // Note: step is NOT included in context - it goes in task prompt

    // 6. User message (additional instructions)
    if let Some(ref message) = components.message {
        parts.push(format!(
            "Additional instructions from user.\n\n\
             <lf:message>\n{}\n</lf:message>",
            message
        ));
    }

    parts.join("\n\n")
}

/// Format step/task for user message (just the step content).
///
/// This is passed as the CLI argument when using `--append-system-prompt-file`.
pub fn format_task_prompt(components: &PromptComponents) -> String {
    let Some(ref step) = components.step else {
        return String::new();
    };

    if let Some(ref content) = step.content {
        format!(
            "<lf:step:{}>\n{}\n</lf:step:{}>",
            step.name, content, step.name
        )
    } else {
        format!("<lf:step:{}>\n</lf:step:{}>", step.name, step.name)
    }
}

/// Write prompt to log file and return the path.
///
/// File format: `{timestamp}-{flow_parents}.{step}.md` or `{timestamp}-{step}.md`
///
/// Ensures `.lf/log/` is in the repo's root `.gitignore`.
pub fn write_prompt_log(
    repo_root: &Path,
    prompt: &str,
    step_name: &str,
    flow_parents: Option<&[String]>,
) -> Result<PathBuf, CoreError> {
    let lf_dir = repo_root.join(".lf");
    let log_dir = lf_dir.join("log");
    fs::create_dir_all(&log_dir)?;

    // Ensure .lf/log/ is in repo .gitignore
    ensure_gitignore_entry(repo_root, ".lf/log/")?;

    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name_part = match flow_parents {
        Some(parents) if !parents.is_empty() => {
            format!("{}.{}", parents.join("."), step_name)
        }
        _ => step_name.to_string(),
    };
    let filename = format!("{}-{}.md", timestamp, name_part);
    let path = log_dir.join(&filename);

    fs::write(&path, prompt)?;
    Ok(path)
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
    use crate::flow::{Direction, Step};
    use std::path::{Path, PathBuf};

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(".lf/steps")).expect("create steps");
        std::fs::create_dir_all(dir.path().join(".lf/directions")).expect("create directions");
        dir
    }

    fn write_file(repo: &Path, path: &str, content: &str) {
        let full_path = repo.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(full_path, content).expect("write file");
    }

    fn write_binary(repo: &Path, path: &str, content: &[u8]) {
        let full_path = repo.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(full_path, content).expect("write binary file");
    }

    #[test]
    fn count_tokens_basic() {
        // tiktoken should give roughly 1 token per 4 chars for English text
        let text = "Hello, world! This is a test.";
        let tokens = count_tokens(text);
        assert!(tokens > 0);
        assert!(tokens < text.len()); // Should be less than byte length
    }

    #[test]
    fn count_tokens_empty() {
        // Empty string should return 1 (minimum)
        assert_eq!(count_tokens(""), 1);
    }

    #[test]
    fn analyze_tokens_empty() {
        let components = PromptComponents::default();
        assert_eq!(analyze_tokens(&components), 0);
    }

    #[test]
    fn analyze_tokens_with_content() {
        let components = PromptComponents {
            docs: vec![Document {
                path: "test.md".to_string(),
                content: "Hello world".to_string(),
                category: "docs".to_string(),
            }],
            clipboard: Some("Clipboard content".to_string()),
            ..Default::default()
        };

        let tokens = analyze_tokens(&components);
        assert!(tokens > 0);
    }

    #[test]
    fn analyze_tokens_counts_all_sections() {
        let components = PromptComponents {
            loopflow_doc: Some("Loopflow doc".to_string()),
            docs: vec![Document {
                path: "test.md".to_string(),
                content: "Doc content".to_string(),
                category: "docs".to_string(),
            }],
            diff: Some("diff content".to_string()),
            diff_files: vec![Document {
                path: "file.rs".to_string(),
                content: "fn main() {}".to_string(),
                category: "diff_files".to_string(),
            }],
            step: Some(Step {
                name: "implement".to_string(),
                content: Some("Implement the feature".to_string()),
                model: None,
                directions: vec![],
                interactive: None,
            }),
            directions: vec![Direction {
                name: "concise".to_string(),
                content: "Be concise".to_string(),
                source: PathBuf::from(".lf/directions/concise.md"),
            }],
            summaries: vec![Document {
                path: "summary.md".to_string(),
                content: "Summary content".to_string(),
                category: "summaries".to_string(),
            }],
            clipboard: Some("Clipboard".to_string()),
            ..Default::default()
        };

        let tokens = analyze_tokens(&components);
        // Should count all sections - tiktoken gives different counts than byte estimation
        assert!(tokens > 0);
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
    fn trim_context_drops_summaries_first() {
        let components = PromptComponents {
            docs: vec![Document {
                path: "doc.md".to_string(),
                content: "Doc content".to_string(),
                category: "docs".to_string(),
            }],
            summaries: vec![Document {
                path: "summary.md".to_string(),
                content: "Summary content that is long enough to matter".to_string(),
                category: "summaries".to_string(),
            }],
            ..Default::default()
        };

        // Set budget to only fit docs, not summaries
        let doc_tokens = count_tokens("Doc content");
        let trimmed = trim_context(components, doc_tokens + 5);

        assert!(trimmed.summaries.is_empty());
        assert_eq!(trimmed.docs.len(), 1);
    }

    #[test]
    fn trim_context_drops_docs_after_summaries() {
        let components = PromptComponents {
            docs: vec![
                Document {
                    path: "doc1.md".to_string(),
                    content: "First document with enough content to exceed token budget easily"
                        .to_string(),
                    category: "docs".to_string(),
                },
                Document {
                    path: "doc2.md".to_string(),
                    content: "Second document also has substantial content for testing".to_string(),
                    category: "docs".to_string(),
                },
            ],
            summaries: vec![],
            step: Some(Step {
                name: "test".to_string(),
                content: Some("x".to_string()), // Minimal step content
                model: None,
                directions: vec![],
                interactive: None,
            }),
            ..Default::default()
        };

        // Get token count of step only
        let step_tokens = count_tokens("x");
        // Budget allows step but not docs
        let trimmed = trim_context(components, step_tokens + 1);
        assert!(trimmed.docs.is_empty());
        assert!(trimmed.step.is_some());
    }

    #[test]
    fn trim_context_drops_diff_after_docs() {
        let components = PromptComponents {
            docs: vec![],
            diff: Some("This is a large diff with many changes across multiple files that will definitely exceed our small token budget".to_string()),
            step: Some(Step {
                name: "test".to_string(),
                content: Some("x".to_string()), // Minimal step content
                model: None,
                directions: vec![],
                interactive: None,
            }),
            ..Default::default()
        };

        // Get token count of step only
        let step_tokens = count_tokens("x");
        // Budget allows step but not diff
        let trimmed = trim_context(components, step_tokens + 1);
        assert!(trimmed.diff.is_none());
        assert!(trimmed.step.is_some());
    }

    #[test]
    fn trim_context_never_drops_step() {
        let components = PromptComponents {
            step: Some(Step {
                name: "implement".to_string(),
                content: Some("Implement the feature with tests".to_string()),
                model: None,
                directions: vec![],
                interactive: None,
            }),
            docs: vec![Document {
                path: "doc.md".to_string(),
                content: "Doc content that will exceed budget".to_string(),
                category: "docs".to_string(),
            }],
            ..Default::default()
        };

        let trimmed = trim_context(components, 5);
        assert!(trimmed.step.is_some());
        assert!(trimmed.docs.is_empty());
    }

    // ==========================================================================
    // format_prompt tests
    // ==========================================================================

    #[test]
    fn format_prompt_basic() {
        let components = PromptComponents {
            run_mode: Some("auto".to_string()),
            ..Default::default()
        };

        let prompt = format_prompt(&components);
        assert!(prompt.contains("Run mode is auto"));
        assert!(prompt.contains("headless"));
        assert!(prompt.contains("scratch/questions.md"));
    }

    #[test]
    fn format_prompt_interactive_mode_no_auto_message() {
        let components = PromptComponents {
            run_mode: Some("interactive".to_string()),
            ..Default::default()
        };

        let prompt = format_prompt(&components);
        assert!(!prompt.contains("Run mode is auto"));
    }

    #[test]
    fn format_prompt_with_wave() {
        let components = PromptComponents {
            wave: Some("rust".to_string()),
            ..Default::default()
        };

        let prompt = format_prompt(&components);
        assert!(prompt.contains("<lf:wave"));
        assert!(prompt.contains("name=\"rust\""));
        assert!(prompt.contains("rust program of work"));
        assert!(prompt.contains("</lf:wave>"));
    }

    #[test]
    fn format_prompt_with_docs() {
        let components = PromptComponents {
            docs: vec![
                Document {
                    path: "README.md".to_string(),
                    content: "# Test Project".to_string(),
                    category: "docs".to_string(),
                },
                Document {
                    path: "STYLE.md".to_string(),
                    content: "# Style Guide".to_string(),
                    category: "docs".to_string(),
                },
            ],
            ..Default::default()
        };

        let prompt = format_prompt(&components);
        assert!(prompt.contains("<lf:docs>"));
        assert!(prompt.contains("</lf:docs>"));
        assert!(prompt.contains("<lf:README>"));
        assert!(prompt.contains("# Test Project"));
        assert!(prompt.contains("</lf:README>"));
        assert!(prompt.contains("<lf:STYLE>"));
        assert!(prompt.contains("# Style Guide"));
        assert!(prompt.contains("Follow STYLE carefully"));
    }

    #[test]
    fn format_prompt_claude_md_gets_follow_note() {
        let components = PromptComponents {
            docs: vec![Document {
                path: "CLAUDE.md".to_string(),
                content: "# Instructions".to_string(),
                category: "docs".to_string(),
            }],
            ..Default::default()
        };
        let prompt = format_prompt(&components);
        assert!(prompt.contains("CLAUDE"));
        assert!(prompt.contains("Follow") || prompt.contains("carefully"));
    }

    #[test]
    fn format_prompt_style_md_gets_follow_note() {
        let components = PromptComponents {
            docs: vec![Document {
                path: "STYLE.md".to_string(),
                content: "# Style Guide".to_string(),
                category: "docs".to_string(),
            }],
            ..Default::default()
        };
        let prompt = format_prompt(&components);
        assert!(prompt.contains("STYLE"));
        assert!(prompt.contains("Follow") || prompt.contains("carefully"));
    }

    #[test]
    fn format_prompt_with_single_direction() {
        let components = PromptComponents {
            directions: vec![Direction {
                name: "concise".to_string(),
                content: "Be concise and direct.".to_string(),
                source: PathBuf::from(".lf/directions/concise.md"),
            }],
            ..Default::default()
        };

        let prompt = format_prompt(&components);
        assert!(prompt.contains("<lf:direction:concise>"));
        assert!(prompt.contains("Be concise and direct."));
        assert!(prompt.contains("</lf:direction:concise>"));
        assert!(prompt.contains("Direction for this work"));
        // Should NOT use plural wrapper for single direction
        assert!(!prompt.contains("<lf:directions>"));
    }

    #[test]
    fn format_prompt_with_multiple_directions() {
        let components = PromptComponents {
            directions: vec![
                Direction {
                    name: "concise".to_string(),
                    content: "Be concise.".to_string(),
                    source: PathBuf::from(".lf/directions/concise.md"),
                },
                Direction {
                    name: "architect".to_string(),
                    content: "Think architecturally.".to_string(),
                    source: PathBuf::from(".lf/directions/architect.md"),
                },
            ],
            ..Default::default()
        };

        let prompt = format_prompt(&components);
        assert!(prompt.contains("<lf:directions>"));
        assert!(prompt.contains("</lf:directions>"));
        assert!(prompt.contains("<lf:direction:concise>"));
        assert!(prompt.contains("<lf:direction:architect>"));
        assert!(prompt.contains("Directions for this work"));
    }

    #[test]
    fn format_prompt_with_step() {
        let components = PromptComponents {
            step: Some(Step {
                name: "implement".to_string(),
                content: Some("Implement the feature described.".to_string()),
                model: None,
                directions: vec![],
                interactive: None,
            }),
            ..Default::default()
        };

        let prompt = format_prompt(&components);
        assert!(prompt.contains("<lf:step:implement>"));
        assert!(prompt.contains("Implement the feature described."));
        assert!(prompt.contains("</lf:step:implement>"));
        assert!(prompt.contains("The step."));
    }

    #[test]
    fn format_prompt_with_step_no_content() {
        let components = PromptComponents {
            step: Some(Step {
                name: "review".to_string(),
                content: None,
                model: None,
                directions: vec![],
                interactive: None,
            }),
            ..Default::default()
        };

        let prompt = format_prompt(&components);
        assert!(prompt.contains("<lf:step:review>"));
        assert!(prompt.contains("</lf:step:review>"));
    }

    #[test]
    fn format_prompt_with_diff() {
        let components = PromptComponents {
            diff: Some("diff --git a/file.rs\n+added line".to_string()),
            ..Default::default()
        };

        let prompt = format_prompt(&components);
        assert!(prompt.contains("<lf:diff>"));
        assert!(prompt.contains("+added line"));
        assert!(prompt.contains("</lf:diff>"));
        assert!(prompt.contains("Changes on this branch"));
    }

    #[test]
    fn format_prompt_with_diff_files() {
        let components = PromptComponents {
            diff_files: vec![Document {
                path: "src/main.rs".to_string(),
                content: "fn main() { println!(\"hello\"); }".to_string(),
                category: "diff_files".to_string(),
            }],
            ..Default::default()
        };

        let prompt = format_prompt(&components);
        assert!(prompt.contains("<lf:files>"));
        assert!(prompt.contains("<lf:file path=\"src/main.rs\">"));
        assert!(prompt.contains("fn main()"));
        assert!(prompt.contains("</lf:file>"));
        assert!(prompt.contains("</lf:files>"));
    }

    #[test]
    fn format_prompt_with_clipboard() {
        let components = PromptComponents {
            clipboard: Some("Error: connection refused".to_string()),
            ..Default::default()
        };

        let prompt = format_prompt(&components);
        assert!(prompt.contains("<lf:clipboard>"));
        assert!(prompt.contains("Error: connection refused"));
        assert!(prompt.contains("</lf:clipboard>"));
        assert!(prompt.contains("Content from clipboard"));
    }

    #[test]
    fn format_prompt_with_summaries() {
        let components = PromptComponents {
            summaries: vec![Document {
                path: "src/".to_string(),
                content: "Source code summary".to_string(),
                category: "summaries".to_string(),
            }],
            ..Default::default()
        };

        let prompt = format_prompt(&components);
        assert!(prompt.contains("<lf:summaries>"));
        assert!(prompt.contains("<lf:summary path=\"src/\">"));
        assert!(prompt.contains("Source code summary"));
        assert!(prompt.contains("</lf:summary>"));
        assert!(prompt.contains("Pre-generated codebase summaries"));
    }

    #[test]
    fn format_prompt_full_assembly() {
        // Test a complete prompt with all sections
        let components = PromptComponents {
            run_mode: Some("auto".to_string()),
            wave: Some("rust".to_string()),
            loopflow_doc: Some("Loopflow instructions".to_string()),
            docs: vec![Document {
                path: "README.md".to_string(),
                content: "# Project".to_string(),
                category: "docs".to_string(),
            }],
            directions: vec![Direction {
                name: "concise".to_string(),
                content: "Be concise.".to_string(),
                source: PathBuf::from(".lf/directions/concise.md"),
            }],
            step: Some(Step {
                name: "implement".to_string(),
                content: Some("Implement it.".to_string()),
                model: None,
                directions: vec![],
                interactive: None,
            }),
            diff: Some("diff content".to_string()),
            clipboard: Some("clipboard content".to_string()),
            ..Default::default()
        };

        let prompt = format_prompt(&components);

        // Verify order: loopflow -> run_mode -> wave -> docs -> directions -> step -> diff -> clipboard
        let loopflow_pos = prompt.find("<lf:loopflow>").unwrap();
        let auto_pos = prompt.find("Run mode is auto").unwrap();
        let wave_pos = prompt.find("<lf:wave").unwrap();
        let docs_pos = prompt.find("<lf:docs>").unwrap();
        let direction_pos = prompt.find("<lf:direction:concise>").unwrap();
        let step_pos = prompt.find("<lf:step:implement>").unwrap();
        let diff_pos = prompt.find("<lf:diff>").unwrap();
        let clipboard_pos = prompt.find("<lf:clipboard>").unwrap();

        assert!(loopflow_pos < auto_pos);
        assert!(auto_pos < wave_pos);
        assert!(wave_pos < docs_pos);
        assert!(docs_pos < direction_pos);
        assert!(direction_pos < step_pos);
        assert!(step_pos < diff_pos);
        assert!(diff_pos < clipboard_pos);
    }

    #[test]
    fn format_prompt_empty_components() {
        let components = PromptComponents::default();
        let prompt = format_prompt(&components);
        // Should not crash, just return empty or minimal content
        assert!(prompt.is_empty() || prompt.len() < 100);
    }

    // ==========================================================================
    // file gathering tests
    // ==========================================================================

    #[test]
    fn gather_files_excludes_gitignored() {
        let repo = init_repo();
        write_file(repo.path(), "src/main.rs", "fn main() {}");
        write_file(repo.path(), "target/debug/main", "binary");
        write_file(repo.path(), ".gitignore", "target/\n*.log");
        write_file(repo.path(), "debug.log", "log content");

        let files = gather_files(
            repo.path(),
            &[
                "src/main.rs".to_string(),
                "target/debug/main".to_string(),
                "debug.log".to_string(),
            ],
        )
        .expect("gather files");

        assert!(files.iter().any(|f| f.path.ends_with("src/main.rs")));
        assert!(!files.iter().any(|f| f.path.contains("target")));
        assert!(!files.iter().any(|f| f.path.ends_with("debug.log")));
    }

    #[test]
    fn gather_files_excludes_lf_directory() {
        let repo = init_repo();
        write_file(repo.path(), "src/lib.rs", "pub fn foo() {}");
        write_file(repo.path(), ".lf/config.yaml", "model: claude");
        write_file(repo.path(), ".lf/steps/debug.md", "# Debug step");

        let files = gather_all_text_files(repo.path()).expect("gather files");

        assert!(files.iter().any(|f| f.path.ends_with("src/lib.rs")));
        assert!(!files.iter().any(|f| f.path.contains(".lf/")));
    }

    #[test]
    fn gather_context_with_specific_files() {
        let repo = init_repo();
        write_file(repo.path(), "src/a.rs", "mod a;");
        write_file(repo.path(), "src/b.rs", "mod b;");
        write_file(repo.path(), "src/c.rs", "mod c;");

        let opts = GatherContextOpts {
            repo_root: repo.path().to_path_buf(),
            files: vec!["src/a.rs".to_string(), "src/c.rs".to_string()],
            lfdocs: false,
            diff_files: false,
            diff: false,
            clipboard: false,
            ..Default::default()
        };
        let ctx = gather_context(&opts).expect("gather context");
        let prompt = format_prompt(&ctx);

        assert!(prompt.contains("mod a;"));
        assert!(prompt.contains("mod c;"));
        assert!(!prompt.contains("mod b;"));
    }

    #[test]
    fn gather_files_deduplicates_requests() {
        let repo = init_repo();
        write_file(repo.path(), "src/main.rs", "fn main() {}");

        let files = gather_files(
            repo.path(),
            &[
                "src/main.rs".to_string(),
                "src/main.rs".to_string(),
                "./src/main.rs".to_string(),
            ],
        )
        .expect("gather files");

        let main_count = files
            .iter()
            .filter(|f| f.path.ends_with("src/main.rs"))
            .count();
        assert_eq!(main_count, 1);
    }

    #[test]
    fn gather_files_skips_binary_files() {
        let repo = init_repo();
        write_file(repo.path(), "src/main.rs", "fn main() {}");
        write_binary(repo.path(), "assets/image.png", &[0x89, 0x50, 0x4E, 0x47]);

        let files = gather_files(
            repo.path(),
            &["src/main.rs".to_string(), "assets/image.png".to_string()],
        )
        .expect("gather files");

        assert!(files.iter().any(|f| f.path.ends_with("src/main.rs")));
        assert!(!files.iter().any(|f| f.path.ends_with("image.png")));
    }

    // ==========================================================================
    // gather_context tests (filesystem-based)
    // ==========================================================================

    #[test]
    fn gather_context_minimal_repo() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let repo = temp.path();

        // Create minimal structure
        std::fs::create_dir_all(repo.join(".lf/steps")).expect("create .lf/steps");
        std::fs::write(repo.join(".lf/steps/test.md"), "Test step content").expect("write step");

        let opts = GatherContextOpts {
            repo_root: repo.to_path_buf(),
            step: Some("test".to_string()),
            lfdocs: false,
            diff_files: false,
            diff: false,
            clipboard: false,
            ..Default::default()
        };

        let result = gather_context(&opts);
        assert!(result.is_ok());
        let components = result.unwrap();
        assert!(components.step.is_some());
        assert_eq!(components.step.as_ref().unwrap().name, "test");
    }

    #[test]
    fn gather_context_with_lfdocs() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let repo = temp.path();

        // Create docs
        std::fs::write(repo.join("README.md"), "# Project").expect("write readme");
        std::fs::create_dir_all(repo.join("scratch")).expect("create scratch");
        std::fs::write(repo.join("scratch/plan.md"), "# Plan").expect("write plan");

        let opts = GatherContextOpts {
            repo_root: repo.to_path_buf(),
            lfdocs: true,
            diff_files: false,
            diff: false,
            clipboard: false,
            ..Default::default()
        };

        let result = gather_context(&opts);
        assert!(result.is_ok());
        let components = result.unwrap();
        assert!(!components.docs.is_empty());

        // Should include README.md
        let readme = components.docs.iter().find(|d| d.path.contains("README"));
        assert!(readme.is_some());
        assert!(readme.unwrap().content.contains("# Project"));
    }

    #[test]
    fn gather_context_with_directions() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let repo = temp.path();

        // Create direction
        std::fs::create_dir_all(repo.join(".lf/directions")).expect("create directions");
        std::fs::write(repo.join(".lf/directions/concise.md"), "Be concise.")
            .expect("write direction");

        let opts = GatherContextOpts {
            repo_root: repo.to_path_buf(),
            directions: vec!["concise".to_string()],
            lfdocs: false,
            diff_files: false,
            diff: false,
            clipboard: false,
            ..Default::default()
        };

        let result = gather_context(&opts);
        assert!(result.is_ok());
        let components = result.unwrap();
        assert_eq!(components.directions.len(), 1);
        assert_eq!(components.directions[0].name, "concise");
        assert!(components.directions[0].content.contains("Be concise"));
    }

    #[test]
    fn directions_from_step_and_cli_combined() {
        let repo = init_repo();
        write_file(
            repo.path(),
            ".lf/steps/impl.md",
            r#"---
directions:
  - thorough
---
# Implement
"#,
        );
        write_file(repo.path(), ".lf/directions/thorough.md", "Be thorough.");
        write_file(repo.path(), ".lf/directions/fast.md", "Be fast.");

        let opts = GatherContextOpts {
            repo_root: repo.path().to_path_buf(),
            step: Some("impl".to_string()),
            directions: vec!["fast".to_string()],
            lfdocs: false,
            diff_files: false,
            diff: false,
            clipboard: false,
            ..Default::default()
        };
        let ctx = gather_context(&opts).expect("gather context");

        assert_eq!(ctx.directions.len(), 2);
        assert_eq!(ctx.directions[0].name, "thorough");
        assert_eq!(ctx.directions[1].name, "fast");
    }

    #[test]
    fn gather_context_run_mode_preserved() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let repo = temp.path();

        let opts = GatherContextOpts {
            repo_root: repo.to_path_buf(),
            run_mode: Some("auto".to_string()),
            lfdocs: false,
            diff_files: false,
            diff: false,
            clipboard: false,
            ..Default::default()
        };

        let result = gather_context(&opts);
        assert!(result.is_ok());
        let components = result.unwrap();
        assert_eq!(components.run_mode, Some("auto".to_string()));
    }

    #[test]
    fn gather_context_wave_preserved() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let repo = temp.path();

        let opts = GatherContextOpts {
            repo_root: repo.to_path_buf(),
            wave: Some("rust-migration".to_string()),
            lfdocs: false,
            diff_files: false,
            diff: false,
            clipboard: false,
            ..Default::default()
        };

        let result = gather_context(&opts);
        assert!(result.is_ok());
        let components = result.unwrap();
        assert_eq!(components.wave, Some("rust-migration".to_string()));
    }

    // ==========================================================================
    // prompt log tests
    // ==========================================================================

    #[test]
    fn write_prompt_log_creates_file() {
        let repo = init_repo();
        let prompt = "Test prompt content";
        let path = write_prompt_log(repo.path(), prompt, "implement", None).unwrap();

        assert!(path.exists());
        assert!(path.to_string_lossy().contains(".lf/log/"));
        assert!(path.to_string_lossy().ends_with("-implement.md"));

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, prompt);
    }

    #[test]
    fn write_prompt_log_adds_to_repo_gitignore() {
        let repo = init_repo();
        let gitignore_path = repo.path().join(".gitignore");

        assert!(!gitignore_path.exists());

        write_prompt_log(repo.path(), "test", "step", None).unwrap();

        assert!(gitignore_path.exists());
        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains(".lf/log/"));
    }

    #[test]
    fn write_prompt_log_with_flow_parents() {
        let repo = init_repo();
        let path = write_prompt_log(
            repo.path(),
            "content",
            "implement",
            Some(&["ship".to_string(), "grind".to_string()]),
        )
        .unwrap();

        assert!(path.to_string_lossy().contains("ship.grind.implement.md"));
    }

    #[test]
    fn write_prompt_log_preserves_existing_gitignore() {
        let repo = init_repo();
        let gitignore_path = repo.path().join(".gitignore");
        fs::write(&gitignore_path, "target/\n.lf/log/\n").unwrap();

        write_prompt_log(repo.path(), "test", "step", None).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();
        // Should not duplicate the entry
        assert_eq!(content, "target/\n.lf/log/\n");
    }

    #[test]
    fn write_prompt_log_appends_to_existing_gitignore() {
        let repo = init_repo();
        let gitignore_path = repo.path().join(".gitignore");
        fs::write(&gitignore_path, "target/\nnode_modules/\n").unwrap();

        write_prompt_log(repo.path(), "test", "step", None).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains("target/"));
        assert!(content.contains("node_modules/"));
        assert!(content.contains(".lf/log/"));
    }

    // ==========================================================================
    // format_context_prompt tests
    // ==========================================================================

    #[test]
    fn format_context_prompt_excludes_step() {
        let components = PromptComponents {
            run_mode: Some("auto".to_string()),
            step: Some(Step {
                name: "implement".to_string(),
                content: Some("Implement the feature.".to_string()),
                model: None,
                directions: vec![],
                interactive: None,
            }),
            ..Default::default()
        };

        let context = format_context_prompt(&components);
        // Should NOT include step content
        assert!(!context.contains("<lf:step:implement>"));
        assert!(!context.contains("Implement the feature."));
        // Should include run mode
        assert!(context.contains("Run mode is auto"));
    }

    #[test]
    fn format_context_prompt_includes_all_context() {
        let components = PromptComponents {
            run_mode: Some("interactive".to_string()),
            docs: vec![Document {
                path: "README.md".to_string(),
                content: "# Project".to_string(),
                category: "docs".to_string(),
            }],
            directions: vec![Direction {
                name: "concise".to_string(),
                content: "Be concise.".to_string(),
                source: PathBuf::from(".lf/directions/concise.md"),
            }],
            clipboard: Some("Error message".to_string()),
            step: Some(Step {
                name: "debug".to_string(),
                content: Some("Fix the error.".to_string()),
                model: None,
                directions: vec![],
                interactive: None,
            }),
            ..Default::default()
        };

        let context = format_context_prompt(&components);
        // Should include context parts
        assert!(context.contains("Run mode is interactive"));
        assert!(context.contains("<lf:docs>"));
        assert!(context.contains("# Project"));
        assert!(context.contains("<lf:direction:concise>"));
        assert!(context.contains("<lf:clipboard>"));
        // Should NOT include step
        assert!(!context.contains("<lf:step:debug>"));
        assert!(!context.contains("Fix the error."));
    }

    #[test]
    fn format_context_prompt_interactive_mode_message() {
        let components = PromptComponents {
            run_mode: Some("interactive".to_string()),
            ..Default::default()
        };

        let context = format_context_prompt(&components);
        assert!(context.contains("Run mode is interactive"));
        assert!(context.contains("ask questions"));
        assert!(context.contains("wait for feedback"));
    }

    // ==========================================================================
    // format_task_prompt tests
    // ==========================================================================

    #[test]
    fn format_task_prompt_returns_step_content() {
        let components = PromptComponents {
            step: Some(Step {
                name: "implement".to_string(),
                content: Some("Implement the feature.".to_string()),
                model: None,
                directions: vec![],
                interactive: None,
            }),
            ..Default::default()
        };

        let task = format_task_prompt(&components);
        assert!(task.contains("<lf:step:implement>"));
        assert!(task.contains("Implement the feature."));
        assert!(task.contains("</lf:step:implement>"));
    }

    #[test]
    fn format_task_prompt_empty_when_no_step() {
        let components = PromptComponents::default();
        let task = format_task_prompt(&components);
        assert!(task.is_empty());
    }

    #[test]
    fn format_task_prompt_step_without_content() {
        let components = PromptComponents {
            step: Some(Step {
                name: "review".to_string(),
                content: None,
                model: None,
                directions: vec![],
                interactive: None,
            }),
            ..Default::default()
        };

        let task = format_task_prompt(&components);
        assert!(task.contains("<lf:step:review>"));
        assert!(task.contains("</lf:step:review>"));
    }
}
