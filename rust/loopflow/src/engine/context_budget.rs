//! Bound implicit launch context while retaining the complete source on disk.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::engine::error::CoreError;
use crate::engine::prompt::{count_tokens, Document, DocumentSource, PromptComponents};
use crate::trace::{ContextAssetKind, ContextDecision, ContextDecisionKind, ContextScope};

pub(crate) const MEMORY_TOKENS: usize = 8_000;
pub(crate) const SCRATCH_TOKENS: usize = 16_000;
pub(crate) const GOAL_TOKENS: usize = 16_000;
pub(crate) const INPUT_TOKENS: usize = 64_000;
pub(crate) const INPUT_BYTES: usize = 512 * 1024;

fn bound_source(
    text: &str,
    tokens: usize,
    bytes: usize,
    repo_root: &Path,
) -> Result<String, CoreError> {
    if text.len() <= bytes && count_tokens(text) <= tokens {
        return Ok(text.to_string());
    }
    let source = preserve_source(text, repo_root)?;
    let notice = format!(
        "\n\n[Context budget: excerpt only. Full text: {}. Read the relevant omitted sections before acting; this excerpt is not the complete instruction.]\n\n",
        source.display(),
    );
    // Include both ends: definitions usually lead, recent direction usually trails.
    let mut keep = bytes.saturating_sub(notice.len()).min(text.len()) / 2;
    loop {
        let mut head = keep;
        while !text.is_char_boundary(head) {
            head -= 1;
        }
        let mut tail = text.len() - keep;
        while !text.is_char_boundary(tail) {
            tail += 1;
        }
        let result = format!("{}{notice}{}", &text[..head], &text[tail..]);
        if keep == 0 || (result.len() <= bytes && count_tokens(&result) <= tokens) {
            return Ok(result);
        }
        keep = keep * 3 / 4;
    }
}

pub(crate) fn bound_context(components: &mut PromptComponents) -> Result<(), CoreError> {
    let repo_root = Path::new(&components.repo_root);
    let memories: Vec<_> = components
        .docs
        .iter_mut()
        .filter(|doc| doc.source == DocumentSource::Wave && doc.path.ends_with("/MEMORY.md"))
        .collect();
    let memory_tokens = memories
        .iter()
        .map(|doc| count_tokens(&doc.content))
        .sum::<usize>()
        .max(MEMORY_TOKENS);
    let memory_bytes = memories
        .iter()
        .map(|doc| doc.content.len())
        .sum::<usize>()
        .max(64 * 1024);
    // Share the collection budget in proportion to source size, preserving
    // ancestor order and each document's attribution and complete source.
    for memory in memories {
        let tokens = count_tokens(&memory.content) * MEMORY_TOKENS / memory_tokens;
        let bytes = memory.content.len() * (64 * 1024) / memory_bytes;
        let bounded = bound_source(&memory.content, tokens, bytes, repo_root)?;
        record_reduction(
            &mut components.budget_decisions,
            &memory.content,
            &bounded,
            ContextAssetKind::Memory,
            ContextScope::Wave,
            &memory.path,
        );
        memory.content = bounded;
    }
    let scratch_docs: Vec<_> = components
        .docs
        .iter()
        .filter(|doc| doc.source == DocumentSource::Scratch)
        .collect();
    let scratch_index = scratch_docs
        .iter()
        .map(|doc| doc.path.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let scratch = scratch_docs
        .iter()
        .map(|doc| format!("# {}\n\n{}", doc.path, doc.content))
        .collect::<Vec<_>>()
        .join("\n\n");
    let scratch = format!("{scratch_index}\n\n{scratch}");
    // Bound the collection, including its index: per-file notices alone
    // can exhaust a budget when a checkout has hundreds of scratch files.
    let bounded = bound_source(&scratch, SCRATCH_TOKENS, 128 * 1024, repo_root)?;
    if bounded != scratch {
        record_reduction(
            &mut components.budget_decisions,
            &scratch,
            &bounded,
            ContextAssetKind::Scratch,
            ContextScope::Repo,
            "scratch/",
        );
        let first = components
            .docs
            .iter()
            .position(|doc| doc.source == DocumentSource::Scratch)
            .expect("nonempty scratch exceeded its budget");
        components
            .docs
            .retain(|doc| doc.source != DocumentSource::Scratch);
        components.docs.insert(
            first,
            Document {
                path: "scratch/".into(),
                content: bounded,
                source: DocumentSource::Scratch,
            },
        );
    }
    if let Some(message) = &mut components.message {
        let bounded = bound_message(message, repo_root)?;
        record_reduction(
            &mut components.budget_decisions,
            message,
            &bounded,
            ContextAssetKind::UserMessage,
            ContextScope::User,
            "launch message",
        );
        *message = bounded;
    }
    Ok(())
}

fn record_reduction(
    decisions: &mut Vec<ContextDecision>,
    original: &str,
    bounded: &str,
    kind: ContextAssetKind,
    scope: ContextScope,
    source: &str,
) {
    if original == bounded {
        return;
    }
    decisions.push(ContextDecision {
        position: decisions.len() as u32,
        kind,
        scope,
        label: source.to_string(),
        source_path: (kind != ContextAssetKind::UserMessage).then(|| source.to_string()),
        decision: ContextDecisionKind::Truncated,
        reason: "launch budget; excerpt names the complete source".into(),
        original_bytes: Some(original.len() as u64),
        original_tokens: Some(count_tokens(original) as u64),
        asset_position: None,
    });
}

pub(crate) fn bound_message(message: &str, repo_root: &Path) -> Result<String, CoreError> {
    bound_source(message, GOAL_TOKENS, 128 * 1024, repo_root)
}

fn preserve_source(text: &str, repo_root: &Path) -> Result<PathBuf, CoreError> {
    let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
    let path = repo_root
        .join(".lf/tmp/context")
        .join(format!("{digest}.md"));
    std::fs::create_dir_all(path.parent().expect("context file has a parent"))?;
    std::fs::write(&path, text.as_bytes())?;
    Ok(path)
}

pub(crate) fn check_input(system: &str, task: &str) -> Result<(), CoreError> {
    let bytes = system.len() + task.len();
    let tokens = count_tokens(system) + count_tokens(task);
    if bytes > INPUT_BYTES || tokens > INPUT_TOKENS {
        return Err(CoreError::ExecutionFailed(format!(
            "launch context exceeds the input budget: {tokens}/{INPUT_TOKENS} tokens, \
             {bytes}/{INPUT_BYTES} bytes. Reduce explicit docs, skill instructions, \
             clipboard or diff context; full memory, scratch and goal sources remain on disk"
        )));
    }
    Ok(())
}
