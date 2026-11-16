//! Shared conversation fixtures for runtime and adapter checks.
use crate::process::attachment;
use crate::session::LfSession;
use crate::store::{open_ephemeral_store, SharedStore, StorageConfig};
use std::ffi::OsString;
#[derive(Debug)]
pub struct SessionHome {
    pub home: tempfile::TempDir,
    previous: Vec<(&'static str, Option<OsString>)>,
    _ambient: crate::test_ambient::EnvGuard,
}

impl Default for SessionHome {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionHome {
    pub fn new() -> Self {
        let ambient = crate::test_ambient::EnvGuard::new();
        let home = tempfile::tempdir().unwrap();
        let previous = ["LF_HOME", "LF_BIN"]
            .into_iter()
            .map(|name| {
                let value = std::env::var_os(name);
                std::env::remove_var(name);
                (name, value)
            })
            .collect();
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        Self {
            home,
            previous,
            _ambient: ambient,
        }
    }

    pub fn path(&self) -> &std::path::Path {
        self.home.path()
    }

    pub async fn store(&self) -> SharedStore {
        std::sync::Arc::new(
            open_ephemeral_store(&StorageConfig::sqlite(self.home.path().join("loopflow.db")))
                .await
                .unwrap(),
        )
    }
}

impl Drop for SessionHome {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
/// A Machine with stored Codex logins, whose shared home `sign_in` moves.
#[derive(Debug)]
pub struct Logins {
    pub store: SharedStore,
    pub database: std::path::PathBuf,
    pub root: tempfile::TempDir,
    _env: crate::test_ambient::EnvGuard,
    _home: SessionHome,
}

/// A real process standing in for a running provider, reaped even when an
/// assertion fails.
#[derive(Debug)]
pub struct RunningAgent(pub std::process::Child);

impl Drop for RunningAgent {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Logins {
    pub async fn new() -> Self {
        let home = SessionHome::new();
        let store = home.store().await;
        let root = tempfile::tempdir().unwrap();
        let env = crate::test_ambient::EnvGuard::clear(&["CODEX_HOME", "CLAUDE_CONFIG_DIR"]);
        std::env::set_var("CODEX_HOME", root.path().join("native"));
        std::env::set_var("CLAUDE_CONFIG_DIR", root.path().join("native-claude"));
        Self {
            store,
            database: home.home.path().join("loopflow.db"),
            root,
            _env: env,
            _home: home,
        }
    }

    pub fn write_codex_auth(home: &std::path::Path, name: &str) {
        use base64::Engine;
        std::fs::create_dir_all(home).unwrap();
        let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            serde_json::json!({"email": format!("{name}@example.com"), "sub": name}).to_string(),
        );
        std::fs::write(
            home.join("auth.json"),
            serde_json::json!({"tokens": {"access_token": "fixture", "id_token": format!("h.{claims}.s")}})
                .to_string(),
        )
        .unwrap();
    }

    pub fn store_login(
        &self,
        provider: crate::provider_auth::Provider,
        name: &str,
        cooldown_until: Option<i64>,
    ) -> crate::store::ProviderAccountId {
        let id = crate::provider_account::parse_account_id(name).unwrap();
        let home = self.root.path().join(provider.as_str()).join(name);
        Self::write_codex_auth(&home, name);
        let mut account = crate::provider_account::new_account(
            provider,
            id.clone(),
            home,
            Some(crate::profile::EmailAddress::parse(&format!("{name}@example.com")).unwrap()),
        );
        account.cooldown_until = cooldown_until;
        crate::provider_account::catalog::upsert_provider_account(&self.store.sqlite, &account)
            .unwrap();
        id
    }

    pub fn sign_in(&self, name: &str) {
        Self::write_codex_auth(&self.root.path().join("native"), name);
    }

    /// A Codex conversation last run on `account`.
    pub fn codex_session(
        &self,
        id: &str,
        model: Option<&str>,
        account: &crate::store::ProviderAccountId,
    ) -> LfSession {
        let session = self
            .store
            .sqlite
            .test_session(id, &crate::session_record::new_artifact_key());
        let thread = format!("thread-{id}");
        rusqlite::Connection::open(&self.database)
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET provider='codex',provider_thread=?2,model=?3 WHERE id=?1",
                rusqlite::params![&session.id, &thread, model],
            )
            .unwrap();
        self.store
            .sqlite
            .pin_provider_session_route(
                crate::provider_auth::Provider::Codex,
                &loopflow_model::id::AgentSessionId::from(thread.as_str()),
                account,
            )
            .unwrap();
        session
    }

    pub fn run(&self, session: &LfSession) -> RunningAgent {
        let parent = loopflow_model::id::LfProcessId::new();
        rusqlite::Connection::open(&self.database)
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
                [&parent],
            )
            .unwrap();
        let attachment = attachment::claim_session_attachment(
            &self.store.sqlite,
            &session.id,
            None,
            &parent,
            true,
        )
        .unwrap();
        let mut command = std::process::Command::new("/bin/sleep");
        command.env_clear().arg("60");
        RunningAgent(
            crate::harness::agent_process::spawn_native(
                command,
                &(
                    self.store.sqlite.clone(),
                    session.id.to_string(),
                    attachment,
                ),
                crate::session::Holder::Caller,
            )
            .unwrap(),
        )
    }

    pub async fn selected(&self, blocked: bool, on: Option<&str>) -> anyhow::Result<Vec<String>> {
        let filter = crate::session::SessionFilter {
            blocked,
            on: on.map(str::to_owned),
            ..Default::default()
        };
        let mut ids: Vec<String> = crate::ops::human_session::list(&self.store, &filter)
            .await?
            .into_iter()
            .map(|session| session.id.to_string())
            .collect();
        ids.sort();
        Ok(ids)
    }
}
