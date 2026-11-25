//! Unified diff parsing and `<lf-diff>` / `<lf-file>` rendering for `lf page build`.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{bail, Context, Result};
use loopflow_execution::ops::task::checkout_file_diff;
use regex::Regex;

use super::html::{escape_html, find_next_element, line_at, validate_repo_relative};

static HUNK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@").expect("hunk regex"));

pub(super) struct DiffExpander<'a> {
    repo_root: &'a Path,
    base: &'a str,
    cache: HashMap<String, ParsedDiff>,
}

impl<'a> DiffExpander<'a> {
    pub(super) fn new(repo_root: &'a Path, base: &'a str) -> Self {
        Self {
            repo_root,
            base,
            cache: HashMap::new(),
        }
    }

    pub(super) fn expand_diffs(&mut self, input: &str) -> Result<String> {
        let mut out = String::with_capacity(input.len());
        let mut cursor = 0;
        while let Some(tag) = find_next_element(input, cursor, "lf-diff")? {
            out.push_str(&input[cursor..tag.start]);
            let line = line_at(input, tag.start);
            let path = tag.attrs.get("path").cloned().ok_or_else(|| {
                anyhow::anyhow!("line {line}: <lf-diff> requires a `path` attribute")
            })?;
            validate_repo_relative(&path, "lf-diff", line)?;
            let range = tag
                .attrs
                .get("lines")
                .map(|raw| parse_line_range(raw, "lf-diff", line))
                .transpose()?;
            let notes = parse_lf_notes(&tag.inner, line)?;
            let parsed = self.get_diff(&path, line)?;
            out.push_str(&render_diff_block(&path, range, parsed, &notes, line)?);
            cursor = tag.end;
        }
        out.push_str(&input[cursor..]);
        Ok(out)
    }

    pub(super) fn expand_files(&self, input: &str) -> Result<String> {
        let mut out = String::with_capacity(input.len());
        let mut cursor = 0;
        while let Some(tag) = find_next_element(input, cursor, "lf-file")? {
            out.push_str(&input[cursor..tag.start]);
            let line = line_at(input, tag.start);
            let path = tag.attrs.get("path").cloned().ok_or_else(|| {
                anyhow::anyhow!("line {line}: <lf-file> requires a `path` attribute")
            })?;
            validate_repo_relative(&path, "lf-file", line)?;
            let full = self.repo_root.join(&path);
            if !full.is_file() {
                bail!("line {line}: <lf-file path=\"{path}\"> does not exist in repository");
            }
            let content = std::fs::read_to_string(&full).with_context(|| {
                format!("line {line}: failed to read <lf-file path=\"{path}\">")
            })?;
            let range = tag
                .attrs
                .get("lines")
                .map(|raw| parse_line_range(raw, "lf-file", line))
                .transpose()?;
            let notes = parse_lf_notes(&tag.inner, line)?;
            out.push_str(&render_file_block(&path, &content, range, &notes, line)?);
            cursor = tag.end;
        }
        out.push_str(&input[cursor..]);
        Ok(out)
    }

    fn get_diff(&mut self, path: &str, line: usize) -> Result<&ParsedDiff> {
        if !self.cache.contains_key(path) {
            let snapshot = checkout_file_diff(self.repo_root, self.base, path).map_err(|err| {
                anyhow::anyhow!("line {line}: <lf-diff path=\"{path}\"> failed: {err}")
            })?;
            if snapshot.binary {
                bail!("line {line}: <lf-diff path=\"{path}\"> is a binary file");
            }
            let parsed = parse_unified_diff(&snapshot.patch);
            if parsed.hunks.is_empty() {
                if !self.repo_root.join(path).exists() {
                    bail!(
                        "line {line}: <lf-diff path=\"{path}\"> path does not exist in repository"
                    );
                }
                bail!(
                    "line {line}: <lf-diff path=\"{path}\"> has no diff against base `{}`",
                    self.base
                );
            }
            self.cache.insert(path.to_owned(), parsed);
        }
        Ok(self
            .cache
            .get(path)
            .expect("diff was just inserted into cache"))
    }
}

#[derive(Debug, Clone)]
struct ParsedDiff {
    hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone)]
struct DiffHunk {
    header: String,
    lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffLineKind {
    Context,
    Add,
    Del,
}

#[derive(Debug, Clone)]
struct DiffLine {
    kind: DiffLineKind,
    old_line: Option<usize>,
    new_line: Option<usize>,
    text: String,
}

fn parse_unified_diff(patch: &str) -> ParsedDiff {
    let mut hunks = Vec::new();
    let mut current: Option<DiffHunk> = None;
    let mut old_ln = 0usize;
    let mut new_ln = 0usize;

    for raw_line in patch.lines() {
        if let Some(caps) = HUNK_RE.captures(raw_line) {
            if let Some(hunk) = current.take() {
                hunks.push(hunk);
            }
            old_ln = caps[1].parse().unwrap_or(1);
            new_ln = caps[2].parse().unwrap_or(1);
            current = Some(DiffHunk {
                header: raw_line.to_owned(),
                lines: Vec::new(),
            });
            continue;
        }
        let Some(hunk) = current.as_mut() else {
            continue;
        };
        if let Some(rest) = raw_line.strip_prefix('+') {
            hunk.lines.push(DiffLine {
                kind: DiffLineKind::Add,
                old_line: None,
                new_line: Some(new_ln),
                text: rest.to_owned(),
            });
            new_ln += 1;
        } else if let Some(rest) = raw_line.strip_prefix('-') {
            hunk.lines.push(DiffLine {
                kind: DiffLineKind::Del,
                old_line: Some(old_ln),
                new_line: None,
                text: rest.to_owned(),
            });
            old_ln += 1;
        } else if let Some(rest) = raw_line.strip_prefix(' ') {
            hunk.lines.push(DiffLine {
                kind: DiffLineKind::Context,
                old_line: Some(old_ln),
                new_line: Some(new_ln),
                text: rest.to_owned(),
            });
            old_ln += 1;
            new_ln += 1;
        } else if raw_line.starts_with("diff --git") {
            if let Some(done) = current.take() {
                hunks.push(done);
            }
        }
    }
    if let Some(hunk) = current {
        hunks.push(hunk);
    }
    ParsedDiff { hunks }
}

#[derive(Debug, Clone)]
enum NoteTarget {
    NewLine(usize),
    OldLine(usize),
}

struct LineNote {
    target: NoteTarget,
    html: String,
    source_line: usize,
}

fn parse_lf_notes(inner: &str, parent_line: usize) -> Result<Vec<LineNote>> {
    let mut notes = Vec::new();
    let mut cursor = 0;
    while let Some(el) = find_next_element(inner, cursor, "lf-note")? {
        let note_line = parent_line + line_at(inner, el.start) - 1;
        let target = if let Some(line_str) = el.attrs.get("line") {
            let n = line_str.parse::<usize>().map_err(|_| {
                anyhow::anyhow!(
                    "line {note_line}: <lf-note line=\"{line_str}\"> must be a positive integer"
                )
            })?;
            NoteTarget::NewLine(n)
        } else if let Some(old_str) = el.attrs.get("old-line") {
            let n = old_str.parse::<usize>().map_err(|_| {
                anyhow::anyhow!(
                    "line {note_line}: <lf-note old-line=\"{old_str}\"> must be a positive integer"
                )
            })?;
            NoteTarget::OldLine(n)
        } else {
            bail!("line {note_line}: <lf-note> requires `line` or `old-line`");
        };
        notes.push(LineNote {
            target,
            html: el.inner.trim().to_owned(),
            source_line: note_line,
        });
        cursor = el.end;
    }
    Ok(notes)
}

fn parse_line_range(raw: &str, elem: &str, line: usize) -> Result<(usize, usize)> {
    let trimmed = raw.trim();
    if let Some((a, b)) = trimmed.split_once('-') {
        let start: usize = a.trim().parse().map_err(|_| {
            anyhow::anyhow!("line {line}: <{elem} lines=\"{raw}\"> invalid start line")
        })?;
        let end: usize = b.trim().parse().map_err(|_| {
            anyhow::anyhow!("line {line}: <{elem} lines=\"{raw}\"> invalid end line")
        })?;
        if start == 0 || end < start {
            bail!("line {line}: <{elem} lines=\"{raw}\"> range must be 1-based with start <= end");
        }
        Ok((start, end))
    } else {
        let single: usize = trimmed.parse().map_err(|_| {
            anyhow::anyhow!("line {line}: <{elem} lines=\"{raw}\"> invalid line number")
        })?;
        if single == 0 {
            bail!("line {line}: <{elem} lines=\"{raw}\"> line numbers are 1-based");
        }
        Ok((single, single))
    }
}

fn render_diff_block(
    path: &str,
    range: Option<(usize, usize)>,
    parsed: &ParsedDiff,
    notes: &[LineNote],
    elem_line: usize,
) -> Result<String> {
    let mut rows = String::new();
    let mut matched_lines = 0usize;
    let mut matched_notes: BTreeSet<usize> = BTreeSet::new();

    for hunk in &parsed.hunks {
        let selected: Vec<&DiffLine> = hunk
            .lines
            .iter()
            .filter(|dl| match range {
                None => true,
                Some((start, end)) => {
                    dl.new_line.is_some_and(|n| (start..=end).contains(&n))
                        || dl.old_line.is_some_and(|o| (start..=end).contains(&o))
                }
            })
            .collect();
        if selected.is_empty() {
            continue;
        }
        rows.push_str(&format!(
            "<tr class=\"lf-hunk\"><td colspan=\"4\">{}</td></tr>\n",
            escape_html(&hunk.header)
        ));
        for dl in selected {
            matched_lines += 1;
            let (cls, sign) = match dl.kind {
                DiffLineKind::Add => ("lf-add", "+"),
                DiffLineKind::Del => ("lf-del", "-"),
                DiffLineKind::Context => ("lf-ctx", " "),
            };
            let old_str = dl.old_line.map(|n| n.to_string()).unwrap_or_default();
            let new_str = dl.new_line.map(|n| n.to_string()).unwrap_or_default();
            rows.push_str(&format!(
                "<tr class=\"{cls}\"><td class=\"lf-ln\">{old_str}</td><td class=\"lf-ln\">{new_str}</td><td class=\"lf-sign\">{sign}</td><td>{}</td></tr>\n",
                escape_html(&dl.text)
            ));
            for (idx, note) in notes.iter().enumerate() {
                let attaches = match note.target {
                    NoteTarget::NewLine(n) => dl.new_line == Some(n),
                    NoteTarget::OldLine(o) => dl.old_line == Some(o),
                };
                if attaches {
                    matched_notes.insert(idx);
                    rows.push_str(&format!(
                        "<tr class=\"lf-line-note\" data-notes><td colspan=\"4\">{}</td></tr>\n",
                        note.html
                    ));
                }
            }
        }
    }

    if matched_lines == 0 {
        let range_label = range
            .map(|(a, b)| format!(" lines=\"{a}-{b}\""))
            .unwrap_or_default();
        bail!(
            "line {elem_line}: <lf-diff path=\"{path}\"{range_label}> matched no lines in the diff"
        );
    }

    for (idx, note) in notes.iter().enumerate() {
        if !matched_notes.contains(&idx) {
            let label = match note.target {
                NoteTarget::NewLine(n) => format!("line=\"{n}\""),
                NoteTarget::OldLine(o) => format!("old-line=\"{o}\""),
            };
            bail!(
                "line {}: <lf-note {label}> does not match any rendered line in <lf-diff path=\"{path}\">",
                note.source_line
            );
        }
    }

    Ok(wrap_codeblock(path, range, !notes.is_empty(), &rows))
}

fn render_file_block(
    path: &str,
    content: &str,
    range: Option<(usize, usize)>,
    notes: &[LineNote],
    elem_line: usize,
) -> Result<String> {
    let all_lines: Vec<&str> = content.lines().collect();
    let (start, end) = match range {
        Some((s, e)) => {
            if s > all_lines.len() {
                bail!(
                    "line {elem_line}: <lf-file path=\"{path}\" lines=\"{s}-{e}\"> starts past end of file ({} lines)",
                    all_lines.len()
                );
            }
            (s, e.min(all_lines.len()))
        }
        None => (1, all_lines.len()),
    };

    let mut rows = String::new();
    let mut matched_notes: BTreeSet<usize> = BTreeSet::new();
    for line_no in start..=end {
        let text = all_lines.get(line_no - 1).copied().unwrap_or("");
        rows.push_str(&format!(
            "<tr><td class=\"lf-ln\">{line_no}</td><td>{}</td></tr>\n",
            escape_html(text)
        ));
        for (idx, note) in notes.iter().enumerate() {
            match note.target {
                NoteTarget::NewLine(n) if n == line_no => {
                    matched_notes.insert(idx);
                    rows.push_str(&format!(
                        "<tr class=\"lf-line-note\" data-notes><td colspan=\"2\">{}</td></tr>\n",
                        note.html
                    ));
                }
                NoteTarget::OldLine(_) => {
                    bail!(
                        "line {}: <lf-note old-line=…> is only valid inside <lf-diff>, not <lf-file>",
                        note.source_line
                    );
                }
                _ => {}
            }
        }
    }

    for (idx, note) in notes.iter().enumerate() {
        if !matched_notes.contains(&idx) {
            let NoteTarget::NewLine(n) = note.target else {
                continue;
            };
            bail!(
                "line {}: <lf-note line=\"{n}\"> is outside rendered lines {start}-{end} of <lf-file path=\"{path}\">",
                note.source_line
            );
        }
    }

    Ok(wrap_codeblock(path, range, !notes.is_empty(), &rows))
}

fn wrap_codeblock(
    path: &str,
    range: Option<(usize, usize)>,
    has_notes: bool,
    rows: &str,
) -> String {
    let range_suffix = range
        .map(|(a, b)| {
            if a == b {
                format!(":{a}")
            } else {
                format!(":{a}-{b}")
            }
        })
        .unwrap_or_default();
    let toggle = if has_notes {
        "<button type=\"button\" class=\"lf-btn-quiet\" data-notes-toggle>Hide line notes</button>"
    } else {
        ""
    };
    let escaped = escape_html(path);
    format!(
        "<div class=\"lf-codeblock\" data-path=\"{escaped}\"><div class=\"lf-codeblock-header\"><span class=\"lf-codeblock-path\">{escaped}{range_suffix}</span>{toggle}</div><div class=\"lf-codeblock-scroll\"><table class=\"lf-code-table\"><tbody>\n{rows}</tbody></table></div></div>"
    )
}

#[cfg(test)]
mod tests {
    use super::parse_unified_diff;

    #[test]
    fn unified_diff_parser_tracks_old_and_new_line_numbers() {
        let patch = "\
diff --git a/src/a.rs b/src/a.rs
--- a/src/a.rs
+++ b/src/a.rs
@@ -10,3 +10,4 @@ fn demo() {
 let x = 1;
-let old = 2;
+let new = 2;
+let extra = 3;
 let tail = 4;
";
        let parsed = parse_unified_diff(patch);
        assert_eq!(parsed.hunks.len(), 1);
        let lines = &parsed.hunks[0].lines;
        assert_eq!(lines[0].old_line, Some(10));
        assert_eq!(lines[0].new_line, Some(10));
        assert_eq!(lines[1].old_line, Some(11));
        assert_eq!(lines[1].new_line, None);
        assert_eq!(lines[2].old_line, None);
        assert_eq!(lines[2].new_line, Some(11));
        assert_eq!(lines[3].new_line, Some(12));
        assert_eq!(lines[4].old_line, Some(12));
        assert_eq!(lines[4].new_line, Some(13));
    }
}
