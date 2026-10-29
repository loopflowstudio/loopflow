import json
import os
import plistlib
import shutil
import stat
import subprocess
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]


def test_main_packages_build_every_green_main_commit_without_deploying():
    main = yaml.load(
        (ROOT / ".github/workflows/main-packages.yml").read_text(), Loader=yaml.BaseLoader
    )
    workflow = yaml.load(
        (ROOT / ".github/workflows/package-build.yml").read_text(), Loader=yaml.BaseLoader
    )

    assert main["name"] == "Packages (main)"
    assert main["on"]["workflow_run"]["workflows"] == ["CI"]
    assert main["on"]["workflow_run"]["branches"] == ["main"]
    packages = main["jobs"]["packages"]
    assert packages["uses"] == "./.github/workflows/package-build.yml"
    assert "workflow_run.conclusion == 'success'" in packages["if"]
    assert "head_sha" in packages["with"]["revision"]
    assert workflow["name"] == "Package build"
    assert set(workflow["on"]) == {"workflow_call"}
    assert "inputs.release_tag" in workflow["env"]["LOOPFLOW_BUILD_PROVENANCE"]
    assert "development" in workflow["env"]["LOOPFLOW_BUILD_PROVENANCE"]
    checkout = workflow["jobs"]["native-packages"]["steps"][0]
    assert checkout["with"]["fetch-depth"] == "0"
    assert workflow["env"]["LOOPFLOW_RELEASE_TAG"] == "${{ inputs.release_tag }}"

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
    assert "package-smoke/lf list" in commands
    assert 'test "$(package-smoke/lf --version)" = "lf ${expected}"' in commands

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


def test_release_build_workflow_is_credential_free():
    release = yaml.load(
        (ROOT / ".github/workflows/release.yml").read_text(),
        Loader=yaml.BaseLoader,
    )

    assert release["jobs"] == {
        "packages": {
            "uses": "./.github/workflows/package-build.yml",
            "with": {
                "release_tag": "${{ inputs.tag }}",
            },
        }
    }
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


def test_release_candidate_is_dispatched_explicitly_before_tagging():
    release = yaml.load(
        (ROOT / ".github/workflows/release.yml").read_text(),
        Loader=yaml.BaseLoader,
    )
    inputs = release["on"]["workflow_dispatch"]["inputs"]
    assert inputs["tag"]["required"] == "true"
    assert set(inputs) == {"tag"}
    assert "push" not in release["on"]
    assert not (ROOT / ".github/workflows/auto-tag.yml").exists()


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


def test_release_cron_keeps_its_machine_scope_and_runs_after_telemetry():
    config = yaml.safe_load((ROOT / ".lf/config.yaml").read_text())
    assert "crons" not in config
    assert yaml.safe_load((ROOT / ".lf/graphs/release-run.yaml").read_text()) == [
        {"cmd": "release run patch"}
    ]

    assert config["release"]["targets"]["default"]["publisher"] == [
        "loopflow-release-publisher",
        "uv",
        "run",
        "python",
        "{repo}/scripts/publish_release.py",
    ]

    bootstrap = (ROOT / "scripts/bootstrap-cron-host.sh").read_text()
    assert "--remote-native" not in bootstrap
    assert "--git-common-dir" in bootstrap
    assert "| crontab -" in bootstrap
    assert "DOPPLER_TOKEN" not in bootstrap
    assert "loopflow-release-publisher" in bootstrap
    assert bootstrap.index("scripts/publish_release.py check") < bootstrap.index("| crontab -")

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


LOCAL_MACHINE = "home_local"
HOST_TOOLS = ("uv", "gh", "cargo", "flyctl", "security", "swift", "xcrun", "claude")


def _executable(path: Path, body: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("#!/bin/sh\n" + body)
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


def _cron_host(
    tmp_path: Path, wave_machine: dict | None, system: str = "Linux"
) -> tuple[Path, dict]:
    """A main checkout and HOME with an installed lf gate and fake host tools."""
    root = tmp_path.resolve()
    repo = root / "loopflow"
    repo.mkdir()
    subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
    (repo / "scripts").mkdir()
    shutil.copy(ROOT / "scripts/bootstrap-cron-host.sh", repo / "scripts")

    home = root / "home"
    status = {"wave": {"name": "infrastructure", "machine": wave_machine}}
    (home / "wave-status.json").parent.mkdir(parents=True)
    (home / "wave-status.json").write_text(json.dumps(status))
    _executable(
        home / ".lf-machine/install/gates/1/lf",
        f"""case "$1 $2" in
  "machine id") echo {LOCAL_MACHINE} ;;
  "wave status") cat "$HOME/wave-status.json" ;;
  "account --json") echo '{{"accounts":[{{"scope":"managed","verification":"accepted"}}]}}' ;;
  "--wave infrastructure") echo "$@" >> "$HOME/ran" ;;
esac
""",
    )
    # A development build earlier on PATH, in a directory with a space.
    tools = root / "Loopflow Dev.app/bin"
    _executable(tools / "lf", "exit 1\n")
    for tool in HOST_TOOLS + ("loopflow-release-publisher",):
        _executable(tools / tool, "exit 0\n")
    _executable(tools / "uname", f"echo {system}\n")
    _executable(
        tools / "crontab",
        # Like crontab, read the whole table before replacing it.
        'if [ "$1" = -l ]; then cat "$HOME/crontab"; '
        'else table="$(cat)"; printf "%s\\n" "$table" > "$HOME/crontab"; fi\n',
    )
    # A gui domain: kickstart runs the job's program as launchd would and
    # print reports how it exited.
    _executable(
        tools / "launchctl",
        """echo "$*" >> "$HOME/launchctl"
case "$1" in
  bootstrap) cp "$3" "$HOME/loaded-${3##*/}" ;;
  kickstart)
    label="${3##*/}"
    program="$(grep -o '<string>[^<]*\\.sh</string>' "$HOME/loaded-$label.plist" | sed 's/<[^>]*>//g')"
    /bin/sh "$program"; echo "$?" > "$HOME/exit-$label" ;;
  print)
    label="${2##*/}"
    [ -e "$HOME/loaded-$label.plist" ] || exit 113
    if [ -e "$HOME/exit-$label" ]; then
      printf '\\tstate = not running\\n\\tlast exit code = %s\\n' "$(cat "$HOME/exit-$label")"
    else
      printf '\\tstate = not running\\n\\tlast exit code = (never exited)\\n'
    fi ;;
esac
""",
    )
    for tool in ("jq", "git"):
        os.symlink(shutil.which(tool), tools / tool)
    (home / "crontab").write_text(
        "*/5 * * * * unrelated-job\n"
        "0 9 * * * /bin/sh old.sh # loopflow:infrastructure:telemetry-daily\n"
    )

    env = {
        "HOME": str(home),
        # A host PATH longer than the 999 bytes cron reads of a command, in
        # components under NAME_MAX: glibc's execvp stops at ENAMETOOLONG.
        "PATH": f"{tools}:{root.joinpath(*['p' * 250] * 6)}:/usr/bin:/bin",
        "LF_HOME": str(home / ".lf"),
        "TMPDIR": str(root),
    }
    return repo, env


def _bootstrap(repo: Path, env: dict) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["bash", "scripts/bootstrap-cron-host.sh", "infrastructure"],
        cwd=repo,
        env=env,
        capture_output=True,
        text=True,
    )


def test_cron_bootstrap_installs_jobs_for_a_wave_running_on_its_local_default(tmp_path):
    repo, env = _cron_host(tmp_path, wave_machine=None)

    result = _bootstrap(repo, env)

    assert result.returncode == 0, result.stdout + result.stderr
    assert "no Machine default; it runs on this Machine" in result.stdout
    home = Path(env["HOME"])
    crontab = (home / "crontab").read_text().splitlines()
    assert crontab[0] == "*/5 * * * * unrelated-job"
    jobs = {line.split("# loopflow:infrastructure:")[1]: line for line in crontab[1:]}
    assert len(jobs) == 2
    assert jobs["telemetry-daily"].startswith("0 9 * * * ")
    assert jobs["release-run"].startswith("0 10 * * * ")

    # cron hands sh the first 999 bytes of the command; it runs the wrapper,
    # which runs the installed gate, not PATH's lf.
    (home / "ran").unlink()
    command = jobs["release-run"].split(" ", 5)[5][:999]
    assert command.startswith("/bin/sh ")
    subprocess.run(["/bin/sh", "-c", command], env={}, check=True)
    assert (home / "ran").read_text() == "--wave infrastructure --batch graph release-run\n"


def test_cron_bootstrap_schedules_macos_jobs_in_the_login_session(tmp_path):
    repo, env = _cron_host(tmp_path, wave_machine=None, system="Darwin")

    result = _bootstrap(repo, env)

    assert result.returncode == 0, result.stdout + result.stderr
    home = Path(env["HOME"])
    # cron's lines for the Wave give way; unrelated entries stay.
    assert (home / "crontab").read_text() == "*/5 * * * * unrelated-job\n"
    agents = home / "Library/LaunchAgents"
    hours = {"telemetry-daily": 9, "release-run": 10}
    for job, hour in hours.items():
        plist = plistlib.loads((agents / f"loopflow.cron.infrastructure.{job}.plist").read_bytes())
        assert plist["StartCalendarInterval"] == {"Hour": hour, "Minute": 0}
        assert plist["ProgramArguments"] == [
            "/bin/sh",
            str(repo / f".lf/cron/infrastructure.{job}.sh"),
        ]
    uid = os.getuid()
    calls = (home / "launchctl").read_text()
    assert f"bootstrap gui/{uid} " in calls
    # The bootstrap's telemetry run went through launchd, not the shell.
    assert f"kickstart -k gui/{uid}/loopflow.cron.infrastructure.telemetry-daily" in calls
    assert (home / "ran").read_text() == "--wave infrastructure --batch graph telemetry-daily\n"


def test_cron_bootstrap_fails_a_red_launchd_telemetry_run(tmp_path):
    repo, env = _cron_host(tmp_path, wave_machine=None, system="Darwin")
    gate = Path(env["HOME"]) / ".lf-machine/install/gates/1/lf"
    gate.write_text(gate.read_text().replace('>> "$HOME/ran" ;;', '>> "$HOME/ran"; exit 3 ;;'))

    result = _bootstrap(repo, env)

    assert result.returncode == 1
    assert "the telemetry run is red" in result.stderr


def test_cron_bootstrap_refuses_a_line_cron_would_truncate(tmp_path):
    # A checkout path near macOS's 1,024-byte limit leaves no room in the line.
    deep = tmp_path.resolve()
    while len(str(deep / "loopflow")) < 930:
        deep /= "d" * min(200, 930 - len(str(deep / "loopflow")))
    deep.mkdir(parents=True)
    repo, env = _cron_host(deep, wave_machine=None)
    before = (Path(env["HOME"]) / "crontab").read_text()

    result = _bootstrap(repo, env)

    assert result.returncode == 1
    assert "cron runs only the first 999" in result.stderr
    assert (Path(env["HOME"]) / "crontab").read_text() == before


def test_cron_bootstrap_accepts_a_wave_placed_on_this_machine(tmp_path):
    repo, env = _cron_host(tmp_path, wave_machine={"id": LOCAL_MACHINE, "route": "local"})

    result = _bootstrap(repo, env)

    assert result.returncode == 0, result.stdout + result.stderr
    assert "is placed on this Machine" in result.stdout


def test_cron_bootstrap_leaves_the_crontab_alone_for_a_wave_placed_elsewhere(tmp_path):
    repo, env = _cron_host(tmp_path, wave_machine={"id": "home_mini", "route": "mini"})
    before = (Path(env["HOME"]) / "crontab").read_text()

    result = _bootstrap(repo, env)

    assert result.returncode == 1
    assert "placed on Machine home_mini" in result.stderr
    assert (Path(env["HOME"]) / "crontab").read_text() == before


def test_release_installer_uses_the_promotion_boundary_to_activate_the_binary():
    installer = (ROOT / "release/install.sh").read_text()

    assert '"$src" install promote \\' in installer
    assert '--cli-target "$dst"' in installer
    assert 'mv -f "$tmp" "$dst"' not in installer


def test_loopflow_ui_gate_keeps_mac_test_runners_signed():
    ci = (ROOT / ".github/workflows/macos.yml").read_text()
    ci_build = (ROOT / "scripts/build_xcode_ci.py").read_text()
    test_script = (ROOT / "scripts/test.py").read_text()
    screenshot_script = (ROOT / "scripts/generate_screenshots.py").read_text()

    for text in (ci, ci_build, test_script, screenshot_script):
        assert "CODE_SIGNING_ALLOWED=NO" not in text
        assert "CODE_SIGNING_REQUIRED=NO" not in text

    assert "uv run python scripts/build_xcode_ci.py" in ci
    assert '"CODE_SIGN_IDENTITY=-"' in ci_build
    assert '"CODE_SIGN_IDENTITY=-"' in test_script
    assert '"CODE_SIGN_IDENTITY=-"' in screenshot_script
    assert "-disableAutomaticPackageResolution" in ci_build


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
    assert "LOOPFLOW_TEST_LF=" in result.stdout
    assert "nextest run --all" in result.stdout or "test --all" in result.stdout
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
    assert "scripts/test_network.py uv run --no-sync pytest python/tests/" in full.stdout
    assert "python/tests/test_release_automation.py" not in full.stdout.split("Plan:", 1)[1]
