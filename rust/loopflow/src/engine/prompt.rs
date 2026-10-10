//! Context gathering and prompt assembly for LLM sessions.
//!
//! This module handles gathering all context components (docs, diff, clipboard, etc.)
//! and assembling them into a formatted prompt.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::engine::error::CoreError;
use crate::engine::flow::{load_skill, Skill};
use crate::repository::RepoId;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tiktoken_rs::CoreBPE;
use tracing::{debug, warn};

/// Source of a context document or context token bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentSource {
    Skill,
    Scratch,
    Wave,
    RepoMemory,
    Docs,
    Summary,
    Diff,
    Clipboard,
}

/// A related repo resolved from the edge graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedRepoContext {
    pub repo_id: RepoId,
    pub path: PathBuf,
}

/// A document included in context.
#[derive(Debug, Clone)]
pub struct Document {
    pub path: String,
    pub content: String,
    pub source: DocumentSource,
}

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

const MAX_EXPLICIT_DOC_FILES: usize = 100;
static BPE: Lazy<Option<CoreBPE>> = Lazy::new(|| tiktoken_rs::cl100k_base().ok());
static CL100K_PIECES: Lazy<Option<fancy_regex::Regex>> = Lazy::new(|| {
    fancy_regex::Regex::new(
        r"'(?i:[sdmt]|ll|ve|re)|[^\r\n\p{L}\p{N}]?+\p{L}++|\p{N}{1,3}+| ?[^\s\p{L}\p{N}]++[\r\n]*+|\s++$|\s*[\r\n]|\s+(?!\S)|\s",
    )
    .ok()
});

/// Specification for document targets to gather.
#[derive(Debug, Clone, Default)]
pub struct GatherSpec {
    pub repo_root: PathBuf,
    /// Explicit docs paths, globs, or directories to include in context.
    pub docs: Vec<String>,
    /// Specific files to include in context.
    pub files: Vec<String>,
    pub include_files: bool,
    /// Wave name for wave/ scoping.
    pub wave: Option<String>,
    /// Related repos resolved from the edge graph.
    pub related_repos: Vec<RelatedRepoContext>,
}

/// Options for gathering context.
#[derive(Debug, Clone, Default)]
pub struct GatherContextOpts {
    pub repo_root: PathBuf,
    pub skill: Option<String>,
    /// User message (positional args after skill/flow name, or inline prompt)
    pub message: Option<String>,
    /// Include loopflow operating guidance.
    pub operate: bool,
    pub surface: Surface,
    /// Explicit docs paths, globs, or directories to include in context.
    pub docs: Vec<String>,
    /// Specific files to include in context.
    pub files: Vec<String>,
    /// Wave name for wave/ scoping.
    pub wave: Option<String>,
    pub include_diff: bool,
    pub include_diff_files: bool,
    pub include_clipboard: bool,
    /// Related repos resolved from the edge graph.
    pub related_repos: Vec<RelatedRepoContext>,
}

impl GatherContextOpts {
    pub fn gather_spec(&self) -> GatherSpec {
        GatherSpec {
            repo_root: self.repo_root.clone(),
            docs: self.docs.clone(),
            files: self.files.clone(),
            include_files: self.include_diff_files,
            wave: self.wave.clone(),
            related_repos: self.related_repos.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Cli,
    /// Retained for recorded app-launch context; no launcher emits this surface.
    Ide,
    Mac,
    Iphone,
    /// A live chat channel (e.g. Discord) where a person is reading. The reply
    /// is the message they see, so the agent speaks plainly and deliberately.
    Chat,
    #[default]
    #[serde(other)]
    Headless,
}

impl Surface {
    /// State whether someone is participating in this conversation before the skill can decide
    /// where a real dependency should route.
    pub fn instructions(self) -> &'static str {
        match self {
            Self::Headless => crate::engine::builtins::SURFACE_HEADLESS,
            Self::Chat => crate::engine::builtins::SURFACE_CHAT,
            _ => crate::engine::builtins::SURFACE_HUMAN_PRESENT,
        }
    }
}

impl std::str::FromStr for Surface {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let surface = match value {
            "cli" => Self::Cli,
            "ide" => Self::Ide,
            "mac" => Self::Mac,
            "iphone" => Self::Iphone,
            "chat" => Self::Chat,
            _ => Self::Headless,
        };
        Ok(surface)
    }
}

/// All components of a prompt before assembly.
#[derive(Debug, Clone, Default)]
pub struct PromptComponents {
    pub surface: Surface,
    pub user_name: Option<String>,
    pub docs: Vec<Document>,
    pub diff: Option<String>,
    pub diff_files: Vec<Document>,
    pub skill: Option<Skill>,
    pub repo_root: String,
    pub clipboard: Option<String>,
    pub summaries: Vec<Document>,
    pub wave: Option<String>,
    /// Include loopflow operating guidance.
    pub operate: bool,
    /// User message (positional args after skill/flow name)
    pub message: Option<String>,
    /// Semantic attribution for generated intent carried in `message`.
    /// Ordinary CLI speech leaves this absent and is attributed to the user.
    pub message_context: Option<(crate::trace::ContextAssetKind, crate::trace::ContextScope)>,
    /// Steers rendered into `message`, when the launch knows them.
    pub steers: Vec<crate::durable::Steer>,
    /// How diff context was tiered
    pub diff_tier: DiffTier,
    /// Number of files changed on branch (for display)
    pub diff_file_count: usize,
}

/// Count tokens using tiktoken (cl100k_base encoding).
/// Falls back to byte length / 3 if tiktoken fails.
pub fn count_tokens(text: &str) -> usize {
    if let Some(bpe) = BPE.as_ref() {
        return std::cmp::max(bpe.encode_ordinary(text).len(), 1);
    }
    // Fallback: rough estimate
    std::cmp::max(text.len() / 3, 1)
}

/// Return cumulative byte ends for the tokens in one exact encoding.
pub(crate) fn token_byte_ends(text: &str) -> Option<Vec<usize>> {
    let bpe = BPE.as_ref()?;
    let tokens = bpe.encode_ordinary(text);
    let mut byte_lengths = HashMap::new();
    let mut byte_end = 0;
    Some(
        tokens
            .into_iter()
            .map(|token| {
                let byte_length = *byte_lengths.entry(token).or_insert_with(|| {
                    bpe.decode_bytes(&[token])
                        .expect("an encoded token must decode")
                        .len()
                });
                byte_end += byte_length;
                byte_end
            })
            .collect(),
    )
}

#[derive(Debug)]
pub(crate) struct PromptTokenAccounting {
    pub total: usize,
    pub prefixes: Vec<usize>,
    pub isolated: Vec<usize>,
}

/// Account for exact prefixes and isolated ranges from one full encoding.
pub(crate) fn account_prompt_tokens(
    text: &str,
    prefix_ends: &[usize],
    ranges: &[(usize, usize)],
) -> Option<PromptTokenAccounting> {
    let bpe = BPE.as_ref()?;
    let regex = CL100K_PIECES.as_ref()?;
    let token_ends = token_byte_ends(text)?;
    let pieces = regex.find_iter(text).collect::<Result<Vec<_>, _>>().ok()?;
    let token_boundaries = token_ends
        .iter()
        .copied()
        .chain([0, text.len()])
        .collect::<HashSet<_>>();
    let pieces_cover_text = pieces.first().is_none_or(|piece| piece.start() == 0)
        && pieces.last().is_none_or(|piece| piece.end() == text.len())
        && pieces
            .windows(2)
            .all(|pair| pair[0].end() == pair[1].start());
    if !pieces_cover_text
        || pieces.iter().any(|piece| {
            !token_boundaries.contains(&piece.start()) || !token_boundaries.contains(&piece.end())
        })
    {
        return None;
    }
    let prefix_count = |end: usize| {
        if end == text.len() {
            return Some(token_ends.len());
        }
        let piece_index = pieces.partition_point(|piece| piece.end() < end);
        let piece = pieces.get(piece_index)?;
        if end < piece.start() || end > piece.end() {
            return None;
        }
        let complete_tokens = token_ends.partition_point(|token_end| *token_end <= piece.start());
        let partial_tokens = if end == piece.start() {
            0
        } else {
            bpe.encode_ordinary(&text[piece.start()..end]).len()
        };
        Some(complete_tokens + partial_tokens)
    };
    let prefixes = prefix_ends
        .iter()
        .map(|end| prefix_count(*end))
        .collect::<Option<Vec<_>>>()?;
    let isolated = ranges
        .iter()
        .map(|(start, end)| {
            if pieces
                .binary_search_by_key(start, |piece| piece.start())
                .is_ok()
                && (*end == text.len()
                    || pieces
                        .binary_search_by_key(end, |piece| piece.start())
                        .is_ok())
            {
                let start_tokens = token_ends.partition_point(|token_end| *token_end <= *start);
                let end_tokens = token_ends.partition_point(|token_end| *token_end <= *end);
                end_tokens - start_tokens
            } else {
                bpe.encode_ordinary(&text[*start..*end]).len()
            }
        })
        .collect();
    Some(PromptTokenAccounting {
        total: token_ends.len(),
        prefixes,
        isolated,
    })
}

/// Gather all prompt components.
pub fn gather_context(opts: &GatherContextOpts) -> Result<PromptComponents, CoreError> {
    let start = Instant::now();
    let repo_root = &opts.repo_root;

    // Load skill
    let skill_start = Instant::now();
    let skill = match &opts.skill {
        Some(skill_name) => Some(load_skill(skill_name, repo_root)?),
        None => None,
    };
    debug!(
        elapsed_ms = skill_start.elapsed().as_millis(),
        "loaded skill"
    );

    let spec = opts.gather_spec();

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
    let mut diff_files = Vec::new();
    for doc in gathered_docs {
        match doc.source {
            DocumentSource::Docs
            | DocumentSource::Scratch
            | DocumentSource::Wave
            | DocumentSource::RepoMemory => docs.push(doc),
            DocumentSource::Summary => summaries.push(doc),
            DocumentSource::Diff => diff_files.push(doc),
            DocumentSource::Skill | DocumentSource::Clipboard => {}
        }
    }
    // Gather diff context (tiered: unified diff or stat)
    let diff_start = Instant::now();
    let (diff, diff_tier, diff_file_count) = if opts.include_diff {
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
    let clipboard = if opts.include_clipboard {
        read_clipboard()
    } else {
        None
    };
    debug!(
        elapsed_ms = clipboard_start.elapsed().as_millis(),
        has_clipboard = clipboard.is_some(),
        "read clipboard"
    );

    debug!(elapsed_ms = start.elapsed().as_millis(), "gathered context");
    Ok(PromptComponents {
        surface: opts.surface,
        user_name: crate::engine::config::participant_name()?,
        docs,
        diff,
        diff_files,
        skill,
        repo_root: repo_root.to_string_lossy().to_string(),
        clipboard,
        summaries,
        wave: opts.wave.clone(),
        operate: opts.operate,
        message: opts.message.clone(),
        message_context: None,
        steers: Vec::new(),
        diff_tier,
        diff_file_count,
    })
}

/// Gather all requested document sources in stable prompt order.
pub fn gather_documents(spec: &GatherSpec) -> Result<Vec<Document>, CoreError> {
    let mut docs = Vec::new();

    // Preserve ambient ordering: scratch -> inherited context -> explicit docs.
    docs.extend(gather_scratch_docs(&spec.repo_root)?);
    let repo_memory = spec.repo_root.join("MEMORY.md");
    if repo_memory.is_file() {
        docs.push(Document {
            path: "MEMORY.md".into(),
            content: fs::read_to_string(repo_memory)?,
            source: DocumentSource::RepoMemory,
        });
    }
    docs.extend(gather_wave_docs(&spec.repo_root, spec.wave.as_deref())?);
    if !spec.docs.is_empty() {
        let explicit_docs = gather_doc_targets(&spec.repo_root, &spec.docs, &spec.related_repos)?;
        if explicit_docs.len() > MAX_EXPLICIT_DOC_FILES {
            return Err(CoreError::ExecutionFailed(format!(
                "--docs resolved to {} files; narrow --docs to {} files or fewer",
                explicit_docs.len(),
                MAX_EXPLICIT_DOC_FILES
            )));
        }
        docs.extend(explicit_docs);
    }

    if spec.include_files {
        let files = if spec.files.is_empty() {
            gather_changed_file_paths(&spec.repo_root)?
        } else {
            spec.files.clone()
        };
        docs.extend(gather_files(&spec.repo_root, &files)?);
    }

    Ok(docs)
}

fn gather_scratch_docs(repo_root: &Path) -> Result<Vec<Document>, CoreError> {
    let mut docs = Vec::new();
    let scratch_dir = repo_root.join("scratch");
    if scratch_dir.is_dir() {
        gather_md_files(&scratch_dir, &mut docs, DocumentSource::Scratch)?;
    }
    for doc in &docs {
        for (line, reason) in scratch_plan_warnings(&doc.content) {
            warn!(path = %doc.path, line, "scratch reference: {reason}");
        }
    }
    Ok(docs)
}

fn scratch_plan_warnings(content: &str) -> Vec<(usize, &'static str)> {
    static FRAMING: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)\b(?:(?:this|current)\s+(?:conversation|session|step|run|kickoff)|(?:selected|current|active)\s+Machine)\b")
            .expect("valid scratch framing expression")
    });
    static MENTION: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"\$[A-Za-z_][A-Za-z0-9_/-]*").expect("valid skill mention expression")
    });
    let mut warnings = Vec::new();
    for (pattern, reason) in [
        (
            &*FRAMING,
            "record decisions and status without execution-relative framing or ambient Machine facts",
        ),
        (
            &*MENTION,
            "dollar-prefixed mentions can activate native skills; use plain skill names in plans",
        ),
    ] {
        for found in pattern.find_iter(content) {
            let line = content[..found.start()]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1;
            warnings.push((line, reason));
        }
    }
    warnings.sort_by_key(|(line, _)| *line);
    warnings.dedup();
    warnings
}

/// Reference text must not activate a provider's native dollar-prefixed skills.
/// Encode every dollar so this does not depend on the installed skill catalog.
/// Source components remain unchanged; submitted bytes carry the notation key.
pub(crate) fn render_reference(text: &str) -> String {
    if !text.contains('$') {
        return text.to_string();
    }
    format!(
        "Reference notation: &#36; represents a literal dollar sign in the source, not a skill invocation.\n\n{}",
        escape_reference(text)
    )
}

pub(crate) fn escape_reference(text: &str) -> String {
    text.replace('$', "&#36;")
}

/// Messages can mix a live request with embedded reference sections and quotes.
pub(crate) fn render_message(message: &str) -> String {
    static REFERENCES: Lazy<Regex> = Lazy::new(|| {
        let sections = [
            "steers",
            "wave-memory",
            "task-workspace",
            "file",
            "summary",
            "diff",
            "clipboard",
        ]
        .map(|tag| format!(r"<lf:{tag}\b[^>]*>.*?</lf:{tag}>"))
        .join("|");
        Regex::new(&format!(r"(?ms){sections}|^[ \t]*>[^\n]*(?:\n|$)"))
            .expect("valid embedded reference expression")
    });
    REFERENCES
        .replace_all(message, |caps: &regex::Captures<'_>| {
            render_reference(&caps[0])
        })
        .into_owned()
}

fn gather_wave_docs(repo_root: &Path, wave: Option<&str>) -> Result<Vec<Document>, CoreError> {
    let mut docs = Vec::new();
    let Some(wave) = wave else {
        return Ok(docs);
    };
    let mut directory = PathBuf::from("wave");
    for segment in Path::new(wave).components() {
        directory.push(segment);
        let absolute = repo_root.join(&directory);
        if !absolute.is_dir() {
            continue;
        }
        let mut paths = fs::read_dir(&absolute)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        paths.retain(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"));
        paths.sort_by_key(|path| {
            (
                path.file_name().is_none_or(|name| name != "README.md"),
                path.clone(),
            )
        });
        for path in paths {
            docs.push(Document {
                path: directory
                    .join(path.file_name().expect("directory entry has a name"))
                    .to_string_lossy()
                    .into_owned(),
                content: fs::read_to_string(&path)?,
                source: DocumentSource::Wave,
            });
        }
    }
    Ok(docs)
}

enum ResolvedDocTarget<'a> {
    Local {
        target: &'a str,
    },
    CrossRepo {
        related: &'a RelatedRepoContext,
        target: &'a str,
    },
}

fn gather_doc_targets(
    repo_root: &Path,
    targets: &[String],
    related_repos: &[RelatedRepoContext],
) -> Result<Vec<Document>, CoreError> {
    let mut docs = Vec::new();
    let mut seen = HashSet::new();

    for target in targets {
        let target = target.trim();
        if target.is_empty() {
            continue;
        }

        match resolve_doc_target(target, related_repos) {
            ResolvedDocTarget::Local { target } => {
                gather_local_doc_target(repo_root, target, &mut seen, &mut docs)?;
            }
            ResolvedDocTarget::CrossRepo { related, target } => {
                let mut related_docs = Vec::new();
                let mut related_seen = HashSet::new();
                gather_local_doc_target(
                    &related.path,
                    target,
                    &mut related_seen,
                    &mut related_docs,
                )?;
                for mut doc in related_docs {
                    doc.path = format!("[{}] {}", related.repo_id, doc.path);
                    docs.push(doc);
                }
            }
        }
    }

    Ok(docs)
}

fn gather_local_doc_target(
    repo_root: &Path,
    target: &str,
    seen: &mut HashSet<PathBuf>,
    docs: &mut Vec<Document>,
) -> Result<(), CoreError> {
    let gitignore = build_gitignore(repo_root);

    if contains_glob_chars(target) {
        gather_glob_docs(repo_root, target, &gitignore, seen, docs)?;
        return Ok(());
    }

    let Some(path) = resolve_path(repo_root, target) else {
        return Ok(());
    };

    if path.is_dir() {
        for doc in gather_directory_docs(repo_root, target, &gitignore) {
            let abs_path = repo_root.join(&doc.path);
            let canonical = abs_path.canonicalize().unwrap_or(abs_path);
            if seen.insert(canonical) {
                docs.push(doc);
            }
        }
        return Ok(());
    }

    if path.is_file() {
        push_doc_file(repo_root, &path, &gitignore, seen, docs);
    }

    Ok(())
}

fn gather_glob_docs(
    repo_root: &Path,
    pattern: &str,
    gitignore: &ignore::gitignore::Gitignore,
    seen: &mut HashSet<PathBuf>,
    docs: &mut Vec<Document>,
) -> Result<(), CoreError> {
    let regex = match Regex::new(&glob_to_regex(pattern.trim_start_matches("./"))) {
        Ok(regex) => regex,
        Err(_) => return Ok(()),
    };
    let walker = ignore::WalkBuilder::new(repo_root)
        .hidden(false)
        .standard_filters(true)
        .build();
    let mut entries = Vec::new();

    for entry in walker {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if !path.is_file() || should_exclude(repo_root, path, gitignore) {
            continue;
        }
        let rel_path = path
            .strip_prefix(repo_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        if regex.is_match(&rel_path) {
            entries.push(path.to_path_buf());
        }
    }

    entries.sort();
    for path in entries {
        push_doc_file(repo_root, &path, gitignore, seen, docs);
    }

    Ok(())
}

fn push_doc_file(
    repo_root: &Path,
    path: &Path,
    gitignore: &ignore::gitignore::Gitignore,
    seen: &mut HashSet<PathBuf>,
    docs: &mut Vec<Document>,
) {
    if should_exclude(repo_root, path, gitignore) {
        return;
    }
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !seen.insert(canonical) {
        return;
    }
    let Some(content) = read_text_file(path) else {
        return;
    };
    let rel_path = path
        .strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    docs.push(Document {
        path: rel_path,
        content,
        source: DocumentSource::Docs,
    });
}

/// Parse an explicit docs target for cross-repo syntax (`repo_name:path`).
///
/// Returns `ResolvedDocTarget::CrossRepo` if the target contains `:` and the repo
/// name matches a related repo. Returns `ResolvedDocTarget::Local` otherwise.
fn resolve_doc_target<'a>(
    target: &'a str,
    related_repos: &'a [RelatedRepoContext],
) -> ResolvedDocTarget<'a> {
    if let Some((repo_name, target_path)) = target.split_once(':') {
        if !repo_name.is_empty() {
            let matches: Vec<_> = related_repos
                .iter()
                .filter(|r| r.repo_id.name() == repo_name)
                .collect();
            match matches.len() {
                1 => {
                    // "studio:" means the whole repo; "studio:swift" means a subdirectory.
                    let resolved_target = if target_path.is_empty() {
                        "."
                    } else {
                        target_path
                    };
                    return ResolvedDocTarget::CrossRepo {
                        related: matches[0],
                        target: resolved_target,
                    };
                }
                0 => {
                    warn!(
                        repo_name = repo_name,
                        "no related repo named '{}', treating as local docs target", repo_name
                    );
                }
                _ => {
                    warn!(
                        repo_name = repo_name,
                        "ambiguous: multiple related repos named '{}', treating as local docs target",
                        repo_name
                    );
                }
            }
        }
    }
    ResolvedDocTarget::Local { target }
}

/// Gather .md docs from directory ancestors and descendants.
///
/// For directory "src/api/handlers", collects .md files from:
/// - src/ (e.g., src/README.md)
/// - src/api/ (e.g., src/api/README.md)
/// - src/api/handlers/ (e.g., src/api/handlers/README.md)
/// - src/api/handlers/** (descendants under the directory, recursively)
///
/// Does NOT include repo root docs unless the target is "." and does NOT include
/// sibling directories.
fn gather_directory_docs(
    repo_root: &Path,
    target: &str,
    gitignore: &ignore::gitignore::Gitignore,
) -> Vec<Document> {
    let target_path = Path::new(target);
    let mut ancestors = Vec::new();

    // Include the target directory itself and its ancestors (excluding repo root)
    if !target_path.as_os_str().is_empty() {
        ancestors.push(target_path.to_path_buf());
    }
    let mut current = target_path.to_path_buf();
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
                    path.is_file()
                        && path.extension().map(|ext| ext == "md").unwrap_or(false)
                        && !should_exclude(repo_root, &path, gitignore)
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

            if let Some(content) = read_text_file(&path) {
                seen.insert(rel_path.clone());
                docs.push(Document {
                    path: rel_path,
                    content,
                    source: DocumentSource::Docs,
                });
            }
        }
    }

    // Gather descendants recursively. The `seen` set already contains the docs
    // directory's own .md files from the ancestor walk, so they won't be
    // double-counted.
    let target_abs = repo_root.join(target_path);
    if target_abs.is_dir() {
        let mut descendant_docs = Vec::new();
        gather_directory_descendants(
            &target_abs,
            repo_root,
            gitignore,
            &mut descendant_docs,
            &mut seen,
        );

        // Prefer shallower descendant docs, then stable path ordering.
        descendant_docs.sort_by(|a, b| {
            let depth_a = a.path.matches('/').count();
            let depth_b = b.path.matches('/').count();
            depth_a.cmp(&depth_b).then_with(|| a.path.cmp(&b.path))
        });

        docs.extend(descendant_docs);
    }

    docs
}

fn gather_directory_descendants(
    dir: &Path,
    repo_root: &Path,
    gitignore: &ignore::gitignore::Gitignore,
    docs: &mut Vec<Document>,
    seen: &mut HashSet<String>,
) {
    if should_exclude(repo_root, dir, gitignore) {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    let mut sorted: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    sorted.sort_by_key(|entry| entry.path());

    for entry in sorted {
        let path = entry.path();
        if path.is_dir() {
            gather_directory_descendants(&path, repo_root, gitignore, docs, seen);
            continue;
        }

        if !path.extension().map(|ext| ext == "md").unwrap_or(false) {
            continue;
        }
        if should_exclude(repo_root, &path, gitignore) {
            continue;
        }

        let rel_path = path
            .strip_prefix(repo_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        if seen.contains(&rel_path) {
            continue;
        }

        if let Some(content) = read_text_file(&path) {
            seen.insert(rel_path.clone());
            docs.push(Document {
                path: rel_path,
                content,
                source: DocumentSource::Docs,
            });
        }
    }
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

    gather_md_files_from(dir, dir, docs, source)
}

fn gather_md_files_from(
    root: &Path,
    dir: &Path,
    docs: &mut Vec<Document>,
    source: DocumentSource,
) -> Result<(), CoreError> {
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();

        if path.is_dir() {
            gather_md_files_from(root, &path, docs, source)?;
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            let content = fs::read_to_string(&path).map_err(|error| {
                CoreError::IoError(format!(
                    "cannot read context file {}: {error}",
                    path.display()
                ))
            })?;
            docs.push(Document {
                path: path
                    .strip_prefix(root.parent().unwrap_or(root))
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string(),
                content,
                source,
            });
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

fn gather_changed_file_paths(repo_root: &Path) -> Result<Vec<String>, CoreError> {
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

    let base_branch =
        crate::engine::git::get_default_branch(repo_root).unwrap_or("main".to_string());
    let diff_ref = format!("origin/{}...HEAD", base_branch);
    let committed_files = git_changed_file_names(repo_root, &diff_ref)?;
    if !committed_files.is_empty() {
        return Ok(committed_files);
    }

    git_changed_file_names(repo_root, "HEAD")
}

fn git_changed_file_names(repo_root: &Path, diff_ref: &str) -> Result<Vec<String>, CoreError> {
    let output = Command::new("git")
        .args(["diff", "--name-only", diff_ref])
        .current_dir(repo_root)
        .output()?;
    if !output.status.success() {
        return Ok(Vec::new());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect())
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

fn contains_glob_chars(value: &str) -> bool {
    value.contains('*') || value.contains('?') || value.contains('[')
}

fn glob_to_regex(pattern: &str) -> String {
    let mut regex = String::from("^");
    let mut chars = pattern.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '*' => {
                if chars.peek() == Some(&'*') {
                    let _ = chars.next();
                    regex.push_str(".*");
                } else {
                    regex.push_str("[^/]*");
                }
            }
            '?' => regex.push_str("[^/]"),
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '\\' => {
                regex.push('\\');
                regex.push(ch);
            }
            _ => regex.push(ch),
        }
    }

    regex.push('$');
    regex
}

/// Files that coding agents load natively. All are skipped from lf docs to
/// avoid duplication — whichever agent runs will pick up its own file.
const AGENT_NATIVE_FILES: &[&str] = &["CLAUDE.md", "AGENTS.md"];

/// Keep one delivery of a document source or the builtin operating guidance.
/// Native instructions remain owned by provider discovery. Equal text at
/// different paths remains distinct, including separate memory scopes.
pub fn drop_duplicate_docs(
    components: &mut PromptComponents,
    repo_root: &Path,
) -> Vec<crate::trace::ContextDecision> {
    use crate::trace::{
        ContextAssetKind as Kind, ContextDecision, ContextDecisionKind, ContextScope,
    };

    // Collect canonical paths of all native files (resolves symlinks)
    let canonical_paths: Vec<_> = AGENT_NATIVE_FILES
        .iter()
        .filter_map(|f| fs::canonicalize(repo_root.join(f)).ok())
        .collect();

    let mut decisions = Vec::new();
    let mut seen = HashSet::new();
    for docs in [&mut components.docs, &mut components.diff_files] {
        docs.retain(|doc| {
            let name = Path::new(&doc.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            let doc_path = repo_root.join(&doc.path);
            let source_path = fs::canonicalize(&doc_path).unwrap_or(doc_path);
            let (kind, scope, reason) =
                if AGENT_NATIVE_FILES.contains(&name) || canonical_paths.contains(&source_path) {
                    (Kind::RepoInstructions, ContextScope::Repo,
                     "provider-native discovery owns this instruction file or its symlink target")
                } else if components.operate
                    && name == "LOOPFLOW.md"
                    && doc.content.trim() == crate::engine::builtins::LOOPFLOW_DOC.trim()
                {
                    (
                        Kind::OperatingInstructions,
                        ContextScope::Global,
                        "the system channel already supplies the builtin operating instructions",
                    )
                } else if !seen.insert((source_path, doc.content.clone())) {
                    let (kind, scope) = match doc.source {
                        DocumentSource::Scratch => (Kind::Scratch, ContextScope::Repo),
                        DocumentSource::Wave => (Kind::Document, ContextScope::Wave),
                        DocumentSource::Diff => (Kind::Diff, ContextScope::Repo),
                        _ => (Kind::Document, ContextScope::Repo),
                    };
                    (
                        kind,
                        scope,
                        "this document source is already supplied in the assembled input",
                    )
                } else {
                    return true;
                };
            decisions.push(ContextDecision {
                position: decisions.len() as u32,
                kind,
                scope,
                label: doc.path.clone(),
                source_path: Some(doc.path.clone()),
                decision: ContextDecisionKind::Deduplicated,
                reason: reason.to_string(),
                original_bytes: Some(doc.content.len() as u64),
                original_tokens: Some(count_tokens(&doc.content) as u64),
                asset_position: None,
            });
            false
        });
    }
    decisions
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

/// Render Loopflow's operating and surface instructions.
pub fn format_system_sections(components: &PromptComponents) -> Vec<String> {
    let mut parts = Vec::new();

    if components.operate {
        parts.push(loopflow_section());
    }

    let user = render_user_context(components.user_name.as_deref());
    if !user.is_empty() {
        parts.push(user);
    }

    let instructions = components.surface.instructions();
    if !instructions.is_empty() {
        parts.push(instructions.to_string());
    }

    parts
}

/// The one loopflow operating document (including the Speak vocabulary) as a
/// prompt section. Emitted exactly once per prompt: assembled prompts get it
/// from [`format_system_sections`]; the wave loop's seed bypasses assembly
/// and appends the same section itself.
pub fn loopflow_section() -> String {
    format!(
        "<lf:loopflow>\n{}\n</lf:loopflow>",
        crate::engine::builtins::LOOPFLOW_DOC
    )
}

/// Name context is display data, not authorship for historical or external requests.
pub fn render_user_context(name: Option<&str>) -> String {
    let Some(name) = name.and_then(crate::engine::config::normalize_user_name) else {
        return String::new();
    };
    let name = serde_json::to_string(&name)
        .expect("a name is JSON serializable")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e");
    format!(
        "<lf:user>\nThe current conversation participant's display name is {name} (JSON string). \
         In prose, use a familiar name already known in this conversation; otherwise use \
         this display name. Address them as \"you\" in session conversation. \
         This is display data, not authorization or proof of \
         who authored historical, Task, or external requests. Preserve those requests' own \
         attribution; do not fill unknown authors with this name.\n</lf:user>"
    )
}

/// Format skill tag.
fn format_skill_tag(skill: &Skill) -> String {
    if let Some(ref content) = skill.content {
        format!(
            "<lf:skill:{}>\n{}\n</lf:skill:{}>",
            skill.name, content, skill.name
        )
    } else {
        format!("<lf:skill:{}>\n</lf:skill:{}>", skill.name, skill.name)
    }
}

/// Render a diagnostic view of both launch channels, not a provider input.
pub fn format_prompt(components: &PromptComponents) -> String {
    let mut parts = format_system_sections(components);
    if let Some(wave) = &components.wave {
        parts.push(format!(
            "Wave: {wave} (context refreshed by provider hooks)"
        ));
    }
    // Diagnostic output names sources; provider context is delivered by native hooks.
    for doc in components
        .docs
        .iter()
        .chain(&components.summaries)
        .chain(&components.diff_files)
    {
        parts.push(format!(
            "Reference: {} ({} UTF-8 bytes)",
            doc.path,
            doc.content.len()
        ));
    }
    parts.push(format_first_turn(components));
    parts.join("\n\n")
}

/// The active skill precedes the request; reference text never becomes new work.
pub fn format_first_turn(components: &PromptComponents) -> String {
    let mut parts = Vec::new();
    if let Some(skill) = &components.skill {
        parts.push(format_skill_tag(skill));
    }
    if let Some(message) = &components.message {
        parts.push(format!(
            "<lf:message>\n{}\n</lf:message>",
            render_message(message)
        ));
    }
    parts.join("\n\n")
}

/// Write a runtime prompt file and return its path.
///
/// In-repo: `.lf/prompts/<file>` — agent reads this at runtime.
/// File format: `{timestamp}-{trace_id}-{unique}-{sources}.{skill}.md`, with the
/// `{trace_id}` segment present when `LF_TRACE_ID` is set, joining the prompt
/// to its command trace.
///
/// Ensures `.lf/prompts/` is in the repo's root `.gitignore`.
pub fn write_prompt_log(
    repo_root: &Path,
    prompt: &str,
    skill_name: &str,
    sources: Option<&[String]>,
) -> Result<PathBuf, CoreError> {
    let prompts_dir = repo_root.join(".lf/prompts");
    fs::create_dir_all(&prompts_dir)?;
    ensure_gitignore_entry(repo_root, ".lf/prompts/")?;

    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    // Replace / with . so namespaced skills (e.g., garden/scan) don't create subdirectories.
    let safe_skill = skill_name.replace('/', ".");
    let name_part = match sources {
        Some(sources) if !sources.is_empty() => {
            format!("{}.{}", sources.join("."), safe_skill)
        }
        _ => safe_skill,
    };
    let trace_part = std::env::var(crate::journal::LF_TRACE_ID_ENV)
        .ok()
        .map(|value| value.trim().replace('/', "."))
        .filter(|value| !value.is_empty());
    let prefix = match trace_part {
        Some(trace_id) => format!("{timestamp}-{trace_id}-"),
        None => format!("{timestamp}-"),
    };
    // A later step must not overwrite context retained by an earlier invocation.
    let mut file = tempfile::Builder::new()
        .prefix(&prefix)
        .suffix(&format!("-{name_part}.md"))
        .tempfile_in(&prompts_dir)?;
    file.write_all(prompt.as_bytes())?;
    let (_, path) = file.keep().map_err(|error| error.error)?;

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::flow::Skill;
    use std::path::{Path, PathBuf};

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(".lf/skills")).expect("create skills");
        dir
    }

    #[test]
    fn scratch_check_flags_growth_thoughts_framing_and_quoted_skill_mentions() {
        let warnings = scratch_plan_warnings(
            "Jack selected local kickoff in this conversation.\n\
             No worker launch is part of this\nstep.\n\
             Its current chapter is unavailable in the selected Machine.\n\
             > $kickoff\n",
        );
        assert_eq!(
            warnings.iter().map(|(line, _)| *line).collect::<Vec<_>>(),
            [1, 2, 4, 5]
        );
        assert!(scratch_plan_warnings(
            "Jack Heart accepted the design on 2026-09-30.\n\
             Build Unit 1, then Unit 2. The transport mechanism remains a proposal.\n\
             Jack invoked kickoff while developing the plan.\n"
        )
        .is_empty());
    }

    #[test]
    fn reference_rendering_preserves_live_requests_and_original_components() {
        let source = "> $kickoff\n`$wave/operate` ${HOME} $HOME $(pwd) $5 café";
        let doc = |source_kind| Document {
            path: "evidence.md".into(),
            content: source.into(),
            source: source_kind,
        };
        let components = PromptComponents {
            docs: vec![
                doc(DocumentSource::Scratch),
                doc(DocumentSource::Docs),
                doc(DocumentSource::Wave),
            ],
            diff: Some(source.into()),
            diff_files: vec![doc(DocumentSource::Diff)],
            summaries: vec![doc(DocumentSource::Summary)],
            clipboard: Some(source.into()),
            message: Some(format!(
                "Use $implement.\n<lf:steers>\n{source}\n</lf:steers>\n\
                 <lf:task-workspace>\n{source}\n</lf:task-workspace>\n\
                 > $kickoff\nKeep building."
            )),
            ..Default::default()
        };
        let submitted = format_prompt(&components);
        assert!(!submitted.contains("$kickoff"));
        assert!(!submitted.contains("$wave/operate"));
        assert!(submitted.contains("&#36;kickoff"));
        assert!(submitted.contains("&#36;{HOME} &#36;HOME &#36;(pwd) &#36;5 café"));
        assert!(submitted.contains("Use $implement."));

        assert_eq!(components.docs[0].content, source);
        assert_eq!(components.docs[2].content, source);
        assert_eq!(
            render_reference(&render_reference(source)),
            render_reference(source)
        );
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

    fn render_full_prompt(components: PromptComponents) -> String {
        format_prompt(&components)
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
    fn full_encoding_boundaries_are_exact_prefix_counts() {
        let text =
            "<lf:file path=\"guide.md\">\nContext quality matters.\n</lf:file>\nUnicode: café.";
        let ends = token_byte_ends(text).expect("cl100k tokenizer");
        for (index, end) in ends.into_iter().enumerate() {
            if text.is_char_boundary(end) {
                assert_eq!(count_tokens(&text[..end]), index + 1);
            }
        }
    }

    #[test]
    fn optimized_prefix_counts_match_direct_encoding() {
        let text =
            "<lf:file path=\"guide.md\">\nContext quality matters.\n</lf:file>\nUnicode: café.";
        let ends = text
            .char_indices()
            .map(|(index, character)| index + character.len_utf8())
            .collect::<Vec<_>>();
        let optimized = account_prompt_tokens(text, &ends, &[])
            .expect("cl100k prefix counts")
            .prefixes;
        let direct = ends
            .iter()
            .map(|end| count_tokens(&text[..*end]))
            .collect::<Vec<_>>();
        assert_eq!(optimized, direct);
    }

    #[test]
    fn optimized_isolated_counts_match_direct_encoding() {
        let text = "<tag>Context quality matters.</tag>\nUnicode: café.";
        let ranges = vec![(0, 5), (5, 29), (29, text.len())];
        let optimized = account_prompt_tokens(text, &[], &ranges)
            .expect("cl100k range counts")
            .isolated;
        let direct = ranges
            .iter()
            .map(|(start, end)| count_tokens(&text[*start..*end]))
            .collect::<Vec<_>>();
        assert_eq!(optimized, direct);
    }

    #[test]
    fn format_prompt_basic() {
        let components = PromptComponents {
            surface: Surface::Headless,
            ..Default::default()
        };

        let prompt = render_full_prompt(components);
        assert!(prompt.contains("Run mode is headless"));
        assert!(prompt.contains("headless"));
        assert!(prompt.contains("scratch/questions.md"));
        assert!(prompt.contains("Output is logged, not displayed"));
        assert!(!prompt.contains("<lf:voice>"));
    }

    #[test]
    fn format_prompt_includes_loopflow_when_enabled() {
        let components = PromptComponents {
            operate: true,
            ..Default::default()
        };

        let prompt = render_full_prompt(components);
        assert!(prompt.contains("<lf:loopflow>"));
        assert!(prompt.contains(crate::engine::builtins::LOOPFLOW_DOC.trim()));
        assert!(prompt.contains("</lf:loopflow>"));
    }

    #[test]
    fn format_prompt_omits_loopflow_when_disabled() {
        let components = PromptComponents {
            operate: false,
            ..Default::default()
        };

        let prompt = render_full_prompt(components);
        assert!(!prompt.contains("<lf:loopflow>"));
        assert!(!prompt.contains(crate::engine::builtins::LOOPFLOW_DOC.trim()));
        assert!(!prompt.contains("lf chat"));
    }

    #[test]
    fn assembled_prompts_deliver_procedures_to_the_owning_skill() {
        let repo = init_repo();
        for name in [
            "implement",
            "debug",
            "unbreak",
            "default",
            "repo/operate",
            "wave/operate",
            "task/operate",
            "repo/session",
            "wave/session",
            "task/session",
        ] {
            let components = gather_context(&GatherContextOpts {
                repo_root: repo.path().to_path_buf(),
                skill: Some(name.to_string()),
                operate: true,
                ..Default::default()
            })
            .expect("assemble customer skill in a neutral repository");
            let prompt = render_full_prompt(components);
            assert_eq!(prompt.matches("<lf:loopflow>").count(), 1);
            assert!(prompt.contains("Execute Here First"));
            assert!(prompt.contains("Checks and Flow boundaries"));
            assert!(prompt.contains("Gate owns\nverification once"));
            assert!(prompt.contains("Checks must run headless"));
            assert!(prompt.contains("lf land"));
            assert!(!prompt.contains("scripts/dev-lf"));
            assert!(!prompt.contains("LOO-267"));

            let scope = name.split_once('/').map(|(scope, _)| scope);
            let orchestrates = scope.is_some();
            assert_eq!(prompt.contains("lf task run"), orchestrates, "{name}");
            assert_eq!(
                prompt.contains("lf wave place"),
                scope == Some("repo"),
                "{name}: lf wave place"
            );
            if scope.is_none() {
                assert!(!prompt.contains("doppler run"), "{name}");
            }
            assert!(!prompt.contains("lf skill show"), "{name}");
        }
    }

    #[test]
    fn each_session_carries_its_operate_procedure_once() {
        for scope in ["repo", "wave", "task"] {
            let operate = crate::engine::builtins::get_builtin_skill(&format!("{scope}/operate"))
                .expect("operate skill");
            let procedure = operate
                .splitn(3, "---\n")
                .nth(2)
                .expect("operate skill has a body");
            let session = crate::engine::builtins::get_builtin_skill(&format!("{scope}/session"))
                .expect("session skill");
            assert_eq!(session.matches(procedure).count(), 1, "{scope}");
            assert!(
                session.contains("nothing schedules your next one"),
                "{scope}"
            );
        }
    }

    #[test]
    fn format_prompt_interactive_surfaces_keep_questions_with_present_human() {
        for surface in [Surface::Cli, Surface::Ide, Surface::Mac, Surface::Iphone] {
            let components = PromptComponents {
                surface,
                ..Default::default()
            };

            let prompt = render_full_prompt(components);
            assert!(
                prompt.contains("You are working directly with the user"),
                "surface {surface:?}"
            );
            assert!(
                prompt.contains("never create another session"),
                "surface {surface:?}"
            );
        }
    }

    #[test]
    fn format_prompt_with_wave() {
        let components = PromptComponents {
            wave: Some("rust".to_string()),
            ..Default::default()
        };

        let prompt = render_full_prompt(components);
        assert!(prompt.contains("Wave: rust"));
        assert!(!prompt.contains("<lf:wave"));
    }

    #[test]
    fn native_instructions_are_not_injected() {
        let repo = init_repo();
        write_file(repo.path(), "AGENTS.md", "Repository instructions");
        write_file(repo.path(), "README.md", "Project documentation");
        let mut components = PromptComponents {
            docs: gather_documents(&GatherSpec {
                repo_root: repo.path().to_path_buf(),
                docs: vec!["AGENTS.md".to_string(), "README.md".to_string()],
                ..Default::default()
            })
            .unwrap(),
            ..Default::default()
        };

        let removed = drop_duplicate_docs(&mut components, repo.path());
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].source_path.as_deref(), Some("AGENTS.md"));
        let prompt = render_full_prompt(components);
        assert!(!prompt.contains("Repository instructions"));
        assert!(prompt.contains("Reference: README.md"));
        assert!(!prompt.contains("Project documentation"));
    }

    #[cfg(unix)]
    #[test]
    fn native_instruction_symlink_aliases_are_not_injected() {
        for (target, link) in [
            ("CONTRIBUTING.md", "AGENTS.md"),
            ("AGENTS.md", "CONTRIBUTING.md"),
        ] {
            let repo = init_repo();
            write_file(repo.path(), target, "Repository instructions");
            std::os::unix::fs::symlink(repo.path().join(target), repo.path().join(link)).unwrap();
            let mut components = PromptComponents {
                docs: vec![Document {
                    path: "CONTRIBUTING.md".to_string(),
                    content: "Repository instructions".to_string(),
                    source: DocumentSource::Docs,
                }],
                ..Default::default()
            };

            let removed = drop_duplicate_docs(&mut components, repo.path());
            assert_eq!(removed.len(), 1);
            assert!(!render_full_prompt(components).contains("Repository instructions"));
        }
    }

    #[test]
    fn format_prompt_with_docs() {
        let components = PromptComponents {
            docs: vec![
                Document {
                    path: "README.md".to_string(),
                    content: "# Test Project".to_string(),
                    source: DocumentSource::Docs,
                },
                Document {
                    path: "CONTRIBUTING.md".to_string(),
                    content: "# Style Guide".to_string(),
                    source: DocumentSource::Docs,
                },
            ],
            ..Default::default()
        };

        let prompt = render_full_prompt(components);
        assert!(prompt.contains("Reference: README.md"));
        assert!(prompt.contains("Reference: CONTRIBUTING.md"));
        assert!(!prompt.contains("# Test Project"));
        assert!(!prompt.contains("# Style Guide"));
    }

    #[test]
    fn format_prompt_with_skill() {
        let components = PromptComponents {
            skill: Some(Skill {
                source: None,
                name: "implement".to_string(),
                content: Some("Implement the feature described.".to_string()),
                agent: None,
                default_agent: None,
                action_style: None,
            }),
            ..Default::default()
        };

        let prompt = render_full_prompt(components);
        assert!(prompt.contains("<lf:skill:implement>"));
        assert!(prompt.contains("Implement the feature described."));
        assert!(prompt.contains("</lf:skill:implement>"));
    }

    #[test]
    fn format_prompt_with_skill_no_content() {
        let components = PromptComponents {
            skill: Some(Skill {
                source: None,
                name: "review".to_string(),
                content: None,
                agent: None,
                default_agent: None,
                action_style: None,
            }),
            ..Default::default()
        };

        let prompt = render_full_prompt(components);
        assert!(prompt.contains("<lf:skill:review>"));
        assert!(prompt.contains("</lf:skill:review>"));
    }

    #[test]
    fn format_prompt_default_components_has_headless_surface() {
        let components = PromptComponents::default();
        let prompt = render_full_prompt(components);
        assert!(prompt.contains("Run mode is headless"));
        assert!(prompt.contains("launch an ordinary contribution explicitly"));
        assert!(prompt.contains("explain the failure in ordinary output and stop"));
        assert!(prompt.contains("For reversible ambiguity"));
    }

    #[test]
    fn surface_parser_unknown_defaults_to_headless() {
        let parsed = "unknown_surface"
            .parse::<Surface>()
            .expect("surface parsing is infallible");
        assert_eq!(parsed, Surface::Headless);
    }

    #[test]
    fn surface_parser_preserves_historical_ide() {
        let parsed = "ide"
            .parse::<Surface>()
            .expect("surface parsing is infallible");
        assert_eq!(parsed, Surface::Ide);
    }

    #[test]
    fn chat_surface_speaks_plainly_and_is_distinct_from_headless() {
        assert_eq!(
            "chat".parse::<Surface>().expect("infallible"),
            Surface::Chat
        );
        let chat = Surface::Chat.instructions();
        assert!(chat.contains("live chat channel"));
        assert!(chat.contains("Never narrate your internal machinery"));
        assert_ne!(chat, Surface::Headless.instructions());
        assert_ne!(chat, Surface::Cli.instructions());
    }

    // ==========================================================================
    // directory docs gathering tests
    // ==========================================================================

    #[test]
    fn gather_directory_docs_includes_ancestors_and_descendants() {
        let repo = init_repo();
        write_file(repo.path(), "src/README.md", "# src");
        write_file(repo.path(), "src/api/README.md", "# api");
        write_file(repo.path(), "src/api/handlers/README.md", "# handlers");
        write_file(repo.path(), "src/api/handlers/v1/README.md", "# v1");

        let gitignore = build_gitignore(repo.path());
        let docs = gather_directory_docs(repo.path(), "src/api", &gitignore);
        let paths: Vec<&str> = docs.iter().map(|doc| doc.path.as_str()).collect();

        assert!(paths.contains(&"src/README.md"));
        assert!(paths.contains(&"src/api/README.md"));
        assert!(paths.contains(&"src/api/handlers/README.md"));
        assert!(paths.contains(&"src/api/handlers/v1/README.md"));

        let doc_count = docs
            .iter()
            .filter(|doc| doc.path == "src/api/README.md")
            .count();
        assert_eq!(doc_count, 1);
    }

    #[test]
    fn gather_directory_docs_excludes_sibling_directories() {
        let repo = init_repo();
        write_file(repo.path(), "src/README.md", "# src");
        write_file(repo.path(), "src/api/README.md", "# api");
        write_file(repo.path(), "src/api/handlers/README.md", "# handlers");
        write_file(repo.path(), "src/web/README.md", "# web");

        let gitignore = build_gitignore(repo.path());
        let docs = gather_directory_docs(repo.path(), "src/api", &gitignore);
        let paths: Vec<&str> = docs.iter().map(|doc| doc.path.as_str()).collect();

        assert!(paths.contains(&"src/api/handlers/README.md"));
        assert!(!paths.contains(&"src/web/README.md"));
    }

    #[test]
    fn gather_documents_caps_explicit_docs_at_100() {
        let repo = init_repo();
        write_file(repo.path(), "src/api/README.md", "# api");
        for i in 0..120 {
            write_file(
                repo.path(),
                &format!("src/api/handlers/doc-{i:03}.md"),
                "# handler doc",
            );
        }

        let opts = GatherSpec {
            repo_root: repo.path().to_path_buf(),
            docs: vec!["src/api".to_string()],
            ..Default::default()
        };

        let error = gather_documents(&opts).expect_err("too many explicit docs");
        assert!(error.to_string().contains("--docs resolved to 121 files"));
    }

    #[test]
    fn gather_documents_directory_docs_respects_excludes() {
        let repo = init_repo();
        write_file(repo.path(), ".gitignore", "docs/private/\n");
        write_file(repo.path(), "docs/README.md", "# public");
        write_file(repo.path(), "docs/private/README.md", "# private");
        write_file(repo.path(), "docs/Cargo.lock", "# lock");
        write_file(repo.path(), ".lf/README.md", "# internal");

        let spec = GatherSpec {
            repo_root: repo.path().to_path_buf(),
            docs: vec![".".to_string()],
            ..Default::default()
        };

        let docs = gather_documents(&spec).expect("gather docs");
        let paths: Vec<&str> = docs.iter().map(|doc| doc.path.as_str()).collect();

        assert!(paths.contains(&"docs/README.md"));
        assert!(!paths.contains(&"docs/private/README.md"));
        assert!(!paths.contains(&"docs/Cargo.lock"));
        assert!(!paths.contains(&".lf/README.md"));
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
        write_file(repo.path(), ".lf/skills/debug.md", "# Debug skill");

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
            include_diff_files: true,
            ..Default::default()
        };
        let ctx = gather_context(&opts).expect("gather context");
        let prompt = format_prompt(&ctx);

        assert!(prompt.contains("src/a.rs"));
        assert!(prompt.contains("src/c.rs"));
        assert!(!prompt.contains("src/b.rs"));
    }

    #[test]
    fn context_delivery_keeps_one_document_also_requested_as_a_changed_file() {
        let repo = init_repo();
        write_file(repo.path(), "guide.md", "Keep rollback available.");
        let mut components = gather_context(&GatherContextOpts {
            repo_root: repo.path().to_path_buf(),
            docs: vec!["guide.md".into()],
            files: vec!["guide.md".into()],
            include_diff_files: true,
            ..Default::default()
        })
        .unwrap();
        let decisions = drop_duplicate_docs(&mut components, repo.path());
        let prompt = format_prompt(&components);

        assert_eq!(prompt.matches("Reference: guide.md").count(), 1);
        assert!(decisions.iter().any(|decision| {
            decision.kind == crate::trace::ContextAssetKind::Diff
                && decision.source_path.as_deref() == Some("guide.md")
                && decision.decision == crate::trace::ContextDecisionKind::Deduplicated
        }));
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
            include_diff_files: true,
            ..Default::default()
        };
        let ctx = gather_context(&opts).expect("gather context");
        let has_diff = ctx.diff.is_some();
        let prompt = format_prompt(&ctx);

        assert!(
            !has_diff,
            "files-only context should not include branch diff"
        );
        assert!(!prompt.contains("<lf:diff>"));
        assert!(prompt.contains("src/a.rs"));
        assert!(!prompt.contains("src/unrelated.rs"));
    }

    #[test]
    fn gather_context_with_diff_files_loads_changed_files() {
        let repo = init_git_repo();
        Command::new("git")
            .args(["checkout", "-b", "feature"])
            .current_dir(repo.path())
            .output()
            .expect("git checkout");
        write_file(repo.path(), "src/changed.rs", "mod changed;");
        write_file(repo.path(), "src/unchanged.rs", "mod unchanged;");
        Command::new("git")
            .args(["add", "src/changed.rs"])
            .current_dir(repo.path())
            .output()
            .expect("git add");

        let opts = GatherContextOpts {
            repo_root: repo.path().to_path_buf(),
            include_diff_files: true,
            ..Default::default()
        };
        let ctx = gather_context(&opts).expect("gather context");
        let prompt = format_prompt(&ctx);

        assert!(prompt.contains("src/changed.rs"));
        assert!(!prompt.contains("src/unchanged.rs"));
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
        std::fs::create_dir_all(repo.join(".lf/skills")).expect("create .lf/skills");
        std::fs::write(repo.join(".lf/skills/test.md"), "Test skill content").expect("write skill");

        let opts = GatherContextOpts {
            repo_root: repo.to_path_buf(),
            skill: Some("test".to_string()),
            ..Default::default()
        };

        let result = gather_context(&opts);
        assert!(result.is_ok());
        let components = result.unwrap();
        assert!(components.skill.is_some());
        assert_eq!(components.skill.as_ref().unwrap().name, "test");
    }

    #[test]
    fn gather_context_loads_scratch_without_root_docs() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let repo = temp.path();

        std::fs::write(repo.join("README.md"), "# Project").expect("write readme");
        std::fs::create_dir_all(repo.join("scratch")).expect("create scratch");
        std::fs::write(repo.join("scratch/plan.md"), "# Plan").expect("write plan");

        let opts = GatherContextOpts {
            repo_root: repo.to_path_buf(),
            ..Default::default()
        };

        let result = gather_context(&opts);
        assert!(result.is_ok());
        let components = result.unwrap();
        assert!(!components.docs.is_empty());

        let readme = components.docs.iter().find(|d| d.path.contains("README"));
        assert!(readme.is_none());

        let scratch = components
            .docs
            .iter()
            .find(|d| d.path.contains("scratch/plan.md"));
        assert!(scratch.is_some());
        assert_eq!(
            scratch.expect("scratch doc should be gathered").source,
            DocumentSource::Scratch
        );
    }

    #[test]
    fn gather_context_loads_every_nested_scratch_markdown_file_in_path_order() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let repo = temp.path();
        std::fs::create_dir_all(repo.join("scratch/nested")).expect("create nested scratch");
        std::fs::write(repo.join("scratch/z.md"), "z research").expect("write z research");
        std::fs::write(repo.join("scratch/a.md"), "a design").expect("write a design");
        std::fs::write(repo.join("scratch/nested/b.md"), "b evidence")
            .expect("write nested evidence");
        std::fs::write(repo.join("scratch/raw.log"), "not implicit context")
            .expect("write non-markdown evidence");

        let components = gather_context(&GatherContextOpts {
            repo_root: repo.to_path_buf(),
            ..Default::default()
        })
        .expect("gather scratch context");
        let scratch = components
            .docs
            .iter()
            .filter(|document| document.source == DocumentSource::Scratch)
            .map(|document| (document.path.as_str(), document.content.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(
            scratch,
            [
                ("scratch/a.md", "a design"),
                ("scratch/nested/b.md", "b evidence"),
                ("scratch/z.md", "z research"),
            ]
        );
    }

    #[test]
    fn gather_context_loads_explicit_docs_targets() {
        let repo = init_repo();
        write_file(repo.path(), "README.md", "# Project");
        write_file(repo.path(), "docs/README.md", "# Docs");
        write_file(repo.path(), "docs/nested/README.md", "# Nested");

        let opts = GatherContextOpts {
            repo_root: repo.path().to_path_buf(),
            docs: vec!["README.md".to_string(), "docs".to_string()],
            ..Default::default()
        };

        let components = gather_context(&opts).expect("gather context");
        let paths: Vec<&str> = components
            .docs
            .iter()
            .map(|doc| doc.path.as_str())
            .collect();

        assert!(paths.contains(&"README.md"));
        assert!(paths.contains(&"docs/README.md"));
        assert!(paths.contains(&"docs/nested/README.md"));
    }

    #[test]
    fn gather_context_surface_preserved() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let repo = temp.path();

        let opts = GatherContextOpts {
            repo_root: repo.to_path_buf(),
            surface: Surface::Cli,
            ..Default::default()
        };

        let result = gather_context(&opts);
        assert!(result.is_ok());
        let components = result.unwrap();
        assert_eq!(components.surface, Surface::Cli);
    }

    #[test]
    fn gather_context_wave_preserved() {
        let _lock = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::flow::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        crate::store::sqlite::SqliteStore::open_ephemeral(&home.path().join("loopflow.db"))
            .unwrap();
        let temp = loopflow_test_support::TestRepo::new();
        let repo = temp.path();

        let opts = GatherContextOpts {
            repo_root: repo.to_path_buf(),
            wave: Some("rust-migration".to_string()),
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
        assert!(path.to_string_lossy().contains(".lf/prompts/"));
        assert!(path.to_string_lossy().ends_with("-implement.md"));

        let next = write_prompt_log(repo.path(), "Next step", "implement", None).unwrap();
        assert_ne!(path, next);
        assert_eq!(fs::read_to_string(&path).unwrap(), prompt);
        assert_eq!(fs::read_to_string(&next).unwrap(), "Next step");
    }

    #[test]
    fn write_prompt_log_retains_each_invocations_context() {
        let repo = init_repo();
        let prompts: Vec<_> = (0..16)
            .map(|turn| {
                let text = format!("Context for invocation {turn}");
                let path = write_prompt_log(repo.path(), &text, "work.context", None).unwrap();
                (path, text)
            })
            .collect();

        for (path, text) in prompts {
            assert_eq!(fs::read_to_string(path).unwrap(), text);
        }
    }

    #[test]
    fn write_prompt_log_adds_to_repo_gitignore() {
        let repo = init_repo();
        let gitignore_path = repo.path().join(".gitignore");

        assert!(!gitignore_path.exists());

        write_prompt_log(repo.path(), "test", "skill", None).unwrap();

        assert!(gitignore_path.exists());
        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains(".lf/prompts/"));
    }

    #[test]
    fn write_prompt_log_with_sources() {
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
        fs::write(&gitignore_path, "target/\n.lf/prompts/\n").unwrap();

        write_prompt_log(repo.path(), "test", "skill", None).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();
        // Should not duplicate the entry
        assert_eq!(content, "target/\n.lf/prompts/\n");
    }

    #[test]
    fn write_prompt_log_appends_to_existing_gitignore() {
        let repo = init_repo();
        let gitignore_path = repo.path().join(".gitignore");
        fs::write(&gitignore_path, "target/\nnode_modules/\n").unwrap();

        write_prompt_log(repo.path(), "test", "skill", None).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains("target/"));
        assert!(content.contains("node_modules/"));
        assert!(content.contains(".lf/prompts/"));
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

    // ── Cross-repo context loading ──────────────────────────────────────

    #[test]
    fn gather_documents_cross_repo_docs_include_target_docs_only() {
        let source_repo = init_repo();
        write_file(source_repo.path(), "AGENTS.md", "source instructions");

        let related_repo = tempfile::tempdir().expect("related tempdir");
        std::fs::write(
            related_repo.path().join("AGENTS.md"),
            "related instructions",
        )
        .unwrap();
        std::fs::write(related_repo.path().join("CONTRIBUTING.md"), "related style").unwrap();
        std::fs::create_dir_all(related_repo.path().join("src")).unwrap();
        std::fs::write(related_repo.path().join("src/README.md"), "src area doc").unwrap();

        let related = RelatedRepoContext {
            repo_id: RepoId::parse("acme/widgets").unwrap(),
            path: related_repo.path().to_path_buf(),
        };

        let spec = GatherSpec {
            repo_root: source_repo.path().to_path_buf(),
            docs: vec!["widgets:src".to_string()],
            related_repos: vec![related],
            ..Default::default()
        };
        let docs = gather_documents(&spec).unwrap();

        // Source-repo scratch/root docs do not auto-load root markdown.
        assert!(docs
            .iter()
            .all(|d| d.path != "AGENTS.md" && d.content != "source instructions"));

        // Related repo root docs are not loaded for a directory docs target.
        assert!(!docs
            .iter()
            .any(|d| d.path == "[acme/widgets] AGENTS.md" && d.content == "related instructions"));

        // Related repo docs target.
        assert!(docs
            .iter()
            .any(|d| d.path.contains("[acme/widgets]") && d.content == "src area doc"));
    }

    #[test]
    fn gather_documents_related_repo_docs_not_loaded_without_explicit_target() {
        let source_repo = init_repo();
        write_file(source_repo.path(), "AGENTS.md", "source instructions");

        let related_repo = tempfile::tempdir().expect("related tempdir");
        std::fs::write(
            related_repo.path().join("AGENTS.md"),
            "related instructions",
        )
        .unwrap();

        let related = RelatedRepoContext {
            repo_id: RepoId::parse("acme/widgets").unwrap(),
            path: related_repo.path().to_path_buf(),
        };

        let spec = GatherSpec {
            repo_root: source_repo.path().to_path_buf(),
            related_repos: vec![related],
            ..Default::default()
        };
        let docs = gather_documents(&spec).unwrap();

        // Source-repo root docs are not ambient.
        assert!(!docs
            .iter()
            .any(|d| d.path == "AGENTS.md" && d.content == "source instructions"));

        // Related repo docs are not loaded without an explicit docs target for that repo.
        assert!(!docs.iter().any(|d| d.path.contains("[acme/widgets]")));
    }

    #[test]
    fn gather_documents_no_related_repos_loads_no_root_docs() {
        let repo = init_repo();
        write_file(repo.path(), "README.md", "hello");

        let spec = GatherSpec {
            repo_root: repo.path().to_path_buf(),
            ..Default::default()
        };
        let docs = gather_documents(&spec).unwrap();
        let explicit_docs: Vec<_> = docs
            .iter()
            .filter(|d| d.source == DocumentSource::Docs)
            .collect();
        assert!(!explicit_docs.iter().any(|d| d.path == "README.md"));
        // No prefixed docs
        assert!(!explicit_docs.iter().any(|d| d.path.starts_with('[')));
    }

    #[test]
    fn gather_documents_related_repo_missing_from_disk() {
        let repo = init_repo();
        write_file(repo.path(), "README.md", "hello");

        let related = RelatedRepoContext {
            repo_id: RepoId::parse("acme/gone").unwrap(),
            path: PathBuf::from("/nonexistent/path/that/does/not/exist"),
        };

        let spec = GatherSpec {
            repo_root: repo.path().to_path_buf(),
            docs: vec!["gone:src".to_string()],
            related_repos: vec![related],
            ..Default::default()
        };
        // Should not error, just warn and skip
        let docs = gather_documents(&spec).unwrap();
        assert!(!docs.iter().any(|d| d.path.contains("[acme/gone]")));
    }

    #[test]
    fn gather_documents_cross_repo_docs() {
        let repo = init_repo();

        let related_repo = tempfile::tempdir().expect("related tempdir");
        std::fs::write(related_repo.path().join("AGENTS.md"), "studio instructions").unwrap();
        std::fs::create_dir_all(related_repo.path().join("swift")).unwrap();
        std::fs::write(related_repo.path().join("swift/README.md"), "swift docs").unwrap();

        let related = RelatedRepoContext {
            repo_id: RepoId::parse("acme/studio").unwrap(),
            path: related_repo.path().to_path_buf(),
        };

        let spec = GatherSpec {
            repo_root: repo.path().to_path_buf(),
            docs: vec!["studio:swift".to_string()],
            related_repos: vec![related],
            ..Default::default()
        };
        let docs = gather_documents(&spec).unwrap();

        assert!(
            docs.iter()
                .any(|d| d.path.contains("[acme/studio]") && d.content == "swift docs"),
            "expected cross-repo docs, got: {:?}",
            docs.iter().map(|d| &d.path).collect::<Vec<_>>()
        );
    }

    #[test]
    fn gather_documents_bare_repo_name_loads_whole_repo() {
        let repo = init_repo();

        let related_repo = tempfile::tempdir().expect("related tempdir");
        std::fs::write(related_repo.path().join("AGENTS.md"), "studio instructions").unwrap();
        std::fs::write(related_repo.path().join("README.md"), "studio readme").unwrap();

        let related = RelatedRepoContext {
            repo_id: RepoId::parse("acme/studio").unwrap(),
            path: related_repo.path().to_path_buf(),
        };

        let spec = GatherSpec {
            repo_root: repo.path().to_path_buf(),
            docs: vec!["studio:".to_string()],
            related_repos: vec![related],
            ..Default::default()
        };
        let docs = gather_documents(&spec).unwrap();

        // Top-level docs loaded (README.md is a descendant of ".")
        assert!(docs
            .iter()
            .any(|d| d.path.contains("[acme/studio]") && d.content == "studio readme"));
    }

    #[test]
    fn gather_documents_local_docs_directory() {
        let repo = init_repo();
        std::fs::create_dir_all(repo.path().join("docs")).unwrap();
        write_file(repo.path(), "docs/README.md", "local docs");

        let spec = GatherSpec {
            repo_root: repo.path().to_path_buf(),
            docs: vec!["docs".to_string()],
            ..Default::default()
        };
        let docs = gather_documents(&spec).unwrap();
        assert!(docs.iter().any(|d| d.content == "local docs"));
        // No prefixed docs
        assert!(!docs.iter().any(|d| d.path.starts_with('[')));
    }

    #[test]
    fn resolve_doc_target_local_without_colon() {
        let result = resolve_doc_target("docs", &[]);
        assert!(matches!(
            result,
            ResolvedDocTarget::Local { target: "docs" }
        ));
    }

    #[test]
    fn resolve_doc_target_cross_repo_match() {
        let related = vec![RelatedRepoContext {
            repo_id: RepoId::parse("acme/studio").unwrap(),
            path: PathBuf::from("/repos/studio"),
        }];
        let result = resolve_doc_target("studio:swift", &related);
        match result {
            ResolvedDocTarget::CrossRepo { related: r, target } => {
                assert_eq!(r.repo_id.name(), "studio");
                assert_eq!(target, "swift");
            }
            _ => panic!("expected CrossRepo"),
        }
    }

    #[test]
    fn resolve_doc_target_unknown_repo_falls_back_to_local() {
        let related = vec![RelatedRepoContext {
            repo_id: RepoId::parse("acme/studio").unwrap(),
            path: PathBuf::from("/repos/studio"),
        }];
        let result = resolve_doc_target("unknown:swift", &related);
        assert!(matches!(
            result,
            ResolvedDocTarget::Local {
                target: "unknown:swift"
            }
        ));
    }

    #[test]
    fn resolve_doc_target_ambiguous_repo_falls_back_to_local() {
        let related = vec![
            RelatedRepoContext {
                repo_id: RepoId::parse("acme/studio").unwrap(),
                path: PathBuf::from("/repos/studio1"),
            },
            RelatedRepoContext {
                repo_id: RepoId::parse("other/studio").unwrap(),
                path: PathBuf::from("/repos/studio2"),
            },
        ];
        let result = resolve_doc_target("studio:swift", &related);
        assert!(matches!(result, ResolvedDocTarget::Local { .. }));
    }

    #[test]
    fn resolve_doc_target_empty_repo_name_treated_as_local() {
        let result = resolve_doc_target(":swift", &[]);
        assert!(matches!(
            result,
            ResolvedDocTarget::Local { target: ":swift" }
        ));
    }

    #[test]
    fn resolve_doc_target_bare_repo_name_resolves_to_root() {
        let related = vec![RelatedRepoContext {
            repo_id: RepoId::parse("acme/studio").unwrap(),
            path: PathBuf::from("/repos/studio"),
        }];
        let result = resolve_doc_target("studio:", &related);
        match result {
            ResolvedDocTarget::CrossRepo { related: r, target } => {
                assert_eq!(r.repo_id.name(), "studio");
                assert_eq!(target, ".");
            }
            _ => panic!("expected CrossRepo, got Local"),
        }
    }
}
