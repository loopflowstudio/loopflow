//! The product schema projection used by build-time and live validation.

use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ProductSchemaObject {
    pub(super) object_type: String,
    pub(super) name: String,
    pub(super) table_name: String,
    pub(super) sql: String,
    pub(super) foreign_keys: Vec<ForeignKeyDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct ForeignKeyDefinition {
    pub(super) id: i64,
    pub(super) sequence: i64,
    pub(super) table: String,
    pub(super) from: String,
    pub(super) to: Option<String>,
    pub(super) on_update: String,
    pub(super) on_delete: String,
    pub(super) match_clause: String,
}

/// The last committed schema this process read from one database file.
#[derive(Debug)]
struct LiveSchema {
    dev: u64,
    ino: u64,
    schema_version: i64,
    schema: Arc<[ProductSchemaObject]>,
}

/// Every product table, view, index, and trigger with defining SQL and
/// explicit foreign-key metadata. `schema_migrations` is bookkeeping rather
/// than product schema, so no migration declares it.
///
/// SQLite bumps `PRAGMA schema_version` on every committed DDL change from any
/// connection or process, so one file read twice at one cookie holds the same
/// objects; the second read is served from memory. In-memory databases and
/// reads inside a write transaction (uncommitted DDL) always read afresh.
pub(crate) fn product_schema(
    conn: &rusqlite::Connection,
) -> rusqlite::Result<Arc<[ProductSchemaObject]>> {
    static LIVE_SCHEMAS: OnceLock<Mutex<HashMap<PathBuf, LiveSchema>>> = OnceLock::new();
    let Some(file) = _cacheable_file(conn)? else {
        return Ok(_read_product_schema(conn)?.into());
    };
    // Read the cookie first: a concurrent DDL commit between the two reads
    // then stores a newer schema under an older cookie the file never returns to.
    let schema_version: i64 = conn.pragma_query_value(None, "schema_version", |row| row.get(0))?;
    let live_schemas = LIVE_SCHEMAS.get_or_init(Mutex::default);
    if let Some(live) = live_schemas
        .lock()
        .expect("live schema cache poisoned")
        .get(&file.path)
        .filter(|live| {
            live.dev == file.dev && live.ino == file.ino && live.schema_version == schema_version
        })
    {
        return Ok(Arc::clone(&live.schema));
    }
    let schema: Arc<[ProductSchemaObject]> = _read_product_schema(conn)?.into();
    live_schemas
        .lock()
        .expect("live schema cache poisoned")
        .insert(
            file.path,
            LiveSchema {
                dev: file.dev,
                ino: file.ino,
                schema_version,
                schema: Arc::clone(&schema),
            },
        );
    Ok(schema)
}

#[derive(Debug)]
struct CacheableFile {
    path: PathBuf,
    dev: u64,
    ino: u64,
}

fn _cacheable_file(conn: &rusqlite::Connection) -> rusqlite::Result<Option<CacheableFile>> {
    let Some(path) = conn.path().filter(|path| !path.is_empty()) else {
        return Ok(None);
    };
    if conn.transaction_state(None::<&str>)? == rusqlite::TransactionState::Write {
        return Ok(None);
    }
    let Ok(metadata) = std::fs::metadata(path) else {
        return Ok(None);
    };
    Ok(Some(CacheableFile {
        path: PathBuf::from(path),
        dev: metadata.dev(),
        ino: metadata.ino(),
    }))
}

fn _read_product_schema(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<ProductSchemaObject>> {
    let mut statement = conn.prepare(
        "SELECT type, name, tbl_name, COALESCE(sql, '')
         FROM sqlite_master
         WHERE type IN ('table', 'view', 'index', 'trigger')
           AND name NOT LIKE 'sqlite_%'
           AND name != 'schema_migrations'
         ORDER BY type, name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut schema = Vec::new();
    for row in rows {
        let (object_type, name, table_name, sql) = row?;
        let foreign_keys = if object_type == "table" {
            let quoted = format!("\"{}\"", name.replace('"', "\"\""));
            let mut foreign_key_statement =
                conn.prepare(&format!("PRAGMA foreign_key_list({quoted})"))?;
            let foreign_keys = foreign_key_statement
                .query_map([], |row| {
                    Ok(ForeignKeyDefinition {
                        id: row.get(0)?,
                        sequence: row.get(1)?,
                        table: row.get(2)?,
                        from: row.get(3)?,
                        to: row.get(4)?,
                        on_update: row.get(5)?,
                        on_delete: row.get(6)?,
                        match_clause: row.get(7)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            foreign_keys
        } else {
            Vec::new()
        };
        schema.push(ProductSchemaObject {
            object_type,
            name,
            table_name,
            sql: sql.trim().to_string(),
            foreign_keys,
        });
    }
    Ok(schema)
}
