use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SqlDialect {
    Sqlite,
    Postgres,
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
    ListRunsAll,
    ListRunsByWave,
    ListRunsLimited,
    ListRunsByWaveLimited,
    GetRunById,
    GetActiveRun,
    CountActiveRuns,
    GetLatestRun,
    InsertRun,
    UpdateRun,
    ListStackRuns,
    FailOrphanedRuns,
    GetLivePrState,
    UpsertLivePrState,
    ListForkRuns,
    UpsertForkRun,
    DeleteForkRuns,
    GetSummaryByWave,
    UpsertSummary,
    ListChatMemoryBlocks,
    UpsertChatMemoryBlock,
    DeleteChatMemoryBlock,
    ListWaveRepos,
    UpsertWaveRepo,
    DeleteWaveReposByWave,
    ResetStaleActiveRepos,
    ListChildWaves,
    RecordRunTokenUsage,
    AggregateTokenUsageByWaveProvider,
    AggregateTokenUsageByRepoProvider,
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
        Self::ListRunsAll,
        Self::ListRunsByWave,
        Self::ListRunsLimited,
        Self::ListRunsByWaveLimited,
        Self::GetRunById,
        Self::GetActiveRun,
        Self::CountActiveRuns,
        Self::GetLatestRun,
        Self::InsertRun,
        Self::UpdateRun,
        Self::ListStackRuns,
        Self::FailOrphanedRuns,
        Self::GetLivePrState,
        Self::UpsertLivePrState,
        Self::ListForkRuns,
        Self::UpsertForkRun,
        Self::DeleteForkRuns,
        Self::GetSummaryByWave,
        Self::UpsertSummary,
        Self::ListChatMemoryBlocks,
        Self::UpsertChatMemoryBlock,
        Self::DeleteChatMemoryBlock,
        Self::ListWaveRepos,
        Self::UpsertWaveRepo,
        Self::DeleteWaveReposByWave,
        Self::ResetStaleActiveRepos,
        Self::ListChildWaves,
        Self::RecordRunTokenUsage,
        Self::AggregateTokenUsageByWaveProvider,
        Self::AggregateTokenUsageByRepoProvider,
    ];
}

const QUERY_COUNT: usize = Query::AggregateTokenUsageByRepoProvider as usize + 1;

#[derive(Debug, Clone, Copy)]
struct QueryDef {
    template: &'static str,
    sqlite_override: Option<&'static str>,
    postgres_override: Option<&'static str>,
}

const QUERY_DEFS: [QueryDef; QUERY_COUNT] = [
    QueryDef {
        template: "SELECT 1",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, name, direction, area, paused, created_at, workers, mode,\n                    primary_flow, goal, metrics, parent_wave_id\n             FROM waves\n             ORDER BY created_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, name, direction, area, paused, created_at, workers, mode,\n                    primary_flow, goal, metrics, parent_wave_id\n             FROM waves\n             WHERE EXISTS (SELECT 1 FROM wave_repos wr WHERE wr.wave_id = waves.id AND wr.repo = {p1})\n             ORDER BY created_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO waves (\n                id, name, direction, area, paused, created_at, workers, mode, primary_flow, goal, metrics, parent_wave_id\n            ) VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7}, {p8}, {p9}, {p10}, {p11}, {p12})\n            ON CONFLICT(id) DO UPDATE SET\n                name = excluded.name,\n                direction = excluded.direction,\n                area = excluded.area,\n                paused = excluded.paused,\n                created_at = excluded.created_at,\n                workers = excluded.workers,\n                mode = excluded.mode,\n                primary_flow = excluded.primary_flow,\n                goal = excluded.goal,\n                metrics = excluded.metrics,\n                parent_wave_id = excluded.parent_wave_id",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, name, direction, area, paused, created_at, workers, mode,\n                    primary_flow, goal, metrics, parent_wave_id\n             FROM waves WHERE id = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, name, direction, area, paused, created_at, workers, mode,\n                    primary_flow, goal, metrics, parent_wave_id\n             FROM waves\n             WHERE name = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "DELETE FROM waves WHERE id = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_task,\n                    snapshot_direction, snapshot_area, snapshot_pr, flow_parents, execution_cursor,\n                    parent_run_id, parent_pr_number, stack_position,\n                    stack_group_id, stack_status, lineage_inferred, target_branch, repair_of\n             FROM runs\n             ORDER BY started_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_task,\n                    snapshot_direction, snapshot_area, snapshot_pr, flow_parents, execution_cursor,\n                    parent_run_id, parent_pr_number, stack_position,\n                    stack_group_id, stack_status, lineage_inferred, target_branch, repair_of\n             FROM runs\n             WHERE wave_id = {p1}\n             ORDER BY started_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_task,\n                    snapshot_direction, snapshot_area, snapshot_pr, flow_parents, execution_cursor,\n                    parent_run_id, parent_pr_number, stack_position,\n                    stack_group_id, stack_status, lineage_inferred, target_branch, repair_of\n             FROM runs\n             ORDER BY started_at DESC\n             LIMIT {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_task,\n                    snapshot_direction, snapshot_area, snapshot_pr, flow_parents, execution_cursor,\n                    parent_run_id, parent_pr_number, stack_position,\n                    stack_group_id, stack_status, lineage_inferred, target_branch, repair_of\n             FROM runs\n             WHERE wave_id = {p1}\n             ORDER BY started_at DESC\n             LIMIT {p2}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_task,\n                    snapshot_direction, snapshot_area, snapshot_pr, flow_parents, execution_cursor,\n                    parent_run_id, parent_pr_number, stack_position,\n                    stack_group_id, stack_status, lineage_inferred, target_branch, repair_of\n             FROM runs WHERE id = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_task,\n                    snapshot_direction, snapshot_area, snapshot_pr, flow_parents, execution_cursor,\n                    parent_run_id, parent_pr_number, stack_position,\n                    stack_group_id, stack_status, lineage_inferred, target_branch, repair_of\n             FROM runs\n             WHERE wave_id = {p1} AND status IN ({p2}, {p3}, {p4})\n             ORDER BY started_at DESC LIMIT 1",
        sqlite_override: None,
        postgres_override: Some(
            "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_task,\n                    snapshot_direction, snapshot_area, snapshot_pr, flow_parents, execution_cursor,\n                    parent_run_id, parent_pr_number, stack_position,\n                    stack_group_id, stack_status, lineage_inferred, target_branch, repair_of\n             FROM runs\n             WHERE wave_id = {p1} AND status = ANY({p2})\n             ORDER BY started_at DESC LIMIT 1",
        ),
    },
    QueryDef {
        template: "SELECT COUNT(*) FROM runs WHERE wave_id = {p1} AND status IN ({p2}, {p3}, {p4})",
        sqlite_override: None,
        postgres_override: Some(
            "SELECT COUNT(*) FROM runs WHERE wave_id = {p1} AND status = ANY({p2})",
        ),
    },
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_task,\n                    snapshot_direction, snapshot_area, snapshot_pr, flow_parents, execution_cursor,\n                    parent_run_id, parent_pr_number, stack_position,\n                    stack_group_id, stack_status, lineage_inferred, target_branch, repair_of\n             FROM runs WHERE wave_id = {p1}\n             ORDER BY started_at DESC LIMIT 1",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO runs (\n                id, wave_id, iteration, step_index, status, worktree, branch,\n                started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_task,\n                snapshot_direction, snapshot_area, snapshot_pr, flow_parents, execution_cursor,\n                parent_run_id, parent_pr_number, stack_position, stack_group_id,\n                stack_status, lineage_inferred, target_branch, repair_of\n            ) VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7}, {p8}, {p9}, {p10}, {p11}, {p12}, {p13}, {p14}, {p15}, {p16}, {p17}, {p18}, {p19}, {p20}, {p21}, {p22}, {p23}, {p24}, {p25}, {p26})",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "UPDATE runs\n             SET iteration = {p1}, step_index = {p2}, status = {p3}, worktree = {p4},\n                 branch = {p5}, started_at = {p6}, ended_at = {p7}, error = {p8},\n                 snapshot_repo = {p9}, snapshot_flow = {p10}, snapshot_task = {p11},\n                 snapshot_direction = {p12}, snapshot_area = {p13}, snapshot_pr = {p14},\n                 flow_parents = {p15}, execution_cursor = {p16},\n                 parent_run_id = {p17}, parent_pr_number = {p18}, stack_position = {p19},\n                 stack_group_id = {p20}, stack_status = {p21}, lineage_inferred = {p22},\n                 target_branch = {p23}, repair_of = {p24}\n             WHERE id = {p25}",
        sqlite_override: None,
        postgres_override: None,
    },
    // NOTE: 'ci-fix' literal must match types::CI_FIX_FLOW constant.
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_task,\n                    snapshot_direction, snapshot_area, snapshot_pr, flow_parents, execution_cursor,\n                    parent_run_id, parent_pr_number, stack_position,\n                    stack_group_id, stack_status, lineage_inferred, target_branch, repair_of\n             FROM runs\n             WHERE wave_id = {p1} AND snapshot_flow <> 'ci-fix'\n             ORDER BY stack_position ASC, started_at ASC, id ASC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "UPDATE runs SET status = {p1}, error = {p2}, ended_at = {p3}\n             WHERE status IN ({p4}, {p5}, {p6})",
        sqlite_override: None,
        postgres_override: Some(
            "UPDATE runs SET status = {p1}, error = {p2}, ended_at = {p3}\n             WHERE status = ANY({p4})",
        ),
    },
    QueryDef {
        template: "SELECT repo_id, pr_number, state, is_draft, head_ref, head_sha, base_ref,\n                    updated_at, merged_at, synced_at\n             FROM live_pr_states\n             WHERE repo_id = {p1} AND pr_number = {p2}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO live_pr_states (\n                repo_id, pr_number, state, is_draft, head_ref, head_sha, base_ref,\n                updated_at, merged_at, synced_at\n            ) VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7}, {p8}, {p9}, {p10})\n            ON CONFLICT(repo_id, pr_number) DO UPDATE SET\n                state = excluded.state,\n                is_draft = excluded.is_draft,\n                head_ref = excluded.head_ref,\n                head_sha = excluded.head_sha,\n                base_ref = excluded.base_ref,\n                updated_at = excluded.updated_at,\n                merged_at = excluded.merged_at,\n                synced_at = excluded.synced_at",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, run_id, step_index, branch_index, status, worktree\n             FROM fork_runs WHERE run_id = {p1} AND step_index = {p2}\n             ORDER BY branch_index ASC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO fork_runs (id, run_id, step_index, branch_index, status, worktree)\n             VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6})\n             ON CONFLICT(id) DO UPDATE SET\n                 run_id = excluded.run_id,\n                 step_index = excluded.step_index,\n                 branch_index = excluded.branch_index,\n                 status = excluded.status,\n                 worktree = excluded.worktree",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "DELETE FROM fork_runs WHERE run_id = {p1} AND step_index = {p2}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, content, source_hash, token_budget, model, created_at\n             FROM summaries WHERE wave_id = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO summaries (id, wave_id, content, source_hash, token_budget, model, created_at)\n             VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7})\n             ON CONFLICT(wave_id) DO UPDATE SET\n                 content = excluded.content,\n                 source_hash = excluded.source_hash,\n                 token_budget = excluded.token_budget,\n                 model = excluded.model,\n                 created_at = excluded.created_at",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT wave_id, name, content, position, updated_at\n             FROM chat_memory_blocks\n             WHERE wave_id = {p1}\n             ORDER BY position ASC, name ASC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO chat_memory_blocks (wave_id, name, content, position, updated_at)\n             VALUES ({p1}, {p2}, {p3}, {p4}, {p5})\n             ON CONFLICT(wave_id, name) DO UPDATE SET\n                 content = excluded.content,\n                 position = excluded.position,\n                 updated_at = excluded.updated_at",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "DELETE FROM chat_memory_blocks WHERE wave_id = {p1} AND name = {p2}",
        sqlite_override: None,
        postgres_override: None,
    },
    // ListWaveRepos
    QueryDef {
        template: "SELECT wave_id, repo, worktree, branch, status, iteration, cycle_start_iteration, position\n             FROM wave_repos WHERE wave_id = {p1}\n             ORDER BY position ASC",
        sqlite_override: None,
        postgres_override: None,
    },
    // UpsertWaveRepo
    QueryDef {
        template: "INSERT INTO wave_repos (wave_id, repo, worktree, branch, status, iteration, cycle_start_iteration, position)\n             VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7}, {p8})\n             ON CONFLICT(wave_id, repo) DO UPDATE SET\n                 worktree = excluded.worktree,\n                 branch = excluded.branch,\n                 status = excluded.status,\n                 iteration = excluded.iteration,\n                 cycle_start_iteration = excluded.cycle_start_iteration,\n                 position = excluded.position",
        sqlite_override: None,
        postgres_override: None,
    },
    // DeleteWaveReposByWave
    QueryDef {
        template: "DELETE FROM wave_repos WHERE wave_id = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    // ResetStaleActiveRepos — mirror ResetStaleActiveWaves onto per-repo rows so
    // the rolled-up wave status un-sticks after an lfd restart.
    QueryDef {
        template: "UPDATE wave_repos SET status = {p1}\n             WHERE status IN ({p2}, {p3})",
        sqlite_override: None,
        postgres_override: Some(
            "UPDATE wave_repos SET status = {p1}\n             WHERE status = ANY({p2})",
        ),
    },
    // ListChildWaves — a chord's contents are its children, ordered by creation.
    QueryDef {
        template: "SELECT id, name, direction, area, paused, created_at, workers, mode,\n                    primary_flow, goal, metrics, parent_wave_id\n             FROM waves\n             WHERE parent_wave_id = {p1}\n             ORDER BY created_at ASC",
        sqlite_override: None,
        postgres_override: None,
    },
    // RecordRunTokenUsage — one row per run, replaced if re-recorded.
    QueryDef {
        template: "INSERT INTO run_token_usage (\n                run_id, wave, provider, model, input_tokens, output_tokens, cache_read_tokens, recorded_at, repo\n            ) VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7}, {p8}, {p9})\n            ON CONFLICT(run_id) DO UPDATE SET\n                wave = excluded.wave,\n                provider = excluded.provider,\n                model = excluded.model,\n                input_tokens = excluded.input_tokens,\n                output_tokens = excluded.output_tokens,\n                cache_read_tokens = excluded.cache_read_tokens,\n                recorded_at = excluded.recorded_at,\n                repo = excluded.repo",
        sqlite_override: None,
        postgres_override: None,
    },
    // AggregateTokenUsageByWaveProvider — totals grouped by wave and provider.
    // CAST keeps postgres SUM(BIGINT) (NUMERIC) readable as i64, matching sqlite.
    QueryDef {
        template: "SELECT wave, provider,\n                    CAST(COALESCE(SUM(input_tokens), 0) AS BIGINT),\n                    CAST(COALESCE(SUM(output_tokens), 0) AS BIGINT),\n                    CAST(COALESCE(SUM(cache_read_tokens), 0) AS BIGINT)\n             FROM run_token_usage\n             GROUP BY wave, provider\n             ORDER BY wave, provider",
        sqlite_override: None,
        postgres_override: None,
    },
    // AggregateTokenUsageByRepoProvider — totals grouped by repo and provider.
    // repo is nullable (rows recorded before migration 043 have NULL); NULLs
    // group together. CAST mirrors the wave/provider aggregate above.
    QueryDef {
        template: "SELECT repo, provider,\n                    CAST(COALESCE(SUM(input_tokens), 0) AS BIGINT),\n                    CAST(COALESCE(SUM(output_tokens), 0) AS BIGINT),\n                    CAST(COALESCE(SUM(cache_read_tokens), 0) AS BIGINT)\n             FROM run_token_usage\n             GROUP BY repo, provider\n             ORDER BY repo, provider",
        sqlite_override: None,
        postgres_override: None,
    },
];

#[derive(Debug)]
struct RenderedCatalog {
    sqlite: Vec<String>,
    postgres: Vec<String>,
}

static RENDERED: LazyLock<RenderedCatalog> = LazyLock::new(|| {
    let mut sqlite = Vec::with_capacity(QUERY_DEFS.len());
    let mut postgres = Vec::with_capacity(QUERY_DEFS.len());

    for query in Query::ALL {
        let def = QUERY_DEFS[query as usize];
        sqlite.push(render_sql(
            def.sqlite_override.unwrap_or(def.template),
            SqlDialect::Sqlite,
        ));
        postgres.push(render_sql(
            def.postgres_override.unwrap_or(def.template),
            SqlDialect::Postgres,
        ));
    }

    RenderedCatalog { sqlite, postgres }
});

pub(crate) fn sql(query: Query, dialect: SqlDialect) -> &'static str {
    let index = query as usize;
    match dialect {
        SqlDialect::Sqlite => RENDERED.sqlite[index].as_str(),
        SqlDialect::Postgres => RENDERED.postgres[index].as_str(),
    }
}

pub(crate) fn list_waves_query(has_repo: bool) -> Query {
    if has_repo {
        Query::ListWavesByRepo
    } else {
        Query::ListWaves
    }
}

pub(crate) fn list_runs_query(has_wave_id: bool, has_limit: bool) -> Query {
    match (has_wave_id, has_limit) {
        (false, false) => Query::ListRunsAll,
        (true, false) => Query::ListRunsByWave,
        (false, true) => Query::ListRunsLimited,
        (true, true) => Query::ListRunsByWaveLimited,
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
                SqlDialect::Postgres => {
                    output.push('$');
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
    fn every_query_renders_for_both_dialects_with_valid_placeholders() {
        for query in Query::ALL {
            let sqlite = std::panic::catch_unwind(|| sql(query, SqlDialect::Sqlite))
                .unwrap_or_else(|_| panic!("sqlite rendering panicked for {query:?}"));
            let postgres = std::panic::catch_unwind(|| sql(query, SqlDialect::Postgres))
                .unwrap_or_else(|_| panic!("postgres rendering panicked for {query:?}"));

            assert!(
                !sqlite.trim().is_empty(),
                "sqlite rendering is empty for {query:?}"
            );
            assert!(
                !postgres.trim().is_empty(),
                "postgres rendering is empty for {query:?}"
            );
            assert!(
                !sqlite.contains("{p"),
                "sqlite rendering still has placeholders for {query:?}"
            );
            assert!(
                !postgres.contains("{p"),
                "postgres rendering still has placeholders for {query:?}"
            );
            assert!(
                rendered_placeholders_are_contiguous(sqlite, b'?'),
                "sqlite placeholders not contiguous for {query:?}: {sqlite}"
            );
            assert!(
                rendered_placeholders_are_contiguous(postgres, b'$'),
                "postgres placeholders not contiguous for {query:?}: {postgres}"
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
            if let Some(postgres_override) = def.postgres_override {
                assert!(
                    placeholders_are_contiguous(postgres_override),
                    "postgres override has non-contiguous placeholders for {query:?}"
                );
            }
        }
    }
}
