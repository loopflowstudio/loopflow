import os
import stat
import subprocess
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]


def test_nightly_packages_workflow_builds_and_smokes_without_deploying():
    workflow = yaml.load(
        (ROOT / ".github/workflows/nightly-packages.yml").read_text(), Loader=yaml.BaseLoader
    )

    assert workflow["name"] == "Packages (nightly)"
    assert workflow["on"]["schedule"] == [{"cron": "0 9 * * *"}]
    assert workflow["jobs"]["regression"] == {"uses": "./.github/workflows/regression-daily.yml"}

    native = workflow["jobs"]["native-packages"]
    assert native["needs"] == "regression"
    targets = {entry["target"] for entry in native["strategy"]["matrix"]["include"]}
    assert targets == {
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
    }

    commands = "\n".join(step.get("run", "") for step in native["steps"])
    assert "cargo build --release" in commands
    assert "tar czf" in commands
    assert "package-smoke/lf --version" in commands
    assert "package-smoke/lfd --version" in commands

    forbidden = [
        "gh release",
        "R2_",
        "aws s3",
        "cloudflarestorage.com",
        "git push",
        "gh workflow run",
    ]
    assert not any(term in commands for term in forbidden)


def test_pull_local_bin_builds_and_installs_binaries(tmp_path: Path):
    repo = tmp_path / "repo"
    repo.mkdir()
    (repo / "Cargo.toml").write_text("[workspace]\nmembers = []\n")
    subprocess.run(["git", "init"], cwd=repo, check=True, stdout=subprocess.DEVNULL)

    fake_bin = tmp_path / "fake-bin"
    fake_bin.mkdir()
    cargo_log = tmp_path / "cargo.log"
    cargo = fake_bin / "cargo"
    cargo.write_text(
        f"""#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" > {cargo_log}
repo=''
while [[ $# -gt 0 ]]; do
    case "$1" in
        --manifest-path)
            repo="$(dirname "$2")"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done
mkdir -p "$repo/target/release"
printf '#!/usr/bin/env bash\necho lf fake\n' > "$repo/target/release/lf"
printf '#!/usr/bin/env bash\necho lfd fake\n' > "$repo/target/release/lfd"
chmod +x "$repo/target/release/lf" "$repo/target/release/lfd"
"""
    )
    cargo.chmod(cargo.stat().st_mode | stat.S_IXUSR)

    install_dir = tmp_path / "local-bin"
    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}:{env['PATH']}"

    result = subprocess.run(
        [
            str(ROOT / "scripts/pull-local-bin.sh"),
            "--repo",
            str(repo),
            "--install-dir",
            str(install_dir),
            "--no-pull",
        ],
        check=True,
        text=True,
        capture_output=True,
        env=env,
    )

    assert "installed:" in result.stdout
    assert "lf fake" in result.stdout
    assert "lfd fake" in result.stdout
    assert (install_dir / "lf").read_text().startswith("#!/usr/bin/env bash")
    assert (install_dir / "lfd").read_text().startswith("#!/usr/bin/env bash")
    assert "build --release -p loopflow --bin lf --bin lfd" in cargo_log.read_text()


def test_pull_local_bin_fetches_then_merges_without_user_pull_config():
    script = (ROOT / "scripts/pull-local-bin.sh").read_text()

    assert 'git -C "$repo" fetch origin "$default_branch"' in script
    assert 'git -C "$repo" merge --ff-only "origin/$default_branch"' in script
    assert 'git -C "$repo" fetch' in script
    assert 'git -C "$repo" merge --ff-only "@{upstream}"' in script
    assert 'git -C "$repo" pull --ff-only' not in script


def test_self_hosted_server_primitives_are_documented_and_runnable():
    help_result = subprocess.run(
        [str(ROOT / "deploy/loopflow-server.sh"), "--help"],
        check=True,
        text=True,
        capture_output=True,
    )
    bootstrap_help = subprocess.run(
        [str(ROOT / "deploy/bootstrap-cron-host.sh"), "--help"],
        check=True,
        text=True,
        capture_output=True,
    )
    private_client_help = subprocess.run(
        [str(ROOT / "deploy/setup-private-client.sh"), "--help"],
        check=True,
        text=True,
        capture_output=True,
    )
    native_host_help = subprocess.run(
        [str(ROOT / "deploy/native-lfd-host.sh"), "--help"],
        check=True,
        text=True,
        capture_output=True,
    )

    assert "up|update|down|status|logs|health" in help_result.stderr
    assert "LOOPFLOW_SECRETS=auto|doppler|env" in help_result.stderr
    assert "LFD_AUTH_TOKEN=..." in help_result.stderr
    assert "bootstrap-cron-host.sh [--repo PATH]" in bootstrap_help.stderr
    assert "--host auto|linux|mac" in bootstrap_help.stderr
    assert "--no-wave" in bootstrap_help.stderr
    assert "setup-private-client.sh" in private_client_help.stderr
    assert "--token-file PATH" in private_client_help.stderr
    assert "--no-concerto" in private_client_help.stderr
    assert "native-lfd-host.sh" in native_host_help.stderr
    assert "install|install-update-agent|update|restart|status|logs|health|serve" in native_host_help.stderr
    assert "LFD_HTTP_ADDR=0.0.0.0:2486" in native_host_help.stderr
    assert "LFD_AUTH_TOKEN_FILE=~/.lf/lfd-token" in native_host_help.stderr

    readme = (ROOT / "deploy/README.md").read_text()
    assert "doppler secrets set LFD_AUTH_TOKEN" in readme
    assert "deploy/COSTS.md" in readme
    assert "$100/month" in readme
    assert "openssl rand -hex 32" in readme
    assert "com.loopflow.lfd.update" in readme
    assert "leave `LF_TLS_MODE` empty" in readme
    assert "Mac mini + Tailscale" in readme
    assert "bootstrap-cron-host.sh" in readme
    assert "one-command host setup" in readme
    assert "/etc/loopflow-server.env" in readme
    assert "loopflow-server-update.timer" in readme
    assert "lfq create root /opt/loopflow" in readme
    assert "deploy/PRIVATE_HOST.md" in readme
    assert "claude,codex,ssh" in readme

    private_readme = (ROOT / "deploy/PRIVATE_HOST.md").read_text()
    assert "<tailscale-host-or-ip>" in private_readme
    assert "http://<tailscale-host-or-ip>:2486" in private_readme
    assert "deploy/setup-private-client.sh --host" in private_readme
    assert "deploy/native-lfd-host.sh install" in private_readme
    assert "deploy/native-lfd-host.sh install-update-agent" in private_readme
    assert "com.loopflow.lfd.update" in private_readme
    assert "Docker Desktop is not required for the native service" in private_readme
    assert "ssh-remote+$LFD_SSH_USER@$LFD_HOST" in private_readme
    assert "LFD_EXECUTOR_CREDENTIALS_MOUNTS=claude,codex,ssh" in private_readme

    compose = (ROOT / "docker/docker-compose.yml").read_text()
    assert "LFD_AUTH_MODE" not in compose
    assert 'LFD_AUTH_TOKEN: "${LFD_AUTH_TOKEN:?set LFD_AUTH_TOKEN}"' in compose

    prod_compose = (ROOT / "deploy/docker-compose.prod.yml").read_text()
    assert "${CADDYFILE:-../deploy/Caddyfile}" in prod_compose

    caddyfile = (ROOT / "deploy/Caddyfile").read_text()
    assert "tls " not in caddyfile

    internal_caddyfile = (ROOT / "deploy/Caddyfile.internal").read_text()
    assert "tls internal" in internal_caddyfile

    script = (ROOT / "deploy/loopflow-server.sh").read_text()
    assert 'docker compose -p "$project_name"' in script
    assert "LFD_AUTH_TOKEN is required for self-hosted remote execution" in script
    assert '(cd "$repo" && doppler run -- "$@")' in script
    assert 'export CADDYFILE="$repo/deploy/Caddyfile.internal"' in script

    bootstrap = (ROOT / "deploy/bootstrap-cron-host.sh").read_text()
    assert "is required for cron-host bootstrap" in bootstrap
    assert "systemctl enable --now loopflow-server.service" in bootstrap
    assert "launchctl bootstrap" in bootstrap
    assert 'data["ProgramArguments"] = [server_script, "up"]' in bootstrap
    assert '"PATH"' in bootstrap
    assert '"DOCKER_CONFIG"' in bootstrap
    assert '"DOCKER_HOST"' in bootstrap
    assert "lfq create root" in bootstrap
    assert "install -m 0600" in bootstrap

    native_host = (ROOT / "deploy/native-lfd-host.sh").read_text()
    assert "scripts/pull-local-bin.sh" in native_host
    assert '"Label": "com.loopflow.lfd"' in native_host
    assert '"serve"' in native_host
    assert 'exec "$lfd_bin" serve' in native_host
    assert "install --force" not in native_host
    assert "launchctl kickstart -k" in native_host
    assert "StartCalendarInterval" in native_host
    assert '"Hour": 4' in native_host
    assert '"Minute": 30' in native_host
    assert '"LFD_AUTH_TOKEN_FILE": token_file' in native_host
    assert 'printf \'%s\\n\' "$LFD_AUTH_TOKEN" > "$token_file"' in native_host
    assert 'chmod 0600 "$token_file"' in native_host
    assert "LFD_AUTH_TOKEN or readable LFD_AUTH_TOKEN_FILE is required for native private lfd host management" in native_host

    private_client = (ROOT / "deploy/setup-private-client.sh").read_text()
    assert "Host is required. Pass --host or set LFD_HOST." in private_client
    assert "LFD_URL" in private_client
    assert "loopflow.connection.token" in private_client
    assert "concerto.connectionSettings.v2" in private_client
    assert "alias lfdhost='ssh" in private_client
    assert "lfq list >/dev/null" in private_client
    assert 'curl -fsS -H "Authorization: Bearer $LFD_TOKEN" "$LFD_URL/status"' in private_client
    assert "lfq status" not in private_client

    service = (ROOT / "deploy/systemd/loopflow-server.service").read_text()
    assert "ExecStart=/opt/loopflow/deploy/loopflow-server.sh up" in service

    timer = (ROOT / "deploy/systemd/loopflow-server-update.timer").read_text()
    assert "OnCalendar=*-*-* 04:15:00" in timer

    plist = (ROOT / "deploy/launchd/loopflow.server.plist").read_text()
    assert "loopflow.server" in plist
    assert "studio.loopflow.server" not in plist
    assert "loopflow-server.sh" in plist

    cost_docs = (ROOT / "deploy/COSTS.md").read_text()
    budget_config = yaml.safe_load((ROOT / "deploy/budget.json").read_text())
    assert budget_config["monthly_budget_usd"] == "100.00"
    assert budget_config["source_of_truth"] == "mercury_company_card"
    assert "scripts/check_monthly_spend.py" in cost_docs
    assert "company card" in cost_docs
    assert "AWS" in cost_docs
    assert "Fly.io" in cost_docs
    assert "Claude / Anthropic" in cost_docs
    assert "Codex / OpenAI" in cost_docs


def test_aws_self_hosted_topology_keeps_secrets_out_of_terraform():
    terraform_dir = ROOT / "deploy/terraform/aws"
    files = [
        "versions.tf",
        "variables.tf",
        "main.tf",
        "outputs.tf",
        "user-data.sh.tftpl",
        "README.md",
    ]
    for name in files:
        assert (terraform_dir / name).exists()

    combined = "\n".join(path.read_text() for path in terraform_dir.iterdir() if path.is_file())
    assert "DOPPLER_TOKEN=dp.st.x" in combined
    assert 'variable "doppler' not in combined.lower()
    assert "sensitive = true" not in combined.lower()
    assert "Secrets intentionally stay out of Terraform state" in combined
    assert "loopflow-server.service" in combined


def test_release_schedule_contract_covers_loopflow_and_cadenza():
    schedule = (ROOT / "release/SCHEDULE.md").read_text()
    assert "Loopflow and Cadenza use the same release rhythm" in schedule
    assert "0 9 * * *" in schedule
    assert "0 12 * * 0" in schedule
    assert (
        "Weekly publishing never runs unless nightly-style package verification passed" in schedule
    )

    weekly = yaml.load(
        (ROOT / ".github/workflows/weekly-release.yml").read_text(), Loader=yaml.BaseLoader
    )
    assert weekly["jobs"]["package-test"]["uses"] == "./.github/workflows/nightly-packages.yml"
    assert weekly["jobs"]["release"]["needs"] == ["tag-check", "package-test"]
    assert weekly["jobs"]["release"]["permissions"]["actions"] == "write"
