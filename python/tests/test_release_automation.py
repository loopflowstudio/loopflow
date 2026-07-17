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

    forbidden = [
        "gh release",
        "R2_",
        "aws s3",
        "cloudflarestorage.com",
        "git push",
        "gh workflow run",
    ]
    assert not any(term in commands for term in forbidden)


def test_token_compress_skill_is_documented_as_preserving_information():
    step = (ROOT / "rust/loopflow/src/engine/builtins/ops/skill/token-compress.md").read_text()
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


def test_spend_guardrails_remain_independent_of_host_deployment():
    cost_docs = (ROOT / "deploy/COSTS.md").read_text()
    budget_config = yaml.safe_load((ROOT / "deploy/budget.json").read_text())
    assert budget_config["monthly_budget_usd"] == "100.00"
    assert budget_config["source_of_truth"] == "mercury_company_card"
    assert "scripts/check_monthly_spend.py" in cost_docs
    assert "company card" in cost_docs


def test_release_schedule_contract_covers_loopflow_and_cadenza():
    schedule = (ROOT / "release/SCHEDULE.md").read_text()
    assert "Loopflow and Cadenza use the same release rhythm" in schedule
    assert "0 9 * * *" in schedule
    assert "0 12 * * 0" in schedule
    assert (
        "Weekly publishing never runs unless nightly-style package verification passed" in schedule
    )
    assert "lf release run patch" in schedule
    assert "release/unreleased/DECISIONS.md" in schedule

    weekly = yaml.load(
        (ROOT / ".github/workflows/weekly-release.yml").read_text(), Loader=yaml.BaseLoader
    )
    assert weekly["jobs"]["package-test"]["uses"] == "./.github/workflows/nightly-packages.yml"
    assert weekly["jobs"]["release"]["needs"] == ["tag-check", "package-test"]
    assert weekly["jobs"]["release"]["permissions"]["actions"] == "write"
    assert weekly["jobs"]["release"]["permissions"]["pull-requests"] == "write"

    commands = "\n".join(step.get("run", "") for step in weekly["jobs"]["release"]["steps"])
    release_steps = weekly["jobs"]["release"]["steps"]
    assert any(
        step.get("uses") == "dopplerhq/secrets-fetch-action@v2.0.0" for step in release_steps
    )
    assert "cargo build --release -p loopflow --bin lf" in commands
    assert "lf release run patch" in commands
    assert "bump_patch_version.sh" not in commands
    assert "git push origin HEAD:main" not in commands
    assert "gh workflow run release.yml" not in commands


def test_release_dmg_build_has_timeouts_and_unbuffered_logs():
    release = yaml.load(
        (ROOT / ".github/workflows/release.yml").read_text(),
        Loader=yaml.BaseLoader,
    )
    build_dmg = release["jobs"]["build-dmg"]
    native_commands = "\n".join(
        step.get("run", "") for step in release["jobs"]["build-native"]["steps"]
    )
    steps = build_dmg["steps"]
    commands = "\n".join(step.get("run", "") for step in steps)
    script = (ROOT / "scripts/release-loopflow.py").read_text()

    assert build_dmg["timeout-minutes"] == "45"
    assert "--bin lf" in native_commands
    assert "tar czf ../../../lf-${{ matrix.target }}.tar.gz lf" in native_commands
    assert "publish-pypi" not in release["jobs"]
    assert "python3 -u scripts/release-loopflow.py" in commands
    assert any(
        step.get("name") == "Build, sign, and notarize DMG" and step.get("timeout-minutes") == "35"
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
    assert "Skipping notarization" not in script
    assert "signing ad-hoc" not in script
    assert "refusing to build a user DMG" in script


def test_loopflow_ui_gate_keeps_mac_test_runners_signed():
    ci = (ROOT / ".github/workflows/ci.yml").read_text()
    test_script = (ROOT / "scripts/test.py").read_text()
    screenshot_script = (ROOT / "scripts/generate_screenshots.py").read_text()
    docs = "\n".join(
        [
            (ROOT / "TESTING.md").read_text(),
            (ROOT / "swift/README.md").read_text(),
        ]
    )

    for text in (ci, test_script, screenshot_script, docs):
        assert "CODE_SIGNING_ALLOWED=NO" not in text
        assert "CODE_SIGNING_REQUIRED=NO" not in text

    assert "CODE_SIGN_IDENTITY=-" in ci
    assert '"CODE_SIGN_IDENTITY=-"' in test_script
    assert '"CODE_SIGN_IDENTITY=-"' in screenshot_script
    assert "-disableAutomaticPackageResolution" in ci
    assert "-disableAutomaticPackageResolution" in docs


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
    assert "cargo clippy --all-targets -- -D warnings" in result.stdout
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
