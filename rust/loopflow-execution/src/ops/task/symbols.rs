use std::{path::Path, sync::LazyLock};

use grep_matcher::Matcher;
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::grep::{literal_matcher, scan_checkout, MAX_HITS};
use super::{file_context, file_store, task_error};
use crate::ops::OpsResult;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskSymbol {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub line: usize,
    #[serde(deserialize_with = "Option::deserialize")]
    pub container: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskSymbols {
    pub hits: Vec<TaskSymbol>,
    pub truncated: bool,
}

pub fn task_symbols(
    issue: &str,
    name: &str,
    path: Option<&str>,
    cancelled: &dyn Fn() -> bool,
) -> OpsResult<TaskSymbols> {
    let checkout = file_context(&file_store()?, issue)?;
    let inventory = crate::observe::checkout_files(&checkout.worktree)?;
    let mut result = TaskSymbols {
        hits: vec![],
        truncated: false,
    };
    if name.is_empty() {
        return Ok(result);
    }
    if name.contains(['\r', '\n']) {
        return Err(task_error("Search for a name on a single line."));
    }
    let path = path.map(super::validate_task_relative_path).transpose()?;
    let paths = inventory
        .paths
        .iter()
        .map(String::as_str)
        .filter(|candidate| path.as_deref().is_none_or(|p| p == *candidate));
    let whole_word = name.chars().all(|c| c.is_alphanumeric() || c == '_');
    let matcher = literal_matcher(name, whole_word)?;
    let stopped = scan_checkout(&checkout.worktree, paths, cancelled, |path, source| {
        let candidate = matcher
            .is_match(source.as_bytes())
            .expect("RegexMatcher cannot fail while matching");
        Ok(!candidate || declarations(path, source, name, &mut result.hits))
    })?;
    result.truncated = stopped || inventory.truncated;
    Ok(result)
}

/// Blanks comments and quoted text while retaining token boundaries and indentation.
#[derive(Default)]
struct CodeMask {
    block_depth: usize,
    quote: Vec<u8>,
    raw: bool,
}

impl CodeMask {
    fn line(&mut self, line: &str, ext: &str) -> String {
        let bytes = line.as_bytes();
        let mut code = bytes.to_vec();
        let mut i = 0;
        while i < bytes.len() {
            let rest = &bytes[i..];
            let start = i;
            if self.block_depth > 0 {
                if rest.starts_with(b"*/") {
                    self.block_depth -= 1;
                    i += 2;
                } else if matches!(ext, "swift" | "rs") && rest.starts_with(b"/*") {
                    self.block_depth += 1;
                    i += 2;
                } else {
                    i += 1;
                }
            } else if !self.quote.is_empty() {
                if rest.starts_with(&self.quote) {
                    i += self.quote.len();
                    self.quote.clear();
                    self.raw = false;
                } else if !self.raw && bytes[i] == b'\\' {
                    i = (i + 2).min(bytes.len());
                } else {
                    i += 1;
                }
            } else if matches!(ext, "rs" | "swift") && bytes[i] == b'#' {
                let hashes = rest.iter().take_while(|&&b| b == b'#').count();
                if rest.get(hashes) == Some(&b'"')
                    && (ext == "swift" || (i > 0 && bytes[i - 1] == b'r'))
                {
                    let width = if ext == "swift" && rest[hashes..].starts_with(b"\"\"\"") {
                        3
                    } else {
                        1
                    };
                    self.quote = vec![b'"'; width];
                    self.quote.extend_from_slice(&rest[..hashes]);
                    self.raw = true;
                    i += hashes + width;
                } else {
                    i += hashes;
                    continue;
                }
            } else if rest.starts_with(b"//") && !matches!(ext, "py" | "pyi")
                || bytes[i] == b'#' && matches!(ext, "py" | "pyi" | "rb" | "sh")
            {
                i = bytes.len();
            } else if rest.starts_with(b"/*") && !matches!(ext, "py" | "pyi") {
                self.block_depth = 1;
                i += 2;
            } else if matches!(bytes[i], b'\'' | b'"' | b'`') {
                // Rust lifetimes have no closing apostrophe.
                if ext == "rs"
                    && bytes[i] == b'\''
                    && rest
                        .get(1)
                        .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
                    && rest.get(2) != Some(&b'\'')
                {
                    i += 1;
                    continue;
                }
                let count = if rest.starts_with(b"\"\"\"") || rest.starts_with(b"'''") {
                    3
                } else {
                    1
                };
                self.quote = rest[..count].into();
                i += count;
            } else {
                i += 1;
                continue;
            }
            code[start..i].fill(b' ');
        }
        String::from_utf8(code)
            .expect("mask replaces complete quoted and commented spans with ASCII")
    }
}

static TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\p{XID_Start}_$][\p{XID_Continue}$]*|::|[^\s]").expect("token pattern")
});

fn identifier(token: &str) -> bool {
    token.starts_with(|c: char| c == '_' || c == '$' || c.is_alphabetic())
}

/// Skip type parameters and use the implementing type in "impl Trait for Type".
fn implementing_type<'a>(tokens: &[&'a str]) -> Option<&'a str> {
    let mut angles = 0usize;
    let mut first = None;
    for &token in tokens {
        match token {
            "<" => angles += 1,
            ">" => angles = angles.saturating_sub(1),
            _ if angles > 0 => {}
            "{" | "where" => break,
            "for" | "::" => first = None,
            _ if first.is_none() && identifier(token) => first = Some(token),
            _ => {}
        }
    }
    first
}

fn emit(
    hits: &mut Vec<TaskSymbol>,
    path: &str,
    name: &str,
    line: usize,
    kind: &str,
    container: Option<&str>,
) -> bool {
    if hits.len() == MAX_HITS {
        return false;
    }
    hits.push(TaskSymbol {
        path: path.into(),
        name: name.into(),
        kind: kind.into(),
        line,
        container: container.map(str::to_owned),
    });
    true
}

fn headings(path: &str, source: &str, name: &str, hits: &mut Vec<TaskSymbol>) -> bool {
    let mut scopes: Vec<(usize, String)> = Vec::new();
    let mut fence = None;
    let mut html_comment = false;
    for (row, line) in source.lines().enumerate() {
        let text = line.trim_start_matches(' ');
        if fence.is_none() && (html_comment || text.starts_with("<!--")) {
            html_comment = !text.contains("-->");
            continue;
        }
        if line.len() - text.len() > 3 {
            continue;
        }
        if let Some(marker) = text.chars().next().filter(|c| matches!(c, '`' | '~')) {
            let length = text.chars().take_while(|&c| c == marker).count();
            if length >= 3 {
                if fence.is_none() {
                    fence = Some((marker, length));
                } else if fence.is_some_and(|(m, n)| {
                    marker == m && length >= n && text[length..].trim().is_empty()
                }) {
                    fence = None;
                }
                continue;
            }
        }
        let level = text.bytes().take_while(|&b| b == b'#').count();
        if fence.is_none()
            && (1..=6).contains(&level)
            && text[level..].starts_with(char::is_whitespace)
        {
            let title = text[level..].trim();
            let unmarked = title.trim_end_matches('#');
            let title = if unmarked.ends_with(char::is_whitespace) {
                unmarked.trim_end()
            } else {
                title
            };
            scopes.retain(|(outer, _)| *outer < level);
            if title == name
                && !emit(
                    hits,
                    path,
                    name,
                    row + 1,
                    "heading",
                    scopes.last().map(|(_, s)| s.as_str()),
                )
            {
                return false;
            }
            scopes.push((level, title.into()));
        }
    }
    true
}

fn declarations(path: &str, source: &str, name: &str, hits: &mut Vec<TaskSymbol>) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if matches!(ext, "md" | "markdown" | "mdx") {
        return headings(path, source, name, hits);
    }
    let python = matches!(ext, "py" | "pyi");
    let mut mask = CodeMask::default();
    let mut scopes: Vec<(usize, String)> = Vec::new();
    let mut depth = 0usize;
    let mut pending = None;
    for (row, line) in source.lines().enumerate() {
        let code = mask.line(line, ext);
        let tokens: Vec<_> = TOKEN.find_iter(&code).map(|m| m.as_str()).collect();
        if tokens.is_empty() {
            continue;
        }
        let indent = code.len() - code.trim_start().len();
        if python {
            scopes.retain(|(outer, _)| *outer < indent);
        }
        let has_arrow = code.contains("=>");
        let has_body = tokens.contains(&"{");
        for (i, &word) in tokens.iter().enumerate() {
            match word {
                "{" => {
                    depth += 1;
                    if let Some(container) = pending.take() {
                        scopes.push((depth, container));
                    }
                }
                "}" => {
                    depth = depth.saturating_sub(1);
                    while scopes.last().is_some_and(|(d, _)| *d > depth) {
                        scopes.pop();
                    }
                    pending = None;
                }
                ";" => pending = None,
                _ => {}
            }
            let next = tokens.get(i + 1).copied();
            let mut receiver = None;
            let (kind, symbol) = match word {
                "impl" if ext == "rs" => ("impl", implementing_type(&tokens[i + 1..])),
                "func" if ext == "go" && next == Some("(") => {
                    let Some(close) = tokens[i + 2..]
                        .iter()
                        .position(|&t| t == ")")
                        .map(|n| i + 2 + n)
                    else {
                        continue;
                    };
                    receiver = tokens[i + 2..close]
                        .iter()
                        .copied()
                        .rfind(|t| identifier(t));
                    ("func", tokens.get(close + 1).copied())
                }
                "class" | "struct" | "enum" | "interface" | "protocol" | "extension" | "trait"
                | "mod" | "type" | "typealias" | "actor" | "func" | "fn" | "function" | "def" => {
                    (word, next)
                }
                "const" | "let" if tokens.get(i + 2) == Some(&"=") && has_arrow => {
                    ("function", next)
                }
                _ if matches!(
                    ext,
                    "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs"
                ) && next == Some("(")
                    && has_body
                    && (i == 0
                        || matches!(
                            tokens[i - 1],
                            "async" | "static" | "private" | "public" | "get" | "set" | "{" | "}"
                        ))
                    && !matches!(word, "if" | "for" | "switch" | "catch" | "while" | "with") =>
                {
                    ("method", Some(word))
                }
                _ => continue,
            };
            let Some(symbol) = symbol.filter(|t| identifier(t)) else {
                continue;
            };
            if symbol == name
                && kind != "impl"
                && kind != "extension"
                && !emit(
                    hits,
                    path,
                    name,
                    row + 1,
                    kind,
                    receiver.or_else(|| scopes.last().map(|(_, s)| s.as_str())),
                )
            {
                return false;
            }
            if python {
                scopes.push((indent, symbol.into()));
            } else {
                pending = Some(symbol.to_owned());
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::declarations;

    #[test]
    fn two_methods_keep_their_containers_and_skip_multiline_comments_and_strings() {
        let swift = r####"struct A {
 /* func read() {} */
 func read() {}
}
struct B {
 let s = """
 func read() {}
 """
 func read() {}
}"####;
        let python = "class A:\n # def read():\n def read(self): pass\n\nclass B:\n s = '''\n def read():\n '''\n def read(self): pass";
        let typescript = "class A {\n /* read() {} */\n read() {}\n}\nclass B {\n s = `\n read() {}\n `;\n read() {}\n}";
        for (path, source) in [("x.swift", swift), ("x.py", python), ("x.ts", typescript)] {
            let mut hits = vec![];
            assert!(declarations(path, source, "read", &mut hits));
            let locations: Vec<_> = hits
                .iter()
                .map(|h| (h.line, h.container.as_deref()))
                .collect();
            assert_eq!(locations, [(3, Some("A")), (9, Some("B"))], "{path}");
        }
    }

    #[test]
    fn a_hundred_declarations_are_complete_until_another_matches() {
        let mut hits = Vec::new();
        assert!(declarations(
            "a.rs",
            &"fn read() {}\n".repeat(100),
            "read",
            &mut hits
        ));
        assert!(declarations(
            "b.rs",
            "let raw = r##\"quoted \" fn read() {}\"##;",
            "read",
            &mut hits
        ));
        assert!(!declarations("c.rs", "fn read() {}", "read", &mut hits));
        assert_eq!(hits.len(), 100);
    }

    #[test]
    fn markdown_go_and_rust_declarations_keep_their_container() {
        for (path, source) in [
            ("x.md", "# A\n## read\n```\n# read\n```\n<!--\n# read\n-->"),
            (
                "x.rs",
                "impl<T> Convert<T> for crate::A<T> {\n fn read(&self) {} }",
            ),
            ("x.go", "package main\nfunc (a *A) read() {}"),
        ] {
            let mut hits = vec![];
            assert!(declarations(path, source, "read", &mut hits));
            assert_eq!(hits.len(), 1);
            assert_eq!((hits[0].line, hits[0].container.as_deref()), (2, Some("A")));
        }
    }

    #[test]
    fn javascript_arrows_and_unlisted_languages_declare_by_keyword() {
        for (path, source, kind) in [
            (
                "x.jsx",
                "// const read = () => {}\nconst read = async () => {}",
                "function",
            ),
            ("x.zig", "const a = 1;\npub fn read() void {}", "fn"),
        ] {
            let mut hits = vec![];
            assert!(declarations(path, source, "read", &mut hits));
            let found: Vec<_> = hits.iter().map(|h| (h.line, h.kind.as_str())).collect();
            assert_eq!(found, [(2, kind)], "{path}");
        }
    }
}
