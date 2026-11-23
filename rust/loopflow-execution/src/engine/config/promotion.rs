//! Move one declaration to its immediate broader destination. Both files are
//! prepared under ordered locks before the receiving file is installed.
use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{ensure, Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use sha2::{Digest, Sha256};

use crate::config::files::{self, edit, Destination, ResolvedDestination};
use crate::config::settings::EffectiveSetting;

pub fn broader(source: &Destination) -> Option<Destination> {
    match source {
        Destination::PersonalRepository { repository }
        | Destination::SharedWave { repository, .. } => Some(Destination::SharedRepository {
            repository: repository.clone(),
        }),
        Destination::PersonalWave { repository, .. } => Some(Destination::PersonalRepository {
            repository: repository.clone(),
        }),
        _ => None,
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Availability {
    None,
    Move { destination: Destination },
    Resume,
}

pub(super) fn offer_moves(files: &mut [super::ScopeFile]) -> Result<()> {
    let directory = crate::store::lf_home_dir().join("settings-moves");
    let pending_moves: std::collections::BTreeSet<_> = match crate::observe::read_dir(&directory) {
        Ok(paths) => paths.into_iter().collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(error) => return Err(error.into()),
    };
    for index in 0..files.len() {
        let Some(destination) = broader(&files[index].destination) else {
            continue;
        };
        let Some(other) = files
            .iter()
            .position(|file| file.destination == destination)
        else {
            continue;
        };
        let keys = files[other].slots.keys().cloned().collect::<Vec<_>>();
        for key in keys {
            let pending = !pending_moves.is_empty()
                && files[index].slots.get(&key).is_some_and(|slot| {
                    pending_moves.contains(&journal(&files[index].destination, &slot.key))
                });
            let empty = matches!(files[other].slots[&key].declared, super::Authored::Absent);
            if let Some(slot) = files[index].slots.get_mut(&key) {
                if pending {
                    slot.promotion = Availability::Resume;
                } else if empty && matches!(slot.declared, super::Authored::Present { .. }) {
                    slot.promotion = Availability::Move {
                        destination: destination.clone(),
                    };
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Review {
    pub changes: Vec<FilePreview>,
    pub effective: EffectiveSetting,
    pub receipt: Receipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FilePreview {
    pub path: PathBuf,
    pub patch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Receipt {
    pub source: edit::Applied,
    pub receiving: edit::Applied,
}

impl Receipt {
    fn inverse(&self) -> Self {
        fn reverse(edit: &edit::Applied) -> edit::Applied {
            edit::Applied {
                before: edit.after.clone(),
                after: edit.before.clone(),
            }
        }
        Self {
            source: reverse(&self.source),
            receiving: reverse(&self.receiving),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Preview,
    Resume,
    Confirm { review: Review },
    Undo { receipt: Receipt },
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    Preview { review: Review },
    Applied { receipt: Receipt },
    Pending,
    Recovered,
}

#[derive(Debug, Serialize, Deserialize)]
struct Pending {
    receipt: Receipt,
    undo: bool,
}

pub(super) fn journal(source: &Destination, key: &str) -> PathBuf {
    let id = Sha256::digest(
        serde_json::to_vec(&(source, key)).expect("configuration address serializes"),
    );
    crate::store::lf_home_dir()
        .join("settings-moves")
        .join(format!("{}.json", hex::encode(id)))
}

struct Document {
    original: String,
    value: Value,
}

struct Move<'a> {
    source: &'a ResolvedDestination,
    receiving: &'a ResolvedDestination,
    key: &'a str,
    documents: BTreeMap<PathBuf, Document>,
}

impl<'a> Move<'a> {
    fn read(
        source: &'a ResolvedDestination,
        receiving: &'a ResolvedDestination,
        key: &'a str,
    ) -> Result<Self> {
        let mut documents = BTreeMap::new();
        for address in [source, receiving] {
            if let std::collections::btree_map::Entry::Vacant(entry) =
                documents.entry(address.path.clone())
            {
                let (original, value) = edit::read_document(address)?;
                entry.insert(Document { original, value });
            }
        }
        Ok(Self {
            source,
            receiving,
            key,
            documents,
        })
    }

    fn change(
        &mut self,
        address: &ResolvedDestination,
        change: &edit::Change,
    ) -> Result<edit::Applied> {
        let document = self
            .documents
            .get_mut(&address.path)
            .expect("loaded both files");
        let setting = edit::Setting::catalogued(address, self.key);
        let before = setting.entries(&document.value)?;
        setting.prepare(&mut document.value, &before, change)
    }

    fn forward(&mut self) -> Result<Receipt> {
        let declared = |address: &ResolvedDestination| {
            let raw = &self.documents[&address.path].value;
            let mut document = raw.clone();
            crate::config::keys::fold(&mut document);
            crate::config::normalize_context(&mut document);
            let setting = edit::Setting::catalogued(address, self.key);
            setting.declared(&document, &setting.entries(raw)?)
        };
        ensure!(
            declared(self.receiving)?.is_none(),
            "{} already contains {}",
            self.receiving.path.display(),
            self.key
        );
        let value =
            declared(self.source)?.context("the source declaration is absent; refresh Settings")?;
        let receiving = self.change(
            self.receiving,
            &edit::Change::Set {
                value: serde_yaml_ng::to_string(&value)?,
            },
        )?;
        let source = self.change(self.source, &edit::Change::Reset)?;
        Ok(Receipt { source, receiving })
    }

    // Recovery can see the receiving write alone. Validate both slots first,
    // including which partial state is possible, before touching either file.
    fn restore(&mut self, pending: &Pending, recovering: bool) -> Result<()> {
        let mut edits = [
            (self.receiving, &pending.receipt.receiving),
            (self.source, &pending.receipt.source),
        ];
        if pending.undo {
            edits.reverse();
        }
        let mut installed = [false; 2];
        for (i, (address, edit)) in edits.iter().enumerate() {
            let actual = edit::Setting::catalogued(address, self.key)
                .entries(&self.documents[&address.path].value)?;
            installed[i] = actual == edit.after;
            ensure!(
                actual == edit.before || (recovering && installed[i]),
                "{} changed in {}; review that file before continuing the move",
                self.key,
                address.path.display()
            );
        }
        ensure!(
            !installed[1] || installed[0],
            "receiving declaration changed in {}; review both files",
            edits[0].0.path.display()
        );
        for ((address, edit), done) in edits.into_iter().zip(installed) {
            if !done {
                self.change(
                    address,
                    &edit::Change::Restore {
                        entries: edit.after.clone(),
                    },
                )?;
            }
        }
        Ok(())
    }

    fn patches(&self) -> Result<Vec<(PathBuf, String)>> {
        self.documents
            .iter()
            .map(|(path, doc)| Ok((path.clone(), files::edited_text(&doc.original, &doc.value)?)))
            .collect()
    }

    fn install(&self, patches: &[(PathBuf, String)], undo: bool) -> Result<()> {
        let mut addresses = vec![self.receiving, self.source];
        if undo {
            addresses.reverse();
        }
        addresses.dedup_by_key(|address| &address.path);
        let mut installed: Option<(&PathBuf, &str)> = None;
        for address in addresses {
            if let Some((path, text)) = installed {
                ensure!(
                    files::read_text(path)? == text,
                    "{} changed during the move; review both files",
                    path.display()
                );
            }
            let before = &self.documents[&address.path].original;
            let (_, after) = patches
                .iter()
                .find(|(path, _)| *path == address.path)
                .expect("prepared both files");
            ensure!(
                files::settings_text(&address.path, address.root())? == *before,
                "{} changed; review the pending move",
                address.path.display()
            );
            if before != after {
                files::atomic_write(&address.path, after)?;
            }
            installed = Some((&address.path, after));
        }
        Ok(())
    }
}

pub fn run(
    source: &Destination,
    key: &str,
    wave: Option<&str>,
    action: &Action,
) -> Result<Outcome> {
    let receiving = broader(source).context("there is no broader destination")?;
    let id = journal(source, key);
    let repository = source.repository().context("select a checkout")?;
    let (source, receiving) = (source.resolve()?, receiving.resolve()?);
    for address in [&source, &receiving] {
        super::validate_edit(address, key)?;
    }
    let paths = [&source.path, &receiving.path]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    for path in &paths {
        if path.ends_with(files::PRIVATE_FILE) && !matches!(action, Action::Preview) {
            files::protect_private_file(path)?;
        }
    }
    let _locks = paths
        .into_iter()
        .map(|path| files::lock(path))
        .collect::<Result<Vec<_>>>()?;
    let mut operation = Move::read(&source, &receiving, key)?;
    let persisted = match std::fs::read(&id) {
        Ok(bytes) => Some(serde_json::from_slice::<Pending>(&bytes)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if let Some(pending) = persisted {
        if !matches!(action, Action::Resume) {
            return Ok(Outcome::Pending);
        }
        operation.restore(&pending, true)?;
        let patches = operation.patches()?;
        operation.install(&patches, pending.undo)?;
        std::fs::remove_file(&id)?;
        return Ok(Outcome::Recovered);
    }
    ensure!(
        !matches!(action, Action::Resume),
        "move already finished; refresh Settings"
    );
    let (receipt, patches, undo) = if let Action::Undo { receipt } = action {
        let inverse = receipt.inverse();
        operation.restore(
            &Pending {
                receipt: inverse.clone(),
                undo: true,
            },
            false,
        )?;
        (inverse, operation.patches()?, true)
    } else {
        let receipt = operation.forward()?;
        let patches = operation.patches()?;
        let effective = crate::observe::preview_files(&patches, || -> Result<_> {
            let resolved = crate::config::resolved(Some(repository), wave)?;
            let typed = crate::config::keys::typed_key(key);
            let parts = typed.split('.').collect::<Vec<_>>();
            let preference = crate::config::settings::preference_parts(&parts)?;
            let target = match &parts[..parts.len() - preference.len()] {
                ["skills", name] => super::ScopeTarget::Skill {
                    name: (*name).into(),
                },
                ["graphs", graph, "steps", step] => super::ScopeTarget::Step {
                    graph: (*graph).into(),
                    step: (*step).into(),
                },
                _ => super::ScopeTarget::General,
            };
            let skill = target.skill(Some(repository))?;
            let settings = match skill {
                Some(skill) => super::effective_settings(&crate::config::settings::resolve(
                    &resolved.config,
                    Some(&skill),
                )?)?,
                None => super::effective_settings(&resolved)?,
            };
            let canonical =
                crate::config::keys::canonical(preference, crate::config::keys::Root::General)
                    .join(".");
            settings.get(&canonical).cloned().context("unknown setting")
        })?;
        let changes = patches
            .iter()
            .map(|(path, after)| FilePreview {
                path: path.clone(),
                patch: edit::preview(&operation.documents[path].original, after),
            })
            .collect();
        let review = Review {
            changes,
            effective,
            receipt: receipt.clone(),
        };
        if !matches!(action, Action::Confirm {review: expected} if expected == &review) {
            return Ok(Outcome::Preview { review });
        }
        (receipt, patches, false)
    };
    if source.path != receiving.path {
        std::fs::create_dir_all(id.parent().expect("journal has a parent"))?;
        files::atomic_write(
            &id,
            &serde_json::to_string(&Pending {
                receipt: receipt.clone(),
                undo,
            })?,
        )?;
    }
    operation.install(&patches, undo)?;
    if source.path != receiving.path {
        std::fs::remove_file(id)?;
    }
    Ok(Outcome::Applied { receipt })
}

#[cfg(test)]
mod tests {
    use super::{journal, run, Action, Outcome, Pending};
    use crate::config::environment::EnvVarGuard;
    use crate::config::files::Destination;
    use crate::engine::config::{get, set};
    use loopflow_test_support::TestRepo;

    #[test]
    fn finish_an_interrupted_move_only_if_both_declarations_still_match() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = TestRepo::new();
        let mine = Destination::PersonalRepository {
            repository: repo.path().into(),
        };
        let shared = super::broader(&mine).unwrap();
        let key = "context.diff";
        set(&mine, key, "true").unwrap();
        let Outcome::Preview { review } = run(&mine, key, None, &Action::Preview).unwrap() else {
            panic!("expected preview")
        };
        let record = journal(&mine, key);
        std::fs::create_dir_all(record.parent().unwrap()).unwrap();
        std::fs::write(
            &record,
            serde_json::to_vec(&Pending {
                receipt: review.receipt.clone(),
                undo: false,
            })
            .unwrap(),
        )
        .unwrap();
        set(&shared, key, "false").unwrap();
        assert!(run(&mine, key, None, &Action::Resume).is_err());
        assert!(record.exists());
        assert_eq!(get(&mine, key).unwrap().value, true);
        set(&shared, key, "true").unwrap();
        set(&mine, "chrome", "false").unwrap();
        assert!(matches!(
            run(&mine, key, None, &Action::Resume).unwrap(),
            Outcome::Recovered
        ));
        assert!(!record.exists());
        assert!(get(&mine, key).unwrap().value.is_null());
        assert_eq!(get(&mine, "chrome").unwrap().value, false);
        run(
            &mine,
            key,
            None,
            &Action::Undo {
                receipt: review.receipt,
            },
        )
        .unwrap();
        assert!(get(&shared, key).unwrap().value.is_null());
        assert_eq!(get(&mine, key).unwrap().value, true);
    }

    #[test]
    fn moving_within_the_private_file_rechecks_the_effective_result() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = TestRepo::new();
        repo.create_file(
            ".lf/wave/design/config.yaml",
            "id: 8ae21a68-8940-4553-a3ab-135133925e35\n",
        );
        let wave = Destination::PersonalWave {
            repository: repo.path().into(),
            wave: "design".into(),
        };
        let private = super::broader(&wave).unwrap();
        let shared = super::broader(&private).unwrap();
        let key = "context.docs";
        set(&wave, key, "[wave.md]").unwrap();
        let perform = |action| run(&wave, key, Some("design"), &action).unwrap();
        let Outcome::Preview { review } = perform(Action::Preview) else {
            panic!("expected preview")
        };
        assert_eq!(review.changes.len(), 1);
        set(&shared, key, "[repo.md]").unwrap();
        let Outcome::Preview { review } = perform(Action::Confirm { review }) else {
            panic!("changed effective result needs review")
        };
        assert_eq!(
            serde_json::to_value(&review.effective.value).unwrap(),
            serde_json::json!(["repo.md", "wave.md"])
        );
        let Outcome::Applied { receipt } = perform(Action::Confirm { review }) else {
            panic!("expected move")
        };
        assert!(get(&wave, key).unwrap().value.is_null());
        set(&wave, "yolo", "false").unwrap();
        perform(Action::Undo { receipt });
        assert!(get(&private, key).unwrap().value.is_null());
        assert_eq!(get(&wave, "yolo").unwrap().value, false);
        assert_eq!(
            serde_json::to_value(get(&wave, key).unwrap().value).unwrap(),
            serde_json::json!(["wave.md"])
        );
    }
}
