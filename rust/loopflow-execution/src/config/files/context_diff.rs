//! The two authored spellings of context.diff. A released context list can
//! coexist with a flat diff key, so edit the scalar where the file stores it.
use std::collections::BTreeMap;

use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
pub enum Spelling {
    #[serde(rename = "diff")]
    Flat,
    #[serde(rename = "context.diff")]
    Nested,
}

impl Spelling {
    fn path(self) -> &'static [&'static str] {
        match self {
            Self::Flat => &["diff"],
            Self::Nested => &["context", "diff"],
        }
    }
}

pub type Declarations = BTreeMap<Spelling, serde_json::Value>;

pub fn declarations(document: &Value) -> Result<Declarations> {
    let mut declarations = BTreeMap::new();
    for spelling in [Spelling::Flat, Spelling::Nested] {
        if let Some(value) = spelling
            .path()
            .iter()
            .try_fold(document, |node, key| node.get(*key))
        {
            declarations.insert(spelling, serde_json::to_value(value)?);
        }
    }
    Ok(declarations)
}

pub(super) fn edit_at(document: &mut Value, prefix: &[&str], value: Option<Value>) -> Result<bool> {
    if prefix.is_empty() {
        return edit(document, value);
    }
    let target = prefix
        .iter()
        .try_fold(&*document, |node, key| node.get(*key));
    if target.is_none() && value.is_none() {
        return Ok(false);
    }
    let mut target = target
        .cloned()
        .unwrap_or_else(|| Value::Mapping(Default::default()));
    ensure!(
        target.is_mapping(),
        "targeted settings must contain a mapping"
    );
    let changed = edit(&mut target, value)?;
    if target.as_mapping().is_some_and(|map| map.is_empty()) {
        super::remove(document, prefix);
    } else {
        super::super::settings::insert(document, prefix, target);
    }
    Ok(changed)
}

fn edit(document: &mut Value, value: Option<Value>) -> Result<bool> {
    let current = declarations(document)?;
    let Some(value) = value else {
        for spelling in current.keys() {
            super::remove(document, spelling.path());
        }
        return Ok(!current.is_empty());
    };
    // Keep a shadowed flat declaration in place when the nested one wins.
    let spelling = if current.contains_key(&Spelling::Nested) {
        Spelling::Nested
    } else if current.contains_key(&Spelling::Flat) || document["context"].is_sequence() {
        Spelling::Flat
    } else {
        Spelling::Nested
    };
    write(document, spelling, value)?;
    Ok(true)
}

fn write(document: &mut Value, spelling: Spelling, value: Value) -> Result<()> {
    if spelling == Spelling::Nested {
        ensure!(
            document.get("context").is_none_or(Value::is_mapping),
            "context changed shape; refresh before editing Diff"
        );
    }
    super::super::settings::validate_value(&["context", "diff"], &value)?;
    super::super::settings::insert(document, spelling.path(), value);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    Set { value: bool },
    Reset,
    Restore { declarations: Declarations },
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Applied {
    pub before: Declarations,
    pub after: Declarations,
}

/// Preview from the same in-memory edit and YAML patcher used under the lock.
pub fn inspect(path: &std::path::Path) -> Result<(Declarations, String)> {
    let original = super::read_text(path)?;
    let mut document = super::parse_document(&original, path)?;
    let diff = declarations(&document)?;
    if diff.is_empty() {
        return Ok((diff, String::new()));
    }
    edit(&mut document, None)?;
    let after = super::edited_text(&original, &document)?;
    let old: Vec<_> = original.lines().collect();
    let new: Vec<_> = after.lines().collect();
    let start = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let end = old[start..]
        .iter()
        .rev()
        .zip(new[start..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let preview = old[start..old.len() - end]
        .iter()
        .map(|line| format!("- {line}"))
        .chain(
            new[start..new.len() - end]
                .iter()
                .map(|line| format!("+ {line}")),
        )
        .collect::<Vec<_>>()
        .join("\n");
    Ok((diff, preview))
}

pub fn apply(path: &std::path::Path, expected: &Declarations, change: &Change) -> Result<Applied> {
    super::update(path, |document| {
        let before = declarations(document)?;
        ensure!(
            &before == expected,
            "Diff changed in {}; refresh before editing it again",
            path.display()
        );
        match change {
            Change::Set { value } => {
                edit(document, Some(Value::Bool(*value)))?;
            }
            Change::Reset => {
                edit(document, None)?;
            }
            Change::Restore { declarations } => {
                for spelling in before.keys().filter(|key| !declarations.contains_key(key)) {
                    super::remove(document, spelling.path());
                }
                for (&spelling, value) in declarations {
                    write(document, spelling, serde_yaml_ng::to_value(value)?)?;
                }
            }
        }
        Ok(Applied {
            before,
            after: declarations(document)?,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{apply, inspect, Change};
    use crate::config::files::Destination;
    use crate::engine::config::{get, set, unset};

    #[test]
    fn editing_legacy_diff_keeps_the_document_list_and_its_comment() {
        let repo = tempfile::tempdir().unwrap();
        let shared = Destination::SharedRepository {
            repository: repo.path().into(),
        };
        let path = shared.resolve().unwrap().path;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let before = "diff: true\ncontext: [README.md] # always include\n";
        std::fs::write(&path, before).unwrap();
        assert_eq!(get(&shared, "context.diff").unwrap().value, true);
        set(&shared, "context.diff", "false").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            before.replace("true", "false")
        );
        unset(&shared, "context.diff").unwrap();
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "context: [README.md] # always include\n"
        );
    }

    #[test]
    fn undo_restores_both_spellings_and_keeps_unrelated_hand_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        std::fs::write(
            &path,
            "diff: false\ncontext:\n  docs: [README.md]\n  diff: true\n",
        )
        .unwrap();
        let (before, _) = inspect(&path).unwrap();
        let reset = apply(&path, &before, &Change::Reset).unwrap();
        assert!(reset.after.is_empty());
        std::fs::write(&path, "context:\n  docs: [guide.md] # changed externally\n").unwrap();
        apply(
            &path,
            &reset.after,
            &Change::Restore {
                declarations: reset.before,
            },
        )
        .unwrap();
        assert_eq!(inspect(&path).unwrap().0, before);
        assert!(std::fs::read_to_string(path)
            .unwrap()
            .contains("docs: [guide.md] # changed externally"));
    }
}
