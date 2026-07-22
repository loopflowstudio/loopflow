import os
import stat
import subprocess
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]


def test_nightly_packages_workflow_builds_and_smokes_without_deploying():
    workflow = yaml.load(
        (ROOT / ".github/workflows/nightly-packages.yml").read_text(), Loader=yaml.BaseLoader
    )

    assert workflow["name"] == "Packages (nightly)"
    assert workflow["on"]["schedule"] == [{"cron": "0 9 * * *"}]

    native = workflow["jobs"]["native-packages"]
    assert "needs" not in native
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
    assert "package-smoke/lf --help" in commands
    assert "package-smoke/lf --list" in commands

    forbidden = [
        "gh release",
        "R2_",
        "aws s3",
        "cloudflarestorage.com",
        "git push",
        "gh workflow run",
    ]
    assert not any(term in commands for term in forbidden)

    acceptance = workflow["jobs"]["release-acceptance"]
    acceptance_commands = "\n".join(step.get("run", "") for step in acceptance["steps"])
    assert "release_acceptance_recovers_from_a_revoked_selected_account" in acceptance_commands


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
    subprocess.run(["git", "config", "gc.auto", "0"], cwd=repo, check=True)
    subprocess.run(["git", "config", "maintenance.auto", "false"], cwd=repo, check=True)
    subprocess.run(["git", "add", "."], cwd=repo, check=True)
    subprocess.run(
        ["git", "commit", "-m", "release: v1.2.3"],
        cwd=repo,
        check=True,
        stdout=subprocess.DEVNULL,
    )
    subprocess.run(["git", "tag", "v1.2.3"], cwd=repo, check=True)

    subjects = [
        "deploy: simplify release packaging",
        "auth: persist provider token",
        "loopflow: improve portfolio status",
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
    assert "## Release infrastructure" in notes
    assert "## Authentication" in notes
    assert "## Loopflow and user surfaces" in notes
    assert "## Agent workflows and developer tooling" in notes
    assert "## Dependency updates" in notes
    assert "serde 1.0.0 → 1.0.1" in notes
    for subject in subjects:
        if not subject.startswith("build(deps)"):
            assert subject in notes


def test_pull_local_bin_builds_and_installs_lf(tmp_path: Path):
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
cat > "$repo/target/release/lf" <<'LF'
#!/usr/bin/env bash
if [ "$1" = "install" ] && [ "$2" = "promote" ]; then
    shift 2
    while [ "$#" -gt 0 ]; do
        if [ "$1" = "--cli-target" ]; then mkdir -p "$(dirname "$2")"; ln -sf "$0" "$2"; fi
        shift
    done
    exit 0
fi
echo lf fake
LF
chmod +x "$repo/target/release/lf"
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
    assert (install_dir / "lf").read_text().startswith("#!/usr/bin/env bash")
    assert "build --release -p loopflow --bin lf" in cargo_log.read_text()
    assert "scripts/install.py refresh" in (ROOT / "scripts/pull-local-bin.sh").read_text()


def test_release_build_workflow_is_credential_free():
    release = yaml.load(
        (ROOT / ".github/workflows/release.yml").read_text(),
        Loader=yaml.BaseLoader,
    )

    assert release["jobs"] == {"packages": {"uses": "./.github/workflows/nightly-packages.yml"}}
    assert "schedule" not in release["on"]

    workflow_text = (ROOT / ".github/workflows/release.yml").read_text()
    forbidden = (
        "secrets.",
        "doppler",
        "flyctl",
        "cargo publish",
        "gh release",
        "R2_",
        "NOTARY_",
    )
    assert not any(term in workflow_text for term in forbidden)
    assert not (ROOT / ".github/workflows/weekly-release.yml").exists()
    assert not (ROOT / ".github/workflows/website-deploy.yml").exists()


def test_auto_tag_dispatch_matches_the_input_free_release_contract():
    release = yaml.load(
        (ROOT / ".github/workflows/release.yml").read_text(),
        Loader=yaml.BaseLoader,
    )
    assert not release["on"]["workflow_dispatch"]

    auto_tag = (ROOT / ".github/workflows/auto-tag.yml").read_text()
    assert 'gh workflow run release.yml --ref "$version"' in auto_tag
    assert "-f tag=" not in auto_tag


def test_host_publisher_owns_credentialed_release_steps():
    publisher = (ROOT / "scripts/publish_release.py").read_text()

    for proof in (
        "check_release_host()",
        "_stage_github_release(artifacts)",
        "_publish_crate()",
        '"versioned_dmg_uploaded"',
        '"scripts/deploy_website.py"',
        '"latest_dmg_uploaded"',
        '"--finalize"',
    ):
        assert proof in publisher

    assert publisher.index('"website_deployed"') < publisher.index('"--finalize"')
    assert publisher.index('"latest_dmg_uploaded"') < publisher.index('"--finalize"')


def test_infrastructure_cron_runs_the_host_release_after_telemetry():
    goal = (ROOT / "wave/infrastructure/GOAL.md").read_text()
    frontmatter = yaml.safe_load(goal.split("---", 2)[1])
    assert frontmatter["crons"] == [
        {"flow": "telemetry-daily", "schedule": "0 0 9 * * *"},
        {"flow": "release-run", "schedule": "0 0 10 * * *"},
    ]

    config = yaml.safe_load((ROOT / ".lf/config.yaml").read_text())
    assert config["release"]["targets"]["default"]["publisher"] == [
        "loopflow-release-publisher",
        "uv",
        "run",
        "python",
        "{repo}/scripts/publish_release.py",
    ]

    bootstrap = (ROOT / "scripts/bootstrap-cron-host.sh").read_text()
    assert "--remote-native" not in bootstrap
    assert 'local_home="$(lf home id)"' in bootstrap
    assert 'placed_home="$(lf status "$wave" --json' in bootstrap
    assert "--git-common-dir" in bootstrap
    assert 'lf cron preflight --wave "$wave"' in bootstrap
    assert '"${minimal_env[@]}" lf cron sync --wave "$wave"' in bootstrap
    assert 'lf cron list --wave "$wave" --json' in bootstrap
    assert 'lf cron trigger' in bootstrap
    assert '--flow telemetry-daily --wait --timeout 15m' in bootstrap
    assert '--flow release-run --wait --timeout 3h' in bootstrap
    assert 'lf cron history --wave "$wave" --days 35' in bootstrap
    assert "env -i" in bootstrap
    assert "DOPPLER_TOKEN" not in bootstrap
    assert "loopflow-release-publisher" in bootstrap
    assert "not local Home %s" not in bootstrap
    assert "printf '%s\\n' \"$local_home\"" not in bootstrap
    assert "bootstrap complete: %s owns" not in bootstrap
    assert "wrong Home in installed cron: {entry!r}" not in bootstrap
    assert bootstrap.index('lf cron preflight --wave "$wave"') < bootstrap.index(
        "scripts/publish_release.py check"
    )
    assert bootstrap.index("scripts/publish_release.py check") < bootstrap.index(
        'lf cron sync --wave "$wave"'
    )

    public_contract = "\n".join(
        [
            (ROOT / ".lf/config.yaml").read_text(),
            bootstrap,
            (ROOT / "release/CRON_HOST.md").read_text(),
        ]
    )
    for private_selector in (
        "doppler",
        "--project",
        "--config",
        "DOPPLER_PROJECT",
        "DOPPLER_CONFIG",
    ):
        assert private_selector not in public_contract


def test_release_installer_uses_the_promotion_boundary_to_activate_the_binary():
    installer = (ROOT / "release/install.sh").read_text()

    assert '"$src" install promote \\' in installer
    assert '--cli-target "$dst"' in installer
    assert '--daemon-source "$daemon_src"' in installer
    assert '--daemon-target "$daemon_dst"' in installer
    assert 'mv -f "$tmp" "$dst"' not in installer


def test_loopflow_ui_gate_keeps_mac_test_runners_signed():
    ci = (ROOT / ".github/workflows/ci.yml").read_text()
    test_script = (ROOT / "scripts/test.py").read_text()
    screenshot_script = (ROOT / "scripts/generate_screenshots.py").read_text()

    for text in (ci, test_script, screenshot_script):
        assert "CODE_SIGNING_ALLOWED=NO" not in text
        assert "CODE_SIGNING_REQUIRED=NO" not in text

    assert "CODE_SIGN_IDENTITY=-" in ci
    assert '"CODE_SIGN_IDENTITY=-"' in test_script
    assert '"CODE_SIGN_IDENTITY=-"' in screenshot_script
    assert "-disableAutomaticPackageResolution" in ci


def test_rust_ci_materializes_drafts_before_running_tests():
    ci = yaml.load((ROOT / ".github/workflows/ci.yml").read_text(), Loader=yaml.BaseLoader)
    steps = ci["jobs"]["rust-test"]["steps"]
    names = [step.get("name") for step in steps]

    materialize = names.index("Materialize draft migrations for tests")
    test = names.index("Run Rust tests")
    command = steps[materialize]["run"]

    assert materialize < test
    assert "scripts/canonicalize_migrations.py" in command
    assert "--materialize-for-tests" in command
    assert '["workspace"]["package"]["version"]' in command


def test_changed_aware_runner_includes_ci_static_checks():
    result = subprocess.run(
        [
            sys.executable,
            str(ROOT / "scripts/test.py"),
            "--list",
            "--rust",
            "--swift",
            "--base",
            "HEAD",
        ],
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    )

    assert "cargo fmt --all -- --check" in result.stdout
    assert "cargo clippy --all-targets --jobs 4 -- -D warnings" in result.stdout
    assert "scripts/materialize_rust_tests.py -- cargo" in result.stdout
    assert "cargo nextest run --all" in result.stdout or "cargo test --all" in result.stdout
    assert "uv run python scripts/check_swift_multiplatform_boundaries.py" in result.stdout

    full = subprocess.run(
        [
            sys.executable,
            str(ROOT / "scripts/test.py"),
            "--list",
            "--all",
            "--base",
            "HEAD",
        ],
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    )
    assert "$ uv run pytest python/tests/" in full.stdout
    assert "python/tests/test_release_automation.py" not in full.stdout.split("Plan:", 1)[1]
