use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SqlDialect {
    Sqlite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub(crate) enum Query {
    HealthCheck,
    ListWaves,
    ListWavesByRepo,
    UpsertWave,
    GetWaveById,
    GetWaveByName,
    DeleteWaveById,
    GetSummaryByWave,
    UpsertSummary,
    ListChatMemoryBlocks,
    UpsertChatMemoryBlock,
    DeleteChatMemoryBlock,
    ListChildWaves,
}

impl Query {
    pub(crate) const ALL: [Self; QUERY_COUNT] = [
        Self::HealthCheck,
        Self::ListWaves,
        Self::ListWavesByRepo,
        Self::UpsertWave,
        Self::GetWaveById,
        Self::GetWaveByName,
        Self::DeleteWaveById,
        Self::GetSummaryByWave,
        Self::UpsertSummary,
        Self::ListChatMemoryBlocks,
        Self::UpsertChatMemoryBlock,
        Self::DeleteChatMemoryBlock,
        Self::ListChildWaves,
    ];
}

const QUERY_COUNT: usize = Query::ListChildWaves as usize + 1;

#[derive(Debug, Clone, Copy)]
struct QueryDef {
    template: &'static str,
    sqlite_override: Option<&'static str>,
}

const QUERY_DEFS: [QueryDef; QUERY_COUNT] = [
    QueryDef {
        template: "SELECT 1",
        sqlite_override: None,
    },
    QueryDef {
        template: "SELECT id, name, direction, area, paused, created_at, workers,\n                    goal, metrics, parent_wave_id,\n                    repo, worktree, branch, status, iteration, cycle_start_iteration\n             FROM waves\n             ORDER BY created_at DESC",
        sqlite_override: None,
    },
    QueryDef {
        template: "SELECT id, name, direction, area, paused, created_at, workers,\n                    goal, metrics, parent_wave_id,\n                    repo, worktree, branch, status, iteration, cycle_start_iteration\n             FROM waves\n             WHERE repo = {p1}\n             ORDER BY created_at DESC",
        sqlite_override: None,
    },
    QueryDef {
        template: "INSERT INTO waves (\n                id, name, direction, area, paused, created_at, workers, goal, metrics, parent_wave_id,\n                repo, worktree, branch, status, iteration, cycle_start_iteration\n            ) VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7}, {p8}, {p9}, {p10}, {p11}, {p12}, {p13}, {p14}, {p15}, {p16})\n            ON CONFLICT(id) DO UPDATE SET\n                name = excluded.name,\n                direction = excluded.direction,\n                area = excluded.area,\n                paused = excluded.paused,\n                created_at = excluded.created_at,\n                workers = excluded.workers,\n                goal = excluded.goal,\n                metrics = excluded.metrics,\n                parent_wave_id = excluded.parent_wave_id,\n                repo = excluded.repo,\n                worktree = excluded.worktree,\n                branch = excluded.branch,\n                status = excluded.status,\n                iteration = excluded.iteration,\n                cycle_start_iteration = excluded.cycle_start_iteration",
        sqlite_override: None,
    },
    QueryDef {
        template: "SELECT id, name, direction, area, paused, created_at, workers,\n                    goal, metrics, parent_wave_id,\n                    repo, worktree, branch, status, iteration, cycle_start_iteration\n             FROM waves WHERE id = {p1}",
        sqlite_override: None,
    },
    QueryDef {
        template: "SELECT id, name, direction, area, paused, created_at, workers,\n                    goal, metrics, parent_wave_id,\n                    repo, worktree, branch, status, iteration, cycle_start_iteration\n             FROM waves\n             WHERE name = {p1}",
        sqlite_override: None,
    },
    QueryDef {
        template: "DELETE FROM waves WHERE id = {p1}",
        sqlite_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, content, source_hash, token_budget, model, created_at\n             FROM summaries WHERE wave_id = {p1}",
        sqlite_override: None,
    },
    QueryDef {
        template: "INSERT INTO summaries (id, wave_id, content, source_hash, token_budget, model, created_at)\n             VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7})\n             ON CONFLICT(wave_id) DO UPDATE SET\n                 content = excluded.content,\n                 source_hash = excluded.source_hash,\n                 token_budget = excluded.token_budget,\n                 model = excluded.model,\n                 created_at = excluded.created_at",
        sqlite_override: None,
    },
    QueryDef {
        template: "SELECT wave_id, name, content, position, updated_at\n             FROM chat_memory_blocks\n             WHERE wave_id = {p1}\n             ORDER BY position ASC, name ASC",
        sqlite_override: None,
    },
    QueryDef {
        template: "INSERT INTO chat_memory_blocks (wave_id, name, content, position, updated_at)\n             VALUES ({p1}, {p2}, {p3}, {p4}, {p5})\n             ON CONFLICT(wave_id, name) DO UPDATE SET\n                 content = excluded.content,\n                 position = excluded.position,\n                 updated_at = excluded.updated_at",
        sqlite_override: None,
    },
    QueryDef {
        template: "DELETE FROM chat_memory_blocks WHERE wave_id = {p1} AND name = {p2}",
        sqlite_override: None,
    },
    // ListChildWaves — a chord's contents are its children, ordered by creation.
    QueryDef {
        template: "SELECT id, name, direction, area, paused, created_at, workers,\n                    goal, metrics, parent_wave_id,\n                    repo, worktree, branch, status, iteration, cycle_start_iteration\n             FROM waves\n             WHERE parent_wave_id = {p1}\n             ORDER BY created_at ASC",
        sqlite_override: None,
    },
];

#[derive(Debug)]
struct RenderedCatalog {
    sqlite: Vec<String>,
}

static RENDERED: LazyLock<RenderedCatalog> = LazyLock::new(|| {
    let mut sqlite = Vec::with_capacity(QUERY_DEFS.len());

    for query in Query::ALL {
        let def = QUERY_DEFS[query as usize];
        sqlite.push(render_sql(
            def.sqlite_override.unwrap_or(def.template),
            SqlDialect::Sqlite,
        ));
    }

    RenderedCatalog { sqlite }
});

pub(crate) fn sql(query: Query, dialect: SqlDialect) -> &'static str {
    let index = query as usize;
    match dialect {
        SqlDialect::Sqlite => RENDERED.sqlite[index].as_str(),
    }
}

pub(crate) fn list_waves_query(has_repo: bool) -> Query {
    if has_repo {
        Query::ListWavesByRepo
    } else {
        Query::ListWaves
    }
}

fn render_sql(template: &str, dialect: SqlDialect) -> String {
    let mut output = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut idx = 0;

    while idx < bytes.len() {
        if let Some((start, end)) = placeholder_bounds(template, idx) {
            let value = &template[start..end];
            match dialect {
                SqlDialect::Sqlite => {
                    output.push('?');
                    output.push_str(value);
                }
            }
            idx = end + 1;
            continue;
        }

        output.push(bytes[idx] as char);
        idx += 1;
    }

    output
}

fn placeholder_bounds(template: &str, idx: usize) -> Option<(usize, usize)> {
    let bytes = template.as_bytes();
    if idx + 2 >= bytes.len() || bytes[idx] != b'{' || bytes[idx + 1] != b'p' {
        return None;
    }

    let start = idx + 2;
    let mut end = start;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == start || end >= bytes.len() || bytes[end] != b'}' {
        panic!("invalid placeholder in SQL template: {template}");
    }

    Some((start, end))
}

#[cfg(test)]
fn extract_placeholder_numbers(template: &str) -> Vec<usize> {
    let bytes = template.as_bytes();
    let mut numbers = Vec::new();
    let mut idx = 0;

    while idx < bytes.len() {
        if let Some((start, end)) = placeholder_bounds(template, idx) {
            let value = template[start..end]
                .parse::<usize>()
                .expect("placeholder number should parse");
            numbers.push(value);
            idx = end + 1;
            continue;
        }

        idx += 1;
    }

    numbers
}

#[cfg(test)]
fn placeholders_are_contiguous(template: &str) -> bool {
    placeholder_numbers_are_contiguous(extract_placeholder_numbers(template))
}

#[cfg(test)]
fn extract_rendered_placeholder_numbers(sql: &str, marker: u8) -> Vec<usize> {
    let bytes = sql.as_bytes();
    let mut numbers = Vec::new();
    let mut idx = 0;

    while idx < bytes.len() {
        if bytes[idx] == marker {
            let start = idx + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }

            if end > start {
                if let Ok(value) = sql[start..end].parse::<usize>() {
                    numbers.push(value);
                }
                idx = end;
                continue;
            }
        }
        idx += 1;
    }

    numbers
}

#[cfg(test)]
fn rendered_placeholders_are_contiguous(sql: &str, marker: u8) -> bool {
    placeholder_numbers_are_contiguous(extract_rendered_placeholder_numbers(sql, marker))
}

#[cfg(test)]
fn placeholder_numbers_are_contiguous(mut numbers: Vec<usize>) -> bool {
    if numbers.is_empty() {
        return true;
    }

    numbers.sort_unstable();
    numbers.dedup();
    numbers
        .iter()
        .enumerate()
        .all(|(idx, number)| *number == idx + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_catalog_definitions_cover_every_variant() {
        assert_eq!(QUERY_DEFS.len(), QUERY_COUNT, "query defs length mismatch");
        assert_eq!(
            Query::ALL.len(),
            QUERY_COUNT,
            "query enum list missing variants"
        );

        let mut seen = vec![false; QUERY_COUNT];
        for query in Query::ALL {
            let index = query as usize;
            assert!(
                index < QUERY_DEFS.len(),
                "query index out of range for {query:?}"
            );
            assert!(
                !seen[index],
                "duplicate query variant in Query::ALL for {query:?}"
            );
            seen[index] = true;

            let def = QUERY_DEFS[index];
            assert!(
                !def.template.trim().is_empty(),
                "empty SQL template for {query:?}"
            );
        }

        assert!(
            seen.into_iter().all(|value| value),
            "missing query definitions"
        );
    }

    #[test]
    fn every_query_renders_for_sqlite_with_valid_placeholders() {
        for query in Query::ALL {
            let sqlite = std::panic::catch_unwind(|| sql(query, SqlDialect::Sqlite))
                .unwrap_or_else(|_| panic!("sqlite rendering panicked for {query:?}"));

            assert!(
                !sqlite.trim().is_empty(),
                "sqlite rendering is empty for {query:?}"
            );
            assert!(
                !sqlite.contains("{p"),
                "sqlite rendering still has placeholders for {query:?}"
            );
            assert!(
                rendered_placeholders_are_contiguous(sqlite, b'?'),
                "sqlite placeholders not contiguous for {query:?}: {sqlite}"
            );
        }
    }

    #[test]
    fn catalog_placeholders_are_contiguous() {
        for query in Query::ALL {
            let def = QUERY_DEFS[query as usize];
            assert!(
                placeholders_are_contiguous(def.template),
                "template has non-contiguous placeholders for {query:?}"
            );
            if let Some(sqlite_override) = def.sqlite_override {
                assert!(
                    placeholders_are_contiguous(sqlite_override),
                    "sqlite override has non-contiguous placeholders for {query:?}"
                );
            }
        }
    }
}
