//! Move one declaration to its immediate broader destination. Both files are
//! prepared under ordered locks before the receiving file is installed.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use sha2::{Digest, Sha256};

use crate::config::files::{self, edit, Destination, ResolvedDestination};
use crate::config::settings::EffectiveSetting;

fn broader(source: &Destination) -> Option<Destination> {
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
    let pending_moves: BTreeSet<_> = match crate::observe::read_dir(&directory) {
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
        let [source, receiving] = files
            .get_disjoint_mut([index, other])
            .expect("a broader destination is distinct");
        for (key, slot) in &mut source.slots {
            let Some(broader_slot) = receiving.slots.get(key) else {
                continue;
            };
            if !pending_moves.is_empty()
                && pending_moves.contains(&journal(&source.destination, &slot.key))
            {
                slot.promotion = Availability::Resume;
            } else if matches!(broader_slot.declared, super::Authored::Absent)
                && matches!(slot.declared, super::Authored::Present { .. })
            {
                slot.promotion = Availability::Move {
                    destination: destination.clone(),
                };
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
    Dismiss,
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
    Dismissed,
}

#[derive(Debug, Serialize, Deserialize)]
struct Pending {
    receipt: Receipt,
    undo: bool,
}

fn journal(source: &Destination, key: &str) -> PathBuf {
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
            crate::config::normalize_preferences(&mut document);
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

    fn review(
        &self,
        repository: &Path,
        wave: Option<&str>,
        patches: &[(PathBuf, String)],
        receipt: Receipt,
    ) -> Result<Review> {
        let effective = crate::observe::preview_files(patches, || {
            super::scope::projected_setting(repository, wave, self.key)
        })?;
        let changes = patches
            .iter()
            .map(|(path, after)| FilePreview {
                path: path.clone(),
                patch: edit::preview(&self.documents[path].original, after),
            })
            .collect();
        Ok(Review {
            changes,
            effective,
            receipt,
        })
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

fn remove_journal(path: &Path) -> Result<()> {
    std::fs::remove_file(path)?;
    crate::observe::invalidate_path(path);
    Ok(())
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
        .collect::<BTreeSet<_>>();
    for path in &paths {
        if path.ends_with(files::PRIVATE_FILE)
            && !matches!(action, Action::Preview | Action::Dismiss)
        {
            files::protect_private_file(path)?;
        }
    }
    let _locks = paths
        .into_iter()
        .map(|path| files::lock(path))
        .collect::<Result<Vec<_>>>()?;
    if matches!(action, Action::Dismiss) {
        remove_journal(&id)?;
        return Ok(Outcome::Dismissed);
    }
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
        remove_journal(&id)?;
        return Ok(Outcome::Recovered);
    }
    let (pending, patches) = match action {
        Action::Dismiss | Action::Resume => {
            anyhow::bail!("move already finished; refresh Settings")
        }
        Action::Undo { receipt } => {
            let pending = Pending {
                receipt: receipt.inverse(),
                undo: true,
            };
            operation.restore(&pending, false)?;
            (pending, operation.patches()?)
        }
        Action::Preview | Action::Confirm { .. } => {
            let receipt = operation.forward()?;
            let patches = operation.patches()?;
            let review = operation.review(repository, wave, &patches, receipt)?;
            if !matches!(action, Action::Confirm {review: expected} if expected == &review) {
                return Ok(Outcome::Preview { review });
            }
            (
                Pending {
                    receipt: review.receipt,
                    undo: false,
                },
                patches,
            )
        }
    };
    let across_files = source.path != receiving.path;
    if across_files {
        std::fs::create_dir_all(id.parent().expect("journal has a parent"))?;
        files::atomic_write(&id, &serde_json::to_string(&pending)?)?;
    }
    operation.install(&patches, pending.undo)?;
    if across_files {
        remove_journal(&id)?;
    }
    Ok(Outcome::Applied {
        receipt: pending.receipt,
    })
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
        let journal_bytes = std::fs::read(&record).unwrap();
        let contents =
            || [&mine, &shared].map(|file| std::fs::read(file.resolve().unwrap().path).unwrap());
        let diverged = contents();
        assert!(matches!(
            run(&mine, key, None, &Action::Dismiss).unwrap(),
            Outcome::Dismissed
        ));
        assert!(!record.exists());
        assert_eq!(contents(), diverged);
        std::fs::write(&record, journal_bytes).unwrap();
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
