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
    mini_client_help = subprocess.run(
        [str(ROOT / "deploy/setup-mini-client.sh"), "--help"],
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
    assert "setup-mini-client.sh" in mini_client_help.stderr
    assert "--token-file PATH" in mini_client_help.stderr
    assert "--no-concerto" in mini_client_help.stderr

    readme = (ROOT / "deploy/README.md").read_text()
    assert "doppler secrets set LFD_AUTH_TOKEN" in readme
    assert "openssl rand -hex 32" in readme
    assert "leave `LF_TLS_MODE` empty" in readme
    assert "Mac mini + Tailscale" in readme
    assert "bootstrap-cron-host.sh" in readme
    assert "one-command host setup" in readme
    assert "/etc/loopflow-server.env" in readme
    assert "loopflow-server-update.timer" in readme
    assert "lfq create root /opt/loopflow" in readme
    assert "deploy/MAC_MINI.md" in readme
    assert "claude,codex,ssh" in readme

    mini_readme = (ROOT / "deploy/MAC_MINI.md").read_text()
    assert "100.96.227.95" in mini_readme
    assert "http://100.96.227.95:2486" in mini_readme
    assert "deploy/setup-mini-client.sh --token" in mini_readme
    assert "cursor --remote ssh-remote+jack@100.96.227.95" in mini_readme
    assert "code --remote ssh-remote+jack@100.96.227.95" in mini_readme
    assert "LFD_EXECUTOR_CREDENTIALS_MOUNTS=claude,codex,ssh" in mini_readme

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
    assert "lfq create root" in bootstrap
    assert "install -m 0600" in bootstrap

    mini_client = (ROOT / "deploy/setup-mini-client.sh").read_text()
    assert "100.96.227.95" in mini_client
    assert "LFD_URL" in mini_client
    assert "loopflow.connection.token" in mini_client
    assert "concerto.connectionSettings.v2" in mini_client
    assert "alias mini='ssh" in mini_client

    service = (ROOT / "deploy/systemd/loopflow-server.service").read_text()
    assert "ExecStart=/opt/loopflow/deploy/loopflow-server.sh up" in service

    timer = (ROOT / "deploy/systemd/loopflow-server-update.timer").read_text()
    assert "OnCalendar=*-*-* 04:15:00" in timer

    plist = (ROOT / "deploy/launchd/loopflow.server.plist").read_text()
    assert "loopflow.server" in plist
    assert "studio.loopflow.server" not in plist
    assert "loopflow-server.sh" in plist


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
