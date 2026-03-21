use std::collections::HashMap;
use std::time::Duration;

use deadpool_postgres::{Manager, Pool};
use tokio_postgres::types::ToSql;
use tokio_postgres::NoTls;

use crate::lfd::attention::{queue_block_attention_item, queue_block_from_attention};
use crate::lfd::id::LfdId;
use crate::lfd::sessions::types::{
    PersistedSessionEvent, Session, SessionConfig, SessionEvent, SessionStatus,
};
use crate::lfd::store::catalog::{
    list_agent_history_query, list_triggers_query, list_wave_runs_query, list_waves_query, sql,
    Query, SqlDialect,
};
use crate::lfd::store::rows::{
    map_activation_log_row, map_agent_row, map_chat_memory_block_row, map_chat_message_row,
    map_fork_run_row, map_live_pr_state_row, map_pending_activation_row, map_repo_edge_row,
    map_repo_row, map_summary_row, map_trigger_row, map_wave_row, map_wave_run_row, now_unix,
    serialize_pr,
};
use crate::lfd::store::token_crypto;
use crate::lfd::store::{ForkRun, ForkRunStatus, SessionFilters, StoreError, StoreResult};
use crate::lfd::types::{
    ActivationLog, AgentRun, AgentStatus, AttentionItem, AttentionKind, AttentionStatus,
    ChatMemoryBlock, ChatMessage, LivePullRequestState, PendingActivation, QueueBlock,
    QueueMergeEvent, Repo, RepoEdge, RepoId, Summary, TerminalSession, TerminalSessionStatus,
    Trigger, Wave, WaveRun, WaveRunStatus, WaveStatus,
};

const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(500),
    Duration::from_secs(2),
];

fn decrypt_token_row(row: &tokio_postgres::Row) -> StoreResult<super::ProviderToken> {
    let provider: String = row.get(0);
    let access_token: String = row.get(1);
    let refresh_token: Option<String> = row.get(2);
    let expires_at: Option<i64> = row.get(3);
    let login: Option<String> = row.get(4);
    let updated_at: i64 = row.get(5);
    let ct: String = row.get(6);
    let encrypted: bool = row.get(7);

    let access_token =
        token_crypto::decrypt_if_needed(&access_token, encrypted).map_err(|error| {
            StoreError::InvalidData(format!(
                "failed to decrypt access token for provider '{provider}': {error}"
            ))
        })?;
    let refresh_token = refresh_token
        .as_deref()
        .map(|token| token_crypto::decrypt_if_needed(token, encrypted))
        .transpose()
        .map_err(|error| {
            StoreError::InvalidData(format!(
                "failed to decrypt refresh token for provider '{provider}': {error}"
            ))
        })?;
    Ok(super::ProviderToken {
        provider,
        access_token,
        refresh_token,
        expires_at,
        login,
        updated_at,
        credential_type: super::CredentialType::from_db(&ct),
    })
}

#[derive(Debug)]
pub struct PostgresStore {
    pool: Pool,
}

impl PostgresStore {
    fn sql(query: Query) -> &'static str {
        sql(query, SqlDialect::Postgres)
    }

    fn map_attention_item_row(row: &tokio_postgres::Row) -> StoreResult<AttentionItem> {
        let kind_raw: String = row.try_get(3)?;
        let status_raw: String = row.try_get(4)?;
        Ok(AttentionItem {
            id: LfdId::from_raw(row.try_get::<_, String>(0)?),
            wave_id: LfdId::from_raw(row.try_get::<_, String>(1)?),
            run_id: row.try_get::<_, Option<String>>(2)?.map(LfdId::from_raw),
            kind: kind_raw
                .parse::<AttentionKind>()
                .map_err(StoreError::InvalidData)?,
            status: status_raw
                .parse::<AttentionStatus>()
                .map_err(StoreError::InvalidData)?,
            title: row.try_get(5)?,
            summary: row.try_get(6)?,
            context: serde_json::from_str(&row.try_get::<_, String>(7)?)
                .unwrap_or(serde_json::Value::Object(Default::default())),
            surfaced_at: crate::lfd::store::rows::unix_to_datetime(row.try_get(8)?),
            viewed_at: row
                .try_get::<_, Option<i64>>(9)?
                .map(crate::lfd::store::rows::unix_to_datetime),
            resolved_at: row
                .try_get::<_, Option<i64>>(10)?
                .map(crate::lfd::store::rows::unix_to_datetime),
        })
    }

    pub async fn connect_async(database_url: &str) -> StoreResult<Self> {
        let pool = build_pool(database_url)?;
        let version = super::migrations::latest_version_postgres_pool(&pool).await?;
        if version.is_empty() {
            return Err(StoreError::InvalidData(
                "postgres schema missing; run `lfd migrate`".to_string(),
            ));
        }
        let store = Self { pool };
        store.migrate_plaintext_provider_tokens().await?;
        Ok(store)
    }

    pub async fn migrate_async(database_url: &str) -> StoreResult<String> {
        let (mut client, connection) = tokio_postgres::connect(database_url, NoTls).await?;
        let connection_task = tokio::spawn(async move {
            let _ = connection.await;
        });
        super::migrations::apply_postgres(&mut client).await?;
        let version = super::migrations::latest_version_postgres_client(&client).await?;
        connection_task.abort();
        Ok(version)
    }

    pub async fn migrate_status_async(database_url: &str) -> StoreResult<String> {
        let (client, connection) = tokio_postgres::connect(database_url, NoTls).await?;
        let connection_task = tokio::spawn(async move {
            let _ = connection.await;
        });
        let result = super::migrations::latest_version_postgres_client(&client).await;
        connection_task.abort();
        result
    }

    async fn with_client<T, F, Fut>(&self, func: F) -> StoreResult<T>
    where
        F: FnOnce(deadpool_postgres::Client) -> Fut,
        Fut: std::future::Future<Output = StoreResult<T>>,
    {
        let client = get_client_with_retry(&self.pool).await?;
        func(client).await
    }

    async fn migrate_plaintext_provider_tokens(&self) -> StoreResult<()> {
        self.with_client(|mut client| async move {
            let rows = client
                .query(
                    "SELECT provider, access_token, refresh_token
                     FROM provider_tokens
                     WHERE encrypted = FALSE",
                    &[],
                )
                .await?;
            if rows.is_empty() {
                return Ok(());
            }

            let tx = client.transaction().await?;
            for row in rows {
                let provider: String = row.get(0);
                let access_token: String = row.get(1);
                let refresh_token: Option<String> = row.get(2);
                let encrypted_access =
                    token_crypto::encrypt_token(&access_token).map_err(|error| {
                        StoreError::InvalidData(format!(
                            "failed to encrypt existing access token for provider '{provider}': {error}"
                        ))
                    })?;
                let encrypted_refresh =
                    token_crypto::encrypt_optional(refresh_token.as_deref()).map_err(|error| {
                        StoreError::InvalidData(format!(
                            "failed to encrypt existing refresh token for provider '{provider}': {error}"
                        ))
                    })?;
                tx.execute(
                        "UPDATE provider_tokens
                         SET access_token = $1,
                             refresh_token = $2,
                             encrypted = TRUE
                         WHERE provider = $3",
                        &[&encrypted_access, &encrypted_refresh, &provider],
                    )
                    .await?;
            }
            tx.commit().await?;
            Ok(())
        })
        .await
    }

    async fn read_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.with_client(|client| async move {
            let rows = if let Some(repo) = repo {
                client
                    .query(Self::sql(list_waves_query(true)), &[&repo])
                    .await?
            } else {
                client
                    .query(Self::sql(list_waves_query(false)), &[])
                    .await?
            };
            rows.iter().map(map_wave_row).collect()
        })
        .await
    }

    async fn upsert_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.with_client(|client| async move {
            let direction_json = serde_json::to_string(wave.direction())?;
            let area_json = serde_json::to_string(wave.area())?;
            let paused: i32 = if wave.status() == WaveStatus::Paused {
                1
            } else {
                0
            };
            let created_at = wave.created_at().map(|dt| dt.unix_timestamp()).unwrap_or(0);

            let workers = wave.workers() as i32;
            let mode = wave.mode().as_str();
            let primary_flow = wave.primary_flow();
            let cron = wave.cron.as_deref();
            client
                .execute(
                    Self::sql(Query::UpsertWave),
                    &[
                        &wave.id().as_str(),
                        &wave.name().as_str(),
                        &wave.repo().as_str(),
                        &direction_json.as_str(),
                        &area_json.as_str(),
                        &paused,
                        &(wave.status().as_i32()),
                        &(wave.iteration() as i32),
                        &(wave.cycle_start_iteration() as i32),
                        &created_at,
                        &workers,
                        &mode,
                        &primary_flow.as_str(),
                        &cron,
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    fn map_session_row(row: &tokio_postgres::Row) -> StoreResult<Session> {
        let config: SessionConfig = serde_json::from_str(row.get::<_, &str>(5))?;
        Ok(Session {
            id: row.get(0),
            harness: row.get(1),
            status: SessionStatus::from_i32(row.get::<_, i32>(2)),
            wave_run_id: row.get(3),
            provider_session_id: row.get(4),
            config,
            created_at: crate::lfd::store::rows::unix_to_datetime(row.get(6)),
            ended_at: row
                .get::<_, Option<i64>>(7)
                .map(crate::lfd::store::rows::unix_to_datetime),
        })
    }

    fn map_terminal_session_row(row: &tokio_postgres::Row) -> StoreResult<TerminalSession> {
        Ok(TerminalSession {
            id: row.get(0),
            wave_id: row.get(1),
            wave_run_id: row.get(2),
            step: row.get(3),
            agent: row.get(4),
            cwd: row.get(5),
            argv: serde_json::from_str(row.get::<_, &str>(6))?,
            env: serde_json::from_str(row.get::<_, &str>(7))?,
            source: row.get(8),
            tmux_name: row.get(9),
            status: TerminalSessionStatus::from_i32(row.get::<_, i32>(10)),
            completion_token: row.get(11),
            created_at: crate::lfd::store::rows::unix_to_datetime(row.get(12)),
            attached_at: row
                .get::<_, Option<i64>>(13)
                .map(crate::lfd::store::rows::unix_to_datetime),
            started_at: row
                .get::<_, Option<i64>>(14)
                .map(crate::lfd::store::rows::unix_to_datetime),
            completed_at: row
                .get::<_, Option<i64>>(15)
                .map(crate::lfd::store::rows::unix_to_datetime),
        })
    }
}

impl PostgresStore {
    pub async fn health_check(&self) -> StoreResult<()> {
        self.with_client(|client| async move {
            client.execute(Self::sql(Query::HealthCheck), &[]).await?;
            Ok(())
        })
        .await
    }

    pub async fn schema_version(&self) -> StoreResult<String> {
        super::migrations::latest_version_postgres_pool(&self.pool).await
    }

    // -- Provider tokens -------------------------------------------------------

    pub async fn get_provider_token(
        &self,
        provider: &str,
    ) -> StoreResult<Option<super::ProviderToken>> {
        let provider = provider.to_string();
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT provider, access_token, refresh_token, expires_at, login, updated_at, credential_type, encrypted
                     FROM provider_tokens WHERE provider = $1",
                    &[&provider],
                )
                .await?;
            let Some(row) = row else {
                return Ok(None);
            };
            Ok(Some(decrypt_token_row(&row)?))
        })
        .await
    }

    pub async fn upsert_provider_token(&self, token: &super::ProviderToken) -> StoreResult<()> {
        let token = token.clone();
        let encrypted_access =
            token_crypto::encrypt_token(&token.access_token).map_err(|error| {
                StoreError::InvalidData(format!(
                    "failed to encrypt access token for provider '{}': {error}",
                    token.provider
                ))
            })?;
        let encrypted_refresh = token_crypto::encrypt_optional(token.refresh_token.as_deref())
            .map_err(|error| {
                StoreError::InvalidData(format!(
                    "failed to encrypt refresh token for provider '{}': {error}",
                    token.provider
                ))
            })?;
        self.with_client(|client| async move {
            client
                .execute(
                    "INSERT INTO provider_tokens (provider, access_token, refresh_token, expires_at, login, updated_at, credential_type, encrypted)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, TRUE)
                     ON CONFLICT(provider) DO UPDATE SET
                        access_token = excluded.access_token,
                        refresh_token = excluded.refresh_token,
                        expires_at = excluded.expires_at,
                        login = excluded.login,
                        updated_at = excluded.updated_at,
                        credential_type = excluded.credential_type,
                        encrypted = excluded.encrypted",
                    &[
                        &token.provider,
                        &encrypted_access,
                        &encrypted_refresh,
                        &token.expires_at,
                        &token.login,
                        &token.updated_at,
                        &token.credential_type.as_str(),
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn delete_provider_token(&self, provider: &str) -> StoreResult<()> {
        let provider = provider.to_string();
        self.with_client(|client| async move {
            client
                .execute(
                    "DELETE FROM provider_tokens WHERE provider = $1",
                    &[&provider],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn list_provider_tokens(&self) -> StoreResult<Vec<super::ProviderToken>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT provider, access_token, refresh_token, expires_at, login, updated_at, credential_type, encrypted
                     FROM provider_tokens ORDER BY provider",
                    &[],
                )
                .await?;
            let mut tokens = Vec::with_capacity(rows.len());
            for row in rows {
                tokens.push(decrypt_token_row(&row)?);
            }
            Ok(tokens)
        })
        .await
    }

    // -- Repos -----------------------------------------------------------------

    pub async fn list_repos(&self) -> StoreResult<Vec<Repo>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT path, repo_id, name, added_at FROM repos ORDER BY path ASC",
                    &[],
                )
                .await?;
            rows.iter().map(map_repo_row).collect()
        })
        .await
    }

    pub async fn get_repo(&self, path: &str) -> StoreResult<Option<Repo>> {
        let path = path.to_string();
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT path, repo_id, name, added_at FROM repos WHERE path = $1 LIMIT 1",
                    &[&path],
                )
                .await?;
            row.as_ref().map(map_repo_row).transpose()
        })
        .await
    }

    pub async fn get_repo_by_repo_id(&self, repo_id: &RepoId) -> StoreResult<Option<Repo>> {
        let repo_id = repo_id.to_string();
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT path, repo_id, name, added_at FROM repos WHERE repo_id = $1 LIMIT 1",
                    &[&repo_id],
                )
                .await?;
            row.as_ref().map(map_repo_row).transpose()
        })
        .await
    }

    pub async fn upsert_repo(&self, repo: &Repo) -> StoreResult<()> {
        let repo = repo.clone();
        self.with_client(|client| async move {
            let repo_id = repo.repo_id.to_string();
            client
                .execute(
                    "INSERT INTO repos (path, repo_id, name, added_at)
                     VALUES ($1, $2, $3, $4)
                     ON CONFLICT(path) DO UPDATE SET repo_id = EXCLUDED.repo_id, name = EXCLUDED.name, added_at = EXCLUDED.added_at",
                    &[
                        &repo.path,
                        &repo_id,
                        &repo.name,
                        &repo.added_at.unix_timestamp(),
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn delete_repo(&self, path: &str) -> StoreResult<()> {
        let path = path.to_string();
        self.with_client(|client| async move {
            client
                .execute("DELETE FROM repos WHERE path = $1", &[&path])
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn list_edges(&self) -> StoreResult<Vec<RepoEdge>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT parent_repo_id, child_repo_id FROM repo_edges ORDER BY parent_repo_id, child_repo_id",
                    &[],
                )
                .await?;
            rows.iter().map(map_repo_edge_row).collect()
        })
        .await
    }

    pub async fn add_edge(&self, edge: &RepoEdge) -> StoreResult<()> {
        let edge = edge.clone();
        self.with_client(|client| async move {
            let parent_repo_id = edge.parent_repo_id.to_string();
            let child_repo_id = edge.child_repo_id.to_string();
            client
                .execute(
                    "INSERT INTO repo_edges (parent_repo_id, child_repo_id) VALUES ($1, $2)
                     ON CONFLICT (parent_repo_id, child_repo_id) DO NOTHING",
                    &[&parent_repo_id, &child_repo_id],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn remove_edge(&self, parent_id: &RepoId, child_id: &RepoId) -> StoreResult<()> {
        let parent_id = parent_id.to_string();
        let child_id = child_id.to_string();
        self.with_client(|client| async move {
            client
                .execute(
                    "DELETE FROM repo_edges WHERE parent_repo_id = $1 AND child_repo_id = $2",
                    &[&parent_id, &child_id],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn children(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>> {
        let repo_id = repo_id.to_string();
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT repos.path, repos.repo_id, repos.name, repos.added_at
                     FROM repo_edges
                     INNER JOIN repos ON repos.repo_id = repo_edges.child_repo_id
                     WHERE repo_edges.parent_repo_id = $1
                     ORDER BY repos.path ASC",
                    &[&repo_id],
                )
                .await?;
            rows.iter().map(map_repo_row).collect()
        })
        .await
    }

    pub async fn parents(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>> {
        let repo_id = repo_id.to_string();
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT repos.path, repos.repo_id, repos.name, repos.added_at
                     FROM repo_edges
                     INNER JOIN repos ON repos.repo_id = repo_edges.parent_repo_id
                     WHERE repo_edges.child_repo_id = $1
                     ORDER BY repos.path ASC",
                    &[&repo_id],
                )
                .await?;
            rows.iter().map(map_repo_row).collect()
        })
        .await
    }

    // -- Sessions ---------------------------------------------------------------

    pub async fn create_session(&self, session: &Session) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(
                    "INSERT INTO sessions (id, harness, status, wave_run_id, provider_session_id, config, created_at, ended_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                    &[
                        &session.id,
                        &session.harness,
                        &session.status.as_i32(),
                        &session.wave_run_id,
                        &session.provider_session_id,
                        &serde_json::to_string(&session.config)?,
                        &session.created_at.unix_timestamp(),
                        &session.ended_at.map(|dt| dt.unix_timestamp()),
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn get_session(&self, session_id: &LfdId) -> StoreResult<Option<Session>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, harness, status, wave_run_id, provider_session_id, config, created_at, ended_at
                     FROM sessions
                     WHERE id = $1",
                    &[&session_id],
                )
                .await?;
            row.as_ref().map(Self::map_session_row).transpose()
        })
        .await
    }

    pub async fn get_active_session_for_wave_run(
        &self,
        wave_run_id: &str,
    ) -> StoreResult<Option<Session>> {
        let wave_run_id = wave_run_id.to_string();
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, harness, status, wave_run_id, provider_session_id, config, created_at, ended_at
                     FROM sessions
                     WHERE wave_run_id = $1 AND status = ANY($2)
                     ORDER BY created_at DESC
                     LIMIT 1",
                    &[
                        &wave_run_id,
                        &&[
                            SessionStatus::Starting.as_i32(),
                            SessionStatus::Active.as_i32(),
                            SessionStatus::Ending.as_i32(),
                        ][..],
                    ],
                )
                .await?;
            row.as_ref().map(Self::map_session_row).transpose()
        })
        .await
    }

    pub async fn update_provider_session_id(
        &self,
        session_id: &LfdId,
        provider_session_id: &str,
    ) -> StoreResult<()> {
        let provider_session_id = provider_session_id.to_string();
        self.with_client(|client| async move {
            let updated = client
                .execute(
                    "UPDATE sessions SET provider_session_id = $2 WHERE id = $1",
                    &[&session_id, &provider_session_id],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn update_session_status(
        &self,
        session_id: &LfdId,
        status: SessionStatus,
        ended_at: Option<i64>,
    ) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated = client
                .execute(
                    "UPDATE sessions
                     SET status = $2, ended_at = COALESCE($3, ended_at)
                     WHERE id = $1",
                    &[&session_id, &status.as_i32(), &ended_at],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn append_session_event(
        &self,
        session_id: &LfdId,
        seq: i64,
        event: &SessionEvent,
        created_at: i64,
    ) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(
                    "INSERT INTO session_events (session_id, seq, event_type, data, created_at)
                     VALUES ($1, $2, $3, $4, $5)",
                    &[
                        &session_id,
                        &seq,
                        &event.event_type(),
                        &serde_json::to_string(event)?,
                        &created_at,
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn list_session_events(
        &self,
        session_id: &LfdId,
        after_seq: Option<i64>,
    ) -> StoreResult<Vec<PersistedSessionEvent>> {
        self.with_client(|client| async move {
            let rows = if let Some(after_seq) = after_seq {
                client
                    .query(
                        "SELECT session_id, seq, data, created_at
                         FROM session_events
                         WHERE session_id = $1 AND seq > $2
                         ORDER BY seq ASC",
                        &[&session_id, &after_seq],
                    )
                    .await?
            } else {
                client
                    .query(
                        "SELECT session_id, seq, data, created_at
                         FROM session_events
                         WHERE session_id = $1
                         ORDER BY seq ASC",
                        &[&session_id],
                    )
                    .await?
            };

            rows.iter()
                .map(|row| {
                    let event: SessionEvent = serde_json::from_str(row.get::<_, &str>(2))?;
                    Ok(PersistedSessionEvent {
                        session_id: row.get(0),
                        seq: row.get(1),
                        event,
                        created_at: crate::lfd::store::rows::unix_to_datetime(row.get(3)),
                    })
                })
                .collect()
        })
        .await
    }

    pub async fn list_sessions_by_statuses(
        &self,
        statuses: &[SessionStatus],
    ) -> StoreResult<Vec<Session>> {
        let status_ints: Vec<i32> = statuses.iter().map(|s| s.as_i32()).collect();
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT id, harness, status, wave_run_id, provider_session_id, config, created_at, ended_at
                     FROM sessions WHERE status = ANY($1)
                     ORDER BY created_at ASC",
                    &[&status_ints],
                )
                .await?;
            rows.iter().map(Self::map_session_row).collect()
        })
        .await
    }

    pub async fn list_events_for_sessions(
        &self,
        session_ids: &[LfdId],
    ) -> StoreResult<HashMap<LfdId, Vec<PersistedSessionEvent>>> {
        if session_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let session_id_strings: Vec<String> = session_ids.iter().map(|id| id.to_string()).collect();
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT session_id, seq, data, created_at
                     FROM session_events
                     WHERE session_id = ANY($1)
                     ORDER BY session_id, seq ASC",
                    &[&session_id_strings],
                )
                .await?;
            let mut result: HashMap<LfdId, Vec<PersistedSessionEvent>> = HashMap::new();
            for row in &rows {
                let session_id: LfdId = row.get(0);
                let event: SessionEvent = serde_json::from_str(row.get::<_, &str>(2))?;
                result
                    .entry(session_id.clone())
                    .or_default()
                    .push(PersistedSessionEvent {
                        session_id,
                        seq: row.get(1),
                        event,
                        created_at: crate::lfd::store::rows::unix_to_datetime(row.get(3)),
                    });
            }
            Ok(result)
        })
        .await
    }

    pub async fn list_sessions_for_wave(&self, wave_id: &str) -> StoreResult<Vec<Session>> {
        let wave_id = wave_id.to_string();
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT s.id, s.harness, s.status, s.wave_run_id, s.provider_session_id, s.config, s.created_at, s.ended_at
                     FROM sessions s
                     JOIN wave_runs wr ON wr.id = s.wave_run_id
                     WHERE wr.wave_id = $1
                     ORDER BY s.created_at ASC",
                    &[&wave_id],
                )
                .await?;
            rows.iter().map(Self::map_session_row).collect()
        })
        .await
    }

    pub async fn list_sessions_filtered(
        &self,
        filters: &SessionFilters,
    ) -> StoreResult<Vec<Session>> {
        enum QueryParam {
            Text(String),
            Int(i64),
        }

        impl QueryParam {
            fn as_tosql(&self) -> &(dyn ToSql + Sync) {
                match self {
                    Self::Text(value) => value,
                    Self::Int(value) => value,
                }
            }
        }

        let filters = filters.clone();
        self.with_client(|client| async move {
            let mut query = String::from(
                "SELECT s.id, s.harness, s.status, s.wave_run_id, s.provider_session_id, s.config, s.created_at, s.ended_at
                 FROM sessions s",
            );
            let mut predicates = Vec::new();
            let mut params: Vec<QueryParam> = Vec::new();

            if filters.wave.is_some() || filters.flow.is_some() {
                query.push_str(" JOIN wave_runs wr ON wr.id = s.wave_run_id");
            }
            if let Some(wave) = filters.wave.as_ref() {
                predicates.push(format!("wr.wave_id = ${}", params.len() + 1));
                params.push(QueryParam::Text(wave.clone()));
            }
            if let Some(flow) = filters.flow.as_ref() {
                predicates.push(format!("wr.snapshot_flow = ${}", params.len() + 1));
                params.push(QueryParam::Text(flow.clone()));
            }
            if let Some(step) = filters.step.as_ref() {
                predicates.push(format!("(s.config::jsonb ->> 'step') = ${}", params.len() + 1));
                params.push(QueryParam::Text(step.clone()));
            }
            if let Some(from) = filters.from {
                predicates.push(format!("s.created_at >= ${}", params.len() + 1));
                params.push(QueryParam::Int(from));
            }
            if let Some(to) = filters.to {
                predicates.push(format!("s.created_at <= ${}", params.len() + 1));
                params.push(QueryParam::Int(to));
            }
            if !predicates.is_empty() {
                query.push_str(" WHERE ");
                query.push_str(&predicates.join(" AND "));
            }
            query.push_str(" ORDER BY s.created_at ASC");

            let param_refs: Vec<&(dyn ToSql + Sync)> =
                params.iter().map(QueryParam::as_tosql).collect();
            let rows = client.query(&query, &param_refs).await?;
            rows.iter().map(Self::map_session_row).collect()
        })
        .await
    }

    const TERMINAL_SESSION_COLS: &str =
        "id, wave_id, wave_run_id, step, agent, cwd, argv, env, source, tmux_name, status, \
         completion_token, created_at, attached_at, started_at, completed_at";

    pub async fn create_terminal_session(&self, session: &TerminalSession) -> StoreResult<()> {
        let cols = Self::TERMINAL_SESSION_COLS;
        self.with_client(|client| async move {
            client
                .execute(
                    &format!(
                        "INSERT INTO terminal_sessions ({cols}) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)"
                    ),
                    &[
                        &session.id,
                        &session.wave_id,
                        &session.wave_run_id,
                        &session.step,
                        &session.agent,
                        &session.cwd,
                        &serde_json::to_string(&session.argv)?,
                        &serde_json::to_string(&session.env)?,
                        &session.source,
                        &session.tmux_name,
                        &session.status.as_i32(),
                        &session.completion_token,
                        &session.created_at.unix_timestamp(),
                        &session.attached_at.map(|dt| dt.unix_timestamp()),
                        &session.started_at.map(|dt| dt.unix_timestamp()),
                        &session.completed_at.map(|dt| dt.unix_timestamp()),
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn get_terminal_session(
        &self,
        session_id: &LfdId,
    ) -> StoreResult<Option<TerminalSession>> {
        let cols = Self::TERMINAL_SESSION_COLS;
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    &format!("SELECT {cols} FROM terminal_sessions WHERE id = $1"),
                    &[&session_id],
                )
                .await?;
            row.as_ref().map(Self::map_terminal_session_row).transpose()
        })
        .await
    }

    pub async fn list_terminal_sessions(
        &self,
        wave_id: Option<&LfdId>,
        statuses: Option<&[TerminalSessionStatus]>,
    ) -> StoreResult<Vec<TerminalSession>> {
        let cols = Self::TERMINAL_SESSION_COLS;
        self.with_client(|client| async move {
            let mut sql = format!("SELECT {cols} FROM terminal_sessions");
            let mut predicates = Vec::new();
            let mut params: Vec<Box<dyn ToSql + Sync + Send>> = Vec::new();

            if let Some(wave_id) = wave_id {
                params.push(Box::new(wave_id.clone()));
                predicates.push(format!("wave_id = ${}", params.len()));
            }
            if let Some(statuses) = statuses {
                let status_ints: Vec<i32> = statuses.iter().map(|status| status.as_i32()).collect();
                params.push(Box::new(status_ints));
                predicates.push(format!("status = ANY(${})", params.len()));
            }
            if !predicates.is_empty() {
                sql.push_str(" WHERE ");
                sql.push_str(&predicates.join(" AND "));
            }
            sql.push_str(" ORDER BY created_at ASC");

            let param_refs: Vec<&(dyn ToSql + Sync)> = params
                .iter()
                .map(|p| p.as_ref() as &(dyn ToSql + Sync))
                .collect();
            let rows = client.query(&sql, &param_refs).await?;
            rows.iter().map(Self::map_terminal_session_row).collect()
        })
        .await
    }

    pub async fn get_active_terminal_session_for_wave_run(
        &self,
        wave_run_id: &LfdId,
    ) -> StoreResult<Option<TerminalSession>> {
        let cols = Self::TERMINAL_SESSION_COLS;
        let active_statuses = vec![
            TerminalSessionStatus::Pending.as_i32(),
            TerminalSessionStatus::Attached.as_i32(),
            TerminalSessionStatus::Running.as_i32(),
        ];
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    &format!(
                        "SELECT {cols} FROM terminal_sessions \
                         WHERE wave_run_id = $1 AND status = ANY($2) \
                         ORDER BY created_at DESC LIMIT 1"
                    ),
                    &[&wave_run_id, &active_statuses],
                )
                .await?;
            row.as_ref().map(Self::map_terminal_session_row).transpose()
        })
        .await
    }

    pub async fn update_terminal_session(&self, session: &TerminalSession) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated = client
                .execute(
                    "UPDATE terminal_sessions
                     SET wave_id = $2,
                         wave_run_id = $3,
                         step = $4,
                         agent = $5,
                         cwd = $6,
                         argv = $7,
                         env = $8,
                         source = $9,
                         tmux_name = $10,
                         status = $11,
                         completion_token = $12,
                         created_at = $13,
                         attached_at = $14,
                         started_at = $15,
                         completed_at = $16
                     WHERE id = $1",
                    &[
                        &session.id,
                        &session.wave_id,
                        &session.wave_run_id,
                        &session.step,
                        &session.agent,
                        &session.cwd,
                        &serde_json::to_string(&session.argv)?,
                        &serde_json::to_string(&session.env)?,
                        &session.source,
                        &session.tmux_name,
                        &session.status.as_i32(),
                        &session.completion_token,
                        &session.created_at.unix_timestamp(),
                        &session.attached_at.map(|dt| dt.unix_timestamp()),
                        &session.started_at.map(|dt| dt.unix_timestamp()),
                        &session.completed_at.map(|dt| dt.unix_timestamp()),
                    ],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.read_waves(repo).await
    }

    pub async fn list_loopable_waves(&self) -> StoreResult<Vec<Wave>> {
        self.with_client(|client| async move {
            let rows = client
                .query(Self::sql(Query::ListLoopableWaves), &[])
                .await?;
            rows.iter().map(map_wave_row).collect()
        })
        .await
    }

    pub async fn list_cron_waves(&self) -> StoreResult<Vec<Wave>> {
        self.with_client(|client| async move {
            let rows = client.query(Self::sql(Query::ListCronWaves), &[]).await?;
            rows.iter().map(map_wave_row).collect()
        })
        .await
    }

    pub async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(Self::sql(Query::GetWaveById), &[&wave_id])
                .await?;
            row.as_ref().map(map_wave_row).transpose()
        })
        .await
    }

    pub async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        let name = name.to_string();
        self.with_client(|client| async move {
            let row = client
                .query_opt(Self::sql(Query::GetWaveByName), &[&name])
                .await?;
            row.as_ref().map(map_wave_row).transpose()
        })
        .await
    }

    pub async fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave).await
    }

    pub async fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave).await
    }

    pub async fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()> {
        let wave_id = wave_id.clone();
        self.with_client(|client| async move {
            client
                .execute(
                    "DELETE FROM attention_items WHERE wave_id = $1",
                    &[&wave_id],
                )
                .await?;
            client
                .execute(Self::sql(Query::DeleteWaveById), &[&wave_id])
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn list_wave_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<WaveRun>> {
        self.with_client(|client| async move {
            let query = Self::sql(list_wave_runs_query(wave_id.is_some(), limit.is_some()));
            let mut params: Vec<Box<dyn ToSql + Sync + Send>> = Vec::new();
            if let Some(wave_id) = wave_id {
                params.push(Box::new(wave_id.clone()));
            }
            if let Some(limit) = limit {
                params.push(Box::new(limit as i64));
            }

            let params_ref: Vec<&(dyn ToSql + Sync)> = params
                .iter()
                .map(|v| v.as_ref() as &(dyn ToSql + Sync))
                .collect();
            let rows = client.query(query, &params_ref).await?;
            rows.iter().map(map_wave_run_row).collect()
        })
        .await
    }

    pub async fn get_wave_run(&self, wave_run_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(Self::sql(Query::GetWaveRunById), &[&wave_run_id])
                .await?;
            row.as_ref().map(map_wave_run_row).transpose()
        })
        .await
    }

    pub async fn get_active_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        self.with_client(|client| async move {
            let statuses = [
                WaveRunStatus::Pending.as_i32(),
                WaveRunStatus::Running.as_i32(),
                WaveRunStatus::Waiting.as_i32(),
            ];
            let row = client
                .query_opt(
                    Self::sql(Query::GetActiveWaveRun),
                    &[&wave_id, &&statuses[..]],
                )
                .await?;
            row.as_ref().map(map_wave_run_row).transpose()
        })
        .await
    }

    pub async fn count_active_wave_runs(&self, wave_id: &LfdId) -> StoreResult<u32> {
        self.with_client(|client| async move {
            let statuses = [
                WaveRunStatus::Pending.as_i32(),
                WaveRunStatus::Running.as_i32(),
                WaveRunStatus::Waiting.as_i32(),
            ];
            let count = client
                .query_one(
                    Self::sql(Query::CountActiveWaveRuns),
                    &[&wave_id, &&statuses[..]],
                )
                .await?
                .get::<_, i64>(0);
            Ok(count.max(0) as u32)
        })
        .await
    }

    pub async fn get_latest_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(Self::sql(Query::GetLatestWaveRun), &[&wave_id])
                .await?;
            row.as_ref().map(map_wave_run_row).transpose()
        })
        .await
    }

    pub async fn create_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        self.with_client(|client| async move {
            let started_at = run
                .started_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);
            let ended_at = run.ended_at.map(|dt| dt.unix_timestamp());
            let flow_parents_json = serde_json::to_string(&run.flow_parents)?;
            let execution_cursor = run.execution_cursor.clone();
            client
                .execute(
                    Self::sql(Query::InsertWaveRun),
                    &[
                        &run.id,
                        &run.wave_id,
                        &(run.iteration as i32),
                        &(run.step_index as i32),
                        &run.status.as_i32(),
                        &run.worktree,
                        &run.branch,
                        &started_at,
                        &ended_at,
                        &run.error,
                        &run.snapshot.repo,
                        &run.snapshot.flow,
                        &serde_json::to_string(&run.snapshot.direction)?,
                        &serde_json::to_string(&run.snapshot.area)?,
                        &serialize_pr(&run.pr)?,
                        &flow_parents_json,
                        &execution_cursor,
                        &run.activation_log_id,
                        &run.parent_run_id,
                        &run.parent_pr_number.map(|value| value as i64),
                        &(run.stack_position as i32),
                        &run.stack_group_id,
                        &run.stack_status.as_i32(),
                        &(if run.lineage_inferred { 1i32 } else { 0i32 }),
                        &run.target_branch,
                        &run.repair_of,
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn update_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        self.with_client(|client| async move {
            let flow_parents_json = serde_json::to_string(&run.flow_parents)?;
            let execution_cursor = run.execution_cursor.clone();
            let updated = client
                .execute(
                    Self::sql(Query::UpdateWaveRun),
                    &[
                        &(run.iteration as i32),
                        &(run.step_index as i32),
                        &run.status.as_i32(),
                        &run.worktree,
                        &run.branch,
                        &run.started_at.map(|dt| dt.unix_timestamp()),
                        &run.ended_at.map(|dt| dt.unix_timestamp()),
                        &run.error,
                        &run.snapshot.repo,
                        &run.snapshot.flow,
                        &serde_json::to_string(&run.snapshot.direction)?,
                        &serde_json::to_string(&run.snapshot.area)?,
                        &serialize_pr(&run.pr)?,
                        &flow_parents_json,
                        &execution_cursor,
                        &run.activation_log_id,
                        &run.parent_run_id,
                        &run.parent_pr_number.map(|value| value as i64),
                        &(run.stack_position as i32),
                        &run.stack_group_id,
                        &run.stack_status.as_i32(),
                        &(if run.lineage_inferred { 1i32 } else { 0i32 }),
                        &run.target_branch,
                        &run.repair_of,
                        &run.id,
                    ],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn list_stack_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<WaveRun>> {
        self.with_client(|client| async move {
            let rows = client
                .query(Self::sql(Query::ListStackRuns), &[&wave_id])
                .await?;
            rows.iter().map(map_wave_run_row).collect()
        })
        .await
    }

    pub async fn get_live_pr_state(
        &self,
        repo_id: &str,
        pr_number: u32,
    ) -> StoreResult<Option<LivePullRequestState>> {
        let repo_id = repo_id.to_string();
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    Self::sql(Query::GetLivePrState),
                    &[&repo_id, &(pr_number as i64)],
                )
                .await?;
            row.as_ref().map(map_live_pr_state_row).transpose()
        })
        .await
    }

    pub async fn upsert_live_pr_state(&self, state: &LivePullRequestState) -> StoreResult<()> {
        let state = state.clone();
        self.with_client(|client| async move {
            client
                .execute(
                    Self::sql(Query::UpsertLivePrState),
                    &[
                        &state.repo_id,
                        &(state.pr_number as i64),
                        &state.state.as_i32(),
                        &(if state.is_draft { 1i32 } else { 0i32 }),
                        &state.head_ref,
                        &state.head_sha,
                        &state.base_ref,
                        &state.updated_at.unix_timestamp(),
                        &state.merged_at.map(|value| value.unix_timestamp()),
                        &state.synced_at.unix_timestamp(),
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn list_attention_items(
        &self,
        status: Option<AttentionStatus>,
        kind: Option<AttentionKind>,
    ) -> StoreResult<Vec<AttentionItem>> {
        let status = status.map(|value| value.as_str().to_string());
        let kind = kind.map(|value| value.as_str().to_string());
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT id, wave_id, run_id, kind, status, title, summary, context, surfaced_at, viewed_at, resolved_at
                     FROM attention_items
                     WHERE ($1::TEXT IS NULL OR status = $1)
                       AND ($2::TEXT IS NULL OR kind = $2)
                     ORDER BY surfaced_at DESC",
                    &[&status, &kind],
                )
                .await?;
            rows.iter().map(Self::map_attention_item_row).collect()
        })
        .await
    }

    pub async fn get_attention_item(
        &self,
        attention_id: &LfdId,
    ) -> StoreResult<Option<AttentionItem>> {
        let attention_id = attention_id.clone();
        self.with_client(|client| async move {
            let row = client.query_opt(
                "SELECT id, wave_id, run_id, kind, status, title, summary, context, surfaced_at, viewed_at, resolved_at
                 FROM attention_items WHERE id = $1",
                &[&attention_id],
            ).await?;
            row.as_ref().map(Self::map_attention_item_row).transpose()
        })
        .await
    }

    pub async fn find_attention_item_for_run(
        &self,
        run_id: &LfdId,
        kind: AttentionKind,
    ) -> StoreResult<Option<AttentionItem>> {
        let run_id = run_id.clone();
        let kind = kind.as_str().to_string();
        let resolved = AttentionStatus::Resolved.as_str().to_string();
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, wave_id, run_id, kind, status, title, summary, context, surfaced_at, viewed_at, resolved_at
                     FROM attention_items
                     WHERE run_id = $1 AND kind = $2 AND status != $3
                     ORDER BY surfaced_at DESC
                     LIMIT 1",
                    &[&run_id, &kind, &resolved],
                )
                .await?;
            row.as_ref().map(Self::map_attention_item_row).transpose()
        })
        .await
    }

    pub async fn find_attention_item_for_run(
        &self,
        run_id: &LfdId,
        kind: AttentionKind,
    ) -> StoreResult<Option<AttentionItem>> {
        let run_id = run_id.clone();
        let kind = kind.as_str().to_string();
        let resolved = AttentionStatus::Resolved.as_str().to_string();
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, wave_id, run_id, kind, status, title, summary, context, surfaced_at, viewed_at, resolved_at
                     FROM attention_items
                     WHERE run_id = $1 AND kind = $2 AND status != $3
                     ORDER BY surfaced_at DESC
                     LIMIT 1",
                    &[&run_id, &kind, &resolved],
                )
                .await?;
            let Some(row) = row else {
                return Ok(None);
            };
            let kind_raw: String = row.try_get(3)?;
            let status_raw: String = row.try_get(4)?;
            Ok(Some(AttentionItem {
                id: LfdId::from_raw(row.try_get::<_, String>(0)?),
                wave_id: LfdId::from_raw(row.try_get::<_, String>(1)?),
                run_id: row.try_get::<_, Option<String>>(2)?.map(LfdId::from_raw),
                kind: kind_raw.parse::<AttentionKind>().map_err(StoreError::InvalidData)?,
                status: status_raw.parse::<AttentionStatus>().map_err(StoreError::InvalidData)?,
                title: row.try_get(5)?,
                summary: row.try_get(6)?,
                context: serde_json::from_str(&row.try_get::<_, String>(7)?)
                    .unwrap_or(serde_json::Value::Object(Default::default())),
                surfaced_at: crate::lfd::store::rows::unix_to_datetime(row.try_get(8)?),
                viewed_at: row
                    .try_get::<_, Option<i64>>(9)?
                    .map(crate::lfd::store::rows::unix_to_datetime),
                resolved_at: row
                    .try_get::<_, Option<i64>>(10)?
                    .map(crate::lfd::store::rows::unix_to_datetime),
            }))
        })
        .await
    }

    pub async fn upsert_attention_item(&self, item: &AttentionItem) -> StoreResult<()> {
        let item = item.clone();
        self.with_client(|client| async move {
            client.execute(
                "INSERT INTO attention_items (id, wave_id, run_id, kind, status, title, summary, context, surfaced_at, viewed_at, resolved_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                 ON CONFLICT(id) DO UPDATE SET
                    wave_id = excluded.wave_id,
                    run_id = excluded.run_id,
                    kind = excluded.kind,
                    status = excluded.status,
                    title = excluded.title,
                    summary = excluded.summary,
                    context = excluded.context,
                    surfaced_at = excluded.surfaced_at,
                    viewed_at = excluded.viewed_at,
                    resolved_at = excluded.resolved_at",
                &[
                    &item.id,
                    &item.wave_id,
                    &item.run_id,
                    &item.kind.as_str(),
                    &item.status.as_str(),
                    &item.title,
                    &item.summary,
                    &serde_json::to_string(&item.context)?,
                    &item.surfaced_at.unix_timestamp(),
                    &item.viewed_at.map(|value: time::OffsetDateTime| value.unix_timestamp()),
                    &item.resolved_at.map(|value: time::OffsetDateTime| value.unix_timestamp()),
                ],
            ).await?;
            Ok(())
        }).await
    }

    pub async fn delete_attention_item(&self, attention_id: &LfdId) -> StoreResult<u32> {
        let attention_id = attention_id.clone();
        self.with_client(|client| async move {
            let deleted = client
                .execute(
                    "DELETE FROM attention_items WHERE id = $1",
                    &[&attention_id],
                )
                .await?;
            Ok(deleted as u32)
        })
        .await
    }

    pub async fn list_queue_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<QueueBlock>> {
        let wave_id = wave_id.clone();
        let blocks = self
            .list_attention_items(
                Some(AttentionStatus::Surfaced),
                Some(AttentionKind::Algedonic),
            )
            .await?
            .into_iter()
            .filter(|item| item.wave_id == wave_id)
            .filter_map(|item| queue_block_from_attention(&item).ok().flatten())
            .collect();
        Ok(blocks)
    }

    pub async fn upsert_queue_block(&self, block: &QueueBlock) -> StoreResult<()> {
        self.upsert_attention_item(&queue_block_attention_item(block))
            .await
    }

    pub async fn delete_queue_block(&self, _wave_id: &LfdId, run_id: &LfdId) -> StoreResult<u32> {
        let attention_id = crate::lfd::attention::attention_id_for_queue_block(run_id);
        let Some(mut item) = self.get_attention_item(&attention_id).await? else {
            return Ok(0);
        };
        item.status = AttentionStatus::Resolved;
        item.resolved_at = Some(time::OffsetDateTime::now_utc());
        self.upsert_attention_item(&item).await?;
        Ok(1)
    }

    pub async fn record_merge_event(&self, event: &QueueMergeEvent) -> StoreResult<bool> {
        let event = event.clone();
        self.with_client(|client| async move {
            let inserted = client
                .execute(
                    "INSERT INTO wave_pr_merge_events (wave_id, pr_number, merged_at, processed_at)
                     VALUES ($1, $2, $3, $4)
                     ON CONFLICT(wave_id, pr_number, merged_at) DO NOTHING",
                    &[
                        &event.wave_id,
                        &(event.pr_number as i64),
                        &event.merged_at.unix_timestamp(),
                        &event.processed_at.unix_timestamp(),
                    ],
                )
                .await?;
            Ok(inserted > 0)
        })
        .await
    }

    pub async fn fail_orphaned_runs(&self) -> StoreResult<u32> {
        self.with_client(|client| async move {
            let statuses = [
                WaveRunStatus::Pending.as_i32(),
                WaveRunStatus::Running.as_i32(),
                WaveRunStatus::Waiting.as_i32(),
            ];
            let updated = client
                .execute(
                    Self::sql(Query::FailOrphanedRuns),
                    &[
                        &WaveRunStatus::Failed.as_i32(),
                        &"orphaned: lfd restarted".to_string(),
                        &now_unix(),
                        &&statuses[..],
                    ],
                )
                .await?;
            Ok(updated as u32)
        })
        .await
    }

    pub async fn list_triggers(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Trigger>> {
        self.with_client(|client| async move {
            let rows = if let Some(wave_id) = wave_id {
                client
                    .query(Self::sql(list_triggers_query(true)), &[&wave_id])
                    .await?
            } else {
                client
                    .query(Self::sql(list_triggers_query(false)), &[])
                    .await?
            };
            rows.iter().map(map_trigger_row).collect()
        })
        .await
    }

    pub async fn list_triggers_by_signal(&self, signal: i32) -> StoreResult<Vec<Trigger>> {
        self.with_client(|client| async move {
            let rows = client
                .query(Self::sql(Query::ListTriggersBySignal), &[&signal])
                .await?;
            rows.iter().map(map_trigger_row).collect()
        })
        .await
    }

    pub async fn get_trigger(&self, trigger_id: &LfdId) -> StoreResult<Option<Trigger>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(Self::sql(Query::GetTriggerById), &[&trigger_id])
                .await?;
            row.as_ref().map(map_trigger_row).transpose()
        })
        .await
    }

    pub async fn create_trigger(&self, trigger: &Trigger) -> StoreResult<()> {
        self.with_client(|client| async move {
            let created_at = trigger
                .created_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);
            let enabled: i32 = if trigger.enabled { 1 } else { 0 };

            client
                .execute(
                    Self::sql(Query::InsertTrigger),
                    &[
                        &trigger.id,
                        &trigger.wave_id,
                        &trigger.signal.as_i32(),
                        &trigger.flow,
                        &trigger.last_main_sha,
                        &trigger.last_triggered_at,
                        &created_at,
                        &enabled,
                        &trigger.source_wave_id,
                        &trigger.max_iterations.map(|v| v as i32),
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn update_trigger(&self, trigger: &Trigger) -> StoreResult<()> {
        self.with_client(|client| async move {
            let enabled: i32 = if trigger.enabled { 1 } else { 0 };
            let updated = client
                .execute(
                    Self::sql(Query::UpdateTrigger),
                    &[
                        &trigger.signal.as_i32(),
                        &trigger.flow,
                        &trigger.last_main_sha,
                        &trigger.last_triggered_at,
                        &enabled,
                        &trigger.source_wave_id,
                        &trigger.max_iterations.map(|v| v as i32),
                        &trigger.id,
                    ],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn delete_trigger(&self, trigger_id: &LfdId) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(Self::sql(Query::DeleteTriggerById), &[&trigger_id])
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn list_pending_activations(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Vec<PendingActivation>> {
        self.with_client(|client| async move {
            let rows = client
                .query(Self::sql(Query::ListPendingActivationsByWave), &[&wave_id])
                .await?;
            rows.iter().map(map_pending_activation_row).collect()
        })
        .await
    }

    pub async fn create_pending_activation(
        &self,
        activation: &PendingActivation,
    ) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(
                    Self::sql(Query::InsertPendingActivation),
                    &[
                        &activation.id,
                        &activation.wave_id,
                        &activation.trigger_id,
                        &activation.reason,
                        &activation.from_sha,
                        &activation.to_sha,
                        &activation.queued_at,
                        &activation.target_branch,
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn update_pending_activation(
        &self,
        activation: &PendingActivation,
    ) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated = client
                .execute(
                    Self::sql(Query::UpdatePendingActivation),
                    &[
                        &activation.reason,
                        &activation.from_sha,
                        &activation.to_sha,
                        &activation.target_branch,
                        &activation.id,
                    ],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn get_pending_for_trigger(
        &self,
        wave_id: &LfdId,
        trigger_id: Option<&LfdId>,
    ) -> StoreResult<Option<PendingActivation>> {
        self.with_client(|client| async move {
            let row = match trigger_id {
                Some(tid) => {
                    client
                        .query_opt(
                            Self::sql(Query::GetPendingActivationForTrigger),
                            &[&wave_id, &tid],
                        )
                        .await?
                }
                None => {
                    client
                        .query_opt(Self::sql(Query::GetPendingActivationForWave), &[&wave_id])
                        .await?
                }
            };
            row.as_ref().map(map_pending_activation_row).transpose()
        })
        .await
    }

    pub async fn delete_pending_activation_by_id(&self, activation_id: &LfdId) -> StoreResult<u32> {
        self.with_client(|client| async move {
            let deleted = client
                .execute(
                    Self::sql(Query::DeletePendingActivationById),
                    &[&activation_id],
                )
                .await?;
            Ok(deleted as u32)
        })
        .await
    }

    pub async fn create_activation_log(&self, log: &ActivationLog) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(
                    Self::sql(Query::InsertActivationLog),
                    &[
                        &log.id,
                        &log.wave_id,
                        &log.trigger_id,
                        &log.reason,
                        &log.outcome.as_str(),
                        &log.created_at,
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn list_activation_log(
        &self,
        wave_id: &LfdId,
        limit: u32,
    ) -> StoreResult<Vec<ActivationLog>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    Self::sql(Query::ListActivationLogByWave),
                    &[&wave_id, &(limit as i32)],
                )
                .await?;
            rows.iter().map(map_activation_log_row).collect()
        })
        .await
    }

    pub async fn get_activation_log(
        &self,
        activation_log_id: &LfdId,
    ) -> StoreResult<Option<ActivationLog>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    Self::sql(Query::GetActivationLogById),
                    &[&activation_log_id],
                )
                .await?;
            row.as_ref().map(map_activation_log_row).transpose()
        })
        .await
    }

    pub async fn list_fork_runs(
        &self,
        wave_run_id: &LfdId,
        step_index: u32,
    ) -> StoreResult<Vec<ForkRun>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    Self::sql(Query::ListForkRuns),
                    &[&wave_run_id, &(step_index as i32)],
                )
                .await?;
            rows.iter().map(map_fork_run_row).collect()
        })
        .await
    }

    pub async fn list_orphaned_fork_runs(&self) -> StoreResult<Vec<ForkRun>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT fr.id, fr.wave_run_id, fr.step_index, fr.branch_index, fr.status, fr.worktree
                     FROM fork_runs fr
                     LEFT JOIN wave_runs wr ON wr.id = fr.wave_run_id
                     WHERE fr.status IN ($1, $2)
                       AND (
                         wr.id IS NULL
                         OR wr.status NOT IN ($3, $4, $5)
                         OR fr.step_index <> wr.step_index
                       )
                     ORDER BY fr.wave_run_id ASC, fr.step_index ASC, fr.branch_index ASC",
                    &[
                        &(ForkRunStatus::Pending as i32),
                        &(ForkRunStatus::Running as i32),
                        &WaveRunStatus::Pending.as_i32(),
                        &WaveRunStatus::Running.as_i32(),
                        &WaveRunStatus::Waiting.as_i32(),
                    ],
                )
                .await?;
            rows.iter().map(map_fork_run_row).collect()
        })
        .await
    }

    pub async fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(
                    Self::sql(Query::UpsertForkRun),
                    &[
                        &fork_run.id,
                        &fork_run.wave_run_id,
                        &(fork_run.step_index as i32),
                        &(fork_run.branch_index as i32),
                        &(fork_run.status as i32),
                        &fork_run.worktree,
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn delete_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<u32> {
        self.with_client(|client| async move {
            let deleted = client
                .execute(
                    Self::sql(Query::DeleteForkRuns),
                    &[&wave_run_id, &(step_index as i32)],
                )
                .await?;
            Ok(deleted as u32)
        })
        .await
    }

    pub async fn list_agents(&self) -> StoreResult<Vec<AgentRun>> {
        self.list_agent_history(None, None, None).await
    }

    pub async fn list_agent_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<AgentRun>> {
        self.with_client(|client| async move {
            let query = Self::sql(list_agent_history_query(
                worktree.is_some(),
                repo.is_some(),
                limit.is_some(),
            ));
            let mut params: Vec<Box<dyn ToSql + Sync + Send>> = Vec::new();

            if let Some(worktree) = worktree {
                params.push(Box::new(worktree.to_string()));
            }
            if let Some(repo) = repo {
                params.push(Box::new(repo.to_string()));
            }
            if let Some(limit) = limit {
                params.push(Box::new(limit as i64));
            }

            let params_ref: Vec<&(dyn ToSql + Sync)> = params
                .iter()
                .map(|v| v.as_ref() as &(dyn ToSql + Sync))
                .collect();
            let rows = client.query(query, &params_ref).await?;
            rows.iter().map(map_agent_row).collect()
        })
        .await
    }

    pub async fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<AgentRun>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(Self::sql(Query::GetAgentById), &[&agent_id])
                .await?;
            row.as_ref().map(map_agent_row).transpose()
        })
        .await
    }

    pub async fn get_waiting_agent_for_wave(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Option<AgentRun>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    Self::sql(Query::GetWaitingAgentForWave),
                    &[&wave_id, &AgentStatus::Waiting.as_i32()],
                )
                .await?;
            row.as_ref().map(map_agent_row).transpose()
        })
        .await
    }

    pub async fn start_agent(&self, agent_run: &AgentRun) -> StoreResult<()> {
        self.with_client(|client| async move {
            let started_at = agent_run
                .started_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);
            let ended_at = agent_run.ended_at.map(|dt| dt.unix_timestamp());
            let pid = agent_run.pid.map(|v| v as i32);
            let container_id = agent_run.container_id.as_deref();
            client
                .execute(
                    Self::sql(Query::InsertAgent),
                    &[
                        &agent_run.id,
                        &agent_run.step,
                        &agent_run.repo,
                        &agent_run.worktree,
                        &agent_run.wave_run_id,
                        &agent_run.status.as_i32(),
                        &started_at,
                        &ended_at,
                        &pid,
                        &container_id,
                        &agent_run.agent,
                        &agent_run.run_mode,
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn update_agent_status(
        &self,
        agent_id: &LfdId,
        status: i32,
        pid: Option<u32>,
        container_id: Option<&str>,
    ) -> StoreResult<()> {
        self.with_client(|client| async move {
            let pid = pid.map(|v| v as i32);
            let container_id = container_id.map(str::to_string);
            let updated = client
                .execute(
                    Self::sql(Query::UpdateAgentStatus),
                    &[&status, &pid, &container_id, &agent_id],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn end_agent(&self, agent_id: &LfdId, status: i32, ended_at: i64) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated = client
                .execute(Self::sql(Query::EndAgent), &[&status, &ended_at, &agent_id])
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn get_active_agents_for_wave(&self, wave_id: &LfdId) -> StoreResult<Vec<AgentRun>> {
        self.with_client(|client| async move {
            let rows = client
                .query(Self::sql(Query::GetActiveAgentsForWave), &[&wave_id])
                .await?;
            rows.iter().map(map_agent_row).collect()
        })
        .await
    }

    pub async fn end_active_agent_for_wave(
        &self,
        wave_id: &LfdId,
        status: i32,
        ended_at: i64,
    ) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated = client
                .execute(
                    Self::sql(Query::EndActiveAgentsForWave),
                    &[&status, &ended_at, &wave_id.as_str()],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn get_stuck_agents(&self, older_than_secs: u64) -> StoreResult<Vec<AgentRun>> {
        self.with_client(|client| async move {
            let cutoff = now_unix() - older_than_secs as i64;
            let rows = client
                .query(Self::sql(Query::GetStuckAgents), &[&cutoff])
                .await?;
            rows.iter().map(map_agent_row).collect()
        })
        .await
    }

    pub async fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(Self::sql(Query::GetSummaryByWave), &[&wave_id])
                .await?;
            row.as_ref().map(map_summary_row).transpose()
        })
        .await
    }

    pub async fn upsert_summary(&self, summary: &Summary) -> StoreResult<()> {
        self.with_client(|client| async move {
            let created_at = summary
                .created_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);

            client
                .execute(
                    Self::sql(Query::UpsertSummary),
                    &[
                        &summary.id,
                        &summary.wave_id,
                        &summary.content,
                        &summary.source_hash,
                        &(summary.token_budget as i32),
                        &summary.agent,
                        &created_at,
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn list_chat_memory_blocks(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Vec<ChatMemoryBlock>> {
        self.with_client(|client| async move {
            let rows = client
                .query(Self::sql(Query::ListChatMemoryBlocks), &[&wave_id])
                .await?;
            rows.iter().map(map_chat_memory_block_row).collect()
        })
        .await
    }

    pub async fn upsert_chat_memory_block(&self, block: &ChatMemoryBlock) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated_at = block
                .updated_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);
            client
                .execute(
                    Self::sql(Query::UpsertChatMemoryBlock),
                    &[
                        &block.wave_id,
                        &block.name,
                        &block.content,
                        &(block.position as i32),
                        &updated_at,
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn delete_chat_memory_block(&self, wave_id: &LfdId, name: &str) -> StoreResult<()> {
        let name = name.to_string();
        self.with_client(|client| async move {
            client
                .execute(Self::sql(Query::DeleteChatMemoryBlock), &[&wave_id, &name])
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn list_chat_messages(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMessage>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT id, wave_id, role, content, created_at
                     FROM chat_messages
                     WHERE wave_id = $1
                     ORDER BY created_at ASC",
                    &[&wave_id],
                )
                .await?;
            rows.iter().map(map_chat_message_row).collect()
        })
        .await
    }

    pub async fn create_chat_message(&self, message: &ChatMessage) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(
                    "INSERT INTO chat_messages (id, wave_id, role, content, created_at)
                     VALUES ($1, $2, $3, $4, $5)",
                    &[
                        &message.id,
                        &message.wave_id,
                        &message.role,
                        &message.content,
                        &message.created_at.unix_timestamp(),
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }
}

fn build_pool(database_url: &str) -> StoreResult<Pool> {
    let config: tokio_postgres::Config = database_url
        .parse()
        .map_err(|err| StoreError::InvalidData(format!("invalid database url: {err}")))?;
    let manager = Manager::new(config, NoTls);
    let pool = Pool::builder(manager)
        .max_size(16)
        .build()
        .map_err(|err| StoreError::InvalidData(format!("failed to build pool: {err}")))?;
    Ok(pool)
}

async fn get_client_with_retry(
    pool: &Pool,
) -> Result<deadpool_postgres::Client, deadpool_postgres::PoolError> {
    let mut last_error = None;
    for (attempt, delay) in RETRY_DELAYS.iter().enumerate() {
        match pool.get().await {
            Ok(client) => return Ok(client),
            Err(err) => {
                last_error = Some(err);
                if attempt < RETRY_DELAYS.len() - 1 {
                    tokio::time::sleep(*delay).await;
                }
            }
        }
    }
    Err(last_error.expect("retry loop always sets last_error"))
}
