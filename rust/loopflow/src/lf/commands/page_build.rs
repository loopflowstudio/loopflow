//! Compiler for `lf page build`: expands body-only `*.page.html` fragments
//! into self-contained HTML review pages with real diffs, components, strict
//! validated `data-lf` operations, theme CSS, work stamp, and shared runtime.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use loopflow_definitions::builtins::{
    get_builtin_page_component, PAGE_BASE_CSS, PAGE_DEFAULT_THEME_CSS, PAGE_RUNTIME_JS,
};
use loopflow_execution::ops::context::ContextFact;
use loopflow_execution::ops::task::{checkout_file_diff, task_diff_base};
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::lf::Cli;
use crate::server::schema;
use crate::server::wire::Operation;

const REQUEST_ID_SENTINEL: &str = "__LF_REQUEST_ID__";

/// Compile `<source>` (`scratch/<name>.page.html`) into `scratch/<name>.html`.
pub(super) fn build(cli: &Cli, source: &Path, base_override: Option<&str>) -> Result<PathBuf> {
    let output_path = output_path_for(source)?;
    let source_abs = if source.is_absolute() {
        source.to_path_buf()
    } else {
        std::env::current_dir()?.join(source)
    };
    let source_text = std::fs::read_to_string(&source_abs)
        .with_context(|| format!("Could not read page source {}", source.display()))?;

    let start_dir = source_abs
        .parent()
        .filter(|dir| dir.is_dir())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let repo_root = loopflow_execution::repo::working_directory_in(&start_dir)
        .or_else(|_| std::env::current_dir())?;
    let repo_root = std::fs::canonicalize(&repo_root).unwrap_or(repo_root);

    check_forbidden_tags(&source_text)?;

    let base = resolve_diff_base(&repo_root, base_override)?;
    let head = git_head_short(&repo_root);
    let work = resolve_work_stamp(cli, &repo_root);

    let mut compiler = Compiler::new(&repo_root, &base);
    let expanded_components = compiler.expand_components(&source_text)?;
    let expanded_diffs = compiler.expand_diffs(&expanded_components)?;
    let expanded_files = compiler.expand_files(&expanded_diffs)?;
    let expanded_ops = compiler.expand_data_lf(&expanded_files)?;
    compiler.check_data_paths(&expanded_ops)?;
    compiler.check_code_shas(&expanded_ops)?;

    let body_html = expanded_ops;
    let build_id = short_sha256(&body_html);
    let output_abs = if output_path.is_absolute() {
        output_path.clone()
    } else {
        std::env::current_dir()?.join(&output_path)
    };
    let rel_output = output_abs
        .strip_prefix(&repo_root)
        .map(Path::to_path_buf)
        .or_else(|_| {
            // Canonicalize parent directory when output file does not exist yet.
            let parent = output_abs
                .parent()
                .and_then(|dir| std::fs::canonicalize(dir).ok());
            let name = output_abs.file_name();
            match (parent, name) {
                (Some(parent), Some(name)) => parent
                    .join(name)
                    .strip_prefix(&repo_root)
                    .map(Path::to_path_buf)
                    .map_err(|_| ()),
                _ => Err(()),
            }
        })
        .unwrap_or_else(|_| output_path.clone());
    let rel_path_str = rel_output.to_string_lossy().replace('\\', "/");

    let stamp = PageStamp {
        work,
        directory: repo_root.to_string_lossy().into_owned(),
        base: base.clone(),
        head,
        path: rel_path_str,
        build_id,
    };
    let stamp_json = serde_json::to_string(&stamp)?.replace("</", "<\\/");

    let theme_css = load_theme_css(&repo_root);
    let title = extract_title(&body_html).unwrap_or_else(|| {
        output_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Review page")
            .to_owned()
    });

    let mut css = String::new();
    css.push_str(&theme_css);
    css.push('\n');
    css.push_str(PAGE_BASE_CSS);
    for extra in compiler.component_styles.values() {
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
    let lower = name.to_ascii_lowercase();
    if !lower.ends_with(".page.html") {
        bail!(
            "Page source must end with `.page.html` (got `{}`); write body fragments to `scratch/<name>.page.html`",
            source.display()
        );
    }
    let stem = &name[..name.len() - ".page.html".len()];
    Ok(source.with_file_name(format!("{stem}.html")))
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
        let output = Command::new("git")
            .current_dir(repo_root)
            .args(["merge-base", "HEAD", &branch])
            .output();
        if let Ok(out) = output {
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
    let digest = hex::encode(Sha256::digest(text.as_bytes()));
    digest[..16].to_owned()
}

fn line_at(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset.min(text.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

/// Reject `<html`, `<head`, `<style`, or `<script` tags in body fragments.
fn check_forbidden_tags(source: &str) -> Result<()> {
    let stripped = strip_html_comments_preserving_newlines(source);
    let re = Regex::new(r"(?i)<\s*(html|head|style|script)\b").expect("static regex");
    if let Some(found) = re.find(&stripped) {
        let caps = re.captures(found.as_str()).expect("matched capture");
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
        if let Some(end) = after.find("-->") {
            let comment = &after[..end];
            for ch in comment.chars() {
                out.push(if ch == '\n' { '\n' } else { ' ' });
            }
            rest = &after[end + 3..];
        } else {
            for ch in after.chars() {
                out.push(if ch == '\n' { '\n' } else { ' ' });
            }
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

struct Compiler<'a> {
    repo_root: &'a Path,
    base: &'a str,
    diff_cache: HashMap<String, ParsedDiff>,
    sha_cache: HashMap<String, bool>,
    component_styles: BTreeMap<String, String>,
    operation_schemas: Option<HashMap<String, Value>>,
}

impl<'a> Compiler<'a> {
    fn new(repo_root: &'a Path, base: &'a str) -> Self {
        Self {
            repo_root,
            base,
            diff_cache: HashMap::new(),
            sha_cache: HashMap::new(),
            component_styles: BTreeMap::new(),
            operation_schemas: None,
        }
    }

    fn operation_schema(&mut self, kind: &str) -> Option<Value> {
        let map = self.operation_schemas.get_or_insert_with(|| {
            let doc = schema::describe();
            let mut ops = HashMap::new();
            for group in ["Write", "Read", "ServerOperation", "MeasuredOperation"] {
                if let Some(arms) = doc["$defs"][group]["oneOf"].as_array() {
                    for arm in arms {
                        if let Some(k) = arm["properties"]["kind"]["const"].as_str() {
                            ops.insert(k.to_owned(), arm.clone());
                        }
                    }
                }
            }
            ops
        });
        map.get(kind).cloned()
    }

    fn expand_components(&mut self, input: &str) -> Result<String> {
        // Expand iteratively (up to depth 4) so nested components resolve cleanly.
        let mut current = input.to_owned();
        for _ in 0..4 {
            let mut changed = false;
            let mut out = String::with_capacity(current.len());
            let mut cursor = 0;
            while let Some(tag) = find_next_element(&current, cursor, "lf-component")? {
                changed = true;
                out.push_str(&current[cursor..tag.start]);
                let line = line_at(&current, tag.start);
                let name = tag.attrs.get("name").cloned().ok_or_else(|| {
                    anyhow::anyhow!("line {line}: <lf-component> requires a `name` attribute")
                })?;
                let (template, style) = self.load_component(&name, line)?;
                if let Some(style) = style {
                    self.component_styles.entry(name.clone()).or_insert(style);
                }
                let rendered = render_component(&template, &tag.attrs, &tag.inner)?;
                out.push_str(&rendered);
                cursor = tag.end;
            }
            out.push_str(&current[cursor..]);
            current = out;
            if !changed {
                break;
            }
        }
        Ok(current)
    }

    fn load_component(&self, name: &str, line: usize) -> Result<(String, Option<String>)> {
        if name.is_empty() || name.contains('/') || name.contains("..") {
            bail!("line {line}: invalid component name `<lf-component name=\"{name}\">`");
        }
        let repo_dir = self.repo_root.join(".lf/components").join(name);
        let repo_template = repo_dir.join("template.html");
        if repo_template.is_file() {
            let html = std::fs::read_to_string(&repo_template).with_context(|| {
                format!(
                    "line {line}: failed to read component template {}",
                    repo_template.display()
                )
            })?;
            let style_path = repo_dir.join("style.css");
            let style = style_path
                .is_file()
                .then(|| std::fs::read_to_string(&style_path).ok())
                .flatten();
            return Ok((html, style));
        }
        if let Some((html, style)) = get_builtin_page_component(name) {
            return Ok((html.to_owned(), style.map(str::to_owned)));
        }
        bail!(
            "line {line}: unknown component `<lf-component name=\"{name}\">` (not in `.lf/components/{name}/template.html` or builtins)"
        )
    }

    fn expand_diffs(&mut self, input: &str) -> Result<String> {
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
            let rendered = render_diff_block(&path, range, parsed, &notes, line)?;
            out.push_str(&rendered);
            cursor = tag.end;
        }
        out.push_str(&input[cursor..]);
        Ok(out)
    }

    fn get_diff(&mut self, path: &str, line: usize) -> Result<&ParsedDiff> {
        if !self.diff_cache.contains_key(path) {
            let snapshot = checkout_file_diff(self.repo_root, self.base, path).map_err(|err| {
                anyhow::anyhow!("line {line}: <lf-diff path=\"{path}\"> failed: {err}")
            })?;
            if snapshot.binary {
                bail!("line {line}: <lf-diff path=\"{path}\"> is a binary file");
            }
            let parsed = parse_unified_diff(&snapshot.patch);
            if parsed.hunks.is_empty() {
                let on_disk = self.repo_root.join(path).exists();
                if !on_disk {
                    bail!(
                        "line {line}: <lf-diff path=\"{path}\"> path does not exist in repository"
                    );
                }
                bail!(
                    "line {line}: <lf-diff path=\"{path}\"> has no diff against base `{}`",
                    self.base
                );
            }
            self.diff_cache.insert(path.to_owned(), parsed);
        }
        Ok(self
            .diff_cache
            .get(path)
            .expect("diff was just inserted into cache"))
    }

    fn expand_files(&self, input: &str) -> Result<String> {
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
            let rendered = render_file_block(&path, &content, range, &notes, line)?;
            out.push_str(&rendered);
            cursor = tag.end;
        }
        out.push_str(&input[cursor..]);
        Ok(out)
    }

    fn expand_data_lf(&mut self, input: &str) -> Result<String> {
        // Match opening tags containing a `data-lf='...'` or `data-lf="..."` attribute.
        let attr_re = Regex::new(r#"(?s)data-lf\s*=\s*(?:'([^']*)'|"([^"]*)")"#)
            .expect("static data-lf regex");
        let mut out = String::with_capacity(input.len());
        let mut cursor = 0;
        for caps in attr_re.captures_iter(input) {
            let full = caps.get(0).expect("full match");
            out.push_str(&input[cursor..full.start()]);
            let line = line_at(input, full.start());
            let elem = enclosing_tag_name(input, full.start()).unwrap_or("element");
            let raw_attr = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str())
                .unwrap_or("");
            let unescaped = unescape_html_attr(raw_attr);
            let strict_json = self.compile_operation_json(&unescaped, elem, line)?;
            out.push_str("data-lf='");
            out.push_str(&escape_single_quoted_attr(&strict_json));
            out.push('\'');
            cursor = full.end();
        }
        out.push_str(&input[cursor..]);
        Ok(out)
    }

    fn compile_operation_json(&mut self, raw: &str, elem: &str, line: usize) -> Result<String> {
        let mut value: Value = serde_json::from_str(raw).map_err(|err| {
            anyhow::anyhow!("line {line}: <{elem} data-lf=…> is not valid JSON: {err}")
        })?;
        let Some(object) = value.as_object_mut() else {
            bail!("line {line}: <{elem} data-lf=…> must be a JSON object");
        };
        let Some(kind) = object
            .get("kind")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            bail!("line {line}: <{elem} data-lf=…> requires a string `kind` field");
        };
        let Some(arm) = self.operation_schema(&kind) else {
            bail!("line {line}: <{elem} data-lf=…> unknown operation kind `{kind}`");
        };

        // Expand missing required properties from the operation's generated JSON Schema.
        let properties = arm.get("properties").and_then(Value::as_object);
        let required: Vec<String> = arm
            .get("required")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();

        for field in required {
            if object.contains_key(&field) {
                continue;
            }
            if field == "directory" {
                object.insert(
                    field,
                    Value::String(self.repo_root.to_string_lossy().into_owned()),
                );
                continue;
            }
            if field == "request_id" {
                object.insert(field, Value::String(REQUEST_ID_SENTINEL.to_owned()));
                continue;
            }
            if let Some(prop_schema) = properties.and_then(|props| props.get(&field)) {
                if let Some(default_value) = default_for_schema(prop_schema) {
                    object.insert(field, default_value);
                }
            }
        }

        // Strict wire validation via deserializing `Operation` (enforces `deny_unknown_fields`).
        let validated: Operation = serde_json::from_value(value).map_err(|err| {
            anyhow::anyhow!("line {line}: <{elem} data-lf=…> invalid `{kind}` operation: {err}")
        })?;
        serde_json::to_string(&validated).context("serialize validated Operation JSON")
    }

    fn check_data_paths(&self, input: &str) -> Result<()> {
        let re = Regex::new(r#"(?s)data-path\s*=\s*(?:'([^']*)'|"([^"]*)")"#)
            .expect("static data-path regex");
        for caps in re.captures_iter(input) {
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
            if !self.repo_root.join(&path).exists() {
                bail!("line {line}: <{elem} data-path=\"{path}\"> does not exist in repository");
            }
        }
        Ok(())
    }

    fn check_code_shas(&mut self, input: &str) -> Result<()> {
        let code_re = Regex::new(r"(?is)<code([^>]*)>(.*?)</code>").expect("static code regex");
        let token_re = Regex::new(r"\b[0-9a-fA-F]{7,40}\b").expect("static hex regex");
        for caps in code_re.captures_iter(input) {
            let full = caps.get(0).expect("full match");
            let attrs = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if attrs.to_ascii_lowercase().contains("data-no-check") {
                continue;
            }
            let body = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            // Skip nested HTML tags inside code if any, inspect plain text.
            let line = line_at(input, full.start());
            for mat in token_re.find_iter(body) {
                let candidate = mat.as_str();
                if !looks_like_git_sha(candidate) {
                    continue;
                }
                if !self.git_object_exists(candidate) {
                    bail!(
                        "line {line}: <code> contains unknown Git SHA `{candidate}` (add `data-no-check` if this is not a commit SHA)"
                    );
                }
            }
        }
        Ok(())
    }

    fn git_object_exists(&mut self, sha: &str) -> bool {
        if let Some(&exists) = self.sha_cache.get(sha) {
            return exists;
        }
        let exists = Command::new("git")
            .current_dir(self.repo_root)
            .args(["cat-file", "-e", sha])
            .output()
            .is_ok_and(|out| out.status.success());
        self.sha_cache.insert(sha.to_owned(), exists);
        exists
    }
}

fn looks_like_git_sha(token: &str) -> bool {
    (7..=40).contains(&token.len())
        && token.bytes().all(|b| b.is_ascii_hexdigit())
        && token.bytes().any(|b| b.is_ascii_digit())
        && token
            .bytes()
            .any(|b| matches!(b, b'a'..=b'f' | b'A'..=b'F'))
}

fn default_for_schema(schema: &Value) -> Option<Value> {
    if schema_allows_null(schema) {
        return Some(Value::Null);
    }
    if schema.get("type").and_then(Value::as_str) == Some("boolean") {
        return Some(Value::Bool(false));
    }
    if schema.get("type").and_then(Value::as_str) == Some("array") {
        return Some(Value::Array(Vec::new()));
    }
    None
}

fn schema_allows_null(schema: &Value) -> bool {
    if let Some(ty) = schema.get("type") {
        if ty.as_str() == Some("null") {
            return true;
        }
        if let Some(arr) = ty.as_array() {
            if arr.iter().any(|v| v.as_str() == Some("null")) {
                return true;
            }
        }
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(variants) = schema.get(key).and_then(Value::as_array) {
            if variants.iter().any(schema_allows_null) {
                return true;
            }
        }
    }
    false
}

fn validate_repo_relative(path: &str, elem: &str, line: usize) -> Result<()> {
    let p = Path::new(path);
    if p.is_absolute()
        || p.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("line {line}: <{elem} path=\"{path}\"> must be a repository-relative path");
    }
    Ok(())
}

// ── Minimal tag & attribute scanner ─────────────────────────────────────────

struct ElementMatch {
    start: usize,
    end: usize,
    attrs: BTreeMap<String, String>,
    inner: String,
}

fn find_next_element(input: &str, from: usize, tag_name: &str) -> Result<Option<ElementMatch>> {
    let open_needle = format!("<{tag_name}");
    let close_needle = format!("</{tag_name}>");
    let mut search = from;
    while let Some(rel) = input[search..].find(&open_needle) {
        let start = search + rel;
        let after_name = start + open_needle.len();
        let next_byte = input.as_bytes().get(after_name).copied();
        if !matches!(next_byte, Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')) {
            search = after_name;
            continue;
        }
        let Some((open_end, self_closing, attr_str)) = scan_opening_tag_end(input, after_name)
        else {
            let line = line_at(input, start);
            bail!("line {line}: unclosed `<{tag_name}>` opening tag");
        };
        let attrs = parse_attributes(attr_str);
        if self_closing {
            return Ok(Some(ElementMatch {
                start,
                end: open_end,
                attrs,
                inner: String::new(),
            }));
        }
        // Track nesting depth in case of nested same-named elements (e.g. components).
        let mut depth = 1usize;
        let mut scan = open_end;
        while depth > 0 {
            let next_open = find_open_tag_start(input, scan, tag_name);
            let next_close = input[scan..].find(&close_needle).map(|off| scan + off);
            match (next_open, next_close) {
                (Some(o), Some(c)) if o < c => {
                    if let Some((nested_end, nested_self_closing, _)) =
                        scan_opening_tag_end(input, o + open_needle.len())
                    {
                        if !nested_self_closing {
                            depth += 1;
                        }
                        scan = nested_end;
                    } else {
                        let line = line_at(input, o);
                        bail!("line {line}: unclosed `<{tag_name}>` opening tag");
                    }
                }
                (_, Some(c)) => {
                    depth -= 1;
                    if depth == 0 {
                        let inner = input[open_end..c].to_owned();
                        return Ok(Some(ElementMatch {
                            start,
                            end: c + close_needle.len(),
                            attrs,
                            inner,
                        }));
                    }
                    scan = c + close_needle.len();
                }
                (_, None) => {
                    let line = line_at(input, start);
                    bail!("line {line}: missing closing `</{tag_name}>` tag");
                }
            }
        }
    }
    Ok(None)
}

fn find_open_tag_start(input: &str, from: usize, tag_name: &str) -> Option<usize> {
    let needle = format!("<{tag_name}");
    let mut search = from;
    while let Some(rel) = input[search..].find(&needle) {
        let start = search + rel;
        let after = start + needle.len();
        if matches!(
            input.as_bytes().get(after).copied(),
            Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')
        ) {
            return Some(start);
        }
        search = after;
    }
    None
}

fn scan_opening_tag_end(input: &str, from: usize) -> Option<(usize, bool, &str)> {
    let bytes = input.as_bytes();
    let mut i = from;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
        } else if b == b'\'' || b == b'"' {
            quote = Some(b);
        } else if b == b'>' {
            let prev = input[from..i].trim_end();
            let self_closing = prev.ends_with('/');
            let attr_str = if self_closing {
                prev.strip_suffix('/').unwrap_or(prev).trim()
            } else {
                prev.trim()
            };
            return Some((i + 1, self_closing, attr_str));
        }
        i += 1;
    }
    None
}

fn parse_attributes(attr_str: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let re = Regex::new(r#"(?s)([a-zA-Z0-9_:-]+)\s*=\s*(?:"([^"]*)"|'([^']*)')"#)
        .expect("static attr regex");
    for caps in re.captures_iter(attr_str) {
        let key = caps[1].to_owned();
        let val = caps
            .get(2)
            .or_else(|| caps.get(3))
            .map(|m| unescape_html_attr(m.as_str()))
            .unwrap_or_default();
        map.insert(key, val);
    }
    map
}

fn enclosing_tag_name(input: &str, pos: usize) -> Option<&str> {
    let before = &input[..pos];
    let lt = before.rfind('<')?;
    let rest = before[lt + 1..].trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(rest.len());
    let name = &rest[..end];
    (!name.is_empty()).then_some(name)
}

// ── Component template rendering (`{{attr}}` + `<slot>` / `<slot name="…">`) ─

fn render_component(
    template: &str,
    attrs: &BTreeMap<String, String>,
    inner: &str,
) -> Result<String> {
    let (named_slots, default_slot) = extract_slots(inner)?;

    // Replace `{{attr}}` placeholders with HTML-escaped attribute values.
    let placeholder_re = Regex::new(r"\{\{\s*([a-zA-Z0-9_-]+)\s*\}\}").expect("static regex");
    let with_attrs = placeholder_re.replace_all(template, |caps: &regex::Captures<'_>| {
        let key = &caps[1];
        attrs
            .get(key)
            .map(|val| escape_html(val))
            .unwrap_or_default()
    });

    // Replace `<slot name="…"></slot>` and `<slot></slot>` / `<slot/>`.
    let named_re =
        Regex::new(r#"(?s)<slot\s+name\s*=\s*(?:"([^"]*)"|'([^']*)')\s*(?:/>|>\s*</slot>)"#)
            .expect("static slot regex");
    let with_named = named_re.replace_all(&with_attrs, |caps: &regex::Captures<'_>| {
        let name = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| m.as_str())
            .unwrap_or("");
        named_slots.get(name).cloned().unwrap_or_default()
    });

    let default_re = Regex::new(r"(?s)<slot\s*(?:/>|>\s*</slot>)").expect("static slot regex");
    let rendered = default_re.replace_all(&with_named, |_caps: &regex::Captures<'_>| {
        default_slot.clone()
    });
    Ok(rendered.into_owned())
}

fn extract_slots(inner: &str) -> Result<(BTreeMap<String, String>, String)> {
    let mut named: BTreeMap<String, String> = BTreeMap::new();
    let mut default_parts = String::new();
    let mut cursor = 0;

    // First handle `<slot name="X">...</slot>` elements inside component children.
    let mut remaining = String::new();
    while let Some(el) = find_next_element(inner, cursor, "slot")? {
        remaining.push_str(&inner[cursor..el.start]);
        if let Some(name) = el.attrs.get("name") {
            named.entry(name.clone()).or_default().push_str(&el.inner);
        } else {
            remaining.push_str(&el.inner);
        }
        cursor = el.end;
    }
    remaining.push_str(&inner[cursor..]);

    // Also support standard `<div slot="before">...</div>` or `<template slot="before">...`.
    let slot_attr_re =
        Regex::new(r#"(?s)<([a-zA-Z0-9_-]+)\b([^>]*\bslot\s*=\s*(?:"([^"]*)"|'([^']*)')[^>]*)>"#)
            .expect("static slot attr regex");
    let mut pos = 0;
    while let Some(caps) = slot_attr_re.captures(&remaining[pos..]) {
        let full = caps.get(0).expect("full match");
        let start = pos + full.start();
        let tag_name = &caps[1];
        let slot_name = caps
            .get(3)
            .or_else(|| caps.get(4))
            .map(|m| m.as_str())
            .unwrap_or("")
            .to_owned();
        if let Some(el) = find_next_element(&remaining, start, tag_name)? {
            default_parts.push_str(&remaining[pos..el.start]);
            if tag_name.eq_ignore_ascii_case("template") {
                named.entry(slot_name).or_default().push_str(&el.inner);
            } else {
                // Keep the element's HTML (minus the `slot="…"` attribute) inside the named slot.
                named.entry(slot_name).or_default().push_str(&el.inner);
            }
            pos = el.end;
        } else {
            default_parts.push_str(&remaining[pos..start + full.len()]);
            pos = start + full.len();
        }
    }
    default_parts.push_str(&remaining[pos..]);
    Ok((named, default_parts))
}

// ── Unified diff parsing & `<lf-diff>` / `<lf-file>` rendering ──────────────

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
    let hunk_re = Regex::new(r"^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@").expect("hunk regex");
    let mut hunks = Vec::new();
    let mut current: Option<DiffHunk> = None;
    let mut old_ln = 0usize;
    let mut new_ln = 0usize;

    for raw_line in patch.lines() {
        if let Some(caps) = hunk_re.captures(raw_line) {
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

    let range_suffix = range
        .map(|(a, b)| {
            if a == b {
                format!(":{a}")
            } else {
                format!(":{a}-{b}")
            }
        })
        .unwrap_or_default();
    let toggle = if notes.is_empty() {
        ""
    } else {
        "<button type=\"button\" class=\"lf-btn-quiet\" data-notes-toggle>Hide line notes</button>"
    };
    Ok(format!(
        "<div class=\"lf-codeblock\" data-path=\"{}\"><div class=\"lf-codeblock-header\"><span class=\"lf-codeblock-path\">{}{}</span>{}</div><div class=\"lf-codeblock-scroll\"><table class=\"lf-code-table\"><tbody>\n{}</tbody></table></div></div>",
        escape_html(path),
        escape_html(path),
        range_suffix,
        toggle,
        rows
    ))
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

    let range_suffix = range
        .map(|(a, b)| {
            if a == b {
                format!(":{a}")
            } else {
                format!(":{a}-{b}")
            }
        })
        .unwrap_or_default();
    let toggle = if notes.is_empty() {
        ""
    } else {
        "<button type=\"button\" class=\"lf-btn-quiet\" data-notes-toggle>Hide line notes</button>"
    };
    Ok(format!(
        "<div class=\"lf-codeblock\" data-path=\"{}\"><div class=\"lf-codeblock-header\"><span class=\"lf-codeblock-path\">{}{}</span>{}</div><div class=\"lf-codeblock-scroll\"><table class=\"lf-code-table\"><tbody>\n{}</tbody></table></div></div>",
        escape_html(path),
        escape_html(path),
        range_suffix,
        toggle,
        rows
    ))
}

fn extract_title(body: &str) -> Option<String> {
    let re = Regex::new(r"(?is)<h1[^>]*>(.*?)</h1>").expect("h1 regex");
    let tag_re = Regex::new(r"<[^>]+>").expect("tag strip regex");
    let caps = re.captures(body)?;
    let plain = tag_re.replace_all(&caps[1], "");
    let trimmed = plain.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_single_quoted_attr(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('\'', "&#39;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn unescape_html_attr(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::{
        check_forbidden_tags, looks_like_git_sha, output_path_for, parse_unified_diff,
        render_component, Compiler, REQUEST_ID_SENTINEL,
    };
    use std::collections::BTreeMap;
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
    fn component_rendering_escapes_attrs_and_fills_default_and_named_slots() {
        let tmpl = "<div data-t=\"{{title}}\"><slot name=\"before\"></slot>|<slot name=\"after\"></slot>|<slot></slot></div>";
        let attrs = BTreeMap::from([("title".into(), "A < B & \"C\"".into())]);
        let inner = "<div slot=\"before\">old</div><div slot=\"after\">new</div><p>body</p>";
        let rendered = render_component(tmpl, &attrs, inner).unwrap();
        assert_eq!(
            rendered,
            "<div data-t=\"A &lt; B &amp; &quot;C&quot;\">old|new|<p>body</p></div>"
        );
    }

    #[test]
    fn sha_detection_requires_digit_and_hex_letter() {
        assert!(looks_like_git_sha("2b6b03ad5e"));
        assert!(looks_like_git_sha("a1b2c3d"));
        assert!(!looks_like_git_sha("1234567")); // digits only
        assert!(!looks_like_git_sha("abcdef0"[..6].as_ref())); // too short
        assert!(!looks_like_git_sha("deadbeef")); // letters only, no digit
        assert!(!looks_like_git_sha("feature")); // non-hex
    }

    #[test]
    fn data_lf_expands_partial_task_create_into_strict_operation_and_rejects_unknown_fields() {
        let mut compiler = Compiler::new(Path::new("/tmp/repo"), "HEAD");
        let strict = compiler
            .compile_operation_json(
                r#"{"kind":"task.create","title":"Prune retry log rows"}"#,
                "button",
                12,
            )
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&strict).unwrap();
        assert_eq!(parsed["kind"], "task.create");
        assert_eq!(parsed["directory"], "/tmp/repo");
        assert_eq!(parsed["request_id"], REQUEST_ID_SENTINEL);
        assert_eq!(parsed["title"], "Prune retry log rows");
        assert_eq!(parsed["run"], false);
        assert!(parsed["wave"].is_null());

        let unknown_field = compiler
            .compile_operation_json(
                r#"{"kind":"task.create","title":"Ok","bogus":1}"#,
                "button",
                14,
            )
            .unwrap_err()
            .to_string();
        assert!(unknown_field.contains("line 14"), "{unknown_field}");
        assert!(unknown_field.contains("<button"), "{unknown_field}");

        let unknown_kind = compiler
            .compile_operation_json(r#"{"kind":"task.fly"}"#, "button", 19)
            .unwrap_err()
            .to_string();
        assert!(unknown_kind.contains("line 19"), "{unknown_kind}");
        assert!(unknown_kind.contains("task.fly"), "{unknown_kind}");
    }

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
