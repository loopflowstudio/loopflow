//! The catalog `build.rs` generates: discovery and merge order. Runtime builds
//! compile it only for tests.

use std::fmt::Write as _;
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

/// Embed migration bytes without retaining a checkout path in generated Rust.
pub fn render_catalog(catalog: &[CatalogEntry]) -> Result<String, String> {
    let mut source = String::from("static MIGRATIONS: &[Migration] = &[\n");
    for entry in catalog {
        let sql = std::fs::read_to_string(&entry.path)
            .map_err(|error| format!("read {}: {error}", entry.path.display()))?;
        writeln!(
            source,
            "    Migration {{ version: {:?}, sql: {:?}, merged: {} }},",
            entry.version, sql, entry.merged
        )
        .expect("writing to a String cannot fail");
    }
    source.push_str("];\n");
    Ok(source)
}

/// Slugs in the order `origin/main`'s first-parent history added them under
/// `directory`, or an empty list when the history is unavailable (no Git, no
/// `origin/main`, a shallow clone): every slug migration is then unmerged.
pub fn merged_order(git_root: &Path, directory: &Path) -> Vec<String> {
    let Ok(relative) = directory.strip_prefix(git_root) else {
        return Vec::new();
    };
    let directories = migration_directories(relative);
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
        .args(&directories)
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
        if !directories
            .iter()
            .any(|directory| path.parent() == Some(directory.as_path()))
        {
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

/// The Git files whose change can move `origin/main` or its history. A
/// missing file is watched through its nearest existing parent, but never the
/// Git directory itself: Cargo scans a directory recursively, so every object
/// any worktree writes would rebuild storage and everything above it.
pub fn reference_watches(git_root: &Path) -> Vec<PathBuf> {
    let mut watches = Vec::new();
    for reference in ["refs/remotes/origin/main", "packed-refs", "shallow"] {
        let output = Command::new("git")
            .args(["rev-parse", "--git-path", reference])
            .current_dir(git_root)
            .output()
            .expect("locate Git reference");
        assert!(output.status.success(), "locate Git reference {reference}");
        let path = PathBuf::from(
            String::from_utf8(output.stdout)
                .expect("UTF-8 Git path")
                .trim(),
        );
        let path = if path.is_absolute() {
            path
        } else {
            git_root.join(path)
        };
        let git_dir = path
            .ancestors()
            .nth(Path::new(reference).components().count())
            .expect("Git directory")
            .to_path_buf();
        if let Some(existing) = path
            .ancestors()
            .take_while(|ancestor| *ancestor != git_dir)
            .find(|ancestor| ancestor.exists())
        {
            watches.push(existing.to_path_buf());
        }
    }
    watches
}

// Both source locations contribute to one first-addition history. A relocation
// must not give an existing slug a new position in the upgrade frontier.
fn migration_directories(relative: &Path) -> Vec<PathBuf> {
    let historical = Path::new("rust/loopflow/src/store/migrations");
    let storage = Path::new("rust/loopflow-storage/src/migrations");
    if relative == historical || relative == storage {
        vec![historical.to_path_buf(), storage.to_path_buf()]
    } else {
        vec![relative.to_path_buf()]
    }
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

    use super::{apply_catalog, merged_order, order_catalog, reference_watches, CatalogEntry};

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

    #[test]
    fn reference_watches_never_include_the_shared_git_directory() {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path().join("main");
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        git(&root, &["init", "--initial-branch=main"]);
        git(&root, &["config", "user.email", "test@loopflow.studio"]);
        git(&root, &["config", "user.name", "Test"]);
        git(&root, &["commit", "--allow-empty", "-m", "initial"]);
        git(&root, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        let worktree = repo.path().join("feature");
        git(&root, &["worktree", "add", worktree.to_str().unwrap()]);
        let common = root.join(".git");

        assert_eq!(
            reference_watches(&worktree.canonicalize().unwrap()),
            [common.join("refs/remotes/origin/main")]
        );
        git(&root, &["pack-refs", "--all"]);
        assert_eq!(
            reference_watches(&root),
            [common.join("refs/remotes"), common.join("packed-refs")]
        );
    }

    #[test]
    fn relocation_keeps_catalog_bytes_schema_and_origin_main_refresh() {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path().canonicalize().unwrap();
        git(&root, &["init", "--initial-branch=main"]);
        git(&root, &["config", "user.email", "test@loopflow.studio"]);
        git(&root, &["config", "user.name", "Test"]);
        let old = root.join("rust/loopflow/src/store/migrations");
        let moved = root.join("rust/loopflow-storage/src/migrations");
        std::fs::create_dir_all(&old).unwrap();
        write(&old, "0.10.001_initial", "CREATE TABLE tasks(id TEXT);");
        write(&old, "z_first", "ALTER TABLE tasks ADD title TEXT;");
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "initial frontier"]);
        git(&root, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        let before = order_catalog(&old, &merged_order(&root, &old)).unwrap();
        let bytes = before
            .iter()
            .map(|entry| std::fs::read(&entry.path).unwrap())
            .collect::<Vec<_>>();
        let schema =
            super::super::migration_schema::product_schema(&apply_catalog(&before).unwrap())
                .unwrap();
        std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
        git(
            &root,
            &[
                "mv",
                "rust/loopflow/src/store/migrations",
                "rust/loopflow-storage/src/migrations",
            ],
        );
        git(&root, &["commit", "-m", "relocate storage"]);
        let after = order_catalog(&moved, &merged_order(&root, &moved)).unwrap();
        assert_eq!(versions(&before), versions(&after));
        assert_eq!(
            bytes,
            after
                .iter()
                .map(|entry| std::fs::read(&entry.path).unwrap())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            *schema,
            *super::super::migration_schema::product_schema(&apply_catalog(&after).unwrap())
                .unwrap()
        );
        write(
            &moved,
            "a_later",
            "CREATE INDEX task_titles ON tasks(title);",
        );
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "dependent migration"]);
        let unmerged = order_catalog(&moved, &merged_order(&root, &moved)).unwrap();
        assert_eq!(
            versions(&unmerged),
            [
                ("0.10.001_initial", true),
                ("z_first", true),
                ("a_later", false)
            ]
        );
        // Only the remote-tracking ref advances; the checkout and SQL stay put.
        git(&root, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        let order = merged_order(&root, &moved);
        assert_eq!(order, ["z_first", "a_later"]);
        let merged = order_catalog(&moved, &order).unwrap();
        assert_eq!(
            versions(&merged),
            [
                ("0.10.001_initial", true),
                ("z_first", true),
                ("a_later", true)
            ]
        );
        apply_catalog(&merged).unwrap();
        // A source snapshot uses the saved order with no Git directory.
        let snapshot = tempfile::tempdir().unwrap();
        for entry in &merged {
            std::fs::copy(
                &entry.path,
                snapshot.path().join(entry.path.file_name().unwrap()),
            )
            .unwrap();
        }
        let packaged = order_catalog(snapshot.path(), &order).unwrap();
        assert_eq!(versions(&merged), versions(&packaged));
        assert_eq!(
            *super::super::migration_schema::product_schema(&apply_catalog(&merged).unwrap())
                .unwrap(),
            *super::super::migration_schema::product_schema(&apply_catalog(&packaged).unwrap())
                .unwrap()
        );
    }
    #[test]
    fn generated_catalog_is_source_root_independent_and_tracks_sql_bytes() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let sql = "CREATE TABLE words(value TEXT DEFAULT 'a\\b');\n-- \"quoted\" 雪\n";
        for root in [first.path(), second.path()] {
            write(root, "0.10.001_initial", sql);
        }
        let a = super::render_catalog(&order_catalog(first.path(), &[]).unwrap()).unwrap();
        let b = super::render_catalog(&order_catalog(second.path(), &[]).unwrap()).unwrap();
        assert_eq!(a, b);
        assert!(a.contains(&format!("sql: {sql:?}")));
        write(second.path(), "0.10.001_initial", "SELECT 1;");
        let changed = super::render_catalog(&order_catalog(second.path(), &[]).unwrap()).unwrap();
        assert_ne!(a, changed);
    }
}
