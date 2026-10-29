//! Migration identities, shared by the build script and the runtime.
//!
//! Released batches (`<major>.<minor>[.<patch>].<NNN>_<name>.sql`) are frozen
//! history in numeric id order. Every later migration is `<slug>.sql`, named in
//! its PR and ordered by the first-parent commit on `origin/main` that added it
//! (see `migration_build.rs`).

use std::fmt;

// -- Identity -----------------------------------------------------------------

/// A released batch's identity: legacy `{major}.{minor}.{ordinal:03}` or
/// release-scoped `{major}.{minor}.{patch}.{ordinal:03}`. Ordering is numeric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MigrationId {
    pub major: u32,
    pub minor: u32,
    pub patch: Option<u32>,
    pub ordinal: u32,
}

impl MigrationId {
    /// The id leading a released version string (`0.10.001_initial`), or `None`
    /// for anything else, such as a slug migration.
    pub fn parse_version(version: &str) -> Option<Self> {
        let (id, _name) = version.split_once('_')?;
        let numbers = id
            .split('.')
            .map(str::parse)
            .collect::<Result<Vec<u32>, _>>()
            .ok()?;
        match numbers.as_slice() {
            [major, minor, ordinal] => Some(MigrationId {
                major: *major,
                minor: *minor,
                patch: None,
                ordinal: *ordinal,
            }),
            [major, minor, patch, ordinal] => Some(MigrationId {
                major: *major,
                minor: *minor,
                patch: Some(*patch),
                ordinal: *ordinal,
            }),
            _ => None,
        }
    }
}

impl fmt::Display for MigrationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.patch {
            Some(patch) => write!(
                f,
                "{}.{}.{}.{:03}",
                self.major, self.minor, patch, self.ordinal
            ),
            None => write!(f, "{}.{}.{:03}", self.major, self.minor, self.ordinal),
        }
    }
}

/// Whether `version` can name a migration added after released batches: a
/// lowercase slug such as `task_graphs`.
pub fn is_slug(version: &str) -> bool {
    version
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && version.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

/// One migration file. `version` is the file stem and the string recorded in
/// `schema_migrations`, so renaming a merged file is a schema break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub version: &'static str,
    pub sql: &'static str,
    /// The newest migration at or before this one that older readers cannot
    /// read past: a reader must know it to open a store that applied this one.
    /// A breaking or released migration names itself.
    pub reader_floor: &'static str,
    /// Whether `origin/main` added these bytes. Only merged migrations reach the
    /// Machine's shared store.
    pub merged: bool,
}

impl Migration {
    pub fn version(&self) -> String {
        self.version.to_string()
    }

    pub fn breaks_readers(&self) -> bool {
        self.reader_floor == self.version
    }
}

/// SQLite splices `ADD COLUMN` definitions in before table constraints, so the
/// old definitions must all survive verbatim and only column definitions are new.
pub fn table_within(old: &str, new: &str) -> bool {
    let (Some((old_head, old_parts, old_tail)), Some((new_head, new_parts, new_tail))) =
        (table_parts(old), table_parts(new))
    else {
        return false;
    };
    const CONSTRAINTS: [&str; 5] = ["CONSTRAINT", "PRIMARY", "UNIQUE", "CHECK", "FOREIGN"];
    old_head == new_head
        && old_tail == new_tail
        && old_parts.iter().all(|part| new_parts.contains(part))
        && new_parts
            .iter()
            .filter(|part| !old_parts.contains(part))
            .all(|part| {
                let keyword = part.split_whitespace().next().unwrap_or("");
                !CONSTRAINTS
                    .iter()
                    .any(|constraint| keyword.eq_ignore_ascii_case(constraint))
            })
}

/// A `CREATE TABLE` statement as its head, top-level definitions and suffix.
fn table_parts(sql: &str) -> Option<(&str, Vec<&str>, &str)> {
    let open = sql.find('(')?;
    let mut parts = Vec::new();
    let mut start = open + 1;
    let mut depth = 0;
    let mut quote = None;
    let mut characters = sql[open..]
        .char_indices()
        .map(|(index, c)| (open + index, c))
        .peekable();
    while let Some((index, character)) = characters.next() {
        if let Some(end) = quote {
            if character == end {
                if end != ']' && characters.peek().is_some_and(|(_, next)| *next == end) {
                    characters.next();
                } else {
                    quote = None;
                }
            }
            continue;
        }
        match character {
            '\'' | '"' | '`' => quote = Some(character),
            '[' => quote = Some(']'),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    parts.push(sql[start..index].trim());
                    return Some((sql[..open].trim(), parts, sql[index + 1..].trim()));
                }
            }
            ',' if depth == 1 => {
                parts.push(sql[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    None
}
