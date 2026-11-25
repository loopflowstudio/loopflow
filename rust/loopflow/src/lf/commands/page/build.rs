//! Compiler for `lf page build`: expands body-only `*.page.html` fragments
//! into self-contained HTML review pages with real diffs, components, strict
//! validated `data-lf` operations, theme CSS, work stamp, and shared runtime.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use anyhow::{bail, Context, Result};
use loopflow_definitions::builtins::{PAGE_BASE_CSS, PAGE_DEFAULT_THEME_CSS, PAGE_RUNTIME_JS};
use loopflow_execution::ops::context::ContextFact;
use loopflow_execution::ops::task::task_diff_base;
use regex::Regex;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::lf::Cli;

use super::html::{
    enclosing_tag_name, escape_html, extract_title, line_at, unescape_html_attr,
    validate_repo_relative,
};
use super::{components, diff, is_page_source, operations};

static FORBIDDEN_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<\s*(html|head|style|script)\b").expect("static tag regex"));
static DATA_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)data-path\s*=\s*(?:'([^']*)'|"([^"]*)")"#).expect("static data-path regex")
});
static CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<code([^>]*)>(.*?)</code>").expect("static code regex"));
static HEX_TOKEN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[0-9a-fA-F]{7,40}\b").expect("static hex regex"));

/// Compile `<source>` (`scratch/<name>.page.html`) into `scratch/<name>.html`.
pub(super) fn build(cli: &Cli, source: &Path, base_override: Option<&str>) -> Result<PathBuf> {
    let output_path = output_path_for(source)?;
    let cwd = std::env::current_dir()?;
    let source_abs = cwd.join(source);
    let source_text = std::fs::read_to_string(&source_abs)
        .with_context(|| format!("Could not read page source {}", source.display()))?;

    let start_dir = source_abs
        .parent()
        .filter(|dir| dir.is_dir())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| cwd.clone());
    let repo_root =
        loopflow_execution::repo::working_directory_in(&start_dir).unwrap_or_else(|_| cwd.clone());
    let repo_root = std::fs::canonicalize(&repo_root).unwrap_or(repo_root);

    check_forbidden_tags(&source_text)?;

    let base = resolve_diff_base(&repo_root, base_override)?;
    let head = git_head_short(&repo_root);
    let work = resolve_work_stamp(cli, &repo_root);

    // Run `data-lf`, `data-path`, and `<code>` SHA checks before diff/file
    // expansion so error line numbers stay close to the source and deleted-file
    // `<lf-diff>` blocks are not rejected as missing on disk.
    let mut component_styles = BTreeMap::new();
    let body = components::expand(&repo_root, &source_text, &mut component_styles)?;
    let body = operations::expand(&repo_root, &body)?;
    check_data_paths(&repo_root, &body)?;
    check_code_shas(&repo_root, &body)?;
    let mut diffs = diff::DiffExpander::new(&repo_root, &base);
    let body = diffs.expand_diffs(&body)?;
    let body_html = diffs.expand_files(&body)?;

    let output_abs = cwd.join(&output_path);
    let rel_output = repo_relative_path(&output_abs, &repo_root, &output_path);
    let stamp = PageStamp {
        work,
        directory: repo_root.to_string_lossy().into_owned(),
        base,
        head,
        path: rel_output.to_string_lossy().replace('\\', "/"),
        build_id: short_sha256(&body_html),
    };
    let stamp_json = serde_json::to_string(&stamp)?.replace("</", "<\\/");

    let title = extract_title(&body_html).unwrap_or_else(|| {
        output_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Review page")
            .to_owned()
    });

    let mut css = load_theme_css(&repo_root);
    css.push('\n');
    css.push_str(PAGE_BASE_CSS);
    for extra in component_styles.values() {
        css.push('\n');
        css.push_str(extra);
    }

    let document = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<style>\n{}\n</style>\n</head>\n<body>\n<main class=\"lf-page\">\n{}\n</main>\n<script type=\"application/json\" id=\"lf-page\">{}</script>\n<script>\n{}\n</script>\n</body>\n</html>\n",
        escape_html(&title),
        css.replace("</style", "<\\/style"),
        body_html.trim(),
        stamp_json,
        PAGE_RUNTIME_JS.replace("</script", "<\\/script"),
    );

    if let Some(parent) = output_abs.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output_abs, document)
        .with_context(|| format!("Could not write compiled page {}", output_path.display()))?;

    Ok(output_path)
}

#[derive(Debug, Serialize)]
struct PageStamp {
    work: WorkStamp,
    directory: String,
    base: String,
    head: String,
    path: String,
    build_id: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WorkStamp {
    Task { issue: String },
    Wave { wave: String },
    Session { session: String },
    Repository,
}

fn output_path_for(source: &Path) -> Result<PathBuf> {
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("Invalid source path {}", source.display()))?;
    if !is_page_source(source) {
        bail!(
            "Page source must end with `.page.html` (got `{}`); write body fragments to `scratch/<name>.page.html`",
            source.display()
        );
    }
    let stem = &name[..name.len() - ".page.html".len()];
    Ok(source.with_file_name(format!("{stem}.html")))
}

fn repo_relative_path(output_abs: &Path, repo_root: &Path, fallback: &Path) -> PathBuf {
    if let Ok(rel) = output_abs.strip_prefix(repo_root) {
        return rel.to_path_buf();
    }
    // Canonicalize parent directory when output file does not exist yet.
    if let (Some(parent), Some(name)) = (
        output_abs
            .parent()
            .and_then(|dir| std::fs::canonicalize(dir).ok()),
        output_abs.file_name(),
    ) {
        if let Ok(rel) = parent.join(name).strip_prefix(repo_root) {
            return rel.to_path_buf();
        }
    }
    fallback.to_path_buf()
}

fn load_theme_css(repo_root: &Path) -> String {
    let repo_theme = repo_root.join(".lf/theme.css");
    if repo_theme.is_file() {
        if let Ok(text) = std::fs::read_to_string(&repo_theme) {
            return text;
        }
    }
    PAGE_DEFAULT_THEME_CSS.to_owned()
}

fn resolve_diff_base(repo_root: &Path, override_base: Option<&str>) -> Result<String> {
    if let Some(base) = override_base {
        return Ok(base.to_owned());
    }
    if let Ok(Some(base)) = task_diff_base(repo_root) {
        return Ok(base);
    }
    if let Ok(branch) = loopflow_execution::git::get_default_branch(repo_root) {
        if let Ok(out) = Command::new("git")
            .current_dir(repo_root)
            .args(["merge-base", "HEAD", &branch])
            .output()
        {
            if out.status.success() {
                let sha = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                if !sha.is_empty() {
                    return Ok(sha);
                }
            }
        }
    }
    Ok("HEAD".to_owned())
}

fn git_head_short(repo_root: &Path) -> String {
    Command::new("git")
        .current_dir(repo_root)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|sha| !sha.is_empty())
        .unwrap_or_else(|| "HEAD".to_owned())
}

fn resolve_work_stamp(cli: &Cli, repo_root: &Path) -> WorkStamp {
    if let Ok(explained) = crate::lf::commands::context::explain_in(
        repo_root,
        cli.wave.as_deref(),
        cli.task.as_deref(),
        None,
    ) {
        if let ContextFact::Bound { value, .. } = explained.task {
            let issue = readable_task_identifier(&value).unwrap_or(value);
            return WorkStamp::Task { issue };
        }
        if let ContextFact::Bound { value, .. } = explained.wave {
            return WorkStamp::Wave { wave: value };
        }
    }
    if let Some(caller) = loopflow_execution::journal::agent_caller() {
        if is_standalone_interactive_session(&caller.session_id) {
            return WorkStamp::Session {
                session: caller.session_id.to_string(),
            };
        }
    }
    WorkStamp::Repository
}

fn readable_task_identifier(raw: &str) -> Option<String> {
    let store = loopflow_execution::store::read_existing_registry()
        .ok()
        .flatten()?;
    let id = loopflow_storage::work::task::TaskId::from_raw(raw);
    store.sqlite.task(&id).ok().flatten().map(|t| t.identifier)
}

fn is_standalone_interactive_session(session_id: &loopflow_model::id::LfSessionId) -> bool {
    let Ok(Some(store)) = loopflow_execution::store::read_existing_registry() else {
        return false;
    };
    let Ok(Some(session)) = store.sqlite.session(session_id) else {
        return false;
    };
    session.interactive && session.graph_lf_process_id.is_none()
}

fn short_sha256(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))[..16].to_owned()
}

/// Reject `<html`, `<head`, `<style`, or `<script` tags in body fragments.
fn check_forbidden_tags(source: &str) -> Result<()> {
    let stripped = strip_html_comments_preserving_newlines(source);
    if let Some(caps) = FORBIDDEN_TAG_RE.captures(&stripped) {
        let found = caps.get(0).expect("full match");
        let tag = caps[1].to_ascii_lowercase();
        let line = line_at(&stripped, found.start());
        bail!(
            "line {line}: <{tag}> is not allowed in a `.page.html` source (body HTML only; theme goes in `.lf/theme.css`)"
        );
    }
    Ok(())
}

fn strip_html_comments_preserving_newlines(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 4..];
        let (comment, next) = match after.find("-->") {
            Some(end) => (&after[..end], &after[end + 3..]),
            None => (after, ""),
        };
        for ch in comment.chars() {
            out.push(if ch == '\n' { '\n' } else { ' ' });
        }
        rest = next;
    }
    out.push_str(rest);
    out
}

fn check_data_paths(repo_root: &Path, input: &str) -> Result<()> {
    for caps in DATA_PATH_RE.captures_iter(input) {
        let full = caps.get(0).expect("full match");
        let line = line_at(input, full.start());
        let elem = enclosing_tag_name(input, full.start()).unwrap_or("element");
        let raw = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| m.as_str())
            .unwrap_or("");
        let path = unescape_html_attr(raw);
        if path.is_empty() {
            continue;
        }
        validate_repo_relative(&path, elem, line)?;
        if !repo_root.join(&path).exists() {
            bail!("line {line}: <{elem} data-path=\"{path}\"> does not exist in repository");
        }
    }
    Ok(())
}

fn check_code_shas(repo_root: &Path, input: &str) -> Result<()> {
    let mut sha_cache: HashMap<String, bool> = HashMap::new();
    for caps in CODE_RE.captures_iter(input) {
        let full = caps.get(0).expect("full match");
        let attrs = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        if attrs.to_ascii_lowercase().contains("data-no-check") {
            continue;
        }
        let body = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let line = line_at(input, full.start());
        for mat in HEX_TOKEN_RE.find_iter(body) {
            let candidate = mat.as_str();
            if !looks_like_git_sha(candidate) {
                continue;
            }
            let exists = *sha_cache.entry(candidate.to_owned()).or_insert_with(|| {
                Command::new("git")
                    .current_dir(repo_root)
                    .args(["cat-file", "-e", candidate])
                    .output()
                    .is_ok_and(|out| out.status.success())
            });
            if !exists {
                bail!(
                    "line {line}: <code> contains unknown Git SHA `{candidate}` (add `data-no-check` if this is not a commit SHA)"
                );
            }
        }
    }
    Ok(())
}

fn looks_like_git_sha(token: &str) -> bool {
    (7..=40).contains(&token.len())
        && token.bytes().all(|b| b.is_ascii_hexdigit())
        && token.bytes().any(|b| b.is_ascii_digit())
        && token
            .bytes()
            .any(|b| matches!(b, b'a'..=b'f' | b'A'..=b'F'))
}

#[cfg(test)]
mod tests {
    use super::{check_forbidden_tags, looks_like_git_sha, output_path_for};
    use std::path::Path;

    #[test]
    fn output_path_strips_page_suffix_and_refuses_other_extensions() {
        assert_eq!(
            output_path_for(Path::new("scratch/ship-review.page.html")).unwrap(),
            Path::new("scratch/ship-review.html")
        );
        assert!(output_path_for(Path::new("scratch/ship-review.html")).is_err());
    }

    #[test]
    fn forbidden_tags_report_element_and_line() {
        let ok = "<h1>Title</h1>\n<!-- <script>ignored</script> -->\n<p>Plain prose</p>";
        assert!(check_forbidden_tags(ok).is_ok());
        let bad = "<h1>Title</h1>\n\n<script>alert(1)</script>";
        let err = check_forbidden_tags(bad).unwrap_err().to_string();
        assert!(err.contains("line 3"), "{err}");
        assert!(err.contains("<script>"), "{err}");
    }

    #[test]
    fn sha_detection_requires_digit_and_hex_letter() {
        assert!(looks_like_git_sha("2b6b03ad5e"));
        assert!(looks_like_git_sha("a1b2c3d"));
        assert!(!looks_like_git_sha("1234567"));
        assert!(!looks_like_git_sha(&"abcdef0"[..6]));
        assert!(!looks_like_git_sha("deadbeef"));
        assert!(!looks_like_git_sha("feature"));
    }
}
