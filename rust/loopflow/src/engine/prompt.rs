//! Context gathering and prompt assembly for LLM sessions.
//!
//! This module handles gathering all context components (docs, diff, clipboard, etc.)
//! and assembling them into a formatted prompt.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::engine::error::CoreError;
use crate::engine::flow::{expand_direction_names, load_direction, load_step, Direction, Step};
use once_cell::sync::Lazy;
use tiktoken_rs::CoreBPE;
use tracing::debug;

/// Source of a context document or context token bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentSource {
    Step,
    Direction,
    Wave,
    WaveMemory,
    Area,
    Diff,
    Clipboard,
    RepoDoc,
    Summary,
}

/// A document included in context.
#[derive(Debug, Clone)]
pub struct Document {
    pub path: String,
    pub content: String,
    pub source: DocumentSource,
}

/// Default maximum tokens for pre-fill context.
pub const DEFAULT_CONTEXT_BUDGET: usize = 75_000;

/// How diff context is represented after tiering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum DiffTier {
    /// Full unified diff (< 15k tokens)
    UnifiedDiff,
    /// Stat-only summary (fallback for large diffs)
    StatOnly,
    /// No diff context
    #[default]
    None,
}

/// Per-source token counts for the assembled context.
#[derive(Debug, Clone, Default)]
pub struct ContextBreakdown {
    pub source_tokens: HashMap<DocumentSource, usize>,
    pub system_tokens: usize,
    /// Display metadata
    pub step_name: Option<String>,
    pub direction_names: Vec<String>,
    pub diff_tier: DiffTier,
    pub diff_file_count: usize,
    pub doc_count: usize,
    pub area_name: Option<String>,
    pub area_doc_count: usize,
    pub has_clipboard: bool,
    pub wave_name: Option<String>,
}

impl ContextBreakdown {
    pub fn total(&self) -> usize {
        self.system_tokens + self.source_tokens.values().sum::<usize>()
    }

    pub fn source_tokens(&self, source: DocumentSource) -> usize {
        self.source_tokens.get(&source).copied().unwrap_or(0)
    }

    fn add_source_tokens(&mut self, source: DocumentSource, tokens: usize) {
        *self.source_tokens.entry(source).or_insert(0) += tokens;
    }

    fn subtract_source_tokens(&mut self, source: DocumentSource, tokens: usize) {
        let entry = self.source_tokens.entry(source).or_insert(0);
        *entry = entry.saturating_sub(tokens);
    }

    fn set_source_tokens(&mut self, source: DocumentSource, tokens: usize) {
        self.source_tokens.insert(source, tokens);
    }
}

/// Specification for which context sources to gather.
#[derive(Debug, Clone, Default)]
pub struct GatherSpec {
    pub sources: Vec<DocumentSource>,
    pub repo_root: PathBuf,
    /// Specific files to include in context.
    pub files: Vec<String>,
    /// Area path for scoped context.
    pub area: Option<String>,
    /// Wave name for wave/ scoping.
    pub wave: Option<String>,
}

impl GatherSpec {
    fn includes(&self, source: DocumentSource) -> bool {
        self.sources.contains(&source)
    }

    fn include_source(&mut self, source: DocumentSource) {
        if !self.includes(source) {
            self.sources.push(source);
        }
    }

    fn normalize(&mut self) {
        if self.wave.is_some() && self.includes(DocumentSource::RepoDoc) {
            self.include_source(DocumentSource::Wave);
            self.include_source(DocumentSource::WaveMemory);
        }
        if self.area.is_some() {
            self.include_source(DocumentSource::Area);
        }
        if !self.files.is_empty() {
            self.include_source(DocumentSource::Diff);
        }
    }
}

/// Build a canonical list of context sources from high-level switches.
pub fn default_gather_sources(
    include_repo_docs: bool,
    include_diff: bool,
    include_clipboard: bool,
) -> Vec<DocumentSource> {
    let mut sources = Vec::new();
    if include_repo_docs {
        sources.push(DocumentSource::RepoDoc);
    }
    if include_diff {
        sources.push(DocumentSource::Diff);
    }
    if include_clipboard {
        sources.push(DocumentSource::Clipboard);
    }
    sources
}

/// Options for gathering context.
#[derive(Debug, Clone, Default)]
pub struct GatherContextOpts {
    pub repo_root: PathBuf,
    pub step: Option<String>,
    /// User message (positional args after step/flow name, or inline prompt)
    pub message: Option<String>,
    pub run_mode: Option<String>,
    pub directions: Vec<String>,
    /// Specific files to include in context.
    pub files: Vec<String>,
    /// Explicit sources to include in the prompt context pipeline.
    pub sources: Vec<DocumentSource>,
    /// Area path for scoped context.
    pub area: Option<String>,
    /// Wave name for wave/ scoping.
    pub wave: Option<String>,
}

impl GatherContextOpts {
    pub fn gather_spec(&self) -> GatherSpec {
        GatherSpec {
            sources: self.sources.clone(),
            repo_root: self.repo_root.clone(),
            files: self.files.clone(),
            area: self.area.clone(),
            wave: self.wave.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptFormatMode {
    #[default]
    Full,
    Context,
    Task,
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
    pub wave_memory: Vec<Document>,
    pub wave: Option<String>,
    pub loopflow_doc: Option<String>,
    /// User message (positional args after step/flow name)
    pub message: Option<String>,
    /// How diff context was tiered
    pub diff_tier: DiffTier,
    /// Number of files changed on branch (for display)
    pub diff_file_count: usize,
    /// Docs gathered from area parent directories
    pub area_docs: Vec<Document>,
    /// Area path for display
    pub area: Option<String>,
}

#[derive(Debug, Clone)]
struct TokenCacheEntry {
    mtime_secs: u64,
    size: u64,
    tokens: usize,
}

/// Count tokens using tiktoken (cl100k_base encoding).
/// Falls back to byte length / 3 if tiktoken fails.
pub fn count_tokens(text: &str) -> usize {
    static BPE: Lazy<Option<CoreBPE>> = Lazy::new(|| tiktoken_rs::cl100k_base().ok());
    if let Some(bpe) = BPE.as_ref() {
        return std::cmp::max(bpe.encode_ordinary(text).len(), 1);
    }
    // Fallback: rough estimate
    std::cmp::max(text.len() / 3, 1)
}

fn cached_doc_tokens(
    doc: &Document,
    repo_root: &Path,
    cache: &mut HashMap<String, TokenCacheEntry>,
) -> usize {
    let abs_path = repo_root.join(&doc.path);
    let key = abs_path.to_string_lossy().to_string();
    let Ok(metadata) = fs::metadata(&abs_path) else {
        return count_tokens(&doc.content);
    };
    let size = metadata.len();
    let mtime_secs = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Some(entry) = cache.get(&key) {
        if entry.size == size && entry.mtime_secs == mtime_secs {
            return entry.tokens;
        }
    }
    let tokens = count_tokens(&doc.content);
    cache.insert(
        key,
        TokenCacheEntry {
            mtime_secs,
            size,
            tokens,
        },
    );
    tokens
}

/// Trim context and return token breakdown without re-tokenizing.
pub fn trim_context_with_breakdown(
    mut components: PromptComponents,
    max_tokens: usize,
) -> (PromptComponents, ContextBreakdown) {
    let repo_root = PathBuf::from(&components.repo_root);
    static TOKEN_CACHE: Lazy<std::sync::Mutex<HashMap<String, TokenCacheEntry>>> =
        Lazy::new(|| std::sync::Mutex::new(HashMap::new()));
    let mut cache = TOKEN_CACHE
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();

    let mut breakdown = ContextBreakdown::default();

    if let Some(ref doc) = components.loopflow_doc {
        breakdown.system_tokens = count_tokens(doc);
    }

    if let Some(ref step) = components.step {
        if let Some(ref content) = step.content {
            breakdown.add_source_tokens(DocumentSource::Step, count_tokens(content));
        }
        breakdown.step_name = Some(step.name.clone());
    }

    for dir in &components.directions {
        breakdown.add_source_tokens(DocumentSource::Direction, count_tokens(&dir.content));
        breakdown.direction_names.push(dir.name.clone());
    }

    let diff_string_tokens = if let Some(ref diff) = components.diff {
        let tokens = count_tokens(diff);
        breakdown.add_source_tokens(DocumentSource::Diff, tokens);
        tokens
    } else {
        0
    };

    let mut diff_file_tokens: Vec<usize> = components
        .diff_files
        .iter()
        .map(|doc| cached_doc_tokens(doc, &repo_root, &mut cache))
        .collect();
    breakdown.add_source_tokens(DocumentSource::Diff, diff_file_tokens.iter().sum::<usize>());
    breakdown.diff_file_count = components.diff_file_count;
    breakdown.diff_tier = components.diff_tier.clone();

    let mut summary_tokens: Vec<usize> = components
        .summaries
        .iter()
        .map(|doc| cached_doc_tokens(doc, &repo_root, &mut cache))
        .collect();
    let mut wave_memory_tokens: Vec<usize> = components
        .wave_memory
        .iter()
        .map(|doc| cached_doc_tokens(doc, &repo_root, &mut cache))
        .collect();
    let mut doc_tokens: Vec<usize> = components
        .docs
        .iter()
        .map(|doc| cached_doc_tokens(doc, &repo_root, &mut cache))
        .collect();
    breakdown.add_source_tokens(
        DocumentSource::Summary,
        summary_tokens.iter().sum::<usize>(),
    );
    breakdown.add_source_tokens(
        DocumentSource::WaveMemory,
        wave_memory_tokens.iter().sum::<usize>(),
    );
    for (doc, tokens) in components.docs.iter().zip(doc_tokens.iter().copied()) {
        breakdown.add_source_tokens(doc.source, tokens);
    }
    breakdown.doc_count = components.docs.len() + components.summaries.len();

    // Area
    let mut area_doc_tokens: Vec<usize> = components
        .area_docs
        .iter()
        .map(|doc| cached_doc_tokens(doc, &repo_root, &mut cache))
        .collect();
    breakdown.add_source_tokens(DocumentSource::Area, area_doc_tokens.iter().sum::<usize>());
    breakdown.area_name = components.area.clone();
    breakdown.area_doc_count = components.area_docs.len();

    if let Some(ref clip) = components.clipboard {
        breakdown.set_source_tokens(DocumentSource::Clipboard, count_tokens(clip));
    }
    breakdown.has_clipboard = components.clipboard.is_some();
    breakdown.wave_name = components.wave.clone();

    let mut total = breakdown.total();
    if total > max_tokens {
        // 1. Drop area docs first (supplementary architectural context)
        while total > max_tokens && !components.area_docs.is_empty() {
            components.area_docs.pop();
            if let Some(tokens) = area_doc_tokens.pop() {
                breakdown.subtract_source_tokens(DocumentSource::Area, tokens);
                total = total.saturating_sub(tokens);
                breakdown.area_doc_count = breakdown.area_doc_count.saturating_sub(1);
            }
        }

        // 2. Drop wave memory docs before general docs/summaries.
        while total > max_tokens && !components.wave_memory.is_empty() {
            components.wave_memory.pop();
            if let Some(tokens) = wave_memory_tokens.pop() {
                breakdown.subtract_source_tokens(DocumentSource::WaveMemory, tokens);
                total = total.saturating_sub(tokens);
            }
        }

        // 3. Drop docs (summaries first, then docs)
        while total > max_tokens && !components.summaries.is_empty() {
            components.summaries.pop();
            if let Some(tokens) = summary_tokens.pop() {
                breakdown.subtract_source_tokens(DocumentSource::Summary, tokens);
                total = total.saturating_sub(tokens);
                breakdown.doc_count = breakdown.doc_count.saturating_sub(1);
            }
        }
        while total > max_tokens && !components.docs.is_empty() {
            let removed = components.docs.pop();
            if let Some(tokens) = doc_tokens.pop() {
                if let Some(doc) = removed {
                    breakdown.subtract_source_tokens(doc.source, tokens);
                }
                total = total.saturating_sub(tokens);
                breakdown.doc_count = breakdown.doc_count.saturating_sub(1);
            }
        }

        // 4. Drop diff context
        while total > max_tokens && !components.diff_files.is_empty() {
            components.diff_files.pop();
            if let Some(tokens) = diff_file_tokens.pop() {
                breakdown.subtract_source_tokens(DocumentSource::Diff, tokens);
                total = total.saturating_sub(tokens);
                breakdown.diff_file_count = breakdown.diff_file_count.saturating_sub(1);
            }
        }
        if total > max_tokens && components.diff.is_some() {
            components.diff = None;
            breakdown.diff_tier = DiffTier::None;
            breakdown.subtract_source_tokens(DocumentSource::Diff, diff_string_tokens);
            total = total.saturating_sub(diff_string_tokens);
        }

        // 5. Drop clipboard as last resort
        if total > max_tokens && components.clipboard.is_some() {
            components.clipboard = None;
            breakdown.set_source_tokens(DocumentSource::Clipboard, 0);
        }
    }

    components.diff_file_count = components.diff_files.len();
    if let Ok(mut guard) = TOKEN_CACHE.lock() {
        *guard = cache;
    }

    (components, breakdown)
}

/// Gather all prompt components.
pub fn gather_context(opts: &GatherContextOpts) -> Result<PromptComponents, CoreError> {
    let start = Instant::now();
    let repo_root = &opts.repo_root;

    // Load step
    let step_start = Instant::now();
    let step = match &opts.step {
        Some(step_name) => Some(load_step(step_name, repo_root)?),
        None => None,
    };
    debug!(elapsed_ms = step_start.elapsed().as_millis(), "loaded step");

    // Load directions
    let directions_start = Instant::now();
    let mut direction_names = Vec::new();
    if let Some(ref step) = step {
        direction_names.extend(step.directions.clone());
    }
    direction_names.extend(opts.directions.clone());
    let expanded_names = expand_direction_names(&direction_names, repo_root);
    let mut directions = Vec::new();
    for name in &expanded_names {
        directions.push(load_direction(name, repo_root)?);
    }
    debug!(
        elapsed_ms = directions_start.elapsed().as_millis(),
        count = directions.len(),
        "loaded directions"
    );

    let mut spec = opts.gather_spec();
    let include_branch_diff = opts.sources.contains(&DocumentSource::Diff);
    spec.normalize();

    // Gather document sources through a single pipeline.
    let docs_start = Instant::now();
    let gathered_docs = gather_documents(&spec)?;
    debug!(
        elapsed_ms = docs_start.elapsed().as_millis(),
        count = gathered_docs.len(),
        "gathered documents"
    );

    let mut docs = Vec::new();
    let mut summaries = Vec::new();
    let mut wave_memory = Vec::new();
    let mut diff_files = Vec::new();
    let mut area_docs = Vec::new();
    for doc in gathered_docs {
        match doc.source {
            DocumentSource::RepoDoc | DocumentSource::Wave => docs.push(doc),
            DocumentSource::Summary => summaries.push(doc),
            DocumentSource::WaveMemory => wave_memory.push(doc),
            DocumentSource::Area => area_docs.push(doc),
            DocumentSource::Diff => diff_files.push(doc),
            DocumentSource::Step | DocumentSource::Direction | DocumentSource::Clipboard => {}
        }
    }
    dedup_documents(&mut diff_files);

    // Gather diff context (tiered: unified diff or stat)
    let diff_start = Instant::now();
    let (diff, diff_tier, diff_file_count) = if include_branch_diff {
        gather_diff_tiered(repo_root)?
    } else {
        (None, DiffTier::None, 0)
    };
    debug!(
        elapsed_ms = diff_start.elapsed().as_millis(),
        ?diff_tier,
        has_diff = diff.is_some(),
        "gathered diff"
    );

    // Gather clipboard
    let clipboard_start = Instant::now();
    let clipboard = if spec.includes(DocumentSource::Clipboard) {
        read_clipboard()
    } else {
        None
    };
    debug!(
        elapsed_ms = clipboard_start.elapsed().as_millis(),
        has_clipboard = clipboard.is_some(),
        "read clipboard"
    );

    // Load bundled LOOPFLOW.md (system instructions, always included)
    let loopflow_doc = Some(crate::engine::builtins::LOOPFLOW_DOC.to_string());

    debug!(elapsed_ms = start.elapsed().as_millis(), "gathered context");
    Ok(PromptComponents {
        run_mode: opts.run_mode.clone(),
        docs,
        diff,
        diff_files,
        step,
        repo_root: repo_root.to_string_lossy().to_string(),
        clipboard,
        directions,
        summaries, // TODO: implement summary loading
        wave_memory,
        wave: opts.wave.clone(),
        loopflow_doc,
        message: opts.message.clone(),
        diff_tier,
        diff_file_count,
        area_docs,
        area: opts.area.clone(),
    })
}

/// Gather all requested document sources in stable prompt order.
pub fn gather_documents(spec: &GatherSpec) -> Result<Vec<Document>, CoreError> {
    let mut docs = Vec::new();

    // Preserve legacy ordering exactly: scratch -> wave -> wave memory -> root docs.
    if spec.includes(DocumentSource::RepoDoc) {
        docs.extend(gather_scratch_docs(&spec.repo_root)?);
    }
    if spec.includes(DocumentSource::Wave) {
        docs.extend(gather_wave_docs(&spec.repo_root, spec.wave.as_deref())?);
    }
    if spec.includes(DocumentSource::WaveMemory) {
        docs.extend(gather_wave_memory_doc(
            &spec.repo_root,
            spec.wave.as_deref(),
        )?);
    }
    if spec.includes(DocumentSource::RepoDoc) {
        docs.extend(gather_repo_root_docs(&spec.repo_root)?);
    }

    if spec.includes(DocumentSource::Area) {
        if let Some(ref area) = spec.area {
            docs.extend(gather_area_docs(&spec.repo_root, area));
        }
    }

    if spec.includes(DocumentSource::Diff) && !spec.files.is_empty() {
        docs.extend(gather_files(&spec.repo_root, &spec.files)?);
    }

    Ok(docs)
}

fn gather_scratch_docs(repo_root: &Path) -> Result<Vec<Document>, CoreError> {
    let mut docs = Vec::new();
    let scratch_dir = repo_root.join("scratch");
    if scratch_dir.is_dir() {
        gather_md_files(&scratch_dir, &mut docs, DocumentSource::RepoDoc)?;
    }
    Ok(docs)
}

fn gather_wave_docs(repo_root: &Path, wave: Option<&str>) -> Result<Vec<Document>, CoreError> {
    let mut docs = Vec::new();

    if let Some(wave_name) = wave {
        let wave_dir = repo_root.join("wave").join(wave_name);
        if wave_dir.is_dir() {
            // README first
            let readme = wave_dir.join("README.md");
            if readme.is_file() {
                if let Ok(content) = fs::read_to_string(&readme) {
                    docs.push(Document {
                        path: format!("wave/{}/README.md", wave_name),
                        content,
                        source: DocumentSource::Wave,
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
                        // Wave memory is gathered separately as DocumentSource::WaveMemory.
                        && path.file_name().map(|n| n != "MEMORY.md").unwrap_or(false)
                })
                .collect();
            entries.sort_by_key(|e| e.path());
            for entry in entries {
                let path = entry.path();
                if let Ok(content) = fs::read_to_string(&path) {
                    docs.push(Document {
                        path: format!(
                            "wave/{}/{}",
                            wave_name,
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ),
                        content,
                        source: DocumentSource::Wave,
                    });
                }
            }
        }
    }

    Ok(docs)
}

fn gather_wave_memory_doc(
    repo_root: &Path,
    wave: Option<&str>,
) -> Result<Vec<Document>, CoreError> {
    let Some(wave_name) = wave else {
        return Ok(Vec::new());
    };

    let memory_path = repo_root.join("wave").join(wave_name).join("MEMORY.md");
    if !memory_path.is_file() {
        return Ok(Vec::new());
    }

    let Ok(content) = fs::read_to_string(&memory_path) else {
        return Ok(Vec::new());
    };

    Ok(vec![Document {
        path: format!("wave/{wave_name}/MEMORY.md"),
        content,
        source: DocumentSource::WaveMemory,
    }])
}

fn gather_repo_root_docs(repo_root: &Path) -> Result<Vec<Document>, CoreError> {
    let mut docs = Vec::new();
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
                source: DocumentSource::RepoDoc,
            });
        }
    }
    Ok(docs)
}

/// Gather .md docs from the area directory and its ancestors.
///
/// For area "src/api/handlers", collects .md files from:
/// - src/ (e.g., src/README.md)
/// - src/api/ (e.g., src/api/README.md)
/// - src/api/handlers/ (e.g., src/api/handlers/README.md)
///
/// Does NOT include the repo root (already gathered by `gather_repo_root_docs`).
fn gather_area_docs(repo_root: &Path, area: &str) -> Vec<Document> {
    let area_path = Path::new(area);
    let mut ancestors = Vec::new();

    // Include the area directory itself and its ancestors (excluding repo root)
    if !area_path.as_os_str().is_empty() {
        ancestors.push(area_path.to_path_buf());
    }
    let mut current = area_path.to_path_buf();
    while let Some(parent) = current.parent() {
        if parent.as_os_str().is_empty() {
            break;
        }
        ancestors.push(parent.to_path_buf());
        current = parent.to_path_buf();
    }

    // Process from shallowest to deepest
    ancestors.reverse();

    let mut docs = Vec::new();
    let mut seen = HashSet::new();

    for ancestor in &ancestors {
        let abs_dir = repo_root.join(ancestor);
        if !abs_dir.is_dir() {
            continue;
        }

        let mut entries: Vec<_> = match fs::read_dir(&abs_dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let path = e.path();
                    path.is_file() && path.extension().map(|ext| ext == "md").unwrap_or(false)
                })
                .collect(),
            Err(_) => continue,
        };
        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let path = entry.path();
            let rel_path = path
                .strip_prefix(repo_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            if seen.contains(&rel_path) {
                continue;
            }

            if let Ok(content) = fs::read_to_string(&path) {
                seen.insert(rel_path.clone());
                docs.push(Document {
                    path: rel_path,
                    content,
                    source: DocumentSource::Area,
                });
            }
        }
    }

    docs
}

/// Recursively gather .md files from a directory.
fn gather_md_files(
    dir: &Path,
    docs: &mut Vec<Document>,
    source: DocumentSource,
) -> Result<(), CoreError> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            gather_md_files(&path, docs, source)?;
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Ok(content) = fs::read_to_string(&path) {
                docs.push(Document {
                    path: path
                        .strip_prefix(dir.parent().unwrap_or(dir))
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string(),
                    content,
                    source,
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

        if path.is_dir() {
            gather_dir_files(repo_root, &path, &gitignore, &mut seen, &mut docs)?;
            continue;
        }

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
            source: DocumentSource::Diff,
        });
    }

    Ok(docs)
}

/// Walk a directory and collect text files, respecting gitignore.
fn gather_dir_files(
    repo_root: &Path,
    dir: &Path,
    gitignore: &ignore::gitignore::Gitignore,
    seen: &mut HashSet<PathBuf>,
    docs: &mut Vec<Document>,
) -> Result<(), CoreError> {
    let walker = ignore::WalkBuilder::new(dir)
        .hidden(true)
        .standard_filters(true)
        .build();

    let mut entries: Vec<_> = walker
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .collect();
    entries.sort_by(|a, b| a.path().cmp(b.path()));

    for entry in entries {
        let path = entry.into_path();

        if should_exclude(repo_root, &path, gitignore) {
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
            source: DocumentSource::Diff,
        });
    }

    Ok(())
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
                source: DocumentSource::Diff,
            });
        }
    }

    Ok(docs)
}

/// Token threshold for tiered diff: below this, include full unified diff.
const DIFF_TIER_THRESHOLD: usize = 15_000;
/// Heuristic limits for attempting full diff.
const DIFF_MAX_FILES_FOR_FULL: usize = 20;
const DIFF_MAX_LINES_FOR_FULL: usize = 800;

/// Gather diff context with automatic tier selection.
///
/// - If unified diff is under 15k tokens, include full diff.
/// - Otherwise, fall back to diff stat (file list + lines changed).
///
/// Returns (diff_string, tier, file_count).
fn gather_diff_tiered(repo_root: &Path) -> Result<(Option<String>, DiffTier, usize), CoreError> {
    let branch_output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(repo_root)
        .output()?;

    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();
    if branch.is_empty() || branch == "main" {
        return Ok((None, DiffTier::None, 0));
    }

    let base_branch =
        crate::engine::git::get_default_branch(repo_root).unwrap_or("main".to_string());
    let diff_ref = format!("origin/{}...HEAD", base_branch);

    // Count committed changes vs base branch. If none, fall back to
    // working tree diff (handles branches with only staged changes).
    let name_output = Command::new("git")
        .args(["diff", "--name-only", &diff_ref])
        .current_dir(repo_root)
        .output()?;

    let committed_count = if name_output.status.success() {
        String::from_utf8_lossy(&name_output.stdout)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count()
    } else {
        0
    };

    let (diff_ref, file_count) = if committed_count > 0 {
        (diff_ref, committed_count)
    } else {
        // No committed changes (or no remote) — try working tree diff against HEAD.
        let head_output = Command::new("git")
            .args(["diff", "--name-only", "HEAD"])
            .current_dir(repo_root)
            .output()?;
        let head_count = if head_output.status.success() {
            String::from_utf8_lossy(&head_output.stdout)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count()
        } else {
            0
        };
        ("HEAD".to_string(), head_count)
    };

    if file_count == 0 {
        return Ok((None, DiffTier::None, 0));
    }

    let shortstat_output = Command::new("git")
        .args(["diff", "--shortstat", &diff_ref])
        .current_dir(repo_root)
        .output()?;
    let shortstat = if shortstat_output.status.success() {
        String::from_utf8_lossy(&shortstat_output.stdout).to_string()
    } else {
        String::new()
    };

    let total_lines = parse_shortstat_total_lines(&shortstat).unwrap_or(0);

    let stat_output = Command::new("git")
        .args(["diff", "--stat", &diff_ref])
        .current_dir(repo_root)
        .output()?;

    let stat = if stat_output.status.success() {
        String::from_utf8_lossy(&stat_output.stdout).to_string()
    } else {
        String::new()
    };

    let allow_full_diff =
        file_count <= DIFF_MAX_FILES_FOR_FULL && total_lines <= DIFF_MAX_LINES_FOR_FULL;

    if allow_full_diff {
        let diff_output = Command::new("git")
            .args(["diff", &diff_ref])
            .current_dir(repo_root)
            .output()?;

        if diff_output.status.success() {
            let diff = String::from_utf8_lossy(&diff_output.stdout).to_string();
            if !diff.trim().is_empty() && count_tokens(&diff) < DIFF_TIER_THRESHOLD {
                return Ok((Some(diff), DiffTier::UnifiedDiff, file_count));
            }
        }
    }

    if !stat.trim().is_empty() {
        return Ok((Some(stat), DiffTier::StatOnly, file_count));
    }

    Ok((None, DiffTier::None, file_count))
}

fn parse_shortstat_total_lines(output: &str) -> Option<usize> {
    let numbers: Vec<usize> = output
        .split_whitespace()
        .filter_map(|token| token.parse::<usize>().ok())
        .collect();
    if numbers.len() <= 1 {
        return Some(0);
    }
    Some(numbers[1..].iter().sum())
}

fn read_clipboard() -> Option<String> {
    crate::engine::clipboard::read()
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

/// File names that are always excluded from context (lock files, etc.).
const ALWAYS_EXCLUDE: &[&str] = &[
    "Cargo.lock",
    "uv.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "Gemfile.lock",
    "poetry.lock",
    "composer.lock",
];

fn should_exclude(repo_root: &Path, path: &Path, gitignore: &ignore::gitignore::Gitignore) -> bool {
    if path
        .components()
        .any(|component| component.as_os_str() == ".lf")
    {
        return true;
    }

    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if ALWAYS_EXCLUDE.contains(&name) {
            return true;
        }
    }

    let relative = path.strip_prefix(repo_root).unwrap_or(path);
    gitignore
        .matched_path_or_any_parents(relative, path.is_dir())
        .is_ignore()
}

/// Files that coding agents load natively. All are skipped from lf docs to
/// avoid duplication — whichever agent runs will pick up its own file.
const AGENT_NATIVE_FILES: &[&str] = &["CLAUDE.md", "AGENTS.md", "GEMINI.md"];

/// Remove docs that duplicate any agent's natively-loaded instruction file.
///
/// Skips all known native files (CLAUDE.md, AGENTS.md, GEMINI.md) and any
/// files they symlink to (e.g. CLAUDE.md -> STYLE.md also drops STYLE.md).
pub fn drop_native_instruction_docs(components: &mut PromptComponents, repo_root: &Path) {
    // Collect canonical paths of all native files (resolves symlinks)
    let canonical_paths: Vec<_> = AGENT_NATIVE_FILES
        .iter()
        .filter_map(|f| fs::canonicalize(repo_root.join(f)).ok())
        .collect();

    components.docs.retain(|doc| {
        let name = Path::new(&doc.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Drop the native files themselves
        if AGENT_NATIVE_FILES.contains(&name) {
            return false;
        }

        // Drop symlink partners (CLAUDE.md -> STYLE.md or STYLE.md -> CLAUDE.md)
        let doc_path = repo_root.join(&doc.path);
        if let Ok(doc_canon) = fs::canonicalize(&doc_path) {
            if canonical_paths.contains(&doc_canon) {
                return false;
            }
        }

        true
    });
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

/// Format direction tags as XML blocks.
fn format_direction_tags(directions: &[Direction]) -> String {
    if directions.len() == 1 {
        let d = &directions[0];
        format!(
            "<lf:direction:{}>\n{}\n</lf:direction:{}>",
            d.name, d.content, d.name
        )
    } else {
        let parts: Vec<String> = directions
            .iter()
            .map(|d| {
                format!(
                    "<lf:direction:{}>\n{}\n</lf:direction:{}>",
                    d.name, d.content, d.name
                )
            })
            .collect();
        format!("<lf:directions>\n{}\n</lf:directions>", parts.join("\n"))
    }
}

/// Render shared reference context sections.
///
/// These sections are common to both `format_prompt` (all-in-one) and
/// `format_context_prompt` (system prompt file). Extracted to avoid duplication.
///
/// Order: loopflow_doc, rlm_doc, run_mode, wave, docs, summaries, area_docs, diff+diff_files.
fn format_reference_sections(components: &PromptComponents) -> Vec<String> {
    let mut parts = Vec::new();

    // System docs (loopflow)
    if let Some(ref doc) = components.loopflow_doc {
        parts.push(format!("<lf:loopflow>\n{}\n</lf:loopflow>", doc));
    }

    // RLM instructions (recursive processing capability)
    if components.loopflow_doc.is_some() {
        parts.push(format!(
            "<lf:rlm>\n{}\n</lf:rlm>",
            crate::engine::builtins::RLM_DOC
        ));
    }

    // Run mode
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

    // Wave context
    if let Some(ref wave) = components.wave {
        let memory_path = format!("wave/{wave}/MEMORY.md");
        let memory_content = components
            .wave_memory
            .iter()
            .find(|doc| doc.path == memory_path)
            .map(|doc| {
                format!(
                    "<lf:memory path=\"{}\">\n{}\n</lf:memory>",
                    memory_path, doc.content
                )
            })
            .unwrap_or_else(|| {
                format!(
                    "<lf:memory path=\"{}\">\n(no memory yet)\n</lf:memory>",
                    memory_path
                )
            });

        parts.push(format!(
            "<lf:wave name=\"{}\">\n\
             You are building toward the {} program of work.\n\
             Wave context is included in docs below.\n\n\
             Persistent wave memory lives at {}.\n\
             Write durable observations for future agents:\n\
             - codebase patterns and conventions discovered\n\
             - user preferences and project norms observed\n\
             - things that worked or failed\n\
             - corrections when memory becomes stale or wrong\n\n\
             Don't write session-specific notes.\n\
             Keep memory concise and update existing entries when possible.\n\n\
             {}\n\
             </lf:wave>",
            wave, wave, memory_path, memory_content
        ));
    }

    // Reference material (docs, summaries)
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
             May include design artifacts (scratch/).\n\n\
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

    // Area docs (parent directory READMEs when -a is set)
    if !components.area_docs.is_empty() {
        let area_label = components.area.as_deref().unwrap_or("area");
        let area_parts: Vec<String> = components
            .area_docs
            .iter()
            .map(|doc| {
                let name = Path::new(&doc.path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| doc.path.clone());
                format!("<lf:{}>\n{}\n</lf:{}>", name, doc.content, name)
            })
            .collect();
        let area_body = area_parts.join("\n\n");
        parts.push(format!(
            "Area docs for `{}`. Architectural context from parent directories.\n\n\
             <lf:area>\n{}\n</lf:area>",
            area_label, area_body
        ));
    }

    // Working context (diff, diff_files)
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

    parts
}

/// Format step tag.
fn format_step_tag(step: &Step) -> String {
    if let Some(ref content) = step.content {
        format!(
            "<lf:step:{}>\n{}\n</lf:step:{}>",
            step.name, content, step.name
        )
    } else {
        format!("<lf:step:{}>\n</lf:step:{}>", step.name, step.name)
    }
}

/// Format prompt content for the requested mode.
///
/// Used by the daemon, ops callers, and prompt log writers.
pub fn format_prompt(mode: PromptFormatMode, components: &PromptComponents) -> String {
    match mode {
        PromptFormatMode::Full => {
            let mut parts = format_reference_sections(components);

            // Task sections: step, directions, clipboard, message
            if let Some(ref step) = components.step {
                parts.push(format!("The step.\n\n{}", format_step_tag(step)));
            }

            if !components.directions.is_empty() {
                let label = if components.directions.len() == 1 {
                    "Direction"
                } else {
                    "Directions"
                };
                parts.push(format!(
                    "{label} for this work.\n\n{}",
                    format_direction_tags(&components.directions)
                ));
            }

            if let Some(ref clipboard) = components.clipboard {
                parts.push(format!(
                    "Content from clipboard.\n\n\
                     <lf:clipboard>\n{}\n</lf:clipboard>",
                    clipboard
                ));
            }

            if let Some(ref message) = components.message {
                parts.push(format!(
                    "Additional instructions from user.\n\n\
                     <lf:message>\n{}\n</lf:message>",
                    message
                ));
            }

            parts.join("\n\n")
        }
        PromptFormatMode::Context => {
            let mut parts = format_reference_sections(components);

            if let Some(ref clipboard) = components.clipboard {
                parts.push(format!(
                    "Content from clipboard.\n\n\
                     <lf:clipboard>\n{}\n</lf:clipboard>",
                    clipboard
                ));
            }

            parts.join("\n\n")
        }
        PromptFormatMode::Task => {
            let mut parts = Vec::new();

            if !components.directions.is_empty() {
                parts.push(format_direction_tags(&components.directions));
            }

            if let Some(ref step) = components.step {
                parts.push(format_step_tag(step));
            }

            if let Some(ref message) = components.message {
                parts.push(message.clone());
            }

            parts.join("\n\n")
        }
    }
}

/// Format context components for system prompt (everything except task).
pub fn format_context_prompt(components: &PromptComponents) -> String {
    format_prompt(PromptFormatMode::Context, components)
}

/// Format task prompt for user message (directions + step + free text).
pub fn format_task_prompt(components: &PromptComponents) -> String {
    format_prompt(PromptFormatMode::Task, components)
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
    // Replace / with . so namespaced steps (e.g., scan/scan-report) don't create subdirectories.
    let safe_step = step_name.replace('/', ".");
    let name_part = match flow_parents {
        Some(parents) if !parents.is_empty() => {
            format!("{}.{}", parents.join("."), safe_step)
        }
        _ => safe_step,
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
    use crate::engine::flow::{Direction, Step};
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
    fn trim_context_under_budget() {
        let mut components = PromptComponents::default();
        components.docs.push(Document {
            path: "test.md".to_string(),
            content: "Short content".to_string(),
            source: DocumentSource::RepoDoc,
        });

        let trimmed = trim_context_with_breakdown(components.clone(), 1000000).0;
        assert_eq!(trimmed.docs.len(), 1);
    }

    #[test]
    fn trim_context_drops_summaries_first() {
        let components = PromptComponents {
            docs: vec![Document {
                path: "doc.md".to_string(),
                content: "Doc content".to_string(),
                source: DocumentSource::RepoDoc,
            }],
            summaries: vec![Document {
                path: "summary.md".to_string(),
                content: "Summary content that is long enough to matter".to_string(),
                source: DocumentSource::Summary,
            }],
            ..Default::default()
        };

        // Set budget to only fit docs, not summaries
        let doc_tokens = count_tokens("Doc content");
        let trimmed = trim_context_with_breakdown(components, doc_tokens + 5).0;

        assert!(trimmed.summaries.is_empty());
        assert_eq!(trimmed.docs.len(), 1);
    }

    #[test]
    fn trim_context_drops_wave_memory_before_summaries() {
        let components = PromptComponents {
            step: Some(Step {
                name: "test".to_string(),
                content: Some("x".to_string()),
                model: None,
                directions: vec![],
                interactive: None,
            }),
            wave_memory: vec![Document {
                path: "wave/living/MEMORY.md".to_string(),
                content: "Wave memory content that should be trimmed first".to_string(),
                source: DocumentSource::WaveMemory,
            }],
            summaries: vec![Document {
                path: "summary.md".to_string(),
                content: "Summary should survive after wave memory is dropped".to_string(),
                source: DocumentSource::Summary,
            }],
            ..Default::default()
        };

        let budget = count_tokens("x")
            + count_tokens("Summary should survive after wave memory is dropped")
            + 1;
        let trimmed = trim_context_with_breakdown(components, budget).0;

        assert!(trimmed.wave_memory.is_empty());
        assert_eq!(trimmed.summaries.len(), 1);
    }

    #[test]
    fn trim_context_drops_docs_after_summaries() {
        let components = PromptComponents {
            docs: vec![
                Document {
                    path: "doc1.md".to_string(),
                    content: "First document with enough content to exceed token budget easily"
                        .to_string(),
                    source: DocumentSource::RepoDoc,
                },
                Document {
                    path: "doc2.md".to_string(),
                    content: "Second document also has substantial content for testing".to_string(),
                    source: DocumentSource::RepoDoc,
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
        let trimmed = trim_context_with_breakdown(components, step_tokens + 1).0;
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
        let trimmed = trim_context_with_breakdown(components, step_tokens + 1).0;
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
                source: DocumentSource::RepoDoc,
            }],
            ..Default::default()
        };

        let trimmed = trim_context_with_breakdown(components, 5).0;
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

        let prompt = format_prompt(PromptFormatMode::Full, &components);
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

        let prompt = format_prompt(PromptFormatMode::Full, &components);
        assert!(!prompt.contains("Run mode is auto"));
    }

    #[test]
    fn format_prompt_with_wave() {
        let components = PromptComponents {
            wave: Some("rust".to_string()),
            ..Default::default()
        };

        let prompt = format_prompt(PromptFormatMode::Full, &components);
        assert!(prompt.contains("<lf:wave"));
        assert!(prompt.contains("name=\"rust\""));
        assert!(prompt.contains("rust program of work"));
        assert!(prompt.contains("</lf:wave>"));
    }

    #[test]
    fn format_prompt_with_wave_memory() {
        let components = PromptComponents {
            wave: Some("living".to_string()),
            wave_memory: vec![Document {
                path: "wave/living/MEMORY.md".to_string(),
                content: "- prefer focused tests\n- run cargo fmt first".to_string(),
                source: DocumentSource::WaveMemory,
            }],
            ..Default::default()
        };

        let prompt = format_prompt(PromptFormatMode::Full, &components);
        assert!(prompt.contains("Persistent wave memory lives at wave/living/MEMORY.md"));
        assert!(prompt.contains("<lf:memory path=\"wave/living/MEMORY.md\">"));
        assert!(prompt.contains("prefer focused tests"));
    }

    #[test]
    fn format_prompt_with_docs() {
        let components = PromptComponents {
            docs: vec![
                Document {
                    path: "README.md".to_string(),
                    content: "# Test Project".to_string(),
                    source: DocumentSource::RepoDoc,
                },
                Document {
                    path: "STYLE.md".to_string(),
                    content: "# Style Guide".to_string(),
                    source: DocumentSource::RepoDoc,
                },
            ],
            ..Default::default()
        };

        let prompt = format_prompt(PromptFormatMode::Full, &components);
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
                source: DocumentSource::RepoDoc,
            }],
            ..Default::default()
        };
        let prompt = format_prompt(PromptFormatMode::Full, &components);
        assert!(prompt.contains("CLAUDE"));
        assert!(prompt.contains("Follow") || prompt.contains("carefully"));
    }

    #[test]
    fn format_prompt_style_md_gets_follow_note() {
        let components = PromptComponents {
            docs: vec![Document {
                path: "STYLE.md".to_string(),
                content: "# Style Guide".to_string(),
                source: DocumentSource::RepoDoc,
            }],
            ..Default::default()
        };
        let prompt = format_prompt(PromptFormatMode::Full, &components);
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

        let prompt = format_prompt(PromptFormatMode::Full, &components);
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

        let prompt = format_prompt(PromptFormatMode::Full, &components);
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

        let prompt = format_prompt(PromptFormatMode::Full, &components);
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

        let prompt = format_prompt(PromptFormatMode::Full, &components);
        assert!(prompt.contains("<lf:step:review>"));
        assert!(prompt.contains("</lf:step:review>"));
    }

    #[test]
    fn format_prompt_with_diff() {
        let components = PromptComponents {
            diff: Some("diff --git a/file.rs\n+added line".to_string()),
            ..Default::default()
        };

        let prompt = format_prompt(PromptFormatMode::Full, &components);
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
                source: DocumentSource::Diff,
            }],
            ..Default::default()
        };

        let prompt = format_prompt(PromptFormatMode::Full, &components);
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

        let prompt = format_prompt(PromptFormatMode::Full, &components);
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
                source: DocumentSource::Summary,
            }],
            ..Default::default()
        };

        let prompt = format_prompt(PromptFormatMode::Full, &components);
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
                source: DocumentSource::RepoDoc,
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

        let prompt = format_prompt(PromptFormatMode::Full, &components);

        // Verify order: loopflow -> run_mode -> wave -> docs -> diff -> step -> direction -> clipboard
        let loopflow_pos = prompt.find("<lf:loopflow>").unwrap();
        let auto_pos = prompt.find("Run mode is auto").unwrap();
        let wave_pos = prompt.find("<lf:wave").unwrap();
        let docs_pos = prompt.find("<lf:docs>").unwrap();
        let diff_pos = prompt.find("<lf:diff>").unwrap();
        let step_pos = prompt.find("<lf:step:implement>").unwrap();
        let direction_pos = prompt.find("<lf:direction:concise>").unwrap();
        let clipboard_pos = prompt.find("<lf:clipboard>").unwrap();

        assert!(loopflow_pos < auto_pos);
        assert!(auto_pos < wave_pos);
        assert!(wave_pos < docs_pos);
        assert!(docs_pos < diff_pos);
        assert!(diff_pos < step_pos);
        assert!(step_pos < direction_pos);
        assert!(direction_pos < clipboard_pos);
    }

    #[test]
    fn format_prompt_empty_components() {
        let components = PromptComponents::default();
        let prompt = format_prompt(PromptFormatMode::Full, &components);
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
            sources: vec![],
            ..Default::default()
        };
        let ctx = gather_context(&opts).expect("gather context");
        let prompt = format_prompt(PromptFormatMode::Full, &ctx);

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
    fn gather_context_with_specific_files_does_not_pull_branch_diff() {
        let repo = init_git_repo();
        Command::new("git")
            .args(["checkout", "-b", "feature"])
            .current_dir(repo.path())
            .output()
            .expect("git checkout");
        write_file(repo.path(), "src/a.rs", "mod a;");
        write_file(repo.path(), "src/unrelated.rs", "mod unrelated;");
        Command::new("git")
            .args(["add", "src/a.rs", "src/unrelated.rs"])
            .current_dir(repo.path())
            .output()
            .expect("git add");

        let opts = GatherContextOpts {
            repo_root: repo.path().to_path_buf(),
            files: vec!["src/a.rs".to_string()],
            sources: vec![],
            ..Default::default()
        };
        let ctx = gather_context(&opts).expect("gather context");
        let prompt = format_prompt(PromptFormatMode::Full, &ctx);

        assert!(
            ctx.diff.is_none(),
            "files-only context should not include branch diff"
        );
        assert!(!prompt.contains("<lf:diff>"));
        assert!(prompt.contains("mod a;"));
        assert!(!prompt.contains("mod unrelated;"));
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
            sources: vec![],
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
            sources: vec![DocumentSource::RepoDoc],
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
            sources: vec![],
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
            sources: vec![],
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
            sources: vec![],
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
            sources: vec![],
            ..Default::default()
        };

        let result = gather_context(&opts);
        assert!(result.is_ok());
        let components = result.unwrap();
        assert_eq!(components.wave, Some("rust-migration".to_string()));
    }

    #[test]
    fn gather_context_reads_wave_memory() {
        let repo = init_repo();
        write_file(repo.path(), "wave/living/README.md", "# Living");
        write_file(
            repo.path(),
            "wave/living/MEMORY.md",
            "- always run rustfmt before commit",
        );

        let opts = GatherContextOpts {
            repo_root: repo.path().to_path_buf(),
            wave: Some("living".to_string()),
            sources: vec![DocumentSource::RepoDoc],
            ..Default::default()
        };

        let result = gather_context(&opts);
        assert!(result.is_ok());
        let components = result.unwrap();
        assert_eq!(components.wave_memory.len(), 1);
        assert_eq!(components.wave_memory[0].source, DocumentSource::WaveMemory);
        assert!(components.wave_memory[0]
            .content
            .contains("always run rustfmt before commit"));
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
                source: DocumentSource::RepoDoc,
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
        assert!(context.contains("<lf:clipboard>"));
        // Should NOT include step or directions (both go in task prompt)
        assert!(!context.contains("<lf:step:debug>"));
        assert!(!context.contains("Fix the error."));
        assert!(!context.contains("<lf:direction:concise>"));
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
    fn format_task_prompt_empty_when_no_step_or_message() {
        let components = PromptComponents::default();
        let task = format_task_prompt(&components);
        assert!(task.is_empty());
    }

    #[test]
    fn format_task_prompt_includes_message() {
        let components = PromptComponents {
            message: Some("fix the login bug".to_string()),
            ..Default::default()
        };
        let task = format_task_prompt(&components);
        assert_eq!(task, "fix the login bug");
    }

    #[test]
    fn format_task_prompt_message_with_step() {
        let components = PromptComponents {
            step: Some(Step {
                name: "debug".to_string(),
                content: Some("Debug the error.".to_string()),
                model: None,
                directions: vec![],
                interactive: None,
            }),
            message: Some("login page crashes".to_string()),
            ..Default::default()
        };
        let task = format_task_prompt(&components);
        assert!(task.contains("<lf:step:debug>"));
        assert!(task.contains("login page crashes"));
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

    // ==========================================================================
    // gather_diff_tiered tests
    // ==========================================================================

    fn init_git_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(dir.path())
            .output()
            .expect("git init");
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir.path())
            .output()
            .expect("git config email");
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir.path())
            .output()
            .expect("git config name");
        std::fs::write(dir.path().join("README.md"), "# Test").expect("write readme");
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .output()
            .expect("git add");
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(dir.path())
            .output()
            .expect("git commit");
        dir
    }

    #[test]
    fn gather_diff_tiered_on_main_returns_none() {
        let repo = init_git_repo();
        let (diff, tier, _) = gather_diff_tiered(repo.path()).unwrap();
        assert!(diff.is_none());
        assert_eq!(tier, DiffTier::None);
    }

    #[test]
    fn gather_diff_tiered_uncommitted_changes_on_branch() {
        let repo = init_git_repo();
        // Create a branch with no commits ahead of main, just dirty working tree
        Command::new("git")
            .args(["checkout", "-b", "feature"])
            .current_dir(repo.path())
            .output()
            .expect("git checkout");
        std::fs::write(repo.path().join("new_file.rs"), "fn hello() {}").expect("write file");
        Command::new("git")
            .args(["add", "new_file.rs"])
            .current_dir(repo.path())
            .output()
            .expect("git add");

        let (diff, tier, count) = gather_diff_tiered(repo.path()).unwrap();
        // Should fall back to working tree diff (HEAD) and find the staged change
        assert!(diff.is_some(), "should find uncommitted changes");
        assert_eq!(tier, DiffTier::UnifiedDiff);
        assert_eq!(count, 1);
        assert!(diff.unwrap().contains("fn hello()"));
    }

    #[test]
    fn gather_diff_tiered_committed_changes_on_branch() {
        let repo = init_git_repo();
        // Set up a bare origin inside the repo's own temp dir to avoid collisions
        let origin_dir = tempfile::tempdir().expect("origin tempdir");
        let origin_path = origin_dir.path().join("origin.git");
        Command::new("git")
            .args([
                "clone",
                "--bare",
                repo.path().to_str().unwrap(),
                origin_path.to_str().unwrap(),
            ])
            .output()
            .expect("git clone bare");
        Command::new("git")
            .args(["remote", "add", "origin", origin_path.to_str().unwrap()])
            .current_dir(repo.path())
            .output()
            .expect("git remote add");
        Command::new("git")
            .args(["fetch", "origin"])
            .current_dir(repo.path())
            .output()
            .expect("git fetch");

        Command::new("git")
            .args(["checkout", "-b", "feature"])
            .current_dir(repo.path())
            .output()
            .expect("git checkout");
        std::fs::write(repo.path().join("committed.rs"), "fn committed() {}").expect("write file");
        Command::new("git")
            .args(["add", "committed.rs"])
            .current_dir(repo.path())
            .output()
            .expect("git add");
        Command::new("git")
            .args(["commit", "-m", "add committed file"])
            .current_dir(repo.path())
            .output()
            .expect("git commit");

        let (diff, tier, count) = gather_diff_tiered(repo.path()).unwrap();
        assert!(diff.is_some(), "should find committed changes");
        assert_eq!(tier, DiffTier::UnifiedDiff);
        assert_eq!(count, 1);
        assert!(diff.unwrap().contains("fn committed()"));
    }
}
