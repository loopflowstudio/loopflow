//! Historical catalogue fixtures with caller-supplied upgrade observations.

use super::{apply_set, MigrationBuild, MigrationObservers, MIGRATIONS};

#[derive(Debug)]
pub(super) struct Section {
    pub(super) index: usize,
    pub(super) sql: &'static str,
    pub(super) start: usize,
    pub(super) body: std::ops::Range<usize>,
}

pub(super) fn section(name: &str) -> Section {
    let marker = format!("-- draft: {name}");
    MIGRATIONS
        .iter()
        .enumerate()
        .find_map(|(index, migration)| {
            if migration.version == name {
                return Some(Section {
                    index,
                    sql: migration.sql,
                    start: 0,
                    body: 0..migration.sql.len(),
                });
            }
            let start = migration.sql.find(&marker)?;
            let sql = migration.sql;
            let body_start = sql[start..]
                .find('\n')
                .map(|offset| start + offset + 1)
                .unwrap_or(sql.len());
            let body_end = sql[body_start..]
                .find("\n-- draft: ")
                .map(|offset| body_start + offset)
                .unwrap_or(sql.len());
            Some(Section {
                index,
                sql,
                start,
                body: body_start..body_end,
            })
        })
        .unwrap_or_else(|| panic!("migration {name} is in the catalog"))
}

pub fn apply_before_migration(
    observers: &MigrationObservers<'_>,
    build: &MigrationBuild,
    conn: &rusqlite::Connection,
    name: &str,
) {
    let section = section(name);
    apply_set(
        observers,
        build,
        conn,
        &MIGRATIONS[..section.index],
        &mut Vec::new(),
    )
    .unwrap();
    conn.execute_batch(&section.sql[..section.start]).unwrap();
}

pub fn upgrade_from_migration(
    observers: &MigrationObservers<'_>,
    build: &MigrationBuild,
    conn: &rusqlite::Connection,
    name: &str,
) {
    super::_migration_transaction(conn, |conn, guards| {
        let section = section(name);
        conn.execute_batch(&section.sql[section.start..])?;
        super::convert_released_data(conn, section.sql)?;
        super::insert_applied_migration(
            build,
            conn,
            &MIGRATIONS[section.index],
            &super::migration_prefix_fingerprint(&MIGRATIONS[..section.index]),
        )?;
        apply_set(observers, build, conn, MIGRATIONS, guards)?;
        super::validate_foreign_keys(conn)
    })
    .unwrap();
}

pub fn apply_pending_migrations(
    observers: &MigrationObservers<'_>,
    build: &MigrationBuild,
    conn: &rusqlite::Connection,
) {
    super::_migration_transaction(conn, |conn, guards| {
        apply_set(observers, build, conn, MIGRATIONS, guards)
    })
    .unwrap();
}

pub fn migration_section_sql(name: &str) -> String {
    let section = section(name);
    section.sql[section.body].to_string()
}
