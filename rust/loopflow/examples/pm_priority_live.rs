use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::engine::config::{load_config_or_default, AsanaConfig, LinearConfig};
use loopflow::lfd;
use loopflow::lfd::pm::asana::AsanaClient;
use loopflow::lfd::pm::linear::LinearClient;
use loopflow::lfd::pm::{PmItem, PmProvider, PmProviderKind};
use loopflow::lfd::store::open_store;
use loopflow::ops::pm::pm_init;
use loopflow::ops::NullProgress;
use tempfile::TempDir;

fn main() -> Result<(), String> {
    let provider = parse_provider()?;
    let name = format!(
        "pm-priority-live-{}",
        time::OffsetDateTime::now_utc().unix_timestamp()
    );
    let temp = TempDir::new().map_err(|err| format!("temp dir: {err}"))?;
    init_git_repo(temp.path())?;
    write_test_repo(temp.path(), &name, provider)?;

    let result = pm_init(temp.path(), &NullProgress).map_err(|err| err.to_string())?;

    let wave_result = result
        .waves
        .first()
        .ok_or_else(|| "no wave results".to_string())?;
    let items = fetch_remote_items(temp.path(), provider, &wave_result.project_id)?;
    let names = items
        .iter()
        .map(|item| item.name.as_str())
        .collect::<Vec<_>>();
    let expected = vec!["First priority", "Second priority", "Third priority"];

    println!("provider: {provider:?}");
    println!("wave: {name}");
    println!("project_id: {}", wave_result.project_id);
    println!("remote order:");
    for (index, item) in items.iter().enumerate() {
        println!("  {}. {}", index + 1, item.name);
    }

    if names == expected {
        println!("PASS: provider returned priorities in local file order");
        return Ok(());
    }

    Err(format!("FAIL: expected {:?}, got {:?}", expected, names))
}

fn parse_provider() -> Result<PmProviderKind, String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("linear") => Ok(PmProviderKind::Linear),
        Some("asana") => Ok(PmProviderKind::Asana),
        Some(value) => Err(format!(
            "unknown provider `{value}`; expected `linear` or `asana`"
        )),
        None => Err("usage: cargo run --example pm_priority_live -- <linear|asana>".to_string()),
    }
}

fn init_git_repo(repo: &Path) -> Result<(), String> {
    run_git(repo, ["init"])?;
    run_git(repo, ["config", "user.name", "Loopflow Test"])?;
    run_git(repo, ["config", "user.email", "loopflow@example.com"])?;
    Ok(())
}

fn run_git<const N: usize>(repo: &Path, args: [&str; N]) -> Result<(), String> {
    let status = Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .map_err(|err| format!("git {:?}: {err}", args))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("git {:?} failed with status {status}", args))
    }
}

fn write_test_repo(repo: &Path, wave: &str, provider: PmProviderKind) -> Result<(), String> {
    let config = load_config_or_default(Some(Path::new(".")));
    let lf_dir = repo.join(".lf");
    fs::create_dir_all(&lf_dir).map_err(|err| format!("create .lf/: {err}"))?;
    fs::write(
        lf_dir.join("config.yaml"),
        render_config(provider, &config.asana, &config.linear),
    )
    .map_err(|err| format!("write .lf/config.yaml: {err}"))?;

    let wave_dir = repo.join("wave").join(wave);
    fs::create_dir_all(&wave_dir).map_err(|err| format!("create wave dir: {err}"))?;
    fs::write(
        wave_dir.join(format!("{wave}.yaml")),
        format!(
            "flow: build\npm:\n  provider: {}\n",
            provider_name(provider)
        ),
    )
    .map_err(|err| format!("write wave yaml: {err}"))?;
    fs::write(
        wave_dir.join("README.md"),
        format!("# {wave}\n\nLive priority-order validation.\n"),
    )
    .map_err(|err| format!("write README: {err}"))?;
    write_item(&wave_dir.join("01-first-priority.md"), "First priority")?;
    write_item(&wave_dir.join("02-second-priority.md"), "Second priority")?;
    write_item(&wave_dir.join("03-third-priority.md"), "Third priority")?;
    Ok(())
}

fn write_item(path: &Path, title: &str) -> Result<(), String> {
    fs::write(path, format!("# {title}\n\nPriority test item.\n"))
        .map_err(|err| format!("write {}: {err}", path.display()))
}

fn render_config(provider: PmProviderKind, asana: &AsanaConfig, linear: &LinearConfig) -> String {
    let mut lines = vec![format!("pm:\n  provider: {}", provider_name(provider))];
    if asana.workspace.is_some() || asana.default_team.is_some() {
        lines.push("asana:".to_string());
        if let Some(workspace) = &asana.workspace {
            lines.push(format!("  workspace: \"{workspace}\""));
        }
        if let Some(default_team) = &asana.default_team {
            lines.push(format!("  default_team: \"{default_team}\""));
        }
    }
    if let Some(team) = &linear.team {
        lines.push("linear:".to_string());
        lines.push(format!("  team: \"{team}\""));
    }
    lines.join("\n") + "\n"
}

fn provider_name(provider: PmProviderKind) -> &'static str {
    match provider {
        PmProviderKind::Asana => "asana",
        PmProviderKind::Linear => "linear",
        _ => "unknown",
    }
}

fn fetch_remote_items(
    repo: &Path,
    provider: PmProviderKind,
    project_id: &str,
) -> Result<Vec<PmItem>, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|err| format!("runtime: {err}"))?;
    rt.block_on(async move {
        let cfg = lfd::storage_config_from_env().map_err(|err| err.to_string())?;
        let store = open_store(&cfg)
            .await
            .map_err(|err| format!("open store: {err}"))?;
        let token = store
            .get_provider_token(provider_name(provider))
            .await
            .map_err(|err| format!("load token: {err}"))?
            .ok_or_else(|| format!("no stored {} credential", provider_name(provider)))?
            .access_token;
        let config = load_config_or_default(Some(repo));
        match provider {
            PmProviderKind::Asana => {
                let client = AsanaClient::new(token, config.asana.clone());
                client
                    .list_items(project_id)
                    .await
                    .map_err(|err| err.to_string())
            }
            PmProviderKind::Linear => {
                let client = LinearClient::new(token, config.linear.team.clone());
                client
                    .list_items(project_id)
                    .await
                    .map_err(|err| err.to_string())
            }
            _ => Err(format!("unsupported provider {:?}", provider)),
        }
    })
}
