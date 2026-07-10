//! `lf tokens` — how big is this codebase, in the units a model pays for.
//!
//! Lines are what a human counts; tokens are what a run costs. They disagree
//! wildly — a minified file or a lockfile is cheap in lines and ruinous in
//! tokens — and the context project is about the second number, so that is the
//! one this measures.
//!
//! Tokens come from the same `count_tokens` (tiktoken `cl100k_base`) the prompt
//! assembler uses to budget context, so the tree here and the context budget
//! there are speaking about the same quantity.
//!
//! Only tracked files are counted: `git ls-files` already honours `.gitignore`,
//! so `target/`, `.build/`, and `node_modules/` never enter the total. A file
//! that is not valid UTF-8 is a binary and is skipped rather than estimated.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::engine::prompt::count_tokens;
use crate::journal::open_ledger;
use crate::lf::output::Colors;
use crate::lfdb::sqlite::SqliteStore;

const NAME_WIDTH: usize = 44;
const NUM_WIDTH: usize = 12;
const MAX_DEPTH: usize = 3;

/// One directory or file in the codebase tree. Wire type: every field required.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodeNode {
    /// Repo-relative path. Empty at the root.
    pub path: String,
    pub name: String,
    pub lines: usize,
    pub bytes: u64,
    pub tokens: usize,
    pub children: Vec<CodeNode>,
}

pub fn run(json: bool, days: Option<u32>) -> Result<()> {
    let root = repo_root()?;

    if let Some(days) = days {
        let history = history(&root, days)?;
        if json {
            println!("{}", serde_json::to_string(&history)?);
        } else {
            print_history(&history);
        }
        return Ok(());
    }

    let files = tracked_files(&root)?;
    let tree = build_tree(&root, &files);
    if json {
        println!("{}", serde_json::to_string(&tree)?);
        return Ok(());
    }
    print_tree(&tree);
    Ok(())
}

/// How big the codebase was, per top-level directory, on each day it changed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodeSnapshot {
    /// `YYYY-MM-DD`, the day of the sampled commit.
    pub date: String,
    pub commit: String,
    pub lines: usize,
    pub tokens: usize,
    pub slices: Vec<CodeSlice>,
}

/// One file extension's weight in a snapshot. `ext` is lowercase and carries no
/// dot; a file with no extension is `(none)`, and the long tail is `other`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodeSlice {
    pub ext: String,
    pub lines: usize,
    pub tokens: usize,
}

/// Beyond this the legend is longer than the chart. The tail collapses to
/// `other` — chosen once across the whole window, so a series never appears in
/// one snapshot and vanishes from the next.
const MAX_EXTENSIONS: usize = 8;
const OTHER: &str = "other";
const NO_EXTENSION: &str = "(none)";

/// `src/main.rs` -> `rs`. A dotfile is not an extension: `.gitignore` has none.
fn extension_of(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    match file.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => ext.to_ascii_lowercase(),
        _ => NO_EXTENSION.to_string(),
    }
}

/// The extensions worth their own series, by total tokens across the window.
fn dominant_extensions(totals: &BTreeMap<String, usize>) -> Vec<String> {
    let mut ranked: Vec<_> = totals.iter().collect();
    ranked.sort_by_key(|(ext, tokens)| (std::cmp::Reverse(**tokens), (*ext).clone()));
    ranked
        .into_iter()
        .take(MAX_EXTENSIONS)
        .map(|(ext, _)| ext.clone())
        .collect()
}

/// One commit per day — the last one that day — over the window.
fn daily_commits(root: &Path, days: u32) -> Result<Vec<(String, String)>> {
    let output = Command::new("git")
        .args([
            "log",
            &format!("--since={days} days ago"),
            "--date=format:%Y-%m-%d",
            "--format=%H %cd",
        ])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(anyhow!("git log failed"));
    }

    // `git log` is newest-first, so the first row for a day is that day's last
    // commit — the state the codebase ended the day in.
    let mut by_day: BTreeMap<String, String> = BTreeMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some((sha, date)) = line.split_once(' ') else {
            continue;
        };
        by_day
            .entry(date.to_string())
            .or_insert_with(|| sha.to_string());
    }
    Ok(by_day.into_iter().collect())
}

/// Every blob in a commit's tree, as (sha, path). No checkout, no working tree.
fn commit_blobs(root: &Path, commit: &str) -> Result<Vec<(String, String)>> {
    let output = Command::new("git")
        .args(["ls-tree", "-r", "--format=%(objectname) %(path)", commit])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(anyhow!("git ls-tree failed for {commit}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(sha, path)| (sha.to_string(), path.to_string()))
        .collect())
}

/// Measure a blob, memoized by its sha. Blob content is immutable, so the cache
/// never goes stale and a year of history tokenizes each file version once.
///
/// Two layers: a process-local map, because the same blob appears in every
/// snapshot it survived (a year of daily commits asks about the same unchanged
/// file ninety times), and the sqlite table, because the answer outlives the
/// process.
fn blob_weight(
    root: &Path,
    store: &SqliteStore,
    memo: &mut HashMap<String, Option<(usize, usize)>>,
    sha: &str,
) -> Option<(usize, usize)> {
    if let Some(cached) = memo.get(sha) {
        return *cached;
    }
    let weight = measure_blob(root, store, sha);
    memo.insert(sha.to_string(), weight);
    weight
}

fn measure_blob(root: &Path, store: &SqliteStore, sha: &str) -> Option<(usize, usize)> {
    if let Ok(Some((lines, _bytes, tokens))) = store.blob_tokens(sha) {
        return Some((lines.max(0) as usize, tokens.max(0) as usize));
    }
    let output = Command::new("git")
        .args(["cat-file", "blob", sha])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    // Binaries are skipped, not estimated: a token count over bytes that are not
    // text is a number with no meaning.
    let text = String::from_utf8(output.stdout).ok()?;
    let lines = text.lines().count();
    let tokens = count_tokens(&text);
    let _ = store.put_blob_tokens(sha, lines as i64, text.len() as i64, tokens as i64);
    Some((lines, tokens))
}

/// One day's measurement, before the long tail of extensions is folded.
struct RawSnapshot {
    date: String,
    commit: String,
    lines: usize,
    tokens: usize,
    /// extension -> (lines, tokens)
    by_ext: BTreeMap<String, (usize, usize)>,
}

fn history(root: &Path, days: u32) -> Result<Vec<CodeSnapshot>> {
    let store = open_ledger()?;
    let mut memo: HashMap<String, Option<(usize, usize)>> = HashMap::new();

    // Measure every day first, then decide which extensions get their own
    // series. Deciding per snapshot would make a series flicker in and out as a
    // language crosses the threshold on a given day.
    let mut raw: Vec<RawSnapshot> = Vec::new();
    let mut window_totals: BTreeMap<String, usize> = BTreeMap::new();

    for (date, commit) in daily_commits(root, days)? {
        let mut by_ext: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let (mut total_lines, mut total_tokens) = (0usize, 0usize);

        for (sha, path) in commit_blobs(root, &commit)? {
            let Some((lines, tokens)) = blob_weight(root, &store, &mut memo, &sha) else {
                continue;
            };
            total_lines += lines;
            total_tokens += tokens;

            let entry = by_ext.entry(extension_of(&path)).or_insert((0, 0));
            entry.0 += lines;
            entry.1 += tokens;
        }

        for (ext, (_, tokens)) in &by_ext {
            *window_totals.entry(ext.clone()).or_insert(0) += tokens;
        }
        raw.push(RawSnapshot {
            date,
            commit,
            lines: total_lines,
            tokens: total_tokens,
            by_ext,
        });
    }

    let dominant = dominant_extensions(&window_totals);
    let snapshots = raw
        .into_iter()
        .map(
            |RawSnapshot {
                 date,
                 commit,
                 lines,
                 tokens,
                 by_ext,
             }| {
                let mut folded: BTreeMap<String, CodeSlice> = BTreeMap::new();
                for (ext, (ext_lines, ext_tokens)) in by_ext {
                    let key = if dominant.contains(&ext) {
                        ext
                    } else {
                        OTHER.to_string()
                    };
                    let slice = folded.entry(key.clone()).or_insert(CodeSlice {
                        ext: key,
                        lines: 0,
                        tokens: 0,
                    });
                    slice.lines += ext_lines;
                    slice.tokens += ext_tokens;
                }
                let mut slices: Vec<_> = folded.into_values().collect();
                slices.sort_by_key(|slice| std::cmp::Reverse(slice.tokens));
                CodeSnapshot {
                    date,
                    commit,
                    lines,
                    tokens,
                    slices,
                }
            },
        )
        .collect();
    Ok(snapshots)
}

fn print_history(snapshots: &[CodeSnapshot]) {
    let colors = Colors::default();
    println!(
        "{bold}{date:<12}  {lines:>num_w$}  {tokens:>num_w$}  {delta:>num_w$}{reset}",
        bold = colors.bold,
        reset = colors.reset,
        date = "DATE",
        lines = "LINES",
        tokens = "TOKENS",
        delta = "Δ TOKENS",
        num_w = NUM_WIDTH,
    );
    let mut previous: Option<usize> = None;
    for snapshot in snapshots {
        let delta = match previous {
            Some(previous) => {
                let change = snapshot.tokens as i64 - previous as i64;
                format!("{change:+}")
            }
            None => "—".to_string(),
        };
        println!(
            "{date:<12}  {lines:>num_w$}  {tokens:>num_w$}  {delta:>num_w$}",
            date = snapshot.date,
            lines = format_int(snapshot.lines),
            tokens = format_int(snapshot.tokens),
            delta = delta,
            num_w = NUM_WIDTH,
        );
        previous = Some(snapshot.tokens);
    }
}

fn repo_root() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if !output.status.success() {
        return Err(anyhow!("not inside a git repository"));
    }
    Ok(PathBuf::from(String::from_utf8(output.stdout)?.trim()))
}

/// Tracked files only — `git ls-files` honours `.gitignore` for free.
fn tracked_files(root: &Path) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(anyhow!("git ls-files failed in {}", root.display()));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect())
}

/// A measured file. `None` for binaries: a token estimate over bytes that are
/// not text is a number with no meaning, and a wrong number is worse than a gap.
fn measure(path: &Path) -> Option<(usize, u64, usize)> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    Some((text.lines().count(), text.len() as u64, count_tokens(&text)))
}

fn build_tree(root: &Path, files: &[PathBuf]) -> CodeNode {
    let mut tree = CodeNode {
        path: String::new(),
        name: root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("repo")
            .to_string(),
        lines: 0,
        bytes: 0,
        tokens: 0,
        children: Vec::new(),
    };

    for relative in files {
        let Some((lines, bytes, tokens)) = measure(&root.join(relative)) else {
            continue;
        };
        insert(&mut tree, relative, lines, bytes, tokens);
    }
    sort_by_tokens(&mut tree);
    tree
}

/// Walk the path, creating directories as we go, and add the file's weight to
/// every ancestor — a directory's tokens are its subtree's tokens.
fn insert(tree: &mut CodeNode, relative: &Path, lines: usize, bytes: u64, tokens: usize) {
    tree.lines += lines;
    tree.bytes += bytes;
    tree.tokens += tokens;

    let components: Vec<_> = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect();

    let mut node = tree;
    let mut prefix = String::new();
    for (index, component) in components.iter().enumerate() {
        if prefix.is_empty() {
            prefix = (*component).to_string();
        } else {
            prefix = format!("{prefix}/{component}");
        }
        let is_file = index + 1 == components.len();

        let position = node
            .children
            .iter()
            .position(|child| child.name == *component);
        let position = match position {
            Some(position) => position,
            None => {
                node.children.push(CodeNode {
                    path: prefix.clone(),
                    name: (*component).to_string(),
                    lines: 0,
                    bytes: 0,
                    tokens: 0,
                    children: Vec::new(),
                });
                node.children.len() - 1
            }
        };
        node = &mut node.children[position];
        node.lines += lines;
        node.bytes += bytes;
        node.tokens += tokens;
        if is_file {
            break;
        }
    }
}

fn sort_by_tokens(node: &mut CodeNode) {
    node.children
        .sort_by_key(|child| std::cmp::Reverse(child.tokens));
    for child in &mut node.children {
        sort_by_tokens(child);
    }
}

fn print_tree(tree: &CodeNode) {
    let colors = Colors::default();
    println!(
        "{bold}{name:<name_w$}  {lines:>num_w$}  {tokens:>num_w$}  {share:>7}{reset}",
        bold = colors.bold,
        reset = colors.reset,
        name = "PATH",
        lines = "LINES",
        tokens = "TOKENS",
        share = "SHARE",
        name_w = NAME_WIDTH,
        num_w = NUM_WIDTH,
    );
    print_node(tree, tree.tokens, 0);
    println!();
    println!(
        "{bold}{total:<name_w$}  {lines:>num_w$}  {tokens:>num_w$}{reset}",
        bold = colors.bold,
        reset = colors.reset,
        total = "TOTAL",
        lines = format_int(tree.lines),
        tokens = format_int(tree.tokens),
        name_w = NAME_WIDTH,
        num_w = NUM_WIDTH,
    );
}

fn print_node(node: &CodeNode, total: usize, depth: usize) {
    if depth > 0 {
        let indent = "  ".repeat(depth - 1);
        let share = if total > 0 {
            (node.tokens as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let label = format!("{indent}{}", node.name);
        println!(
            "{label:<name_w$}  {lines:>num_w$}  {tokens:>num_w$}  {share:>6.1}%",
            label = truncate(&label, NAME_WIDTH),
            lines = format_int(node.lines),
            tokens = format_int(node.tokens),
            share = share,
            name_w = NAME_WIDTH,
            num_w = NUM_WIDTH,
        );
    }
    if depth >= MAX_DEPTH {
        return;
    }
    for child in &node.children {
        print_node(child, total, depth + 1);
    }
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    let head: String = value.chars().take(width.saturating_sub(1)).collect();
    format!("{head}\u{2026}")
}

fn format_int(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::{
        dominant_extensions, extension_of, insert, sort_by_tokens, CodeNode, MAX_EXTENSIONS,
        NO_EXTENSION,
    };
    use std::collections::BTreeMap;
    use std::path::Path;

    fn empty(name: &str) -> CodeNode {
        CodeNode {
            path: String::new(),
            name: name.to_string(),
            lines: 0,
            bytes: 0,
            tokens: 0,
            children: Vec::new(),
        }
    }

    #[test]
    fn an_extension_is_the_suffix_and_a_dotfile_has_none() {
        assert_eq!(extension_of("rust/src/main.rs"), "rs");
        assert_eq!(extension_of("Cargo.lock"), "lock");
        assert_eq!(
            extension_of("README.MD"),
            "md",
            "extensions fold to lowercase"
        );
        assert_eq!(
            extension_of(".gitignore"),
            NO_EXTENSION,
            "a dotfile is not an extension"
        );
        assert_eq!(extension_of("Makefile"), NO_EXTENSION);
        assert_eq!(extension_of("scripts/helpers.sh"), "sh");
    }

    /// The tail is chosen once for the whole window, so a language never appears
    /// in one snapshot's legend and vanishes from the next.
    #[test]
    fn only_the_dominant_extensions_get_their_own_series() {
        let totals: BTreeMap<String, usize> = (0..12)
            .map(|index| (format!("ext{index}"), 1000 - index * 10))
            .collect();
        let dominant = dominant_extensions(&totals);

        assert_eq!(dominant.len(), MAX_EXTENSIONS);
        assert_eq!(dominant[0], "ext0", "the heaviest extension leads");
        assert!(
            !dominant.contains(&"ext11".to_string()),
            "the tail folds to `other`"
        );
    }

    #[test]
    fn a_directory_weighs_what_its_subtree_weighs() {
        let mut tree = empty("repo");
        insert(&mut tree, Path::new("src/a.rs"), 10, 100, 40);
        insert(&mut tree, Path::new("src/b.rs"), 5, 50, 20);
        insert(&mut tree, Path::new("README.md"), 3, 30, 12);

        assert_eq!(tree.tokens, 72);
        assert_eq!(tree.lines, 18);

        let src = tree.children.iter().find(|c| c.name == "src").expect("src");
        assert_eq!(src.tokens, 60, "a directory is the sum of its files");
        assert_eq!(src.children.len(), 2);
        assert_eq!(src.path, "src");
    }

    #[test]
    fn children_sort_by_tokens_so_the_expensive_paths_read_first() {
        let mut tree = empty("repo");
        insert(&mut tree, Path::new("small.rs"), 1, 10, 5);
        insert(&mut tree, Path::new("huge.rs"), 1, 10, 500);
        sort_by_tokens(&mut tree);

        assert_eq!(tree.children[0].name, "huge.rs");
    }

    /// Lines and tokens disagree, and the flame is about the second: a lockfile
    /// is cheap to read and expensive to send.
    #[test]
    fn a_file_contributes_to_every_ancestor_exactly_once() {
        let mut tree = empty("repo");
        insert(&mut tree, Path::new("a/b/c.rs"), 7, 70, 28);

        let a = &tree.children[0];
        let b = &a.children[0];
        let c = &b.children[0];
        assert_eq!((a.tokens, b.tokens, c.tokens), (28, 28, 28));
        assert_eq!(c.path, "a/b/c.rs");
        assert!(c.children.is_empty());
    }
}
