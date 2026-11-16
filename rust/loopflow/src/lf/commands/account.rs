#[path = "account_status.rs"]
pub(crate) mod account_status;
use crate::lf::AccountCommand;
use anyhow::{Context, Result};
use loopflow_execution::provider_account::operations::{self as operations, Settings, Target};
use loopflow_execution::provider_auth::Provider;
use loopflow_execution::store::RoutingState;

pub fn run(
    cmd: Option<&AccountCommand>,
    target: Option<&str>,
    cached: bool,
    json: bool,
    agent: Option<&str>,
) -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    match cmd {
        Some(cmd) => rt.block_on(run_async(cmd, agent)),
        None => {
            let target = target.map(Target::parse).transpose()?;
            rt.block_on(account_status::run(target, cached, json))
        }
    }
}

pub(crate) async fn run_async(cmd: &AccountCommand, agent: Option<&str>) -> Result<()> {
    match cmd {
        AccountCommand::Next {
            repo,
            default,
            json,
        } => super::profile::show_next(repo.as_deref(), *default, *json, agent).await,
        AccountCommand::Use { login } => operations::use_login(login).await,
        AccountCommand::Pause { login } => {
            operations::set_rotation(login, RoutingState::ExplicitOnly).await
        }
        AccountCommand::Unpause { login } => {
            operations::set_rotation(login, RoutingState::Automatic).await
        }
        AccountCommand::Disconnect { target } => operations::disconnect_target(target).await,
        AccountCommand::Connect {
            target,
            chrome_profile,
            import,
            api_key,
        } => operations::connect_target(target, chrome_profile.as_deref(), *import, *api_key).await,
        AccountCommand::RedeemReset {
            login,
            idempotency_key,
            credit_id,
            json,
        } => {
            let (provider, login) = Target::parse_login(login, "redeem-reset")?;
            let redemption = operations::redeem_reset(
                provider.unwrap_or(Provider::Codex).as_str(),
                &login,
                idempotency_key.as_deref(),
                credit_id.as_deref(),
            )
            .await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&redemption)?);
            } else {
                println!(
                    "{}: {} (attempt {})",
                    redemption.login, redemption.outcome, redemption.idempotency_key
                );
                println!("Before:");
                print_reset_usage(&redemption.before);
                println!("After:");
                match (&redemption.after, &redemption.refresh_error) {
                    (Some(usage), _) => print_reset_usage(usage),
                    (None, error) => println!("  unknown; {}", error.as_deref().unwrap_or("")),
                }
            }
            Ok(())
        }
        AccountCommand::Set {
            target,
            login_email,
            plan,
            clear_plan,
            paid_through,
            clear_paid_through,
            clear_cooldown,
            chrome_profile,
            clear_chrome_profiles,
        } => {
            operations::set(
                target,
                &Settings {
                    login_email: login_email.clone(),
                    plan: plan.clone(),
                    clear_plan: *clear_plan,
                    paid_through: paid_through.clone(),
                    clear_paid_through: *clear_paid_through,
                    clear_cooldown: *clear_cooldown,
                    chrome_profiles: chrome_profile.clone(),
                    clear_chrome_profiles: *clear_chrome_profiles,
                },
            )
            .await
        }
    }
}

fn print_reset_usage(usage: &loopflow_execution::provider_account::operations::UsageSnapshot) {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    for window in &usage.windows {
        if window.resets_at.is_some_and(|reset| reset <= now) {
            println!("  {}: usage unknown; reset passed", window.window);
        } else {
            println!(
                "  {}: {}% used, {}% left",
                window.window,
                window.used_percent,
                100u8.saturating_sub(window.used_percent)
            );
        }
    }
    println!(
        "  banked resets: {}",
        usage
            .reset_credits
            .as_ref()
            .map(|credits| credits.available_count.to_string())
            .unwrap_or_else(|| "unknown".into())
    );
}
