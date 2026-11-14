//! The catalog `build.rs` generates: discovery and merge order. Runtime builds
//! compile it only for tests.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::migration_catalog::{is_slug, MigrationId};

/// A migration file as the build script found it, in catalog order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub version: String,
    pub path: PathBuf,
    pub merged: bool,
}

/// Released batches in id order, then slug migrations in merge order with
/// unmerged ones last (by name). `merged_order` lists slugs in the order
/// `origin/main` added them.
pub fn order_catalog(
    directory: &Path,
    merged_order: &[String],
) -> Result<Vec<CatalogEntry>, String> {
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
        .map(|(_, version, path)| CatalogEntry {
            version,
            path,
            merged: true,
        })
        .chain(slugs.into_iter().map(|(version, path)| {
            let merged = position(&version).is_some();
            CatalogEntry {
                version,
                path,
                merged,
            }
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

/// Apply the ordered catalog to an empty schema.
pub fn apply_catalog(catalog: &[CatalogEntry]) -> Result<rusqlite::Connection, String> {
    let connection = rusqlite::Connection::open_in_memory().map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations (version TEXT PRIMARY KEY, applied_at INTEGER NOT NULL);",
        )
        .map_err(|error| error.to_string())?;
    for entry in catalog {
        let sql = std::fs::read_to_string(&entry.path)
            .map_err(|error| format!("read {}: {error}", entry.path.display()))?;
        connection
            .execute_batch(&sql)
            .map_err(|error| format!("apply {}: {error}", entry.version))?;
    }
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::{apply_catalog, merged_order, order_catalog, CatalogEntry};

    fn write(directory: &Path, name: &str, sql: &str) {
        std::fs::write(directory.join(format!("{name}.sql")), sql).unwrap();
    }

    fn versions(catalog: &[CatalogEntry]) -> Vec<(&str, bool)> {
        catalog
            .iter()
            .map(|entry| (entry.version.as_str(), entry.merged))
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

    #[test]
    fn a_migration_that_does_not_apply_names_itself() {
        let directory = tempfile::tempdir().unwrap();
        write(
            directory.path(),
            "0.10.001_initial",
            "CREATE TABLE waves (id TEXT);",
        );
        write(directory.path(), "drop_badges", "DROP TABLE badges;");
        let ordered = order_catalog(directory.path(), &["drop_badges".into()]).unwrap();

        let error = apply_catalog(&ordered).unwrap_err();

        assert!(error.starts_with("apply drop_badges:"), "{error}");
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
}
