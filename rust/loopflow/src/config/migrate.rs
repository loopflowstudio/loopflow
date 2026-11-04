//! Recoverable file cutover. The journal records before/after documents, never
//! participates in preference resolution, and survives an interrupted SQL upgrade.
use anyhow::{ensure, Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Edit {
    path: PathBuf,
    before: String,
    after: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct Cutover {
    source: String,
    edits: Vec<Edit>,
}

fn table(conn: &Connection, name: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [name],
        |row| row.get(0),
    )?)
}
fn scoped(conn: &Connection, kind: &str, id: &str) -> Result<Vec<(PathBuf, Vec<String>)>> {
    let recover = || -> Result<Vec<(PathBuf, Vec<String>)>> {
        Ok(vec![(
            super::placement::home(conn)?.join("settings-recovery.yaml"),
            vec![kind.into(), id.into()],
        )])
    };
    if kind == "repo" {
        let mut query = conn.prepare("SELECT DISTINCT repo FROM repository_plans")?;
        let paths = query
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut destinations = std::collections::BTreeSet::new();
        for path in paths {
            let repo = Path::new(&path);
            if crate::repository::RepoId::discover(repo)
                .ok()
                .is_some_and(|repo| repo.as_str() == id)
            {
                destinations.insert(super::files::private_path(repo)?);
            }
        }
        // Preserve the old origin-wide policy even when existing clones can receive it.
        let mut out = recover()?;
        out.extend(destinations.into_iter().map(|path| (path, vec![])));
        return Ok(out);
    }
    let scope = match kind {
        "global" | "default" => {
            return Ok(vec![(
                super::placement::home(conn)?.join("config.yaml"),
                vec![],
            )])
        }
        "repository" => {
            crate::durable::DefaultScope::Repository(crate::durable::RepositoryId::from_raw(id))
        }
        "wave" => crate::durable::DefaultScope::Wave(
            crate::id::WaveId::parse(id).map_err(anyhow::Error::msg)?,
        ),
        _ => anyhow::bail!("unknown preference scope {kind}"),
    };
    match super::placement::location(conn, Some(&scope)) {
        Ok((path, prefix)) if path.parent().is_some_and(Path::is_dir) => Ok(vec![(path, prefix)]),
        _ => recover(),
    }
}

type Export = BTreeMap<PathBuf, Value>;
fn put(exports: &mut Export, path: PathBuf, prefix: &[String], key: &str, value: Value) {
    let path_parts: Vec<_> = prefix
        .iter()
        .map(String::as_str)
        .chain(key.split('.'))
        .collect();
    super::settings::insert(
        exports
            .entry(path)
            .or_insert(Value::Mapping(Default::default())),
        &path_parts,
        value,
    );
}

fn export(conn: &Connection) -> Result<Export> {
    let mut exports = Export::new();
    if table(conn, "scope_settings")? {
        let mut query = conn.prepare("SELECT scope_kind,scope_id,key,value FROM scope_settings ORDER BY scope_kind,scope_id,key")?;
        for row in query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })? {
            let (kind, id, key, text) = row?;
            let key = key
                .strip_prefix("skill_agents.")
                .map(|name| format!("skills.{name}.agent"))
                .unwrap_or(key);
            for (path, prefix) in scoped(conn, &kind, &id)? {
                put(
                    &mut exports,
                    path,
                    &prefix,
                    &key,
                    super::settings::parse_value(&key, &text)?,
                );
            }
        }
    }
    if table(conn, "scope_defaults")? {
        let mut query = conn.prepare("SELECT scope_kind,scope_id,tasks_machine_id,session_machine_id,tasks_capacity FROM scope_defaults")?;
        for row in query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })? {
            let (kind, id, tasks, session, capacity) = row?;
            for (path, prefix) in scoped(conn, &kind, &id)? {
                for (key, value) in [
                    ("machine.tasks", tasks.clone()),
                    ("machine.session", session.clone()),
                    ("machine.capacity", capacity.clone()),
                ] {
                    if let Some(value) = value {
                        put(&mut exports, path.clone(), &prefix, key, value.into());
                    }
                }
            }
        }
    }
    if table(conn, "provider_routes")? {
        let mut query = conn.prepare("SELECT scope,scope_id,provider,account_id FROM provider_routes ORDER BY scope,scope_id,provider,position")?;
        let mut routes: BTreeMap<(String, String, String), Vec<String>> = BTreeMap::new();
        for row in query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })? {
            let (kind, id, provider, account) = row?;
            routes
                .entry((kind, id, provider))
                .or_default()
                .push(account);
        }
        for ((kind, id, provider), accounts) in routes {
            for (path, prefix) in scoped(conn, &kind, &id)? {
                put(
                    &mut exports,
                    path,
                    &prefix,
                    &format!("account.{provider}"),
                    serde_yaml_ng::to_value(&accounts)?,
                );
            }
        }
    }
    let old_machines: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('machines') WHERE name='label')",
        [],
        |row| row.get(0),
    )?;
    if old_machines {
        let mut query =
            conn.prepare("SELECT id,label,route FROM machines WHERE label IS NOT NULL")?;
        for row in query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })? {
            let (id, label, route) = row?;
            let path = super::placement::home(conn)?.join("config.yaml");
            let value = serde_yaml_ng::to_value(super::placement::MachineDeclaration {
                id: crate::durable::MachineId::from_raw(id.clone()),
            })?;
            // Labels are literal map keys, including dots.
            super::settings::insert(
                exports
                    .entry(path.clone())
                    .or_insert(Value::Mapping(Default::default())),
                &["machines", &label],
                value,
            );
            super::settings::insert(
                exports.get_mut(&path).expect("export exists"),
                &["connections", &id],
                route.into(),
            );
        }
    }
    let old_accounts: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('provider_accounts') WHERE name='routing_state')", [], |row| row.get(0))?;
    if old_accounts {
        let mut query = conn.prepare("SELECT provider,account_id,login_email,routing_state,plan,paid_through FROM provider_accounts")?;
        for row in query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<i32>>(5)?,
            ))
        })? {
            let (provider, id, email, routing, plan, paid) = row?;
            let declaration = super::placement::AccountDeclaration {
                login_email: email
                    .as_deref()
                    .map(crate::profile::EmailAddress::parse)
                    .transpose()
                    .map_err(anyhow::Error::msg)?,
                routing_state: Some(
                    crate::store::RoutingState::from_db(&routing).map_err(anyhow::Error::msg)?,
                ),
                plan,
                paid_through: paid
                    .map(time::Date::from_julian_day)
                    .transpose()?
                    .map(|date| date.to_string()),
            };
            let path = super::placement::home(conn)?.join("config.yaml");
            super::settings::insert(
                exports
                    .entry(path)
                    .or_insert(Value::Mapping(Default::default())),
                &["accounts", &provider, &id],
                serde_yaml_ng::to_value(declaration)?,
            );
        }
    }
    if table(conn, "auth_browser_bindings")? {
        let mut query = conn.prepare("SELECT provider,account_id,profile_id FROM auth_browser_bindings ORDER BY provider,account_id,position")?;
        let mut declarations: BTreeMap<String, super::placement::AuthBrowsers> = BTreeMap::new();
        for row in query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })? {
            let (provider, account, profile) = row?;
            let declaration = declarations.entry(provider).or_default();
            match account {
                Some(account) => declaration
                    .accounts
                    .entry(account)
                    .or_default()
                    .push(profile),
                None => declaration.default.push(profile),
            }
        }
        if !declarations.is_empty() {
            put(
                &mut exports,
                super::placement::home(conn)?.join("config.yaml"),
                &[],
                "auth_browsers",
                serde_yaml_ng::to_value(declarations)?,
            );
        }
    }
    let old_capacity: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('ondemands') WHERE name='max_machines')",
        [],
        |row| row.get(0),
    )?;
    if old_capacity {
        let path = super::placement::home(conn)?.join("config.yaml");
        let mut query = conn.prepare("SELECT name,off FROM pools")?;
        for row in query.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?))
        })? {
            let (name, off) = row?;
            super::settings::insert(
                exports
                    .entry(path.clone())
                    .or_insert(Value::Mapping(Default::default())),
                &["pools", &name],
                serde_yaml_ng::to_value(super::placement::PoolDeclaration { off })?,
            );
        }
        let mut query = conn.prepare(
            "SELECT name,provider,template,max_machines,max_usd_per_hour,hours FROM ondemands",
        )?;
        for row in query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u32>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f64>(5)?,
            ))
        })? {
            let (name, provider, hardware, max_machines, price, hours) = row?;
            let declaration = super::placement::OnDemandDeclaration {
                provider: crate::rental::Provider::parse(&provider)
                    .context("known rental provider")?,
                hardware: serde_json::from_str(&hardware)?,
                max_machines,
                max_usd_per_hour: crate::rental::Usd::parse(&price).map_err(anyhow::Error::msg)?,
                hours,
            };
            super::settings::insert(
                exports
                    .entry(path.clone())
                    .or_insert(Value::Mapping(Default::default())),
                &["ondemands", &name],
                serde_yaml_ng::to_value(declaration)?,
            );
        }
    }
    Ok(exports)
}

fn leaves(value: &Value, prefix: &str, out: &mut BTreeMap<String, Value>) {
    if let Some(map) = value.as_mapping() {
        for (key, value) in map {
            if let Some(key) = key.as_str() {
                leaves(
                    value,
                    &if prefix.is_empty() {
                        key.into()
                    } else {
                        format!("{prefix}.{key}")
                    },
                    out,
                );
            }
        }
    } else {
        out.insert(prefix.into(), value.clone());
    }
}

fn overlaps(before: &Value, incoming: &Value) -> bool {
    let mut old = BTreeMap::new();
    let mut new = BTreeMap::new();
    leaves(before, "", &mut old);
    leaves(incoming, "", &mut new);
    new.iter().any(|(key, value)| {
        old.iter().any(|(existing, prior)| {
            // Removing DB source authority can change a different scope or target of
            // the same preference too, even when the same-address value is equal.
            key.rsplit('.').next() == existing.rsplit('.').next()
                && (key != existing || value != prior)
        })
    })
}

fn apply_edit(edit: &Edit) -> Result<()> {
    let current = super::files::read_text(&edit.path)?;
    if current == edit.after {
        return Ok(());
    }
    ensure!(
        current == edit.before,
        "{} changed during settings cutover; its edits were preserved",
        edit.path.display()
    );
    if edit
        .path
        .file_name()
        .is_some_and(|name| name == super::files::PRIVATE_FILE)
    {
        super::files::protect_private_file(&edit.path)?;
    }
    super::files::atomic_write(&edit.path, &edit.after)
}

pub(crate) fn preflight(conn: &Connection, guards: &mut Vec<File>) -> Result<()> {
    cutover(conn, false, true, guards).map(|_| ())
}

fn cutover(
    conn: &Connection,
    accept: bool,
    apply: bool,
    guards: &mut Vec<File>,
) -> Result<Vec<PathBuf>> {
    let mut exports = export(conn)?;
    let source = serde_json::to_string(&exports)?;
    // In-memory schema proofs have no host preferences. Production cutovers
    // always have a database path and never infer a home from ambient state.
    if conn.path().is_none_or(str::is_empty) {
        ensure!(
            exports.is_empty(),
            "a preference upgrade fixture must use a file-backed database"
        );
        return Ok(vec![]);
    }
    let home = super::placement::home(conn)?;
    let global = home.join("config.yaml");
    let global_value = super::files::read(&global)?;
    if let Some(repos) = global_value.get("repos").and_then(Value::as_mapping) {
        for (repo, value) in repos {
            let repo = repo
                .as_str()
                .context("personal repos keys must name checkout paths")?;
            let path = if Path::new(repo).is_dir() {
                super::files::private_path(Path::new(repo))?
            } else {
                home.join("settings-recovery.yaml")
            };
            let value = if Path::new(repo).is_dir() {
                value.clone()
            } else {
                let mut recovery = Value::Mapping(Default::default());
                super::settings::insert(&mut recovery, &["personal_repos", repo], value.clone());
                recovery
            };
            let destination = exports
                .entry(path)
                .or_insert(Value::Mapping(Default::default()));
            let mut merged = value;
            super::files::overlay(&mut merged, destination);
            *destination = merged;
        }
        exports
            .entry(global.clone())
            .or_insert(Value::Mapping(Default::default()));
    }
    if exports.is_empty() {
        return Ok(vec![]);
    }
    let journal = home.join("settings-cutover.json");
    guards.push(super::files::lock(&journal)?);
    if let Ok(text) = std::fs::read_to_string(&journal) {
        let saved: Cutover = serde_json::from_str(&text)?;
        if saved.source == source {
            for edit in &saved.edits {
                guards.push(super::files::lock(&edit.path)?);
                let text = super::files::read_text(&edit.path)?;
                ensure!(
                    text == edit.before || text == edit.after,
                    "{} changed after cutover was prepared; inspect {} before retrying",
                    edit.path.display(),
                    journal.display()
                );
            }
            if apply {
                for edit in &saved.edits {
                    apply_edit(edit)?;
                }
            }
            return Ok(saved.edits.into_iter().map(|edit| edit.path).collect());
        }
        anyhow::bail!(
            "migration inputs changed; preserved recovery evidence is at {}",
            journal.display()
        );
    }
    let mut conflicts = Vec::new();
    let db_global = exports.get(&global).cloned().unwrap_or(Value::Null);
    let mut query = conn.prepare(
        "SELECT DISTINCT repo FROM repository_plans UNION SELECT DISTINCT repo FROM wave_addresses",
    )?;
    for repo in query.query_map([], |row| row.get::<_, String>(0))? {
        let repo = PathBuf::from(repo?);
        if !repo.is_dir() {
            continue;
        }
        let private = super::files::private_path(&repo)?;
        if overlaps(&super::files::read(&private)?, &db_global) {
            conflicts.push(private.display().to_string());
        }
    }
    let mut edits = Vec::new();
    for (path, incoming) in exports {
        guards.push(super::files::lock(&path)?);
        let before = super::files::read_text(&path)?;
        let mut value = super::files::read(&path)?;
        if overlaps(&value, &incoming) {
            conflicts.push(path.display().to_string());
        }
        // A global DB declaration can previously have beaten narrower file
        // declarations in any exported repository, not just this document.
        if path != global && overlaps(&value, &db_global) {
            conflicts.push(path.display().to_string());
        }
        super::files::overlay(&mut value, &incoming);
        if path == global {
            super::files::remove(&mut value, &["repos"]);
        }
        let after = super::files::edited_text(&before, &value)?;
        edits.push(Edit {
            path,
            before,
            after,
        });
    }
    ensure!(accept || conflicts.is_empty(), "settings cutover needs an explicit precedence decision for {}; `lf config migrate --accept` preserves DB declarations at their authored scopes and accepts file precedence; no database preferences were removed", conflicts.join(", "));
    let paths = edits.iter().map(|edit| edit.path.clone()).collect();
    if apply {
        super::files::atomic_write(
            &journal,
            &serde_json::to_string_pretty(&Cutover {
                source,
                edits: edits.clone(),
            })?,
        )?;
        for edit in edits {
            apply_edit(&edit)?;
        }
    }
    Ok(paths)
}

pub fn run(accept: bool) -> Result<()> {
    let path = crate::store::database_path_from_env()?;
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    conn.execute_batch("BEGIN EXCLUSIVE")?;
    let mut guards = Vec::new();
    let result = cutover(&conn, accept, accept, &mut guards);
    conn.execute_batch("ROLLBACK")?;
    for path in result? {
        println!(
            "{} {}",
            if accept { "prepared" } else { "would migrate" },
            path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{cutover, preflight};
    use crate::store::migrations::apply_before_migration;

    fn fixture(home: &std::path::Path) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
        apply_before_migration(&conn, "settings_authority");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/settings_authority_released.sql"
        ))
        .unwrap();
        conn
    }

    #[test]
    fn global_database_choice_detects_a_narrower_file_even_without_repository_exports() {
        let home = tempfile::tempdir().unwrap();
        let conn = fixture(home.path());
        let repo = loopflow_test_support::TestRepo::new();
        conn.execute(
            "UPDATE repository_plans SET repo=?1",
            [repo.path().to_str().unwrap()],
        )
        .unwrap();
        let path = super::super::files::private_path(repo.path()).unwrap();
        std::fs::write(&path, "skills: {implement: {agent: claude}}\n").unwrap();
        assert!(preflight(&conn, &mut Vec::new())
            .unwrap_err()
            .to_string()
            .contains("precedence decision"));
        assert!(!home.path().join("config.yaml").exists());
    }

    #[test]
    fn cutover_preserves_targets_and_retries_after_a_partial_file_write() {
        let home = tempfile::tempdir().unwrap();
        let conn = fixture(home.path());
        let path = home.path().join("config.yaml");
        std::fs::write(&path, "# personal\nchrome: false\n").unwrap();
        preflight(&conn, &mut Vec::new()).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&first).unwrap();
        assert_eq!(value["skills"]["implement"]["agent"], "codex");
        assert_eq!(value["skills"]["design"]["agent"], "codex:o3");
        assert!(first.contains("# personal"));
        std::fs::write(&path, "# personal\nchrome: false\n").unwrap();
        preflight(&conn, &mut Vec::new()).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), first);
    }

    #[test]
    fn colliding_file_preferences_require_explicit_resolution_without_losing_either_copy() {
        let home = tempfile::tempdir().unwrap();
        let conn = fixture(home.path());
        let path = home.path().join("config.yaml");
        let original = "skills: {implement: {agent: claude}}\n";
        std::fs::write(&path, original).unwrap();
        assert!(preflight(&conn, &mut Vec::new()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        cutover(&conn, true, true, &mut Vec::new()).unwrap();
        let value: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(value["skills"]["implement"]["agent"], "codex");
        let journal = std::fs::read_to_string(home.path().join("settings-cutover.json")).unwrap();
        assert!(journal.contains("claude"));
    }
    #[test]
    fn absent_origin_route_is_exported_without_becoming_a_global_override() {
        let home = tempfile::tempdir().unwrap();
        let conn = fixture(home.path());
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/settings_routes_released.sql"
        ))
        .unwrap();
        preflight(&conn, &mut Vec::new()).unwrap();
        let recovery =
            crate::config::files::read(&home.path().join("settings-recovery.yaml")).unwrap();
        assert_eq!(
            recovery["repo"]["missing/clone"]["account"]["codex"][0],
            "one"
        );
        assert_eq!(
            recovery["repo"]["missing/clone"]["account"]["codex"][1],
            "two"
        );
        let global = crate::config::files::read(&home.path().join("config.yaml")).unwrap();
        assert!(global.get("account").is_none());
        assert_eq!(global["pools"]["workers"]["off"], true);
        assert_eq!(global["ondemands"]["code"]["max_machines"], 3);
        assert_eq!(
            global["auth_browsers"]["codex"]["accounts"]["one"][0],
            "Work"
        );
    }
}
