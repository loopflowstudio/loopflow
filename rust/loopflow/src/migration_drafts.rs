use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationDraft {
    pub id: &'static str,
    pub name: &'static str,
    pub dependencies: &'static [&'static str],
    pub sql: &'static str,
    pub checksum: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftMigration {
    pub id: String,
    pub name: String,
    pub dependencies: Vec<String>,
    pub sql: String,
    pub checksum: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DraftManifestErrorCategory {
    Io,
    InvalidFilename,
    ReservedMarker,
    MissingName,
    NameMismatch,
    MissingId,
    InvalidId,
    IdMismatch,
    DuplicateName,
    ReleasedNameCollision,
    SelfDependency,
    MissingDependency,
    Cycle,
}

impl DraftManifestErrorCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Io => "io",
            Self::InvalidFilename => "invalid_filename",
            Self::ReservedMarker => "reserved_marker",
            Self::MissingName => "missing_name",
            Self::NameMismatch => "name_mismatch",
            Self::MissingId => "missing_id",
            Self::InvalidId => "invalid_id",
            Self::IdMismatch => "id_mismatch",
            Self::DuplicateName => "duplicate_name",
            Self::ReleasedNameCollision => "released_name_collision",
            Self::SelfDependency => "self_dependency",
            Self::MissingDependency => "missing_dependency",
            Self::Cycle => "cycle",
        }
    }
}

#[derive(Debug)]
pub struct DraftManifestError {
    pub category: DraftManifestErrorCategory,
    message: String,
}

impl DraftManifestError {
    fn new(category: DraftManifestErrorCategory, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
        }
    }
}

impl fmt::Display for DraftManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "draft manifest [{}]: {}",
            self.category.as_str(),
            self.message
        )
    }
}

impl std::error::Error for DraftManifestError {}

pub fn read_draft_manifest(
    drafts_dir: &Path,
    released_names: &BTreeSet<String>,
) -> Result<Vec<DraftMigration>, DraftManifestError> {
    let entries = match fs::read_dir(drafts_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(DraftManifestError::new(
                DraftManifestErrorCategory::Io,
                format!("read {}: {error}", drafts_dir.display()),
            ));
        }
    };
    let mut paths = entries
        .map(|entry| {
            entry.map(|entry| entry.path()).map_err(|error| {
                DraftManifestError::new(
                    DraftManifestErrorCategory::Io,
                    format!("read entry from {}: {error}", drafts_dir.display()),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();

    let mut drafts = Vec::new();
    for path in paths {
        if path.is_dir() || path.extension().is_none_or(|extension| extension != "sql") {
            continue;
        }
        let filename = path
            .file_name()
            .and_then(|filename| filename.to_str())
            .ok_or_else(|| {
                DraftManifestError::new(
                    DraftManifestErrorCategory::InvalidFilename,
                    format!("{} has a non-UTF-8 filename", path.display()),
                )
            })?;
        let (name, file_id) = parse_draft_filename(filename).ok_or_else(|| {
            DraftManifestError::new(
                DraftManifestErrorCategory::InvalidFilename,
                format!(
                    "draft {filename} is not `<snake_case_name>__<id>.sql` — run scripts/new_migration.py"
                ),
            )
        })?;
        let text = fs::read_to_string(&path).map_err(|error| {
            DraftManifestError::new(
                DraftManifestErrorCategory::Io,
                format!("read {}: {error}", path.display()),
            )
        })?;
        if text.lines().any(|line| {
            header_value(line, "draft").is_some_and(|value| is_snake_name(value.trim()))
        }) {
            return Err(DraftManifestError::new(
                DraftManifestErrorCategory::ReservedMarker,
                format!("draft {filename} uses reserved `-- draft:` release provenance"),
            ));
        }
        let header_name = find_header(&text, "name").ok_or_else(|| {
            DraftManifestError::new(
                DraftManifestErrorCategory::MissingName,
                format!("draft {filename} has no `-- name:` header"),
            )
        })?;
        if !is_snake_name(header_name) {
            return Err(DraftManifestError::new(
                DraftManifestErrorCategory::MissingName,
                format!("draft {filename} has no valid `-- name:` header"),
            ));
        }
        if header_name != name {
            return Err(DraftManifestError::new(
                DraftManifestErrorCategory::NameMismatch,
                format!("draft {filename} header names {header_name:?}, not {name:?}"),
            ));
        }
        let header_id = find_header(&text, "id").ok_or_else(|| {
            DraftManifestError::new(
                DraftManifestErrorCategory::MissingId,
                format!("draft {filename} has no `-- id:` header"),
            )
        })?;
        if !is_draft_id(header_id) {
            return Err(DraftManifestError::new(
                DraftManifestErrorCategory::InvalidId,
                format!(
                    "draft {filename} id {header_id:?} is not a 128-bit token (32 hex chars) — run scripts/new_migration.py"
                ),
            ));
        }
        if header_id != file_id {
            return Err(DraftManifestError::new(
                DraftManifestErrorCategory::IdMismatch,
                format!("draft {filename} header id {header_id:?} disagrees with its filename"),
            ));
        }
        let dependencies = find_header(&text, "depends_on")
            .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("none"))
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let sql = draft_body(&text);
        let checksum = hex::encode(Sha256::digest(sql.as_bytes()));
        drafts.push(DraftMigration {
            id: file_id.to_string(),
            name: name.to_string(),
            dependencies,
            sql,
            checksum,
        });
    }

    order_drafts(drafts, released_names)
}

pub fn read_released_names(migrations_dir: &Path) -> Result<BTreeSet<String>, DraftManifestError> {
    let entries = match fs::read_dir(migrations_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(error) => {
            return Err(DraftManifestError::new(
                DraftManifestErrorCategory::Io,
                format!("read {}: {error}", migrations_dir.display()),
            ));
        }
    };
    let mut names = BTreeSet::new();
    for entry in entries {
        let path = entry
            .map_err(|error| {
                DraftManifestError::new(
                    DraftManifestErrorCategory::Io,
                    format!("read entry from {}: {error}", migrations_dir.display()),
                )
            })?
            .path();
        let Some(filename) = path.file_name().and_then(|filename| filename.to_str()) else {
            continue;
        };
        let Some((legacy_name, release_batch)) = parse_migration_filename(filename) else {
            continue;
        };
        if let Some(name) = legacy_name {
            names.insert(name.to_string());
            continue;
        }
        if release_batch {
            let text = fs::read_to_string(&path).map_err(|error| {
                DraftManifestError::new(
                    DraftManifestErrorCategory::Io,
                    format!("read {}: {error}", path.display()),
                )
            })?;
            names.extend(text.lines().filter_map(|line| {
                let value = header_value(line, "draft")?.trim();
                is_snake_name(value).then(|| value.to_string())
            }));
        }
    }
    Ok(names)
}

fn order_drafts(
    drafts: Vec<DraftMigration>,
    released_names: &BTreeSet<String>,
) -> Result<Vec<DraftMigration>, DraftManifestError> {
    let mut by_name = BTreeMap::new();
    for draft in drafts {
        let name = draft.name.clone();
        if by_name.insert(name.clone(), draft).is_some() {
            return Err(DraftManifestError::new(
                DraftManifestErrorCategory::DuplicateName,
                format!(
                    "two drafts share the readable name {name:?} in this cut — rename one before releasing"
                ),
            ));
        }
    }
    for draft in by_name.values() {
        if released_names.contains(&draft.name) {
            return Err(DraftManifestError::new(
                DraftManifestErrorCategory::ReleasedNameCollision,
                format!(
                    "draft {} collides with a released migration of the same name",
                    draft.name
                ),
            ));
        }
        for dependency in &draft.dependencies {
            if dependency == &draft.name {
                return Err(DraftManifestError::new(
                    DraftManifestErrorCategory::SelfDependency,
                    format!("draft {} depends on itself", draft.name),
                ));
            }
            if !by_name.contains_key(dependency) && !released_names.contains(dependency) {
                return Err(DraftManifestError::new(
                    DraftManifestErrorCategory::MissingDependency,
                    format!(
                        "draft {} depends on {dependency:?}, which is neither a draft in this cut nor an already-released migration",
                        draft.name
                    ),
                ));
            }
        }
    }

    let mut indegree = by_name
        .values()
        .map(|draft| {
            let dependencies = draft
                .dependencies
                .iter()
                .filter(|dependency| by_name.contains_key(*dependency))
                .collect::<BTreeSet<_>>();
            (draft.name.clone(), dependencies.len())
        })
        .collect::<BTreeMap<_, _>>();
    let mut dependents = by_name
        .keys()
        .map(|name| (name.clone(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for draft in by_name.values() {
        for dependency in draft.dependencies.iter().collect::<BTreeSet<_>>() {
            if let Some(names) = dependents.get_mut(dependency) {
                names.insert(draft.name.clone());
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(name, _)| name.clone())
        .collect::<BTreeSet<_>>();
    let mut ordered_names = Vec::new();
    while let Some(name) = ready.pop_first() {
        ordered_names.push(name.clone());
        for dependent in &dependents[&name] {
            let degree = indegree
                .get_mut(dependent)
                .expect("every dependent is a draft");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(dependent.clone());
            }
        }
    }
    if ordered_names.len() != by_name.len() {
        let stuck = by_name
            .keys()
            .filter(|name| !ordered_names.contains(name))
            .cloned()
            .collect::<Vec<_>>();
        return Err(DraftManifestError::new(
            DraftManifestErrorCategory::Cycle,
            format!(
                "draft dependencies form a cycle among: {}",
                stuck.join(", ")
            ),
        ));
    }
    Ok(ordered_names
        .into_iter()
        .map(|name| {
            by_name
                .remove(&name)
                .expect("ordered draft came from the manifest")
        })
        .collect())
}

fn parse_draft_filename(filename: &str) -> Option<(&str, &str)> {
    let stem = filename.strip_suffix(".sql")?;
    let split = stem.len().checked_sub(34)?;
    (stem.get(split..split + 2)? == "__").then_some(())?;
    let name = stem.get(..split)?;
    let id = stem.get(split + 2..)?;
    (is_snake_name(name) && is_draft_id(id)).then_some((name, id))
}

fn parse_migration_filename(filename: &str) -> Option<(Option<&str>, bool)> {
    let stem = filename.strip_suffix(".sql")?;
    let (version, name) = stem.split_once('_')?;
    if name.is_empty()
        || !name.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return None;
    }
    let numbers = version.split('.').collect::<Vec<_>>();
    if !matches!(numbers.len(), 3 | 4)
        || numbers.iter().any(|number| {
            number.is_empty() || !number.chars().all(|character| character.is_ascii_digit())
        })
        || numbers.last().is_none_or(|ordinal| ordinal.len() != 3)
    {
        return None;
    }
    match numbers.len() {
        3 => Some((Some(name), false)),
        4 => Some((None, true)),
        _ => None,
    }
}

fn find_header<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|line| header_value(line, key).map(str::trim))
}

fn header_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line
        .strip_prefix("--")?
        .trim_start_matches([' ', '\t'])
        .strip_prefix(key)?
        .strip_prefix(':')?;
    Some(rest.trim_start_matches([' ', '\t']))
}

fn draft_body(text: &str) -> String {
    let body = text
        .lines()
        .filter(|line| {
            !["name", "id", "depends_on"]
                .iter()
                .any(|key| header_value(line, key).is_some())
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .to_string();
    if body.is_empty() {
        body
    } else {
        body + "\n"
    }
}

fn is_snake_name(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_lowercase())
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn is_draft_id(value: &str) -> bool {
    value.len() == 32
        && value
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    use serde::Deserialize;

    use super::{read_draft_manifest, read_released_names, DraftMigration};

    #[derive(Debug, Deserialize)]
    struct Fixture {
        cases: Vec<Case>,
    }

    #[derive(Debug, Deserialize)]
    struct Case {
        name: String,
        drafts: Vec<File>,
        #[serde(default)]
        released: Vec<File>,
        expected: Option<Vec<ExpectedDraft>>,
        error: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct File {
        filename: String,
        text: String,
    }

    #[derive(Debug, Deserialize)]
    struct ExpectedDraft {
        id: String,
        name: String,
        dependencies: Vec<String>,
        sql: String,
        checksum: String,
    }

    #[test]
    fn draft_manifest_matches_shared_golden_cases() {
        let fixture: Fixture = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/migrations/draft_manifest.json"
        )))
        .unwrap();

        for case in fixture.cases {
            let directory = tempfile::tempdir().unwrap();
            let migrations_dir = directory.path().join("migrations");
            let drafts_dir = migrations_dir.join("drafts");
            fs::create_dir_all(&drafts_dir).unwrap();
            write_files(&drafts_dir, &case.drafts);
            write_files(&migrations_dir, &case.released);
            let released = read_released_names(&migrations_dir).unwrap();
            let result = read_draft_manifest(&drafts_dir, &released);

            match (case.expected, case.error) {
                (Some(expected), None) => {
                    let expected = expected
                        .into_iter()
                        .map(|draft| DraftMigration {
                            id: draft.id,
                            name: draft.name,
                            dependencies: draft.dependencies,
                            sql: draft.sql,
                            checksum: draft.checksum,
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(result.unwrap(), expected, "{}", case.name);
                }
                (None, Some(category)) => assert_eq!(
                    result.unwrap_err().category.as_str(),
                    category,
                    "{}",
                    case.name
                ),
                _ => panic!("{} has neither one result nor one error", case.name),
            }
        }
    }

    fn write_files(directory: &Path, files: &[File]) {
        for file in files {
            fs::write(directory.join(&file.filename), &file.text).unwrap();
        }
    }

    #[test]
    fn absent_draft_directory_is_an_empty_manifest() {
        let directory = tempfile::tempdir().unwrap();

        assert_eq!(
            read_draft_manifest(&directory.path().join("absent"), &BTreeSet::new()).unwrap(),
            Vec::new()
        );
    }
}
