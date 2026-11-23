use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::{file_context, file_store, snapshot::scratch_paths};
use crate::ops::OpsResult;

const LOCAL_PATH_LIMIT: usize = 2_000;
const MATCH_LIMIT: usize = 100;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskFileInventory {
    pub paths: Vec<String>,
    pub scratch: Vec<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub latest_scratch_page: Option<String>,
    pub truncated: bool,
}

pub fn task_find(
    issue: &str,
    query: Option<&str>,
    limit: Option<usize>,
) -> OpsResult<TaskFileInventory> {
    let checkout = file_context(&file_store()?, issue)?;
    let inventory = crate::observe::checkout_files(&checkout.worktree)?;
    Ok(inventory.matches(query.unwrap_or(""), limit))
}

impl TaskFileInventory {
    pub(crate) fn scan(worktree: &Path) -> io::Result<Self> {
        let root = worktree.canonicalize()?;
        let output = Command::new("git")
            .args([
                "ls-files",
                "-z",
                "--cached",
                "--others",
                "--exclude-standard",
                "--deduplicate",
            ])
            .current_dir(&root)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        let (scratch, truncated) = scratch_paths(&root)?;
        let mut paths: BTreeSet<String> = scratch.into_iter().collect();
        for bytes in output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
        {
            let path = std::str::from_utf8(bytes).map_err(io::Error::other)?;
            paths.insert(path.to_string());
        }
        let mut scratch = Vec::new();
        let mut latest = None;
        let mut files = Vec::new();
        for path in paths {
            let absolute = root.join(&path);
            let Ok(target) = absolute.canonicalize() else {
                continue;
            };
            if !target.starts_with(&root) {
                continue;
            }
            let Ok(metadata) = fs::metadata(target) else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            if path.starts_with("scratch/") {
                if scratch.len() < LOCAL_PATH_LIMIT {
                    scratch.push(path.clone());
                }
                if is_scratch_page(Path::new(&path)) {
                    let modified = metadata.modified()?;
                    if latest.as_ref().is_none_or(|(time, _)| modified > *time) {
                        latest = Some((modified, path.clone()));
                    }
                }
            }
            files.push(path);
        }
        Ok(Self {
            paths: files,
            scratch,
            latest_scratch_page: latest.map(|(_, path)| path),
            truncated,
        })
    }

    fn matches(&self, query: &str, limit: Option<usize>) -> Self {
        let query = query.to_lowercase();
        let limit = limit.unwrap_or(if query.is_empty() {
            LOCAL_PATH_LIMIT
        } else {
            MATCH_LIMIT
        });
        let (paths, omitted) = if query.is_empty() {
            (
                self.paths.iter().take(limit).cloned().collect(),
                self.paths.len() > limit,
            )
        } else {
            let mut matches: Vec<_> = self
                .paths
                .iter()
                .filter_map(|path| {
                    let lower = path.to_lowercase();
                    let span = match_span(&lower, &query)?;
                    let name = lower.rsplit('/').next().expect("a path has a filename");
                    Some((
                        !path.starts_with("scratch/"),
                        !name.contains(&query),
                        span,
                        path,
                    ))
                })
                .collect();
            let omitted = matches.len() > limit;
            if omitted {
                matches.select_nth_unstable(limit);
                matches.truncate(limit);
            }
            matches.sort_unstable();
            (
                matches
                    .into_iter()
                    .map(|(_, _, _, path)| path.clone())
                    .collect(),
                omitted,
            )
        };
        Self {
            paths,
            scratch: self.scratch.clone(),
            latest_scratch_page: self.latest_scratch_page.clone(),
            truncated: self.truncated || omitted,
        }
    }
}

fn is_scratch_page(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("html"))
}

fn match_span(path: &str, query: &str) -> Option<usize> {
    let mut positions = path.char_indices();
    let mut first = None;
    let mut last = 0;
    for ch in query.chars() {
        (last, _) = positions.find(|(_, candidate)| *candidate == ch)?;
        first.get_or_insert(last);
    }
    Some(last - first.unwrap_or(0))
}
