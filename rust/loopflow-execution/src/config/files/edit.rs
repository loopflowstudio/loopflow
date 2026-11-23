//! A checked edit carries the authored YAML entries its writer can change.
//! Paths stay in components, including literal keys containing dots.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use super::{parse_document, remove, ResolvedDestination};
use crate::config::{
    keys::{self, Root},
    settings,
};
use crate::engine::{Error, ErrorCode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Entry {
    pub path: Vec<String>,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    Set { value: String },
    Reset,
    Restore { entries: Vec<Entry> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Applied {
    pub before: Vec<Entry>,
    pub after: Vec<Entry>,
}

fn at<'a>(node: &'a Value, path: &[impl AsRef<str>]) -> Option<&'a Value> {
    path.iter()
        .try_fold(node, |node, key| node.get(key.as_ref()))
}

fn refs(path: &[String]) -> Vec<&str> {
    path.iter().map(String::as_str).collect()
}

// A released context list coexists with flat fields. Setting docs replaces
// both contributions to that list. Other fields leave the list untouched.
pub(super) fn context_field(
    document: &mut Value,
    written: &[&str],
    preference: &[&str],
    value: Option<Value>,
    root: Root,
) -> Result<bool> {
    let prefix = &written[..written.len() - preference.len()];
    let context = [prefix, &["context"]].concat();
    let list = at(document, &context).is_some_and(Value::is_sequence);
    let paths: Vec<_> = keys::spellings(preference, root)
        .into_iter()
        .map(|path| [prefix.iter().map(|s| (*s).to_owned()).collect(), path].concat())
        .collect();
    let existing = paths.iter().find(|path| at(document, path).is_some());
    if let Some(value) = value {
        settings::validate_value(preference, &value)?;
        if list && preference == ["context", "docs"] {
            for path in &paths {
                remove(document, &refs(path));
            }
            settings::insert(document, &context, value);
        } else {
            let path = existing.unwrap_or_else(|| {
                if list {
                    paths.last().expect("flat context field")
                } else {
                    &paths[0]
                }
            });
            insert(document, &refs(path), value)?;
        }
        Ok(true)
    } else {
        let mut removed = false;
        for path in &paths {
            removed |= remove(document, &refs(path));
        }
        if list && preference == ["context", "docs"] {
            removed |= remove(document, &context);
        }
        Ok(removed)
    }
}

fn insert(document: &mut Value, path: &[&str], value: Value) -> Result<()> {
    for length in 0..path.len() {
        ensure!(
            at(document, &path[..length]).is_none_or(Value::is_mapping),
            Error::conflict(format!(
                "{} changed shape; refresh before editing",
                path[..length].join(".")
            ))
        );
    }
    settings::insert(document, path, value);
    Ok(())
}

#[derive(Debug)]
pub struct Setting<'a> {
    destination: &'a ResolvedDestination,
    key: &'a str,
    path: Vec<String>,
    prefix: Vec<String>,
    root: Root,
}

impl<'a> Setting<'a> {
    pub fn catalogued(destination: &'a ResolvedDestination, key: &'a str) -> Self {
        let typed = keys::typed(&key.split('.').collect::<Vec<_>>());
        let target_len = match refs(&typed).as_slice() {
            ["skills", _, ..] => 2,
            ["graphs", _, "steps", _, ..] => 4,
            _ => 0,
        };
        let prefix = [destination.prefix.clone(), typed[..target_len].to_owned()].concat();
        let path = [destination.prefix.clone(), typed].concat();
        let root = if destination.prefix.is_empty() {
            destination.root()
        } else {
            Root::Wave
        };
        Self {
            destination,
            key,
            path,
            prefix,
            root,
        }
    }

    fn paths(&self, document: &Value) -> Vec<Vec<String>> {
        let key = &self.path[self.prefix.len()..];
        let mut paths = Vec::new();
        for prefix in self.prefixes() {
            let node = at(document, &prefix).unwrap_or(&Value::Null);
            let mut tails = keys::spellings(&refs(key), self.root);
            if key == ["planning"] {
                tails.push(vec!["linear".into(), "team".into()]);
            }
            if key == ["context", "docs"] && node.get("context").is_some_and(Value::is_sequence) {
                tails.push(vec!["context".into()]);
            }
            if key == ["context"] {
                tails.extend(
                    keys::CONTEXT_ALIASES
                        .iter()
                        .map(|(old, _)| vec![(*old).into()]),
                );
            }
            paths.extend(
                tails
                    .into_iter()
                    .map(|tail| [prefix.clone(), tail].concat()),
            );
        }
        paths
    }

    fn prefixes(&self) -> Vec<Vec<String>> {
        keys::spellings(&refs(&self.prefix), Root::General)
    }

    pub fn entries(&self, document: &Value) -> Result<Vec<Entry>> {
        self.paths(document)
            .into_iter()
            .filter_map(|path| {
                at(document, &path).map(|value| {
                    Ok(Entry {
                        path,
                        value: serde_json::to_value(value)?,
                    })
                })
            })
            .collect()
    }

    pub fn declared(&self, normalized: &Value, entries: &[Entry]) -> Result<Option<Value>> {
        let Some(first) = entries.first() else {
            return Ok(None);
        };
        if self.key == "repo.tracker" {
            if let Some(node) = at(normalized, &self.prefix) {
                return Ok(Some(keys::tracker_value(node)));
            }
        }
        match at(normalized, &self.path) {
            Some(value) => Ok(Some(value.clone())),
            None => Ok(Some(serde_yaml_ng::to_value(&first.value)?)),
        }
    }

    pub(super) fn edit(&self, document: &mut Value, text: Option<&str>) -> Result<()> {
        if self.prefix.is_empty() {
            return self.edit_block(document, text);
        }
        let prefixes = self.prefixes();
        let selected = prefixes
            .iter()
            .find(|path| at(document, path).is_some())
            .unwrap_or(&self.prefix);
        for prefix in &prefixes {
            let path = refs(prefix);
            let existing = at(document, &path);
            let value = text.filter(|_| prefix == selected);
            if existing.is_none() && value.is_none() {
                continue;
            }
            let mut block = existing
                .cloned()
                .unwrap_or_else(|| Value::Mapping(Default::default()));
            ensure!(
                block.is_mapping(),
                "settings at {} must be a mapping",
                path.join(".")
            );
            self.edit_block(&mut block, value)?;
            if block.as_mapping().is_some_and(|map| map.is_empty()) {
                remove(document, &path);
            } else {
                insert(document, &path, block)?;
            }
        }
        Ok(())
    }

    fn edit_block(&self, document: &mut Value, text: Option<&str>) -> Result<()> {
        if self.key == "repo.tracker" {
            keys::gather(document, &["planning"], self.root);
            return keys::edit_tracker(
                document,
                &[],
                text.map(serde_yaml_ng::from_str).transpose()?,
            );
        }
        let parsed = text
            .map(|text| self.destination.parse_value(self.key, text))
            .transpose()?;
        let key = &self.path[self.prefix.len()..];
        super::edit_document(document, &refs(key), parsed, self.root)?;
        Ok(())
    }

    pub fn apply(&self, expected: &[Entry], change: &Change) -> Result<Applied> {
        super::update_settings(
            &self.destination.path,
            self.destination.root(),
            |document| {
                self.prepare(document, expected, change)
                    .map_err(|error| Error::refused(&error, ErrorCode::Invalid).into())
            },
        )
    }

    pub fn prepare(
        &self,
        document: &mut Value,
        expected: &[Entry],
        change: &Change,
    ) -> Result<Applied> {
        let before = self.entries(document)?;
        ensure!(
            before == expected,
            Error::conflict(format!(
                "{} changed in {}; refresh before editing",
                self.key,
                self.destination.path.display()
            ))
        );
        match change {
            Change::Set { value } => self.edit(document, Some(value))?,
            Change::Reset => self.edit(document, None)?,
            Change::Restore { entries } => {
                let mut restored = Value::Mapping(Default::default());
                for entry in entries {
                    insert(
                        &mut restored,
                        &refs(&entry.path),
                        serde_yaml_ng::to_value(&entry.value)?,
                    )?;
                }
                ensure!(
                    self.entries(&restored)? == *entries,
                    "restore contains another setting"
                );
                for entry in &before {
                    remove(document, &refs(&entry.path));
                }
                for entry in entries {
                    ensure!(
                        at(document, &entry.path).is_none(),
                        Error::conflict(format!(
                            "another declaration occupies {}; refresh before restoring",
                            entry.path.join(".")
                        ))
                    );
                    insert(
                        document,
                        &refs(&entry.path),
                        serde_yaml_ng::to_value(&entry.value)?,
                    )?;
                }
            }
        }
        Ok(Applied {
            before,
            after: self.entries(document)?,
        })
    }
}

pub fn preview(original: &str, after: &str) -> String {
    let old: Vec<_> = original.lines().collect();
    let new: Vec<_> = after.lines().collect();
    let start = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let end = old[start..]
        .iter()
        .rev()
        .zip(new[start..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    old[start..old.len() - end]
        .iter()
        .map(|s| format!("- {s}"))
        .chain(new[start..new.len() - end].iter().map(|s| format!("+ {s}")))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn read_document(destination: &ResolvedDestination) -> Result<(String, Value)> {
    let path = &destination.path;
    let text = super::settings_text(path, destination.root())?;
    let document =
        parse_document(&text, path).with_context(|| format!("Read {}", path.display()))?;
    Ok((text, document))
}

#[cfg(test)]
mod tests {
    use super::{read_document, Change, Setting};
    use crate::config::files::Destination;
    use crate::engine::config::{get, set, unset};

    #[test]
    fn declared_settings_distinguish_authored_empty_from_derived_defaults() {
        let repo = tempfile::tempdir().unwrap();
        let address = Destination::SharedRepository {
            repository: repo.path().into(),
        }
        .resolve()
        .unwrap();
        let raw: serde_yaml_ng::Value =
            serde_yaml_ng::from_str("graph: auto\ncontext: {}\n").unwrap();
        let mut normalized = raw.clone();
        crate::config::normalize_preferences(&mut normalized);
        let auto = Setting::catalogued(&address, "auto");
        assert!(auto
            .declared(&normalized, &auto.entries(&raw).unwrap())
            .unwrap()
            .is_none());
        let context = Setting::catalogued(&address, "context");
        assert_eq!(
            context
                .declared(&normalized, &context.entries(&raw).unwrap())
                .unwrap(),
            Some(serde_yaml_ng::Value::Mapping(Default::default()))
        );
    }

    #[test]
    fn legacy_fields_edit_in_place_and_undo_restores_every_spelling() {
        let repo = tempfile::tempdir().unwrap();
        let shared = Destination::SharedRepository {
            repository: repo.path().into(),
        };
        let address = shared.resolve().unwrap();
        let path = &address.path;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = "diff: true\ncontext: [README.md] # always include\n";
        std::fs::write(path, original).unwrap();
        set(&shared, "context.diff", "false").unwrap();
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            original.replace("true", "false")
        );
        unset(&shared, "context.diff").unwrap();
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "context: [README.md] # always include\n"
        );
        std::fs::write(
            path,
            "diff: false\ncontext:\n  diff: true\n  docs: [README.md]\n",
        )
        .unwrap();
        let diff = Setting::catalogued(&address, "context.diff");
        let before = diff.entries(&read_document(&address).unwrap().1).unwrap();
        let reset = diff.apply(&before, &Change::Reset).unwrap();
        assert!(reset.after.is_empty());
        std::fs::write(path, "context:\n  docs: [guide.md] # changed externally\n").unwrap();
        diff.apply(
            &reset.after,
            &Change::Restore {
                entries: reset.before,
            },
        )
        .unwrap();
        assert_eq!(
            diff.entries(&read_document(&address).unwrap().1).unwrap(),
            before
        );
        assert_eq!(get(&shared, "context.diff").unwrap().value, true);
        assert!(std::fs::read_to_string(path)
            .unwrap()
            .contains("docs: [guide.md] # changed externally"));
    }
}
