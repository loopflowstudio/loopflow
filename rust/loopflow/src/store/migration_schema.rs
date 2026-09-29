//! The product schema projection used by build-time and live validation.

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

/// Every product table, index, and trigger with defining SQL and
/// explicit foreign-key metadata. `schema_migrations` is bookkeeping rather
/// than product schema, so no migration declares it.
pub(crate) fn product_schema(
    conn: &rusqlite::Connection,
) -> rusqlite::Result<Vec<ProductSchemaObject>> {
    let mut statement = conn.prepare(
        "SELECT type, name, tbl_name, COALESCE(sql, '')
         FROM sqlite_master
         WHERE type IN ('table', 'index', 'trigger')
           AND name NOT LIKE 'sqlite_%'
           AND name NOT IN ('schema_migrations', 'development_migrations')
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
