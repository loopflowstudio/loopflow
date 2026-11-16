#[cfg(test)]
use crate::provider_account::operations::account_status::login_browser;
use crate::provider_account::operations::account_status::{
    local_time, newest_reading, project, route_notes, AccountReport, AccountRow, RouteNote, Scope,
    Verification, STALE_READING_SECS,
};
use crate::provider_account::operations::format_relative_delta;
use crate::provider_auth::Provider;
use anyhow::Result;
use time::OffsetDateTime;

/// `lf account` and `lf account <login|provider>`.
pub(crate) async fn run(target: Option<super::Target>, cached: bool, json: bool) -> Result<()> {
    let detail = target.is_some();
    let (report, store, only) = project(target, cached).await?;
    let now = OffsetDateTime::now_utc().unix_timestamp();
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if detail {
        let width = std::env::var("COLUMNS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(80);
        let text = if only.is_some() {
            render_login(&report, width, now)
        } else {
            render(&report, width, now)
        };
        print!("{text}");
    } else {
        let notes = match &store {
            Some(store) => route_notes(store).await?,
            None => vec![],
        };
        print!("{}", render_table(&report, &notes, now));
    }
    Ok(())
}

fn launch_summary(row: &AccountRow, unavailable_only: bool) -> String {
    // Claude's model-less entry repeats every model's hold; it stands alone on Codex.
    let named = row.launches.iter().any(|launch| launch.model.is_some());
    let entries = || {
        row.launches
            .iter()
            .filter(move |launch| launch.model.is_some() == named)
    };
    let mut parts = Vec::new();
    if !unavailable_only {
        let ready: Vec<_> = entries()
            .filter(|launch| launch.eligible)
            .map(|launch| launch.model.as_deref().unwrap_or("ready"))
            .collect();
        if !ready.is_empty() {
            parts.push(ready.join(", "));
        }
    }
    // Credential, routing and identity blockers hold every model of the login.
    let login_wide: Vec<_> = entries()
        .next()
        .into_iter()
        .flat_map(|launch| &launch.blockers)
        .filter(|blocker| !blocker.starts_with("cooling "))
        .cloned()
        .collect();
    if !login_wide.is_empty() {
        parts.push("none".into());
        parts.extend(login_wide);
    }
    for launch in entries() {
        if let Some(until) = launch.out_until {
            let name = launch
                .model
                .as_deref()
                .map(|model| format!("{model} "))
                .unwrap_or_default();
            parts.push(format!("{name}out until {}", local_time(until)));
        }
    }
    parts.join(" · ")
}

fn next_action(row: &AccountRow, now: i64) -> String {
    let provider = row.provider.as_str();

    if row.scope == Scope::Local {
        return if row.cached_credential_state == "active"
            && row.expires_at.is_none_or(|expires| expires > now)
        {
            "use the service; launch checks required access".into()
        } else {
            format!("lf account connect {provider} (ambient access is not checked here)")
        };
    }
    if row.routing.as_deref() == Some("disabled") {
        return "disabled for routing; lf account next explains the route".into();
    }
    if row.cooldown_until.is_some_and(|until| until > now) {
        return format!("wait for cooldown, then lf account {provider}");
    }
    if row.routing.as_deref() == Some("explicit_only") {
        return format!(
            "paused: name it with --account, or lf account unpause {provider}={}",
            row.login.as_deref().unwrap_or("<login>")
        );
    }
    format!("lf account {provider} (refresh credential and capacity evidence)")
}

/// A login's browsers: the bindings that name it, the profiles they point
/// at, and whether this Chrome has each one signed in.
fn render_login(report: &AccountReport, width: usize, now: i64) -> String {
    let Some(row) = report.accounts.first() else {
        return String::new();
    };
    let mut lines = row_lines(row, now);
    if let Some(browser) = &report.browser {
        if browser.bindings.is_empty() {
            lines.push(format!(
                "  browser: none saved; lf account set {provider}={login} --chrome-profile <profile>",
                provider = row.provider.as_str(),
                login = row.login.as_deref().unwrap_or("<login>")
            ));
        }
        for binding in &browser.bindings {
            let Some(profile) = browser
                .profiles
                .iter()
                .find(|profile| profile.id == binding.profile_id)
            else {
                continue;
            };
            let signed_in = browser
                .local_choices
                .iter()
                .find(|choice| choice.directory == profile.chrome_directory)
                .map(|choice| match &choice.login {
                    Some(login) => format!("signed in as {login}"),
                    None => "not signed in".into(),
                })
                .unwrap_or_else(|| "not in this Chrome".into());
            lines.push(format!(
                "  browser {}: {} · {} · expects {} · {signed_in}",
                binding.position + 1,
                profile.id,
                profile.chrome_directory,
                profile
                    .expected_login
                    .as_ref()
                    .map(|e| e.as_str())
                    .unwrap_or("unconstrained")
            ));
        }
        if let Some(reason) = &browser.discovery_diagnostic {
            lines.push(format!("  {reason}"));
        }
    }
    wrap_lines(lines, width)
}

/// `lf account <provider>`: every login and local credential, then every
/// saved browser.
fn render(report: &AccountReport, width: usize, now: i64) -> String {
    let mut lines = Vec::new();
    for (local, title) in [(false, "Managed accounts"), (true, "Local services")] {
        lines.push(title.into());
        let mut count = 0;
        for row in report
            .accounts
            .iter()
            .filter(|row| (row.scope == Scope::Local) == local)
        {
            count += 1;
            lines.extend(row_lines(row, now));
        }
        if count == 0 {
            lines.push("  none".into());
        }
        lines.push(String::new());
    }
    if let Some(browser) = &report.browser {
        lines.push("Browser profiles".into());
        for profile in &browser.profiles {
            lines.push(format!(
                "{} · {} · expects {}",
                profile.id,
                profile.chrome_directory,
                profile
                    .expected_login
                    .as_ref()
                    .map(|e| e.as_str())
                    .unwrap_or("unconstrained")
            ));
            for binding in browser
                .bindings
                .iter()
                .filter(|b| b.profile_id == profile.id)
            {
                lines.push(format!(
                    "  {} / {} · choice {}",
                    binding.provider,
                    binding
                        .account_id
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "local credentials".into()),
                    binding.position + 1
                ));
            }
        }
        lines.push("Local Chrome choices".into());
        for choice in &browser.local_choices {
            lines.push(format!(
                "  {} · {} · {}",
                choice.name,
                choice.directory,
                choice.login.as_deref().unwrap_or("not signed in")
            ));
        }
        if let Some(reason) = &browser.discovery_diagnostic {
            lines.push(format!("  {reason}"));
        }
    }
    wrap_lines(lines, width)
}

/// One login or local credential: identity, auth, plan, routing, what to do
/// next, and each usage window with its age and source.
fn row_lines(row: &AccountRow, now: i64) -> Vec<String> {
    let mut lines = Vec::new();
    let identity = match (&row.account_id, &row.login) {
        (Some(id), Some(login)) => format!("{id} · {login}"),
        (Some(id), None) => id.to_string(),
        (None, Some(login)) => login.clone(),
        (None, None) => "local credentials".into(),
    };
    lines.push(format!("{:<12} {identity}", row.provider.as_str()));
    let evidence = match row.verification {
        Verification::Accepted => "active (verified)".into(),
        Verification::Rejected => "needs login (identity or credential rejected)".into(),
        Verification::Unavailable => format!(
            "cached {} · verification unavailable",
            row.cached_credential_state
        ),
        Verification::NotChecked => {
            format!("cached {} · not checked", row.cached_credential_state)
        }
    };
    lines.push(format!("  auth: {evidence}"));
    if row.scope == Scope::Managed {
        let plan = match (row.configured_plan.as_deref(), row.observed_plan.as_deref()) {
            (Some(set), Some(seen)) if set == seen => set.to_string(),
            (Some(set), Some(seen)) => format!("{set} set · {seen} observed"),
            (Some(set), None) => format!("{set} set"),
            (None, Some(seen)) => format!("{seen} observed"),
            (None, None) => "unknown".into(),
        };
        lines.push(format!("  plan: {plan}"));
        lines.push(format!("  launches: {}", launch_summary(row, false)));
    }
    let mut policy = Vec::new();
    if let Some(routing) = row.routing.as_ref().filter(|r| r.as_str() != "automatic") {
        policy.push(routing.replace('_', "-"));
    }
    if let Some(until) = row.cooldown_until.filter(|until| *until > now) {
        policy.push(format!(
            "cooling for {}",
            format_relative_delta(until - now)
        ));
    }
    if !policy.is_empty() {
        lines.push(format!("  routing: {}", policy.join(" · ")));
    }
    if let (Scope::Managed, Some(expires)) = (&row.scope, row.expires_at) {
        let state = if expires > now { "until" } else { "expired" };
        lines.push(format!(
            "  access token {state} {}; the laptop's `lf machine connect` renews it",
            date(expires)
        ));
    }
    if let Some(reason) = &row.diagnostic {
        lines.push(format!("  {reason}"));
    }
    if let Some(recovery) = &row.recovery {
        lines.push(format!("  recover: {recovery}"));
    } else {
        lines.push(format!("  next: {}", next_action(row, now)));
    }
    if row.windows.is_empty() {
        lines.push("  usage: unknown".into());
    }
    for window in &row.windows {
        let freshness = if row.verified_windows.contains(&window.window) {
            "live".to_string()
        } else {
            format!(
                "cached {} ago",
                format_relative_delta(now - window.observed_at)
            )
        };
        let reset = match window.resets_at {
            Some(reset) if reset <= now => "reset passed; refresh needed".into(),
            Some(reset) => format!("resets {}", timestamp(reset)),
            None => "reset unknown".into(),
        };
        let usage = if window.resets_at.is_some_and(|reset| reset <= now) {
            "usage unknown".to_string()
        } else {
            format!(
                "{}% used, {}% left",
                window.used_percent,
                100u8.saturating_sub(window.used_percent)
            )
        };
        lines.push(format!(
            "  {}: {usage} · {freshness} · {reset}",
            window.window
        ));
        lines.push(format!(
            "    source={} observed={} plan={}",
            window.source,
            timestamp(window.observed_at),
            window.plan.as_deref().unwrap_or("unknown")
        ));
    }
    if row.provider == Provider::Codex && row.scope == Scope::Managed {
        match &row.reset_credits {
            Some(credits) => {
                lines.push(format!(
                    "  banked resets: {} available (live)",
                    credits.available_count
                ));
                for credit in credits.credits.iter().flatten() {
                    let expiry = credit
                        .expires_at
                        .map(timestamp)
                        .unwrap_or_else(|| "unknown".into());
                    lines.push(format!(
                        "    {}: {} · expires {expiry}",
                        credit.id, credit.status
                    ));
                }
            }
            None => lines.push("  banked resets: unknown".into()),
        }
    }
    lines
}

fn wrap_lines(lines: Vec<String>, width: usize) -> String {
    lines
        .into_iter()
        .map(|line| wrap_line(&line, width.max(40)))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

const BAR_WIDTH: usize = 16;

/// One row per stored login: the weekly window as a bar and percent left,
/// when it resets, and what marks it; then a line per thing to fix, with the
/// command that fixes it.
fn render_table(report: &AccountReport, notes: &[RouteNote], now: i64) -> String {
    let rows: Vec<_> = report
        .accounts
        .iter()
        .filter(|row| row.scope == Scope::Managed)
        .collect();
    if rows.is_empty() {
        return "No stored logins. `lf account connect claude=<email>` adds one\n".into();
    }
    let names: Vec<String> = rows
        .iter()
        .map(|row| short_login(row, &report.accounts))
        .collect();
    let login_width = names
        .iter()
        .map(|name| name.chars().count())
        .max()
        .unwrap_or(0);
    let mut lines = Vec::new();
    let mut fixes = Vec::new();
    for (row, name) in rows.iter().zip(&names) {
        let weekly = crate::provider_account::tightest_weekly(&row.windows, now);
        let (bar, left) = match weekly {
            Some(window) => {
                let left = 100u8.saturating_sub(window.used_percent);
                let filled = usize::from(left) * BAR_WIDTH / 100;
                (
                    format!("{}{}", "━".repeat(filled), "─".repeat(BAR_WIDTH - filled)),
                    format!("{left:>3}% left"),
                )
            }
            None => ("╌".repeat(BAR_WIDTH), "   ? left".to_string()),
        };
        let reset = weekly
            .and_then(|window| window.resets_at)
            .map(local_time)
            .unwrap_or_default();
        let mut marks = Vec::new();
        if row.active {
            marks.push("● active".to_string());
        }
        if row.held {
            marks.push("held".to_string());
        }
        match row.routing.as_deref() {
            Some("explicit_only") => marks.push("paused".to_string()),
            Some("disabled") => marks.push("disabled".to_string()),
            _ => {}
        }
        if let Some(at) = newest_reading(&row.windows).filter(|at| now - at > STALE_READING_SECS) {
            marks.push(format!("{} ago", format_relative_delta(now - at)));
        }
        lines.push(
            format!(
                "  {:<6}  {name:<login_width$}  {bar}  {left}  {reset:<9}  {}",
                row.provider.as_str(),
                marks.join(" · ")
            )
            .trim_end()
            .to_string(),
        );
        let indent = " ".repeat(10 + login_width);
        let unavailable = launch_summary(row, true);
        if !unavailable.is_empty() {
            lines.push(format!("{indent}{unavailable}"));
        }
        if let Some(recovery) = &row.recovery {
            fixes.push((
                format!("{} {name} needs a login", row.provider.as_str()),
                recovery.clone(),
            ));
        }
    }
    for note in notes {
        let provider = note.provider.as_str();
        let (scope, command) = match &note.scope {
            crate::profile::RouteScope::Default => (
                "default order",
                format!("lf unset account.{provider} --global"),
            ),
            crate::profile::RouteScope::Repo(_) => {
                ("order here", format!("lf unset account.{provider}"))
            }
            crate::profile::RouteScope::Wave(wave) => (
                "Wave order",
                format!("lf --wave {wave} unset account.{provider}"),
            ),
        };
        fixes.push((
            format!(
                "{provider} {scope} uses {} of {}",
                note.named, note.rotating
            ),
            command,
        ));
    }
    if !fixes.is_empty() {
        lines.push(String::new());
        let width = fixes
            .iter()
            .map(|(what, _)| what.chars().count())
            .max()
            .unwrap_or(0);
        for (what, command) in fixes {
            lines.push(format!("  ! {what:<width$}   {command}"));
        }
    }
    lines.join("\n") + "\n"
}

/// `jack@` when no other login of the provider starts that way, else the
/// whole email: what `lf account use` accepts for it.
fn short_login(row: &AccountRow, rows: &[AccountRow]) -> String {
    let Some(login) = row.login.as_deref() else {
        return row
            .account_id
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
    };
    let Some((local, _)) = login.split_once('@') else {
        return login.to_string();
    };
    let prefix = format!("{local}@");
    let shared = rows.iter().any(|other| {
        other.provider == row.provider
            && other.scope == Scope::Managed
            && other.account_id != row.account_id
            && other.login.as_deref().is_some_and(|other| {
                other
                    .to_ascii_lowercase()
                    .starts_with(&prefix.to_ascii_lowercase())
            })
    });
    if shared {
        login.to_string()
    } else {
        prefix
    }
}

fn date(seconds: i64) -> String {
    OffsetDateTime::from_unix_timestamp(seconds)
        .map(|moment| moment.date().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

fn timestamp(seconds: i64) -> String {
    OffsetDateTime::from_unix_timestamp(seconds)
        .ok()
        .and_then(|date| {
            date.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_else(|| "unknown".into())
}

// Preserve every character of selectors; continuation lines never collide with columns.
fn wrap_line(line: &str, width: usize) -> String {
    let mut remaining: String = line.chars().filter(|c| !c.is_control()).collect();
    let mut lines = Vec::new();
    while remaining.chars().count() > width {
        let boundary = remaining
            .char_indices()
            .nth(width)
            .expect("line exceeds width")
            .0;
        let split = remaining[..boundary]
            .rfind(char::is_whitespace)
            .filter(|split| *split > 4)
            .unwrap_or(boundary);
        lines.push(remaining[..split].to_string());
        remaining = format!("    {}", remaining[split..].trim_start());
    }
    lines.push(remaining);
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{login_browser, render, render_login, render_table, AccountReport, RouteNote};
    use crate::provider_account::operations::account_status::report as project_report;
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn claude_usage_reconciliation_account_handlers_refresh_both_logins() {
        use crate::provider_account::{
            account_held_until, new_account, resolve_provider_account_exact,
        };
        use crate::provider_auth::Provider;
        use crate::store::{AccountLimitWindow, ProviderAccountId, StorageConfig};
        let _env = crate::journal::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        let _homes = crate::test_ambient::EnvGuard::clear(&[
            "LF_HOME",
            "CLAUDE_CONFIG_DIR",
            "LF_ACCOUNT_SELECTION",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "ANTHROPIC_API_KEY",
        ]);
        let native = root.path().join("native");
        std::env::set_var("LF_HOME", root.path().join("machine"));
        std::env::set_var("CLAUDE_CONFIG_DIR", &native);
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                root.path().join("machine/loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let reset = now + 86_400;
        let mut accounts = Vec::new();
        let mut responses = Vec::new();
        for (id, login, weekly, fable) in [
            ("first", "jack@loopflow.studio", 66, 11),
            ("second", "jackstah@gmail.com", 65, 100),
        ] {
            let home = root.path().join(id);
            std::fs::create_dir_all(&home).unwrap();
            let credential = serde_json::json!({"claudeAiOauth":{
                "accessToken": format!("fixture-{id}"), "expiresAt":4102444800000i64
            }})
            .to_string();
            std::fs::write(home.join(".credentials.json"), &credential).unwrap();
            let mut account = new_account(
                Provider::Claude,
                ProviderAccountId::parse(id).unwrap(),
                home,
                Some(crate::profile::EmailAddress::parse(login).unwrap()),
            );
            account.observed_email = Some(login.into());
            account.observed_subject = Some(id.into());
            account.observed_credential_digest = Some(
                crate::provider_account::identity::credential_digest(&credential),
            );
            store.upsert_provider_account(&account).await.unwrap();
            // Historical stream alias, before normalized writes.
            store
                .upsert_provider_account_limits(
                    "claude",
                    &account.account_id,
                    &[AccountLimitWindow {
                        window: "rejected: seven_day_overage_included".into(),
                        used_percent: 100,
                        resets_at: Some(reset),
                        plan: None,
                        model: Some("fable".into()),
                        limited_until: Some(reset + 5),
                    }],
                    "stream",
                )
                .await
                .unwrap();
            responses.push((
                200,
                serde_json::json!({"limits":[
                    {"group":"weekly", "percent":weekly, "resets_at":super::timestamp(reset)},
                    {"group":"weekly", "percent":fable, "resets_at":super::timestamp(reset),
                        "scope":{"model":{"display_name":"Fable"}}}
                ]})
                .to_string(),
            ));
            responses.push((
                200,
                serde_json::json!({"account":{"email":login, "uuid":id}}).to_string(),
            ));
            accounts.push(account);
        }
        let cached = project_report(
            Some(crate::provider_account::operations::Target::Provider(
                Provider::Claude,
            )),
            true,
        )
        .await
        .unwrap();
        assert!(cached
            .accounts
            .iter()
            .filter(|row| row.scope == super::Scope::Managed)
            .all(|row| row
                .launches
                .iter()
                .any(|launch| launch.model.as_deref() == Some("fable") && !launch.eligible)));
        let blocked = crate::provider_account::routing::next(None, Some("claude:fable"))
            .await
            .unwrap();
        assert_eq!(
            blocked[0].outcome,
            crate::provider_account::routing::NextOutcome::Blocked
        );
        assert!(blocked[0].reason.contains("for fable until"));
        for account in &accounts {
            assert!(blocked[0]
                .reason
                .contains(crate::provider_account::account_login(account)));
        }
        let (_endpoints, server) =
            crate::subscription::observation_tests::serve(responses.clone(), |_| {}).await;
        let report = project_report(
            Some(crate::provider_account::operations::Target::Provider(
                Provider::Claude,
            )),
            false,
        )
        .await
        .unwrap();
        server.await.unwrap();
        let managed: Vec<_> = report
            .accounts
            .iter()
            .filter(|row| row.account_id.is_some())
            .collect();
        assert_eq!(managed.len(), 2);
        assert!(
            managed
                .iter()
                .all(|row| row.verification == super::Verification::Accepted
                    && row.cooldown_until.is_none()
                    && row.verified_windows.contains(&"seven_day_fable".to_owned())
                    && row
                        .windows
                        .iter()
                        .all(|window| window.window != "seven_day_overage_included")),
            "{managed:?}"
        );
        for (row, held) in managed.iter().zip([false, true]) {
            let fable = row
                .launches
                .iter()
                .find(|launch| launch.model.as_deref() == Some("fable"))
                .unwrap();
            assert_eq!(fable.eligible, !held);
            assert_eq!(fable.out_until, held.then_some(reset + 5));
            let sonnet = row
                .launches
                .iter()
                .find(|launch| launch.model.as_deref() == Some("sonnet"))
                .unwrap();
            assert!(sonnet.eligible);
        }
        let json = serde_json::to_value(&report).unwrap();
        assert!(json.to_string().contains("seven_day_fable"));
        let next = crate::provider_account::routing::next(None, Some("claude:fable"))
            .await
            .unwrap();
        assert_eq!(next[0].model.as_deref(), Some("fable"));
        assert_eq!(next[0].account_id.as_ref(), Some(&accounts[0].account_id));
        assert_eq!(next[1].model, None);
        let unknown = crate::provider_account::routing::next(None, Some("claude:future-sonnet"))
            .await
            .unwrap();
        assert_eq!(
            unknown[0].outcome,
            crate::provider_account::routing::NextOutcome::Ready
        );
        assert_eq!(unknown[0].model.as_deref(), Some("future-sonnet"));
        let text = super::render(&report, 100, now);
        assert!(text.contains("fable out until"), "{text}");
        let limits = store.provider_account_limits(Some("claude")).await.unwrap();
        assert_eq!(
            account_held_until(&accounts[0], &limits, Some("fable"), now),
            None
        );
        assert_eq!(
            account_held_until(&accounts[1], &limits, Some("fable"), now),
            Some(reset + 5)
        );
        for account in &accounts {
            assert_eq!(
                account_held_until(account, &limits, Some("sonnet"), now),
                None
            );
        }
        // Exercise the real account output handlers with the committed cached result.
        super::run(
            Some(crate::provider_account::operations::Target::Provider(
                Provider::Claude,
            )),
            true,
            true,
        )
        .await
        .unwrap();
        super::run(
            Some(crate::provider_account::operations::Target::Provider(
                Provider::Claude,
            )),
            true,
            false,
        )
        .await
        .unwrap();
        let next = crate::provider_account::routing::next(None, None)
            .await
            .unwrap();
        assert_eq!(next[0].account_id.as_ref(), Some(&accounts[0].account_id));
        for model in ["fable", "sonnet"] {
            let route = resolve_provider_account_exact(
                Provider::Claude,
                None,
                None,
                None,
                Some(model),
                Some(root.path()),
            )
            .await
            .unwrap()
            .unwrap();
            if model == "fable" {
                assert_eq!(route.account_id(), &accounts[0].account_id);
            }
        }
        // The same requested family reaches routing when reopening a recorded conversation.
        let conversation: loopflow_model::id::AgentSessionId = "fixture-conversation".into();
        std::fs::create_dir_all(native.join("projects/fixture")).unwrap();
        std::fs::write(
            native.join("projects/fixture/fixture-conversation.jsonl"),
            "{}\n",
        )
        .unwrap();
        store
            .pin_provider_session_route(Provider::Claude, &conversation, &accounts[1].account_id)
            .await
            .unwrap();
        let resumed = resolve_provider_account_exact(
            Provider::Claude,
            Some(&conversation),
            Some(&accounts[1].account_id),
            None,
            Some("sonnet"),
            Some(root.path()),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(accounts
            .iter()
            .any(|account| &account.account_id == resumed.account_id()));
        assert_eq!(
            store.provider_account_limits(Some("claude")).await.unwrap(),
            limits
        );
        drop(_endpoints);
        let writer = store.clone();
        let first = accounts[0].account_id.clone();
        let (_endpoints, server) =
            crate::subscription::observation_tests::serve(responses, move |index| {
                if index == 0 {
                    writer
                        .sqlite
                        .record_provider_account_capacity(
                            "claude",
                            &first,
                            &crate::store::AccountCapacity {
                                utilization_percent: Some(100),
                                cooldown_until: Some(reset + 5),
                                cooldown_reason: Some("rejected: seven_day".into()),
                                windows: vec![AccountLimitWindow {
                                    window: "seven_day".into(),
                                    used_percent: 100,
                                    resets_at: Some(reset),
                                    model: None,
                                    plan: None,
                                    limited_until: Some(reset + 5),
                                }],
                            },
                        )
                        .unwrap();
                }
            })
            .await;
        let raced = project_report(
            Some(crate::provider_account::operations::Target::Provider(
                Provider::Claude,
            )),
            false,
        )
        .await
        .unwrap();
        server.await.unwrap();
        let first = raced
            .accounts
            .iter()
            .find(|row| row.account_id.as_ref() == Some(&accounts[0].account_id))
            .unwrap();
        assert_eq!(first.verification, super::Verification::Accepted);
        assert_eq!(first.cooldown_until, Some(reset + 5));
        assert!(first.verified_windows.is_empty());
        assert!(first.diagnostic.as_deref().unwrap().contains("superseded"));
        assert_eq!(
            first
                .windows
                .iter()
                .find(|window| window.window == "seven_day")
                .unwrap()
                .used_percent,
            100
        );
        let next = crate::provider_account::routing::next(None, None)
            .await
            .unwrap();
        assert!(next[0].reason.contains("fable"), "{:?}", next[0]);
    }

    #[test]
    fn account_report_fixture_preserves_missingness_and_readable_widths() {
        let fixture = include_str!("../../../../../tests/fixtures/dto/auth_status.json");
        let report: AccountReport = serde_json::from_str(fixture).unwrap();
        let rejected = report
            .accounts
            .iter()
            .find(|row| row.verification == super::Verification::Rejected)
            .unwrap();
        assert_eq!(rejected.cached_credential_state, "missing");
        assert_eq!(
            serde_json::to_value(&report).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
        for width in [80, 120] {
            let text = render(&report, width, 1_800_000_000);
            assert!(text.lines().all(|line| line.chars().count() <= width));
            for evidence in [
                "someone-with-a-very-long-but-valid-username@engineering.department.example.com",
                "session: 22% used, 78% left · live",
                "weekly:opus: usage unknown · cached",
                "reset passed; refresh needed",
                "reset unknown",
                "usage: unknown",
                "next: lf account claude",
                "wait for cooldown, then lf account claude",
                "needs login (identity or credential rejected)",
                "verification unavailable",
                "cached expired",
                "cached uninspected",
                "Local services",
                "source=poll observed=2027-01-15T08:00:00Z plan=unknown",
                "Browser profiles",
                "personal · Profile 3 · expects personal@example.com",
                "work · Profile 7 · expects unconstrained",
                "claude / personal · choice 2",
                "linear / local credentials · choice 1",
                "Spare · Profile 9 · not signed in",
                "access token until 2027-05-11",
                "access token expired 2026-09-21",
            ] {
                assert!(
                    text.replace("\n    ", "").contains(evidence),
                    "missing {evidence}: {text}"
                );
            }
            assert!(!text.contains(": 0% used"));
            assert!(!text.contains("96%"));
            assert!(!text.contains("unlimited"));
            // A decisive rejection renders its verdict, never the pre-verification cache.
            assert!(!text.contains("cached connected · needs login"));
            assert!(!text.contains("cached missing · needs login"));
            println!("--- {width} columns ---\n{text}");
        }
        for field in ["model", "eligible", "out_until", "blockers"] {
            let mut incomplete = serde_json::to_value(&report).unwrap();
            incomplete["accounts"][0]["launches"][0]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(
                serde_json::from_value::<AccountReport>(incomplete).is_err(),
                "{field}"
            );
        }
        let mut incomplete = serde_json::to_value(&report).unwrap();
        incomplete["accounts"][0]
            .as_object_mut()
            .unwrap()
            .remove("launches");
        assert!(serde_json::from_value::<AccountReport>(incomplete).is_err());
        let mut incomplete = serde_json::to_value(&report).unwrap();
        incomplete["accounts"][0]
            .as_object_mut()
            .unwrap()
            .remove("windows");
        assert!(serde_json::from_value::<AccountReport>(incomplete).is_err());
    }

    #[test]
    fn a_login_alone_shows_that_login_and_its_browsers() {
        let fixture = include_str!("../../../../../tests/fixtures/dto/auth_status.json");
        let report: AccountReport = serde_json::from_str(fixture).unwrap();
        let now = 1_800_000_000;
        let login = |id: &str| {
            report
                .accounts
                .iter()
                .find(|row| {
                    row.account_id
                        .as_ref()
                        .is_some_and(|account| account.as_str() == id)
                })
                .unwrap()
        };
        let browser = || {
            serde_json::from_value(serde_json::to_value(report.browser.as_ref().unwrap()).unwrap())
                .unwrap()
        };
        let personal = login_browser(browser(), login("personal"));
        assert_eq!(personal.bindings.len(), 2, "{personal:?}");
        assert_eq!(personal.profiles.len(), 2);
        assert_eq!(personal.local_choices.len(), 2, "Spare is nobody's");
        let text = render_login(
            &AccountReport {
                accounts: vec![serde_json::from_value(
                    serde_json::to_value(login("personal")).unwrap(),
                )
                .unwrap()],
                browser: Some(personal),
            },
            120,
            now,
        );
        println!("{text}");
        for expected in [
            "personal · someone-with-a-very-long-but-valid-username@engineering.department.example.com",
            "auth: active (verified)",
            "plan: max",
            "session: 22% used, 78% left · live · resets 2027-01-15T09:00:00Z",
            "source=poll observed=2027-01-15T08:00:00Z plan=unknown",
            "weekly:opus: usage unknown · cached 1d ago · reset passed; refresh needed",
            "browser 1: personal · Profile 3 · expects personal@example.com · signed in as personal@example.com",
            "browser 2: work · Profile 7 · expects unconstrained · not signed in",
        ] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
        for absent in [
            "Managed accounts",
            "Local services",
            "Browser profiles",
            "Local Chrome choices",
            "linear",
            "Spare",
            "missing@example.com",
            "source=managed_home",
        ] {
            assert!(!text.contains(absent), "unexpected {absent}: {text}");
        }
        let unbound = login_browser(browser(), login("missing"));
        assert!(unbound.bindings.is_empty() && unbound.profiles.is_empty());
        let text = render_login(
            &AccountReport {
                accounts: vec![serde_json::from_value(
                    serde_json::to_value(login("missing")).unwrap(),
                )
                .unwrap()],
                browser: Some(unbound),
            },
            120,
            now,
        );
        assert!(text.contains("plan: unknown"), "{text}");
        assert!(
            text.contains(
                "browser: none saved; lf account set claude=missing@example.com --chrome-profile <profile>"
            ),
            "{text}"
        );
    }

    #[test]
    fn claude_capacity_summaries_show_shared_blockers_once() {
        let fixture = include_str!("../../../../../tests/fixtures/dto/auth_status.json");
        let mut report: AccountReport = serde_json::from_str(fixture).unwrap();
        let row = &mut report.accounts[0];
        assert_eq!(
            super::launch_summary(row, false),
            "fable, opus, sonnet, haiku"
        );
        assert_eq!(super::launch_summary(row, true), "");
        let healthy: AccountReport = serde_json::from_str(fixture).unwrap();
        let single = AccountReport {
            accounts: vec![healthy.accounts.into_iter().next().unwrap()],
            browser: None,
        };
        assert_eq!(render_table(&single, &[], 1_800_000_000).lines().count(), 1);
        for reason in ["credential is missing", "routing is explicit_only"] {
            for launch in &mut row.launches {
                launch.eligible = false;
                launch.blockers = vec![reason.into()];
            }
            let expected = format!("none · {reason}");
            assert_eq!(super::launch_summary(row, false), expected);
            assert_eq!(super::launch_summary(row, true), expected);
        }
        row.launches[0].out_until = Some(1_800_086_400);
        row.launches[0].blockers.push("cooling for fable".into());
        let summary = super::launch_summary(row, false);
        assert!(summary.starts_with("none · routing is explicit_only · fable out until"));
        assert_eq!(summary.matches("routing is explicit_only").count(), 1);

        row.launches.retain(|launch| launch.model.is_none());
        row.launches[0].eligible = true;
        row.launches[0].blockers.clear();
        assert_eq!(super::launch_summary(row, false), "ready");
        assert_eq!(super::launch_summary(row, true), "");
        row.launches[0].eligible = false;
        row.launches[0].out_until = Some(1_800_086_400);
        row.launches[0].blockers.push("cooling account-wide".into());
        assert!(super::launch_summary(row, false).starts_with("out until "));
    }

    #[test]
    fn the_overview_is_one_row_per_login_with_fixes_under_it() {
        let fixture = include_str!("../../../../../tests/fixtures/dto/auth_status.json");
        let mut report: AccountReport = serde_json::from_str(fixture).unwrap();
        let now = 1_800_000_000;
        report.accounts[0]
            .windows
            .push(crate::store::AccountLimitRow {
                provider: "claude".into(),
                account_id: crate::store::ProviderAccountId::parse("primary").unwrap(),
                window: "seven_day".into(),
                used_percent: 69,
                resets_at: Some(now + 86_400),
                plan: None,
                observed_at: now - 60,
                source: "poll".into(),
                model: None,
                limited_until: None,
            });
        let fable = crate::store::AccountLimitRow {
            window: "seven_day_fable".into(),
            used_percent: 100,
            model: Some("fable".into()),
            limited_until: Some(now + 86_400),
            ..report.accounts[0].windows[2].clone()
        };
        report.accounts[0].windows.push(fable);
        let launch = report.accounts[0]
            .launches
            .iter_mut()
            .find(|launch| launch.model.as_deref() == Some("fable"))
            .unwrap();
        launch.eligible = false;
        launch.out_until = Some(now + 86_400);
        launch.blockers = vec!["cooling for fable".into()];
        let notes = [RouteNote {
            provider: crate::provider_auth::Provider::Codex,
            scope: crate::profile::RouteScope::Default,
            named: 1,
            rotating: 4,
        }];
        let text = render_table(&report, &notes, now);
        println!("{text}");
        let rows: Vec<_> = text
            .lines()
            .filter(|line| line.starts_with("  claude") || line.starts_with("  codex"))
            .collect();
        assert_eq!(rows.len(), 5, "managed logins only: {text}");
        assert!(rows[0].contains("31% left") && rows[0].contains("● active · held"));
        assert!(
            rows[1].contains("? left"),
            "no reading is unknown, never 0%"
        );
        assert!(rows[4].contains("paused"));
        for expected in [
            "fable out until",
            "out until",
            "! claude missing@ needs a login",
            "lf account connect claude=missing@example.com",
            "! codex default order uses 1 of 4",
            "lf unset account.codex --global",
        ] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
        assert!(!text.contains("opus, sonnet, haiku"), "{text}");
        assert!(!text.contains("launch out until"), "{text}");
        assert!(!text.contains(" 0% left"));
        assert!(!text.contains("local credentials"));
    }
}
