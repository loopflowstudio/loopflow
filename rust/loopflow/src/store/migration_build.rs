//! The catalog `build.rs` generates: discovery, merge order and the additivity
//! proof that computes each migration's reader floor. Runtime builds compile it
//! only for tests.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::migration_catalog::{is_slug, table_within, MigrationId};

/// A migration file as the build script found it, in catalog order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub version: String,
    pub path: PathBuf,
    pub sql: String,
    pub reader_floor: String,
    pub merged: bool,
}

/// Released batches in id order, then slug migrations in merge order with
/// unmerged ones last (by name). `merged_order` lists slugs in the order
/// `origin/main` added them.
pub fn order_catalog(
    directory: &Path,
    merged_order: &[String],
) -> Result<Vec<(String, PathBuf, bool)>, String> {
    let mut released = Vec::new();
    let mut slugs = Vec::new();
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|error| format!("read {}: {error}", directory.display()))?
            .path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("sql") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("{} has no UTF-8 name", path.display()))?
            .to_string();
        if let Some(id) = MigrationId::parse_version(&stem) {
            released.push((id, stem, path));
        } else if is_slug(&stem) {
            slugs.push((stem, path));
        } else {
            return Err(format!(
                "{} is neither a released batch nor a lowercase slug migration",
                path.display()
            ));
        }
    }
    released.sort();
    let position = |slug: &str| merged_order.iter().position(|merged| merged == slug);
    slugs.sort_by(
        |(left, _), (right, _)| match (position(left), position(right)) {
            (Some(left), Some(right)) => left.cmp(&right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.cmp(right),
        },
    );
    Ok(released
        .into_iter()
        .map(|(_, stem, path)| (stem, path, true))
        .chain(slugs.into_iter().map(|(stem, path)| {
            let merged = position(&stem).is_some();
            (stem, path, merged)
        }))
        .collect())
}

/// Slugs in the order `origin/main`'s first-parent history added them under
/// `directory`, or an empty list when the history is unavailable (no Git, no
/// `origin/main`, a shallow clone): every slug migration is then unmerged.
pub fn merged_order(git_root: &Path, directory: &Path) -> Vec<String> {
    let Ok(relative) = directory.strip_prefix(git_root) else {
        return Vec::new();
    };
    let shallow = Command::new("git")
        .args(["rev-parse", "--is-shallow-repository"])
        .current_dir(git_root)
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "true");
    if shallow {
        return Vec::new();
    }
    let Ok(output) = Command::new("git")
        .args([
            "log",
            "origin/main",
            "--first-parent",
            "--diff-merges=first-parent",
            "--no-renames",
            "--diff-filter=A",
            "--reverse",
            "--name-only",
            "--format=",
            "--",
        ])
        .arg(relative)
        .current_dir(git_root)
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let mut order = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let path = Path::new(line);
        if path.parent() != Some(relative) {
            continue;
        }
        let Some(stem) = path
            .extension()
            .filter(|extension| *extension == "sql")
            .and_then(|_| path.file_stem())
            .and_then(|stem| stem.to_str())
        else {
            continue;
        };
        if is_slug(stem) && !order.iter().any(|known| known == stem) {
            order.push(stem.to_string());
        }
    }
    order
}

/// Apply the ordered catalog to an empty schema and compute each migration's
/// reader floor: released batches and breaking slugs name themselves; an
/// additive slug keeps the floor before it.
pub fn build_catalog(
    ordered: Vec<(String, PathBuf, bool)>,
) -> Result<(Vec<CatalogEntry>, rusqlite::Connection), String> {
    let connection = rusqlite::Connection::open_in_memory().map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations (version TEXT PRIMARY KEY, applied_at INTEGER NOT NULL);",
        )
        .map_err(|error| error.to_string())?;
    let mut catalog: Vec<CatalogEntry> = Vec::new();
    for (version, path, merged) in ordered {
        let sql = std::fs::read_to_string(&path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        let reader_floor = if MigrationId::parse_version(&version).is_some() {
            connection
                .execute_batch(&sql)
                .map_err(|error| format!("apply {version}: {error}"))?;
            version.clone()
        } else {
            let additive = is_additive(&connection, &sql)
                .map_err(|error| format!("apply {version}: {error}"))?;
            match catalog.last() {
                Some(previous) if additive => previous.reader_floor.clone(),
                _ => version.clone(),
            }
        };
        catalog.push(CatalogEntry {
            version,
            path,
            sql,
            reader_floor,
            merged,
        });
    }
    Ok((catalog, connection))
}

// -- Additivity ---------------------------------------------------------------

/// Apply `sql` and report whether every reader of the schema before it can still
/// read the schema after it: existing objects survive unchanged, tables only
/// gain nullable or defaulted columns, new objects are tables or non-unique
/// indexes, and no statement writes an existing table. `-- breaks: <reason>`
/// forces a break; nothing declares a migration additive.
pub fn is_additive(connection: &rusqlite::Connection, sql: &str) -> rusqlite::Result<bool> {
    use rusqlite::hooks::{AuthAction, Authorization};
    use std::sync::{Arc, Mutex};

    let before = schema_rows(connection)?;
    let tables: Vec<String> = before
        .iter()
        .filter(|row| row.0 == "table")
        .map(|row| row.1.clone())
        .collect();
    let mut columns = Vec::new();
    for table in &tables {
        columns.push((table.clone(), table_columns(connection, table)?));
    }
    let writes = Arc::new(Mutex::new(false));
    let observed = Arc::clone(&writes);
    let existing = tables.clone();
    connection.authorizer(Some(move |context: rusqlite::hooks::AuthContext<'_>| {
        let table = match context.action {
            AuthAction::Insert { table_name }
            | AuthAction::Delete { table_name }
            | AuthAction::Update { table_name, .. } => Some(table_name),
            _ => None,
        };
        if table.is_some_and(|table| existing.iter().any(|known| known == table)) {
            *observed.lock().expect("authorizer flag") = true;
        }
        Authorization::Allow
    }))?;
    let applied = connection.execute_batch(sql);
    let reset =
        connection.authorizer(None::<fn(rusqlite::hooks::AuthContext<'_>) -> Authorization>);
    applied?;
    reset?;
    let wrote = *writes.lock().expect("authorizer flag");
    if wrote
        || sql.lines().any(|line| {
            line.trim_start()
                .strip_prefix("--")
                .is_some_and(|rest| rest.trim_start().starts_with("breaks:"))
        })
    {
        return Ok(false);
    }
    let after = schema_rows(connection)?;
    for (kind, name, table, definition) in &before {
        let Some(now) = after.iter().find(|row| &row.0 == kind && &row.1 == name) else {
            return Ok(false);
        };
        if (&now.2, &now.3) != (table, definition)
            && (kind != "table" || !table_within(definition, &now.3))
        {
            return Ok(false);
        }
    }
    for (kind, name, table, definition) in &after {
        if before.iter().any(|row| &row.0 == kind && &row.1 == name) {
            continue;
        }
        let unique_index = definition
            .split_whitespace()
            .take(2)
            .map(str::to_ascii_uppercase)
            .eq(["CREATE", "UNIQUE"]);
        if kind == "index" && tables.contains(table) && unique_index {
            return Ok(false);
        }
        if kind != "table" && kind != "index" {
            return Ok(false);
        }
    }
    for (table, old) in &columns {
        let new = table_columns(connection, table)?;
        if new.len() < old.len() || new[..old.len()] != old[..] {
            return Ok(false);
        }
        for (_name, _kind, required, default, primary, hidden) in &new[old.len()..] {
            let defaulted = default.as_deref().is_some_and(|value| value != "NULL");
            if (*required && !defaulted) || *primary || *hidden {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

type SchemaRow = (String, String, String, String);
type ColumnRow = (String, String, bool, Option<String>, bool, bool);

fn schema_rows(connection: &rusqlite::Connection) -> rusqlite::Result<Vec<SchemaRow>> {
    let mut statement = connection.prepare(
        "SELECT type, name, tbl_name, coalesce(sql, '') FROM sqlite_master
         WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })?;
    rows.collect()
}

fn table_columns(
    connection: &rusqlite::Connection,
    table: &str,
) -> rusqlite::Result<Vec<ColumnRow>> {
    let mut statement = connection.prepare(
        "SELECT name, type, \"notnull\", dflt_value, pk, hidden FROM pragma_table_xinfo(?1)",
    )?;
    let rows = statement.query_map([table], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get::<_, i64>(2)? != 0,
            row.get(3)?,
            row.get::<_, i64>(4)? != 0,
            row.get::<_, i64>(5)? != 0,
        ))
    })?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::{build_catalog, is_additive, merged_order, order_catalog};

    fn write(directory: &Path, name: &str, sql: &str) {
        std::fs::write(directory.join(format!("{name}.sql")), sql).unwrap();
    }

    fn versions(ordered: &[(String, std::path::PathBuf, bool)]) -> Vec<(&str, bool)> {
        ordered
            .iter()
            .map(|(version, _, merged)| (version.as_str(), *merged))
            .collect()
    }

    #[test]
    fn released_batches_come_first_then_merge_order_then_unmerged_by_name() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["0.10.002_x", "0.9.001_y", "b_slug", "a_slug", "c_slug"] {
            write(directory.path(), name, "SELECT 1;");
        }
        let ordered = order_catalog(directory.path(), &["c_slug".into(), "b_slug".into()]).unwrap();
        assert_eq!(
            versions(&ordered),
            [
                ("0.9.001_y", true),
                ("0.10.002_x", true),
                ("c_slug", true),
                ("b_slug", true),
                ("a_slug", false),
            ]
        );
        write(directory.path(), "Not-A-Slug", "SELECT 1;");
        assert!(order_catalog(directory.path(), &[]).is_err());
    }

    fn git(repo: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(status.status.success(), "git {args:?}");
    }

    #[test]
    fn merge_order_follows_origin_main_and_counts_a_move_as_an_addition() {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path();
        git(root, &["init", "--initial-branch=main"]);
        git(root, &["config", "user.email", "test@loopflow.studio"]);
        git(root, &["config", "user.name", "Test"]);
        let migrations = root.join("migrations");
        std::fs::create_dir_all(migrations.join("drafts")).unwrap();
        for (path, message) in [
            ("migrations/second.sql", "second"),
            ("migrations/drafts/first.sql", "draft"),
        ] {
            std::fs::write(root.join(path), "SELECT 1;").unwrap();
            git(root, &["add", path]);
            git(root, &["commit", "-m", message]);
        }
        git(
            root,
            &["mv", "migrations/drafts/first.sql", "migrations/first.sql"],
        );
        git(root, &["commit", "-m", "move"]);
        git(root, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        std::fs::write(migrations.join("branch_only.sql"), "SELECT 1;").unwrap();
        git(root, &["add", "migrations/branch_only.sql"]);
        git(root, &["commit", "-m", "branch work"]);

        let root = root.canonicalize().unwrap();
        assert_eq!(
            merged_order(&root, &root.join("migrations")),
            ["second", "first"]
        );
    }

    fn additive(before: &str, sql: &str) -> bool {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection.execute_batch(before).unwrap();
        is_additive(&connection, sql).unwrap()
    }

    #[test]
    fn only_changes_every_older_reader_survives_are_additive() {
        let before = "CREATE TABLE waves (id TEXT PRIMARY KEY, name TEXT NOT NULL);";
        for sql in [
            "ALTER TABLE waves ADD COLUMN colour TEXT;",
            "ALTER TABLE waves ADD COLUMN rank INTEGER NOT NULL DEFAULT 0;",
            "CREATE TABLE badges (wave_id TEXT); INSERT INTO badges VALUES ('w');",
            "CREATE INDEX waves_name ON waves(name);",
        ] {
            assert!(additive(before, sql), "{sql}");
        }
        for sql in [
            "ALTER TABLE waves ADD COLUMN rank INTEGER NOT NULL;",
            "DROP TABLE waves;",
            "ALTER TABLE waves RENAME COLUMN name TO title;",
            "INSERT INTO waves VALUES ('w', 'name');",
            "DELETE FROM waves;",
            "CREATE UNIQUE INDEX waves_name ON waves(name);",
            "CREATE VIEW wave_names AS SELECT name FROM waves;",
            "-- breaks: a new status older readers reject\nCREATE TABLE badges (wave_id TEXT);",
        ] {
            assert!(!additive(before, sql), "{sql}");
        }
    }

    #[test]
    fn a_floor_rises_only_at_a_breaking_migration() {
        let directory = tempfile::tempdir().unwrap();
        write(
            directory.path(),
            "0.10.001_initial",
            "CREATE TABLE waves (id TEXT PRIMARY KEY);",
        );
        write(
            directory.path(),
            "add_colour",
            "ALTER TABLE waves ADD COLUMN colour TEXT;",
        );
        write(directory.path(), "drop_waves", "DROP TABLE waves;");
        write(
            directory.path(),
            "add_badges",
            "CREATE TABLE badges (id TEXT);",
        );
        let order = ["add_colour", "drop_waves", "add_badges"].map(String::from);
        let ordered = order_catalog(directory.path(), &order).unwrap();
        let (catalog, _) = build_catalog(ordered).unwrap();
        let floors: Vec<(&str, &str)> = catalog
            .iter()
            .map(|entry| (entry.version.as_str(), entry.reader_floor.as_str()))
            .collect();
        assert_eq!(
            floors,
            [
                ("0.10.001_initial", "0.10.001_initial"),
                ("add_colour", "0.10.001_initial"),
                ("drop_waves", "drop_waves"),
                ("add_badges", "drop_waves"),
            ]
        );
    }
}
