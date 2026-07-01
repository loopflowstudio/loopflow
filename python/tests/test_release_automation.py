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


def test_token_compress_step_is_documented_as_preserving_information():
    step = (ROOT / "rust/loopflow/src/engine/builtins/ops/step/token-compress.md").read_text()
    docs = (ROOT / "docs/index.md").read_text()
    readme = (ROOT / "README.md").read_text()

    assert "Compress text into a target token budget" in step
    assert "Compression is not truncation" in step
    assert "Do not summarize a list by taking the first items" in step
    assert "Omitted" in step
    assert "lf token-compress" in docs
    assert "Do not take the first N commits" in docs
    assert "| `token-compress` |" in readme


def test_bump_patch_version_groups_long_commit_lists_without_dropping_commits(tmp_path: Path):
    repo = tmp_path / "repo"
    repo.mkdir()
    (repo / "Cargo.toml").write_text('[package]\nname = "loopflow"\nversion = "1.2.3"\n')
    (repo / "pyproject.toml").write_text('[project]\nname = "loopflow"\nversion = "1.2.3"\n')
    (repo / "RELEASE_NOTES.md").write_text("# v1.2.3\n\nPrevious release.\n")
    scripts = repo / "scripts"
    scripts.mkdir()
    script = scripts / "bump_patch_version.sh"
    script.write_text((ROOT / "scripts/bump_patch_version.sh").read_text())
    script.chmod(script.stat().st_mode | stat.S_IXUSR)

    subprocess.run(["git", "init"], cwd=repo, check=True, stdout=subprocess.DEVNULL)
    subprocess.run(["git", "config", "user.name", "Test"], cwd=repo, check=True)
    subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=repo, check=True)
    subprocess.run(["git", "add", "."], cwd=repo, check=True)
    subprocess.run(
        ["git", "commit", "-m", "release: v1.2.3"],
        cwd=repo,
        check=True,
        stdout=subprocess.DEVNULL,
    )
    subprocess.run(["git", "tag", "v1.2.3"], cwd=repo, check=True)

    subjects = [
        "deploy: add native host updater",
        "lfd: persist remote token file",
        "concerto: improve portfolio status",
        "lf: hand off steps through vendor skills",
        "build(deps): bump serde from 1.0.0 to 1.0.1 (#1)",
    ]
    subjects.extend(f"workflow: generated change {index}" for index in range(55))

    for index, subject in enumerate(subjects):
        (repo / "change.txt").write_text(f"{index}\n")
        subprocess.run(["git", "add", "change.txt"], cwd=repo, check=True)
        subprocess.run(
            ["git", "commit", "-m", subject],
            cwd=repo,
            check=True,
            stdout=subprocess.DEVNULL,
        )

    result = subprocess.run(
        [str(script), "v1.2.3", str(len(subjects))],
        cwd=repo,
        check=True,
        text=True,
        capture_output=True,
    )

    notes = (repo / "RELEASE_NOTES.md").read_text()
    assert "next=1.2.4" in result.stdout
    assert "# v1.2.4" in notes
    assert f"Weekly auto-release with {len(subjects)} commits since `v1.2.3`." in notes
    assert "Commits are grouped by theme instead of truncated" in notes
    assert "## Release and self-hosting infrastructure" in notes
    assert "## Authentication and remote execution" in notes
    assert "## Concerto and user surfaces" in notes
    assert "## Agent workflows and developer tooling" in notes
    assert "## Dependency updates" in notes
    assert "serde 1.0.0 → 1.0.1" in notes
    for subject in subjects:
        if subject.startswith("build(deps)"):
            continue
        assert subject in notes


def test_pull_local_bin_builds_and_installs_binaries(tmp_path: Path):
    repo = tmp_path / "repo"
    repo.mkdir()
    (repo / "Cargo.toml").write_text("[workspace]\nmembers = []\n")
    scripts = repo / "scripts"
    scripts.mkdir()
    (scripts / "install.py").write_text((ROOT / "scripts/install.py").read_text())
    (scripts / "bundle_version.py").write_text((ROOT / "scripts/bundle_version.py").read_text())
    subprocess.run(["git", "init"], cwd=repo, check=True, stdout=subprocess.DEVNULL)

    fake_bin = tmp_path / "fake-bin"
    fake_bin.mkdir()
    cargo_log = tmp_path / "cargo.log"
    cargo = fake_bin / "cargo"
    cargo.write_text(
        f"""#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" > {cargo_log}
repo="$PWD"
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
    assert "scripts/install.py refresh" in (ROOT / "scripts/pull-local-bin.sh").read_text()


def test_install_refresh_fetches_then_merges_without_user_pull_config():
    install = (ROOT / "scripts/install.py").read_text()
    wrapper = (ROOT / "scripts/pull-local-bin.sh").read_text()

    assert '["git", "fetch", "origin", default_branch]' in install
    assert '["git", "merge", "--ff-only", f"origin/{default_branch}"]' in install
    assert '["git", "fetch"]' in install
    assert '["git", "merge", "--ff-only", "@{upstream}"]' in install
    assert "pull --ff-only" not in install
    assert "scripts/install.py" in wrapper
    assert "cargo build" not in wrapper


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
    tailscale_host_help = subprocess.run(
        [str(ROOT / "deploy/tailscale-lfd-host.sh"), "--help"],
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
    assert (
        "install|install-update-agent|update|restart|status|logs|health|serve"
        in native_host_help.stderr
    )
    assert "LFD_HTTP_ADDR=0.0.0.0:2486" in native_host_help.stderr
    assert "LFD_AUTH_TOKEN_FILE=~/.lf/lfd-token" in native_host_help.stderr
    assert "tailscale-lfd-host.sh" in tailscale_host_help.stderr
    assert "serve-off" in tailscale_host_help.stderr
    assert "TS_HTTPS_PORT=443" in tailscale_host_help.stderr
    assert "serve              Internal launchd entrypoint" not in tailscale_host_help.stderr

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
    assert "deploy/tailscale-lfd-host.sh install" in readme
    assert "https://<host>.<tailnet>.ts.net" in readme

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
    assert "scripts/install.py" in native_host
    assert "scripts/pull-local-bin.sh" not in native_host
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
    assert (
        "LFD_AUTH_TOKEN or readable LFD_AUTH_TOKEN_FILE is required "
        "for native private lfd host management"
    ) in native_host

    tailscale_host = (ROOT / "deploy/tailscale-lfd-host.sh").read_text()
    assert 'export LFD_HTTP_ADDR="127.0.0.1:${port}"' in tailscale_host
    assert '"$ts" serve --bg --https="$https_port" "http://127.0.0.1:${port}"' in tailscale_host
    assert '"$ts" serve --https="$https_port" off' in tailscale_host
    assert 'install | install-update-agent | update | restart | status | logs | health | serve-off)' in tailscale_host
    assert 'install | install-update-agent | update | restart | status | logs | health | serve | serve-off)' not in tailscale_host

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
    assert "lf op release run patch" in schedule
    assert "release/unreleased/DECISIONS.md" in schedule

    weekly = yaml.load(
        (ROOT / ".github/workflows/weekly-release.yml").read_text(), Loader=yaml.BaseLoader
    )
    assert weekly["jobs"]["package-test"]["uses"] == "./.github/workflows/nightly-packages.yml"
    assert weekly["jobs"]["release"]["needs"] == ["tag-check", "package-test"]
    assert weekly["jobs"]["release"]["permissions"]["actions"] == "write"
    assert weekly["jobs"]["release"]["permissions"]["pull-requests"] == "write"

    commands = "\n".join(
        step.get("run", "") for step in weekly["jobs"]["release"]["steps"]
    )
    release_steps = weekly["jobs"]["release"]["steps"]
    assert any(
        step.get("uses") == "dopplerhq/secrets-fetch-action@v2.0.0"
        for step in release_steps
    )
    assert "cargo build --release -p loopflow --bin lf" in commands
    assert "lf op release run patch" in commands
    assert "bump_patch_version.sh" not in commands
    assert "git push origin HEAD:main" not in commands
    assert "gh workflow run release.yml" not in commands


def test_release_dmg_build_has_timeouts_and_unbuffered_logs():
    release = yaml.load(
        (ROOT / ".github/workflows/release.yml").read_text(),
        Loader=yaml.BaseLoader,
    )
    build_dmg = release["jobs"]["build-dmg"]
    steps = build_dmg["steps"]
    commands = "\n".join(step.get("run", "") for step in steps)
    script = (ROOT / "scripts/release-concerto.py").read_text()

    assert build_dmg["timeout-minutes"] == "45"
    assert "python3 -u scripts/release-concerto.py" in commands
    assert any(
        step.get("name") == "Build, sign, and notarize DMG"
        and step.get("timeout-minutes") == "35"
        for step in steps
    )
    assert any(
        step.get("name") == "Upload DMG to R2" and step.get("timeout-minutes") == "5"
        for step in steps
    )
    assert "timeout=30 * 60" in script
    assert "notarytool" in script
    assert "Timed out after" in script
    assert "flush=True" in script
