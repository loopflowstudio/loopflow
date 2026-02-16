use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::{Command, Output};

use crate::lfd::config::{CredentialMount, ExecutorType, LfdConfig, StorageType};

const POSTGRES_DATABASE_URL: &str = "postgres://lfd:lfd@postgres:5432/lfd";
const POSTGRES_IMAGE: &str = "postgres:16-alpine";

#[derive(Debug, Clone)]
struct ComposeServiceStatus {
    service: String,
    state: String,
    health: Option<String>,
}

pub fn ensure_docker_available() -> Result<()> {
    let version = Command::new("docker")
        .arg("--version")
        .output()
        .context("failed to run docker --version")?;
    if !version.status.success() {
        bail!("Container mode requires Docker. Install Docker Desktop, OrbStack, or Colima, then run `lfd install` again.");
    }

    let info = Command::new("docker")
        .arg("info")
        .output()
        .context("failed to run docker info")?;
    if !info.status.success() {
        bail!("Container mode requires a running Docker runtime. Start Docker Desktop, OrbStack, or Colima, then run `lfd install` again.");
    }

    Ok(())
}

pub fn managed_compose_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    Ok(home.join(".lf").join("docker-compose.yml"))
}

fn override_compose_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    Ok(home.join(".lf").join("docker-compose.override.yml"))
}

pub fn compose_files() -> Result<Vec<PathBuf>> {
    let managed = managed_compose_path()?;
    let override_file = override_compose_path()?;
    let mut files = vec![managed];
    if override_file.exists() {
        files.push(override_file);
    }
    Ok(files)
}

pub fn write_managed_compose_file(config: &LfdConfig) -> Result<PathBuf> {
    let path = managed_compose_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let content = render_compose_file(config)?;
    std::fs::write(&path, content)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

pub fn remove_managed_compose_file() -> Result<()> {
    let path = managed_compose_path()?;
    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(())
}

pub fn pull() -> Result<()> {
    let status = compose_status(&["pull"])?;
    if !status.success() {
        bail!("docker compose pull failed (exit {status})");
    }
    Ok(())
}

pub fn down() -> Result<()> {
    let status = compose_status(&["down"])?;
    if !status.success() {
        bail!("docker compose down failed (exit {status})");
    }
    Ok(())
}

fn service_status() -> Result<Vec<ComposeServiceStatus>> {
    let output = compose_output(&["ps", "--format", "json"])?;
    if !output.status.success() {
        bail!(
            "docker compose ps failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = stdout.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }

    if text.starts_with('[') {
        let rows: Vec<ComposePsRow> = serde_json::from_str(text)
            .context("failed parsing docker compose json status array")?;
        return Ok(rows.into_iter().map(ComposeServiceStatus::from).collect());
    }

    let mut rows = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let row: ComposePsRow =
            serde_json::from_str(line).context("failed parsing docker compose json status row")?;
        rows.push(ComposeServiceStatus::from(row));
    }

    Ok(rows)
}

pub fn teardown() -> Result<()> {
    let _ = down();
    let _ = remove_managed_compose_file();
    Ok(())
}

pub fn print_service_status() {
    match service_status() {
        Ok(rows) if rows.is_empty() => println!("compose: no services"),
        Ok(rows) => {
            for row in rows {
                let health = row.health.unwrap_or_else(|| "n/a".to_string());
                println!(
                    "service: {} state={} health={}",
                    row.service, row.state, health
                );
            }
        }
        Err(err) => println!("compose: unhealthy ({err})"),
    }
}

pub fn compose_program_args() -> Result<Vec<String>> {
    let mut args = vec!["compose".to_string()];
    for file in compose_files()? {
        args.push("-f".to_string());
        args.push(file.to_string_lossy().to_string());
    }
    args.push("up".to_string());
    Ok(args)
}

fn compose_status(args: &[&str]) -> Result<std::process::ExitStatus> {
    Ok(compose_command(args)?.status()?)
}

fn compose_output(args: &[&str]) -> Result<Output> {
    Ok(compose_command(args)?.output()?)
}

fn compose_command(args: &[&str]) -> Result<Command> {
    let mut command = Command::new("docker");
    command.arg("compose");
    for file in compose_files()? {
        command.arg("-f").arg(file);
    }
    command.args(args);
    Ok(command)
}

fn render_compose_file(config: &LfdConfig) -> Result<String> {
    if config.storage != StorageType::Postgres {
        bail!("container mode requires postgres storage")
    }
    if config.executor.r#type != ExecutorType::Docker {
        bail!("container mode requires docker executor")
    }

    let mut credential_mounts = Vec::new();
    for mount in &config.executor.credentials.mounts {
        let (host_path, container_path) = resolve_credential_mount(mount)?;
        credential_mounts.push(format!(
            "      - {}:{}:ro",
            host_path.display(),
            container_path
        ));
    }

    let mut credential_env = Vec::new();
    for env in &config.executor.credentials.env {
        let key = env.trim();
        if key.is_empty() {
            continue;
        }
        credential_env.push(format!("      {key}: \"${{{key}:-}}\""));
    }

    let extra_mounts = if credential_mounts.is_empty() {
        String::new()
    } else {
        format!("\n{}", credential_mounts.join("\n"))
    };
    let extra_env = if credential_env.is_empty() {
        String::new()
    } else {
        format!("\n{}", credential_env.join("\n"))
    };

    Ok(format!(
        "services:\n  lfd:\n    image: {image}\n    ports:\n      - \"${{LFD_PORT:-2486}}:2486\"\n    environment:\n      LFD_HTTP_ADDR: \"0.0.0.0:2486\"\n      LFD_STORAGE: postgres\n      LFD_DATABASE_URL: \"{database_url}\"\n      LFD_EXECUTOR_TYPE: docker\n      LFD_EXECUTOR_IMAGE: \"{image}\"\n      LFD_AUTH_PROVIDER: \"${{LFD_AUTH_PROVIDER:-local}}\"\n      LFD_AUTH_TOKEN: \"${{LFD_AUTH_TOKEN:-}}\"{extra_env}\n    env_file:\n      - path: .env\n        required: false\n    volumes:\n      - /var/run/docker.sock:/var/run/docker.sock\n      - lfd-data:/root/.lf{extra_mounts}\n    depends_on:\n      postgres:\n        condition: service_healthy\n    healthcheck:\n      test: [\"CMD\", \"curl\", \"-sf\", \"http://localhost:2486/health\"]\n      interval: 10s\n      timeout: 5s\n      retries: 3\n      start_period: 30s\n    restart: unless-stopped\n\n  postgres:\n    image: {postgres_image}\n    environment:\n      POSTGRES_USER: lfd\n      POSTGRES_PASSWORD: lfd\n      POSTGRES_DB: lfd\n    volumes:\n      - pgdata:/var/lib/postgresql/data\n    healthcheck:\n      test: [\"CMD-SHELL\", \"pg_isready -U lfd\"]\n      interval: 5s\n      timeout: 5s\n      retries: 5\n    restart: unless-stopped\n\nvolumes:\n  pgdata:\n  lfd-data:\n",
        image = config.executor.image,
        database_url = POSTGRES_DATABASE_URL,
        postgres_image = POSTGRES_IMAGE,
        extra_env = extra_env,
        extra_mounts = extra_mounts,
    ))
}

fn resolve_credential_mount(mount: &CredentialMount) -> Result<(PathBuf, String)> {
    let relative = resolve_credential_mount_name(mount.name())?;
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("home directory not available"))?;
    let host_path = home.join(relative);
    let container_path = format!("/root/{relative}");
    Ok((host_path, container_path))
}

fn resolve_credential_mount_name(name: &str) -> Result<&'static str> {
    let normalized = name.trim().to_ascii_lowercase();
    let key = normalized.strip_prefix("~/").unwrap_or(&normalized);
    match key {
        "claude" | ".claude" => Ok(".claude"),
        "codex" | ".codex" => Ok(".codex"),
        "gemini" | ".config/gemini" => Ok(".config/gemini"),
        "gitconfig" | ".gitconfig" => Ok(".gitconfig"),
        "ssh" | ".ssh" => Ok(".ssh"),
        "gnupg" | ".gnupg" => Ok(".gnupg"),
        _ => bail!(
            "unknown credential mount '{name}'. allowed mounts: claude, codex, gemini, gitconfig, ssh, gnupg"
        ),
    }
}

#[derive(Debug, Deserialize)]
struct ComposePsRow {
    #[serde(rename = "Service")]
    service: String,
    #[serde(rename = "State")]
    state: Option<String>,
    #[serde(rename = "Health")]
    health: Option<String>,
}

impl From<ComposePsRow> for ComposeServiceStatus {
    fn from(value: ComposePsRow) -> Self {
        Self {
            service: value.service,
            state: value.state.unwrap_or_else(|| "unknown".to_string()),
            health: value.health,
        }
    }
}
