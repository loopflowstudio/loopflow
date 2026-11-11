//! Best-effort file reads. Setters keep using the strict parser before writing.
use std::path::Path;

use serde_yaml_ng::{Mapping, Value};
use yaml_edit::{ParseErrorKind, YamlFile, YamlNode};

pub(crate) fn parse(text: &str, path: &Path) -> Mapping {
    _parse(text, path, true).0
}

fn _parse(text: &str, path: &Path, report: bool) -> (Mapping, String) {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    loop {
        let source = lines.join("\n");
        let error = match serde_yaml_ng::from_str::<Value>(&source) {
            Ok(Value::Null) => return (Mapping::new(), source),
            Ok(Value::Mapping(value)) => return (value, source),
            Ok(_) => {
                if report {
                    warn(
                        path,
                        Some(1),
                        "skipping content that is not a configuration mapping",
                    );
                }
                return (Mapping::new(), source);
            }
            Err(error) => error,
        };
        let parsed = YamlFile::parse(&source);
        let root = parsed.tree().document().and_then(|doc| doc.as_mapping());
        let diagnostic = parsed.positioned_errors().first();
        let offset = root.as_ref().and_then(|map| {
            let node = YamlNode::Mapping(map.clone());
            _duplicate(&node)
                .or_else(|| diagnostic.and_then(|error| _malformed_start(&node, error)))
        });
        let line = root
            .is_none()
            .then(|| {
                lines
                    .iter()
                    .position(|line| _meaningful(line))
                    .map(|index| index + 1)
            })
            .flatten()
            .or_else(|| {
                offset
                    .map(|offset| yaml_edit::byte_offset_to_line_column(&source, offset).line)
                    .or_else(|| diagnostic.map(|error| error.start_position(&source).line))
                    .or_else(|| error.location().map(|location| location.line()))
            })
            .unwrap_or(1);
        let Some(start) = (0..line.min(lines.len()))
            .rev()
            .find(|index| _meaningful(&lines[*index]))
        else {
            return (Mapping::new(), source);
        };
        let indentation = _indent(&lines[start]);
        let end = (start + 1..lines.len())
            .find(|index| _meaningful(&lines[*index]) && _indent(&lines[*index]) <= indentation)
            .unwrap_or(lines.len());
        if report {
            warn(
                path,
                Some(start + 1),
                "skipping malformed YAML entry and its indented contents",
            );
        }
        for line in &mut lines[start..end] {
            line.clear();
        }
    }
}

fn _meaningful(line: &str) -> bool {
    !line.trim().is_empty() && !line.trim_start().starts_with('#')
}

fn _indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn _children(node: &YamlNode) -> Vec<YamlNode> {
    match node {
        YamlNode::Mapping(map) => map.into_iter().map(|(_, value)| value).collect(),
        YamlNode::Sequence(sequence) => sequence.into_iter().collect(),
        _ => Vec::new(),
    }
}

fn _duplicate(node: &YamlNode) -> Option<usize> {
    if let YamlNode::Mapping(map) = node {
        let mut seen = Vec::new();
        for (key, _) in map {
            if let Ok(value) = serde_yaml_ng::from_str::<Value>(&key.to_string()) {
                if seen.contains(&value) {
                    return Some(key.byte_range().start as usize);
                }
                seen.push(value);
            }
        }
    }
    _children(node).iter().find_map(_duplicate)
}

fn _malformed_start(node: &YamlNode, error: &yaml_edit::PositionedParseError) -> Option<usize> {
    let range = node.byte_range();
    if error.range.start < range.start || error.range.start > range.end {
        return None;
    }
    if let Some(offset) = _children(node)
        .iter()
        .find_map(|child| _malformed_start(child, error))
    {
        return Some(offset);
    }
    let text = node.to_string();
    let text = text.trim();
    let unclosed = (text.starts_with('[') && !text.ends_with(']'))
        || (text.starts_with('{') && !text.ends_with('}'))
        || (matches!(error.kind, ParseErrorKind::UnterminatedString) && node.is_scalar());
    unclosed.then_some(range.start as usize)
}

pub(super) fn warn(path: &Path, line: Option<usize>, reason: &str) {
    let location = line.map(|line| format!(":{line}")).unwrap_or_default();
    eprintln!("warning: {}{location}: {reason}", path.display());
}

pub(super) fn warn_setting(path: &Path, keys: &[String], reason: &str) {
    let (file, keys) = path
        .to_str()
        .and_then(|path| path.rsplit_once("#waves."))
        .map(|(file, id)| {
            let mut source_keys = vec!["waves".into(), id.into()];
            source_keys.extend_from_slice(keys);
            (std::path::PathBuf::from(file), source_keys)
        })
        .unwrap_or_else(|| (path.to_owned(), keys.to_vec()));
    let line = crate::observe::read_to_string(&file).ok().and_then(|text| {
        let (_, text) = _parse(&text, &file, false);
        let parsed = YamlFile::parse(&text);
        let root = parsed.tree().document()?.as_mapping()?;
        super::keys::spellings(
            &keys.iter().map(String::as_str).collect::<Vec<_>>(),
            super::keys::Root::General,
        )
        .into_iter()
        .find_map(|spelling| {
            let mut map = root.clone();
            for (index, name) in spelling.iter().enumerate() {
                let (key, value) = (&map).into_iter().find(|(key, _)| {
                    serde_yaml_ng::from_str::<String>(&key.to_string())
                        .ok()
                        .as_ref()
                        == Some(name)
                })?;
                if index + 1 == spelling.len() {
                    return Some(
                        yaml_edit::byte_offset_to_line_column(
                            &text,
                            key.byte_range().start as usize,
                        )
                        .line,
                    );
                }
                map = value.as_mapping()?.clone();
            }
            None
        })
    });
    warn(
        &file,
        line,
        &format!(
            "skipping {}: {reason}",
            if keys.is_empty() {
                "entry".into()
            } else {
                keys.join(".")
            }
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::parse;
    use serde_yaml_ng::Value;
    use std::path::Path;

    #[test]
    fn malformed_entries_leave_valid_settings_before_and_after_them() {
        for broken in [
            "broken line",
            "broken: [unclosed",
            "broken: \"unclosed",
            "??? junk",
            "broken\n  agent: claude:fable",
        ] {
            let text = format!("default_agent: codex\n{broken}\nchrome: true\n");
            let value = Value::Mapping(parse(&text, Path::new("config.yaml")));
            assert_eq!(value["default_agent"], "codex", "{text}");
            assert_eq!(value["chrome"], true, "{text}");
            assert!(value.get("agent").is_none(), "{text}");
        }
        let value = parse(
            "broken line\ndefault_agent: codex\n",
            Path::new("config.yaml"),
        );
        assert_eq!(value["default_agent"], "codex");
    }

    #[test]
    fn nested_malformed_entries_keep_their_siblings_and_parent() {
        let value = Value::Mapping(parse("skills:\n  kickoff:\n    agent: claude:fable\n    broken line\n  qa:\n    agent: claude:opus\nchrome: true\n", Path::new("config.yaml")));
        assert_eq!(value["skills"]["kickoff"]["agent"], "claude:fable");
        assert_eq!(value["skills"]["qa"]["agent"], "claude:opus");
        assert_eq!(value["chrome"], true);
    }

    #[test]
    fn duplicate_entries_keep_the_first_value_and_later_settings() {
        let value = parse(
            "default_agent: codex\ndefault_agent: claude\nchrome: true\n",
            Path::new("config.yaml"),
        );
        assert_eq!(value["default_agent"], "codex");
        assert_eq!(value["chrome"], true);
    }

    #[test]
    fn comments_and_valid_multiline_values_keep_yaml_semantics() {
        let text = "# ignored\ncontext:\n  docs: [\n    'a:b.md',\n    c.md\n  ]\nmessage: |\n  not a mapping: at all\n  # inside the string\nchrome: true # comment\n";
        let expected: Value = serde_yaml_ng::from_str(text).unwrap();
        assert_eq!(
            Value::Mapping(parse(text, Path::new("config.yaml"))),
            expected
        );
        let text = "message: |+\n  trailing newlines\n\n";
        assert_eq!(
            Value::Mapping(parse(text, Path::new("config.yaml"))),
            serde_yaml_ng::from_str::<Value>(text).unwrap()
        );
    }
}
