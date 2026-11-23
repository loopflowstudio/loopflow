use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{sinks::UTF8, SearcherBuilder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::checkout_file::{open_file, parent_directory};
use super::{file_context, file_store, task_error, TaskFileInventory, MAX_FILE_BYTES};
use crate::ops::OpsResult;

pub(super) const MAX_HITS: usize = 100;
const EXCERPT_BYTES: usize = 240;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskGrepHit {
    pub path: String,
    pub line: u64,
    /// Half-open UTF-8 byte offsets in the source line at this revision.
    pub start: usize,
    pub end: usize,
    pub revision: String,
    /// Byte offset of the excerpt in the same source line.
    pub excerpt_start: usize,
    pub excerpt: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskGrepResults {
    pub hits: Vec<TaskGrepHit>,
    pub truncated: bool,
}

pub fn task_grep(
    issue: &str,
    query: &str,
    whole_word: bool,
    cancelled: &dyn Fn() -> bool,
) -> OpsResult<TaskGrepResults> {
    let checkout = file_context(&file_store()?, issue)?;
    let inventory = crate::observe::checkout_files(&checkout.worktree)?;
    search(&checkout.worktree, &inventory, query, whole_word, cancelled)
}

fn search(
    root: &Path,
    inventory: &TaskFileInventory,
    query: &str,
    whole_word: bool,
    cancelled: &dyn Fn() -> bool,
) -> OpsResult<TaskGrepResults> {
    let mut result = TaskGrepResults {
        hits: Vec::new(),
        truncated: false,
    };
    if query.is_empty() {
        return Ok(result);
    }
    if query.contains(['\n', '\r']) {
        return Err(task_error("Search for text on a single line."));
    }
    let matcher = RegexMatcherBuilder::new()
        .fixed_strings(true)
        .word(whole_word)
        .build(query)
        .map_err(task_error)?;
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .bom_sniffing(false)
        .build();
    result.truncated = scan_checkout(
        root,
        inventory.paths.iter().map(String::as_str),
        cancelled,
        |path, source| {
            let mut revision = None;
            let mut omitted = false;
            searcher.search_slice(
                &matcher,
                source.as_bytes(),
                UTF8(|line, text| {
                    let text = text.strip_suffix('\n').unwrap_or(text);
                    let text = text.strip_suffix('\r').unwrap_or(text);
                    matcher
                        .find_iter(text.as_bytes(), |span| {
                            if result.hits.len() == MAX_HITS {
                                omitted = true;
                                return false;
                            }
                            let start = span.start();
                            let excerpt_start = text.ceil_char_boundary(start.saturating_sub(60));
                            let excerpt_end = text.floor_char_boundary(
                                (excerpt_start + EXCERPT_BYTES).min(text.len()),
                            );
                            let revision = revision.get_or_insert_with(|| {
                                hex::encode(Sha256::digest(source.as_bytes()))
                            });
                            result.hits.push(TaskGrepHit {
                                path: path.into(),
                                line,
                                start,
                                end: span.end(),
                                revision: revision.clone(),
                                excerpt_start,
                                excerpt: text[excerpt_start..excerpt_end].into(),
                            });
                            true
                        })
                        .expect("RegexMatcher cannot fail while matching");
                    Ok(!omitted)
                }),
            )?;
            Ok(!omitted)
        },
    )? || inventory.truncated;
    Ok(result)
}

/// Returns true when the visitor stopped before consuming every match.
pub(super) fn scan_checkout<'a>(
    root: &Path,
    paths: impl Iterator<Item = &'a str>,
    cancelled: &dyn Fn() -> bool,
    mut visit: impl FnMut(&str, &str) -> OpsResult<bool>,
) -> OpsResult<bool> {
    let root = root.canonicalize()?;
    let checkout = File::open(&root)?;
    // Scan one bounded buffer at a time. Retaining every file as an observed
    // Bytes fact would keep rereading the checkout after the picker closes.
    let mut bytes = Vec::new();
    for path in paths {
        if cancelled() {
            return Err(task_error("Search cancelled"));
        }
        let source = match read_search_file(&root, &checkout, path, &mut bytes) {
            Ok(None) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(task_error(format!("Cannot search {path:?}: {error}"))),
            Ok(Some(source)) => source,
        };
        if !visit(path, source)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn read_search_file<'a>(
    root: &Path,
    checkout: &File,
    path: &str,
    bytes: &'a mut Vec<u8>,
) -> io::Result<Option<&'a str>> {
    // Inventory entries can be deleted or retargeted between keystrokes.
    let target = root.join(path).canonicalize()?;
    let Ok(relative) = target.strip_prefix(root) else {
        return Ok(None);
    };
    if relative.as_os_str().is_empty() {
        return Ok(None);
    }
    let (parent, name) = parent_directory(checkout, relative)?;
    let file = open_file(&parent, &name)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES as u64 {
        return Ok(None);
    }
    bytes.clear();
    file.take(MAX_FILE_BYTES as u64 + 1).read_to_end(bytes)?;
    if bytes.len() > MAX_FILE_BYTES || bytes.contains(&0) {
        return Ok(None);
    }
    Ok(std::str::from_utf8(bytes).ok())
}

#[cfg(test)]
mod tests {
    use super::{search, TaskFileInventory, EXCERPT_BYTES, MAX_FILE_BYTES};
    use std::{fs, os::unix::fs::symlink};

    #[test]
    fn literal_search_counts_occurrences_and_skips_binary_large_and_retargeted_files() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        fs::write(outside.path(), "a.b").unwrap();
        symlink(outside.path(), root.path().join("link")).unwrap();
        fs::write(
            root.path().join("large"),
            "a.b".repeat(MAX_FILE_BYTES / 3 + 1),
        )
        .unwrap();
        fs::write(root.path().join("binary"), b"a.b\0").unwrap();
        fs::write(root.path().join("invalid"), b"a.b\xff").unwrap();
        let file = root.path().join("text");
        fs::write(
            &file,
            format!("{}\r\n😀 {}axb", "x".repeat(500), "a.b ".repeat(100)),
        )
        .unwrap();
        let inventory = TaskFileInventory {
            paths: ["binary", "gone", "invalid", "large", "link", "text"]
                .map(String::from)
                .into(),
            scratch: vec![],
            latest_scratch_page: None,
            truncated: false,
        };
        let exact = search(root.path(), &inventory, "a.b", false, &|| false).unwrap();
        assert_eq!(exact.hits.len(), 100);
        assert!(!exact.truncated);
        let first = &exact.hits[0];
        assert_eq!(
            (first.path.as_str(), first.line, first.start, first.end),
            ("text", 2, 5, 8)
        );
        let last = exact.hits.last().unwrap();
        assert!(last.excerpt_start > 0 && last.excerpt.len() <= EXCERPT_BYTES);
        for hit in &exact.hits {
            let local = hit.start - hit.excerpt_start..hit.end - hit.excerpt_start;
            assert_eq!(&hit.excerpt[local], "a.b");
        }
        fs::write(file, "a.b ".repeat(101)).unwrap();
        let overflow = search(root.path(), &inventory, "a.b", true, &|| false).unwrap();
        assert_eq!(overflow.hits.len(), 100);
        assert!(overflow.truncated);
        assert_ne!(overflow.hits[0].revision, first.revision);
        let reads = std::cell::Cell::new(0);
        let stopped = search(root.path(), &inventory, "absent", false, &|| {
            reads.set(reads.get() + 1);
            reads.get() == 2
        });
        assert_eq!(stopped.unwrap_err().to_string(), "Search cancelled");
    }
}
