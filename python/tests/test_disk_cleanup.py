import json
import os
import plistlib
import shutil
import subprocess
import sys
import time
from pathlib import Path
from types import SimpleNamespace

import disk_cleanup as cleanup
import pytest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "disk-cleanup.sh"

pytestmark = pytest.mark.skipif(sys.platform != "darwin", reason="macOS maintenance script")


def _activity(paths=(), builders=()):
    processes = {999999: cleanup.Process(999999, 1, os.getuid(), "fixture", "cargo")}
    files = tuple(cleanup.OpenPath(999999, "txt", path) for path in paths)
    files += tuple(cleanup.OpenPath(999999, "cwd", path) for path in builders)
    return cleanup.Activity(processes, files)


def _write(path: Path, age_days: float, size: int = 4096) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"x" * size)
    _age(path, age_days)


def _age(path: Path, age_days: float) -> None:
    stamp = time.time() - age_days * 86400
    os.utime(path, (stamp, stamp))


def _age_tree(root: Path, age_days: float) -> None:
    for directory, _, _ in sorted(os.walk(root), key=lambda entry: -len(entry[0])):
        _age(Path(directory), age_days)


def _run(
    tmp_path: Path,
    stubs: dict[str, str] | None = None,
    exit_code: int = 0,
    args: tuple[str, ...] = (),
) -> str:
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir(exist_ok=True)
    for tool, body in {"uv": '[ "$2" != dir ] || echo "$HOME/.cache/uv"', **(stubs or {})}.items():
        stub = bin_dir / tool
        stub.write_text(f"#!/bin/sh\n{body}\n")
        stub.chmod(0o755)
    env = {
        "HOME": str(tmp_path / "home"),
        "LF_HOME": str(tmp_path / "home/.lf"),
        "LOOPFLOW_SRC": str(tmp_path / "home/src"),
        "TMPDIR": str(tmp_path),
        "PATH": f"{bin_dir}:/usr/bin:/bin:/usr/sbin:/sbin",
    }
    config = tmp_path / "home/.lf-machine/maintenance/disk-cleanup/config.json"
    config.parent.mkdir(parents=True, exist_ok=True)
    config.write_text('{"enabled": true}')
    # Redirect only the host-wide OS lock into this disposable fixture. The
    # runner and flock behavior are unchanged; concurrent real recovery cannot
    # make preservation assertions pass merely because cleanup skipped.
    launcher = (
        "import sys; from pathlib import Path; "
        f"sys.path.insert(0, {str(SCRIPT.parent)!r}); "
        "import resource_envelope, disk_cleanup; "
        f"disk_cleanup._temporary_roots = lambda: {{Path({str(tmp_path / 'temp')!r})}}; "
        f"resource_envelope.Path = lambda p: Path({str(tmp_path)!r}) if p == '/tmp' else Path(p); "
        "sys.argv = sys.argv[1:]; sys.exit(disk_cleanup.main())"
    )
    result = subprocess.run(
        ["/usr/bin/python3", "-c", launcher, str(SCRIPT), *args],
        env=env,
        capture_output=True,
        text=True,
        timeout=600,
    )
    assert result.returncode == exit_code, result.stdout + result.stderr
    return result.stdout + result.stderr


def test_durable_history_and_databases_survive(tmp_path: Path) -> None:
    lf = tmp_path / "home/.lf"
    paths = [
        "loopflow.db",
        "loopflow.db-wal",
        "accounts/codex/a/sessions/old.jsonl",
        "runs/ab/oldest/events.jsonl",
        "logs/old.log",
        "copies/retired/loopflow.db",
        "loopflow.db.backup-old",
        "loopflow.db.backup-new",
    ]
    for name in paths:
        _write(lf / name, 40)
    _age_tree(lf, 40)
    _run(tmp_path)
    assert all((lf / name).exists() for name in paths)


def test_idle_ignored_build_output_goes_and_recent_or_tracked_output_stays(
    tmp_path: Path,
) -> None:
    repo = tmp_path / "home/src/kata"
    repo.mkdir(parents=True)
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    (repo / ".gitignore").write_text("target/\nnode_modules/\n")
    _write(repo / "target/debug/kata", 3)
    _write(repo / "node_modules/left-pad/index.js", 0)
    _write(repo / ".venv/bin/python", 3)
    _age_tree(repo / "target", 3)
    _age_tree(repo / ".venv", 3)

    out = _run(tmp_path)

    assert not (repo / "target").exists(), out
    assert (repo / "node_modules").exists(), out
    assert (repo / ".venv").exists(), out
    assert repo.exists()


def test_linked_worktree_loses_output_idle_for_four_hours(tmp_path: Path) -> None:
    src = tmp_path / "home/src"
    repo = src / "kata"
    repo.mkdir(parents=True)
    git = ["git", "-c", "user.name=t", "-c", "user.email=t@t"]
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    (repo / ".gitignore").write_text("target/\n")
    subprocess.run([*git, "-C", str(repo), "add", "."], check=True)
    subprocess.run([*git, "-C", str(repo), "commit", "-qm", "init"], check=True)
    idle, recent = src / "kata.idle", src / "kata.recent"
    for wt in (idle, recent):
        subprocess.run(["git", "-C", str(repo), "worktree", "add", "-q", str(wt)], check=True)
    _write(idle / "target/debug/kata", 0.25)
    _age_tree(idle / "target", 0.25)
    _write(recent / "target/debug/kata", 0.001)
    _age_tree(recent / "target", 0.001)

    out = _run(tmp_path)

    assert not (idle / "target").exists(), out
    assert (recent / "target").exists(), out
    assert "written in the last" in out


def test_general_cache_and_trash_are_observation_only(tmp_path: Path) -> None:
    home = tmp_path / "home"
    roots = (home / "Library/Caches", home / ".Trash")
    for root in roots:
        _write(root / "old/data", 40)
        _age_tree(root, 40)
    _run(tmp_path)
    assert all((root / "old/data").exists() for root in roots)


def test_failed_open_file_inspection_stops_all_cleanup(tmp_path: Path) -> None:
    cache = tmp_path / "home/Library/Caches/old/data"
    _write(cache, 40)
    _age_tree(cache.parent, 40)

    out = _run(tmp_path, {"lsof": "exit 1"}, exit_code=1)

    assert cache.exists(), out
    assert "open-file inspection failed; stopping cleanup" in out


def test_idle_shell_in_checkout_does_not_retain_generated_output(tmp_path: Path) -> None:
    repo = tmp_path / "home/src/kata"
    repo.mkdir(parents=True)
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    (repo / ".gitignore").write_text("target/\n")
    _write(repo / "target/debug/kata", 3)
    _age_tree(repo / "target", 3)
    process = subprocess.Popen(["/bin/sleep", "600"], cwd=repo)
    try:
        out = _run(tmp_path)
        assert not (repo / "target/debug/kata").exists(), out
        assert (repo / ".gitignore").exists(), out
    finally:
        process.terminate()
        process.wait(timeout=10)


def _git(repo: Path, *args: str, env: dict[str, str] | None = None) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo), *args], env=env, capture_output=True, text=True, check=True
    )
    return result.stdout.strip()


@pytest.fixture
def cleaner(tmp_path: Path, monkeypatch):
    home = tmp_path / "home"
    home.mkdir()
    monkeypatch.setenv("HOME", str(home))
    monkeypatch.setenv("LF_HOME", str(home / ".lf"))
    monkeypatch.setenv("LOOPFLOW_SRC", str(home / "src"))
    public = home / ".local/bin"
    public.mkdir(parents=True)
    tool = public / "uv"
    tool.write_text('#!/bin/sh\n[ "$2" != dir ] || echo "$HOME/.cache/uv"\n')
    tool.chmod(0o755)
    monkeypatch.setenv("PATH", f"{public}:/usr/bin:/bin:/usr/sbin:/sbin")
    monkeypatch.setattr(cleanup, "_temporary_roots", lambda: {tmp_path / "temp"})
    runner = cleanup.Cleaner(False, 1800)
    monkeypatch.setattr(runner, "_activity", _activity)
    return runner


def _build_fixture(runner, age_minutes: int, size: int = 4096) -> Path:
    repo = runner.src / "kata"
    repo.mkdir(parents=True)
    _git(repo, "init", "-q")
    (repo / ".gitignore").write_text("target/\n")
    _write(repo / "target/debug/output", age_minutes / 1440, size)
    _age_tree(repo / "target", age_minutes / 1440)
    return repo


@pytest.mark.parametrize(
    "protection", [None, "recent", "open", "builder", "tracked", "not-ignored"]
)
def test_nested_worktree_discovery_uses_owning_checkout(cleaner, monkeypatch, protection) -> None:
    repo = _build_fixture(cleaner, 300)
    _git(repo, "add", ".gitignore")
    _git(repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "base")
    worktree = repo / "work/teams/tasks/nested/feature"
    _git(repo, "worktree", "add", "-qb", "feature", str(worktree))
    source = worktree / "unpublished.rs"
    source.write_text("unpublished work")
    _git(worktree, "add", "unpublished.rs")
    _git(worktree, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "unpublished")
    head = _git(worktree, "rev-parse", "HEAD")
    source.write_text("unfinished changes")
    output = worktree / "packages/one/two/three/target/debug/output"
    _write(output, 3)
    _age_tree(output.parent.parent, 3)
    if protection == "recent":
        _age(output, 0)
    elif protection == "open":
        _age(output, 0.5)
        monkeypatch.setattr(
            cleaner, "_activity", lambda: _activity(frozenset({output}), frozenset())
        )
    elif protection == "builder":
        monkeypatch.setattr(
            cleaner, "_activity", lambda: _activity(frozenset(), frozenset({worktree}))
        )
    elif protection == "tracked":
        _git(worktree, "add", "-f", str(output))
    elif protection == "not-ignored":
        (worktree / ".gitignore").write_text("")

    assert cleaner.run() == 0
    assert output.exists() == (protection is not None)
    assert source.read_text() == "unfinished changes"
    assert _git(worktree, "rev-parse", "HEAD") == head
    assert (repo / "target").exists() == (protection == "builder")


@pytest.mark.parametrize("link_ancestor", [False, True])
def test_nested_discovery_does_not_follow_symlinks(cleaner, link_ancestor: bool) -> None:
    repo = _build_fixture(cleaner, 300)
    external = cleaner.home / "outside"
    output = external / "target/debug/output"
    _write(output, 3)
    _age_tree(external, 3)
    nested = repo / "nested/deep"
    nested.mkdir(parents=True)
    (nested / ("alias" if link_ancestor else "target")).symlink_to(
        external if link_ancestor else external / "target"
    )
    assert cleaner.run() == 0
    assert output.exists()
    assert not (repo / "target").exists()


def test_generated_root_with_embedded_checkout_preserves_unpublished_source(cleaner) -> None:
    repo = _build_fixture(cleaner, 300)
    nested = repo / "target/debug/dependency"
    nested.mkdir()
    _git(nested, "init", "-q")
    _write(nested / "unpublished.rs", 3)
    _age_tree(repo / "target", 3)
    assert cleaner.run() == 0
    assert (nested / "unpublished.rs").exists()
    assert (nested / ".git").exists()
    assert any(entry.get("reason") == "embedded Git checkout" for entry in cleaner.entries)


def test_global_derived_data_reclaims_idle_projects_and_preserves_open_recent_and_symlinks(
    tmp_path: Path,
) -> None:
    home = tmp_path / "home"
    derived = home / "Library/Developer/Xcode/DerivedData"
    for name in ("idle", "open", "recent"):
        _write(derived / name / "Build/output", 0.5, 1024 * 1024)
        _age_tree(derived / name, 3)
    _age(derived / "recent/Build/output", 0)
    external = home / "unpublished/source.swift"
    _write(external, 3)
    (derived / "alias").symlink_to(external.parent)
    with (derived / "open/Build/output").open("rb"):
        out = _run(tmp_path)
    assert not (derived / "idle/Build").exists(), out
    assert (derived / "open/Build/output").exists(), out
    assert (derived / "recent/Build/output").exists(), out
    assert external.exists(), out
    receipt = json.loads((home / "Library/Logs/loopflow-disk-cleanup.json").read_text())
    assert receipt["removed_allocated_bytes"] >= 1024 * 1024


@pytest.mark.parametrize("git_checkout", [False, True])
def test_global_derived_data_reclaims_build_beside_unpublished_package_source(
    cleaner, git_checkout: bool
) -> None:
    project = cleaner.home / "Library/Developer/Xcode/DerivedData/project"
    source = project / "SourcePackages/checkouts/package"
    source.mkdir(parents=True)
    if git_checkout:
        _git(source, "init", "-q")
    _write(source / "unpublished.swift", 3)
    for name in ("Build", "Index.noindex"):
        _write(project / name / "output", 3)
    cache = project.parent / "ModuleCache.noindex"
    _write(cache / "module", 3)
    _age_tree(cache, 3)
    _age_tree(project, 3)
    assert cleaner.run() == 0
    assert not (project / "Build").exists()
    assert not (project / "Index.noindex").exists()
    assert not cache.exists()
    # Removing Build updates the project mtime. Age it again to prove that a
    # later unattended cycle preserves source even without Git metadata.
    _age_tree(project, 3)
    assert cleaner.run() == 0
    assert (source / "unpublished.swift").exists()
    assert (source / ".git").exists() == git_checkout


@pytest.mark.parametrize("protection", ["tracked", "embedded-checkout", "symlink-ancestor"])
def test_global_derived_data_preserves_source_and_symlink_targets(cleaner, protection) -> None:
    root = cleaner.home / "Library/Developer/Xcode"
    output = root / "DerivedData/project/Build/output"
    _write(output, 3)
    if protection == "tracked":
        _git(root, "init", "-q")
        (root / ".gitignore").write_text("DerivedData/\n")
        _git(root, "add", "-f", str(output))
    elif protection == "embedded-checkout":
        _git(output.parent, "init", "-q")
        _write(output.parent / "unpublished.swift", 3)
    else:
        external = cleaner.home / "external"
        (root / "DerivedData").rename(external)
        (root / "DerivedData").symlink_to(external)
    _age_tree(root, 3)
    assert cleaner.run() == 0
    assert output.exists()


def test_build_discovery_limit_reports_incomplete_and_retains_uninspected_output(
    cleaner, monkeypatch
) -> None:
    repo = _build_fixture(cleaner, 300)
    monkeypatch.setattr(cleanup, "DISCOVERY_MAX_ENTRIES", 1)
    assert cleaner.run() == 1
    assert (repo / "target/debug/output").exists()
    assert any("build discovery exceeded" in failure for failure in cleaner.failures)


@pytest.mark.parametrize(
    "free_gib,minutes,removed", [(80, 120, False), (50, 120, True), (10, 30, True), (10, 5, False)]
)
def test_pressure_changes_retention_without_removing_recent_output(
    cleaner, monkeypatch, free_gib: int, minutes: int, removed: bool
) -> None:
    repo = _build_fixture(cleaner, minutes)
    monkeypatch.setattr(
        cleanup.shutil, "disk_usage", lambda _: SimpleNamespace(free=free_gib * 1024**3)
    )
    assert cleaner.run() == 0
    assert (repo / "target").exists() != removed
    assert (repo / ".gitignore").exists()


def test_reclaims_large_idle_profile_beside_live_output_and_unpublished_source(
    cleaner, monkeypatch
) -> None:
    repo = _build_fixture(cleaner, 300, 128 * 1024**2)
    live = repo / "target/release/live"
    _write(live, 0.5)
    _age_tree(repo / "target", 0.5)
    source = repo / "unpublished.rs"
    source.write_text("unfinished work")
    monkeypatch.setattr(cleaner, "_activity", lambda: _activity(frozenset({live}), frozenset()))
    assert cleaner.run() == 0
    assert not (repo / "target/debug").exists()
    assert live.exists()
    assert source.read_text() == "unfinished work"
    assert cleaner.removed_bytes >= 128 * 1024**2


def test_pressure_returns_to_normal_after_space_is_recovered(cleaner, monkeypatch) -> None:
    repo = _build_fixture(cleaner, 120, 8192)
    first = repo / "target/debug"
    second = repo / "target/release"
    _write(second / "other", 120 / 1440)
    _age_tree(second, 120 / 1440)
    # A recent root selects separate profiles; the largest one restores the target.
    monkeypatch.setattr(
        cleanup.shutil,
        "disk_usage",
        lambda _: SimpleNamespace(free=(10 if first.exists() else 80) * 1024**3),
    )
    assert cleaner.run() == 0
    assert not first.exists()
    assert second.exists()


@pytest.mark.parametrize("protection", ["builder", "tracked", "symlink"])
def test_protected_build_output_is_retained(cleaner, monkeypatch, protection: str) -> None:
    repo = _build_fixture(cleaner, 300)
    protected = repo / "target/debug/output"
    if protection == "builder":
        monkeypatch.setattr(cleaner, "_activity", lambda: _activity(frozenset(), frozenset({repo})))
    elif protection == "tracked":
        _git(repo, "add", "-f", "target/debug/output")
    else:
        outside = cleaner.home / "outside"
        (repo / "target").rename(outside)
        (repo / "target").symlink_to(outside)
        protected = outside / "debug/output"
    assert cleaner.run() == 0
    assert protected.exists()


def test_nested_symlink_is_removed_without_following_it(cleaner) -> None:
    repo = _build_fixture(cleaner, 300)
    source = cleaner.home / "outside/source"
    _write(source, 0)
    link = repo / "target/debug/link"
    link.symlink_to(source.parent)
    stamp = time.time() - 300 * 60
    os.utime(link, (stamp, stamp), follow_symlinks=False)
    _age_tree(repo / "target", 300 / 1440)
    assert cleaner.run() == 0
    assert not (repo / "target").exists()
    assert source.exists()


def test_dry_run_reports_candidates_and_never_calls_native_cleanup(cleaner) -> None:
    repo = _build_fixture(cleaner, 300)
    (cleaner.home / ".local/bin/uv").write_text("#!/bin/sh\nexit 99\n")
    cleaner.dry_run = True
    assert cleaner.run() == 0
    assert (repo / "target/debug/output").exists()
    assert any(entry["status"] == "eligible" for entry in cleaner.entries)
    assert cleaner.removed_bytes == 0


def test_partial_cache_prune_failure_is_nonzero_after_reclamation(cleaner) -> None:
    repo = _build_fixture(cleaner, 300)
    ((cleaner.home / ".local/bin") / "uv").write_text("#!/bin/sh\necho broken >&2\nexit 7\n")
    assert cleaner.run() == 1
    assert not (repo / "target").exists()
    receipt = json.loads(cleaner.log.with_suffix(".json").read_text())
    assert receipt["exit_code"] == 1
    assert any("broken" in failure for failure in receipt["failures"])
    assert receipt["removed_allocated_bytes"] > 0


def test_failed_content_inspection_retains_output_and_reports_failure(cleaner, monkeypatch) -> None:
    repo = _build_fixture(cleaner, 300)

    def denied(candidate):
        raise PermissionError("inspection denied")

    monkeypatch.setattr(cleaner, "_measure", denied)
    assert cleaner.run() == 1
    assert (repo / "target/debug/output").exists()
    assert any("inspection denied" in failure for failure in cleaner.failures)


def test_child_deadline_kills_its_process_group(cleaner) -> None:
    marker = cleaner.home / "child-finished"
    start = time.time()
    with pytest.raises(subprocess.TimeoutExpired):
        cleaner._command(["/bin/sh", "-c", f'(sleep 1; touch "{marker}") & wait'], timeout=0.1)
    assert time.time() - start < 1
    time.sleep(1.1)
    assert not marker.exists()


def test_wall_budget_reports_incomplete_and_next_run_acquires_lock(tmp_path: Path) -> None:
    out = _run(tmp_path, {"lsof": "sleep 10"}, exit_code=1, args=("--budget-seconds", "0.2"))
    assert "wall-clock budget exhausted" in out
    out = _run(tmp_path, {"lsof": "printf 'p1\\ncinit\\nfcwd\\nn/\\n'"})
    assert "done:" in out


def test_activity_distinguishes_a_builder_from_an_idle_shell(cleaner, monkeypatch) -> None:
    repo = _build_fixture(cleaner, 300)
    monkeypatch.delattr(cleaner, "_activity")
    monkeypatch.setattr(
        cleaner,
        "_command",
        lambda *args, **kwargs: f"p123\nccargo\nfcwd\nn{repo}\np456\nczsh\nfcwd\nn{repo.parent}\n",
    )
    monkeypatch.setattr(
        cleaner,
        "_processes",
        lambda: {
            123: cleanup.Process(123, 1, os.getuid(), "fixture", "cargo"),
            456: cleanup.Process(456, 1, os.getuid(), "fixture", "zsh"),
        },
    )
    assert (
        cleaner._activity().reason(cleanup.Candidate(repo / "target", "build", repo))
        == "builder in checkout"
    )


@pytest.fixture
def schedule(cleaner, monkeypatch):
    scripts = cleaner.home / "standalone-runner"
    scripts.mkdir(parents=True)
    for name in (
        "disk-cleanup.sh",
        "disk_cleanup.py",
        "cleanup_evidence.py",
        "resource_envelope.py",
        "performance-budgets.json",
    ):
        shutil.copyfile(SCRIPT.parent / name, scripts / name)
    monkeypatch.setattr(cleanup, "__file__", str(scripts / "disk_cleanup.py"))
    public = cleaner.home / ".local/bin"
    launchctl = public / "launchctl"
    launchctl.write_text("""#!/bin/sh
case "$1" in
print)
  if [ -f "$HOME/launch-state" ]; then cat "$HOME/launch-state";
  else echo 'Could not find service' >&2; exit 113; fi ;;
bootstrap) echo 'state = waiting' > "$HOME/launch-state" ;;
bootout) rm "$HOME/launch-state" ;;
esac
""")
    launchctl.chmod(0o755)
    crontab = public / "crontab"
    crontab.write_text("""#!/bin/sh
case "$1" in
-l) cat "$HOME/cron" ;;
-) cat > "$HOME/cron" ;;
esac
""")
    crontab.chmod(0o755)
    (cleaner.home / "cron").write_text(
        "0 0 * * * keep-me\n17 * * * * old skill # loopflow:disk-cleanup\n"
    )
    return cleaner.home / "Library/LaunchAgents/com.loopflow.disk-cleanup.plist"


def test_launchd_install_replaces_only_old_cron_and_runs_mechanical_entrypoint(
    schedule, cleaner
) -> None:
    assert cleanup._schedule("install") == 0
    payload = plistlib.loads(schedule.read_bytes())
    assert payload["StartCalendarInterval"] == {"Minute": 17}
    assert (cleaner.home / "cron").read_text() == "0 0 * * * keep-me\n"
    args = payload["ProgramArguments"]
    assert args[:2] == ["/usr/bin/env", "-i"]
    assert args[-3] == "/bin/bash"
    assert str(cleaner.state / "versions") in args[-2]
    assert args[-1] == "run"
    assert cleaner.src.as_posix() not in args[-2]
    assert next(arg for arg in args if arg.startswith("PATH=")).startswith(
        f"PATH={cleaner.home}/.local/bin:"
    )
    assert not any("LF_BIN=" in arg or "skill" == arg for arg in args)
    # Execute the installed stable shell path with the generated clean environment.
    # --help is read-only and avoids the real host recovery lock in this proof.
    result = subprocess.run([*args, "--help"], capture_output=True, text=True, check=True)
    assert "--dry-run" in result.stdout
    assert cleanup._schedule("install") == 0
    assert cleanup._schedule("uninstall") == 0
    assert not schedule.exists()
    assert (cleaner.home / "cron").read_text() == "0 0 * * * keep-me\n"


def test_install_and_uninstall_preserve_running_job(schedule, cleaner) -> None:
    schedule.parent.mkdir(parents=True)
    schedule.write_text("running job")
    (cleaner.home / "launch-state").write_text("state = running\npid = 12345\n")
    for action in ("install", "uninstall"):
        with pytest.raises(RuntimeError, match="cleanup is running"):
            cleanup._schedule(action)
    assert schedule.read_text() == "running job"
    assert "old skill" in (cleaner.home / "cron").read_text()


def test_dead_process_releases_lock_without_stale_lock_removal(tmp_path: Path) -> None:
    code = (
        "import sys; from pathlib import Path; "
        f"sys.path.insert(0, {str(SCRIPT.parent)!r}); import resource_envelope as r; "
        f"r.Path = lambda p: Path({str(tmp_path)!r}) if p == '/tmp' else Path(p); "
        "lock = r._lock_recovery(); assert lock is not None; print('locked', flush=True); "
        "sys.stdin.read()"
    )
    holder = subprocess.Popen(
        [sys.executable, "-c", code], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True
    )
    try:
        assert holder.stdout.readline().strip() == "locked"
        assert "skipped: another cleanup" in _run(tmp_path)
    finally:
        holder.kill()
        holder.communicate(timeout=5)
    assert "done:" in _run(tmp_path, {"lsof": "printf 'p1\\ncinit\\nfcwd\\nn/\\n'"})


def test_builder_outside_checkout_does_not_retain_output(cleaner, monkeypatch) -> None:
    repo = _build_fixture(cleaner, 300)
    monkeypatch.setattr(
        cleaner, "_activity", lambda: _activity(frozenset(), frozenset({Path("/")}))
    )
    assert cleaner.run() == 0
    assert not (repo / "target").exists()


def test_output_opened_after_inventory_is_retained(cleaner, monkeypatch) -> None:
    repo = _build_fixture(cleaner, 300)
    artifact = repo / "target/debug/output"
    snapshots = iter(
        [
            _activity(frozenset(), frozenset()),
            _activity(frozenset({artifact}), frozenset()),
        ]
    )
    monkeypatch.setattr(cleaner, "_activity", lambda: next(snapshots))
    assert cleaner.run() == 0
    assert artifact.exists()


def test_launchd_bootstrap_failure_restores_existing_job(schedule, cleaner) -> None:
    assert cleanup._schedule("install") == 0
    original = schedule.read_bytes()
    (cleaner.home / "cron").write_text("17 * * * * old # loopflow:disk-cleanup\n")
    launchctl = (cleaner.home / ".local/bin") / "launchctl"
    script = launchctl.read_text()
    script = script.replace(
        "bootstrap) echo",
        'bootstrap) if [ ! -f "$HOME/failed-once" ]; then '
        'touch "$HOME/failed-once"; exit 5; fi; echo',
    )
    launchctl.write_text(script)
    with pytest.raises(subprocess.CalledProcessError):
        cleanup._schedule("install")
    assert schedule.read_bytes() == original
    assert (cleaner.home / "launch-state").exists()
    assert "old # loopflow:disk-cleanup" in (cleaner.home / "cron").read_text()


def test_dry_run_does_not_truncate_log_history(cleaner) -> None:
    cleaner.log.parent.mkdir(parents=True)
    history = b"retained history\n" + b"x" * (11 * 1024**2)
    cleaner.log.write_bytes(history)
    cleaner.dry_run = True
    assert cleaner.run() == 0
    assert cleaner.log.read_bytes().startswith(history)


@pytest.mark.parametrize("log_name,size", [("log", 0), ("log", 11 * 1024**2), ("disk_log", 0)])
def test_failed_log_write_does_not_prevent_reclamation(
    cleaner, monkeypatch, log_name, size
) -> None:
    repo = _build_fixture(cleaner, 300)
    _write(getattr(cleaner, log_name), 0, size)
    original = Path.open

    def full(path, *args, **kwargs):
        if path == getattr(cleaner, log_name):
            raise OSError("No space left on device")
        return original(path, *args, **kwargs)

    monkeypatch.setattr(Path, "open", full)

    def failed_output(*args, **kwargs):
        raise OSError("output filesystem full")

    monkeypatch.setattr(cleanup, "print", failed_output, raising=False)
    assert cleaner.run() == 1
    assert not (repo / "target").exists()
    assert any("No space left on device" in failure for failure in cleaner.failures)
    receipt = json.loads(cleaner.log.with_suffix(".json").read_text())
    assert receipt["removed_allocated_bytes"] > 0
    assert any(entry["status"] == "removed" for entry in receipt["candidates"])


def test_final_log_failure_is_in_receipt_and_exit_status(cleaner, monkeypatch) -> None:
    repo = _build_fixture(cleaner, 300)

    def failed_final_output(message, **kwargs):
        if "done:" in message:
            raise OSError("final output unavailable")
        print(message, **kwargs)

    monkeypatch.setattr(cleanup, "print", failed_final_output, raising=False)
    assert cleaner.run() == 1
    assert not (repo / "target").exists()
    receipt = json.loads(cleaner.log.with_suffix(".json").read_text())
    assert receipt["exit_code"] == 1
    assert any("final output unavailable" in failure for failure in receipt["failures"])
    assert receipt["removed_allocated_bytes"] > 0


def test_pytest_garbage_and_known_temp_only(cleaner) -> None:
    root = next(iter(cleaner.temp_roots))
    garbage = root / "pytest-of-test/garbage-1234abcd"
    old = root / "pytest-of-test/pytest-12"
    current = root / "pytest-of-test/pytest-13"
    unknown = root / "unknown-demo"
    known = root / "lf-worlds-test"
    for path in (garbage, old, current, unknown, known):
        _write(path / "data", 3)
        (path / ".cleanup-evidence.json").write_text('{"outcome": "passed"}')
        _age(path / ".cleanup-evidence.json", 3)
        _age_tree(path, 3)
    _git(old, "init", "-q")
    for path in (old / ".git").rglob("*"):
        _age(path, 3)
    _age_tree(old, 3)
    _write(current / "data", 0)
    (old.parent / "pytest-current").symlink_to(current)
    assert cleaner.run() == 0
    assert not garbage.exists()
    assert not old.exists()
    assert not known.exists()
    assert current.exists() and unknown.exists()


def test_chrome_requires_opt_in_retains_newest_and_held_copy(cleaner, monkeypatch) -> None:
    temp = cleaner.home / "darwin/T"
    temp.mkdir(parents=True)
    cleaner.temp_roots = {temp}
    root = temp.parent / "X/com.google.Chrome.code_sign_clone"
    copies = [root / f"code_sign_clone.{name}" for name in ("old", "held", "new")]
    for copy, days in zip(copies, (3, 2, 1.5)):
        _write(copy / "Google Chrome.app.bundle/data", days)
        _age_tree(copy, days)
    assert cleaner.run() == 0
    assert all(copy.exists() for copy in copies)
    cleaner.config["chrome_copies"] = True
    monkeypatch.setattr(
        cleaner, "_activity", lambda: _activity({copies[1] / "Google Chrome.app.bundle/data"})
    )
    assert cleaner.run() == 0
    assert not copies[0].exists()
    assert copies[1].exists() and copies[2].exists()
    assert cleaner.stopped == []


@pytest.mark.parametrize("descriptor", ["txt", "cwd", "3u"])
def test_activity_identifies_candidate_owner_not_repository_neighbor(cleaner, descriptor) -> None:
    candidate = cleanup.Candidate(cleaner.home / "output", "build")
    activity = cleanup.Activity(
        {},
        (
            cleanup.OpenPath(123, descriptor, candidate.path / "binary"),
            cleanup.OpenPath(456, "cwd", cleaner.home),
        ),
    )
    assert activity.owners(candidate) == {123}


@pytest.mark.parametrize("force", [False, True])
def test_stops_real_demo_store_owner_and_descendant_before_removal(
    cleaner, monkeypatch, force
) -> None:
    demo = cleaner.home / ".lf-machine/demos/test"
    _write(demo / "store.db", 3)
    marker = demo / ".disposable-coding-demo"
    marker.write_text("disposable runtime; evidence stored elsewhere")
    _age(marker, 3)
    (demo / ".cleanup-evidence.json").write_text('{"outcome": "passed"}')
    _age(demo / ".cleanup-evidence.json", 3)
    _age_tree(demo, 3)
    # A descendant deliberately outlives a graceful parent and holds no direct
    # candidate handle. Teardown must retain its identity after reparenting.
    code = (
        "import subprocess, sys, signal, time; "
        + ("signal.signal(signal.SIGTERM, signal.SIG_IGN); " if force else "")
        + "f=open(sys.argv[1], 'rb'); "
        "child=subprocess.Popen(['/bin/sleep', '600']); "
        "print(child.pid, flush=True); time.sleep(600)"
    )
    owner = subprocess.Popen(
        [sys.executable, "-c", code, str(demo / "store.db")], stdout=subprocess.PIPE, text=True
    )
    neighbor = subprocess.Popen(["/bin/sleep", "600"], cwd=cleaner.home)
    try:
        child = int(owner.stdout.readline())
        monkeypatch.delattr(cleaner, "_activity")
        monkeypatch.setattr(cleanup, "OWNER_GRACE_SECONDS", 0.2)
        assert cleaner.run() == 0
        owner.wait(timeout=3)
        assert not demo.exists()
        assert neighbor.poll() is None
        stopped = {item["pid"] for item in cleaner.stopped}
        assert owner.pid in stopped and child in stopped
        assert any(item["signal"] == "SIGKILL" for item in cleaner.stopped) == force
    finally:
        if owner.poll() is None:
            owner.kill()
        owner.wait(timeout=3)
        neighbor.terminate()
        neighbor.wait(timeout=3)
        if "child" in locals():
            try:
                os.kill(child, 9)
            except ProcessLookupError:
                pass


def test_stops_executable_mapping_owner_without_killing_unrelated_work(
    cleaner, monkeypatch
) -> None:
    repo = _build_fixture(cleaner, 3000)
    binary = repo / "target/debug/sleeper"
    shutil.copyfile("/bin/sleep", binary)
    binary.chmod(0o755)
    _age(binary, 3)
    _age_tree(repo / "target", 3)
    owner = subprocess.Popen([str(binary), "600"], cwd=cleaner.home)
    try:
        monkeypatch.delattr(cleaner, "_activity")
        monkeypatch.setattr(cleanup, "OWNER_GRACE_SECONDS", 0.2)
        assert cleaner.run() == 0
        owner.wait(timeout=3)
        assert not (repo / "target").exists()
        assert (repo / ".gitignore").exists()
    finally:
        if owner.poll() is None:
            owner.kill()
        owner.wait(timeout=3)


@pytest.mark.parametrize(
    "failure", ["unknown", "changed", "permission", "surviving", "interrupted"]
)
def test_owner_teardown_failure_preserves_output(cleaner, monkeypatch, failure) -> None:
    repo = _build_fixture(cleaner, 3000)
    artifact = repo / "target/debug/output"
    process = cleanup.Process(999999, 1, os.getuid(), "original", "sleeper")
    activity = cleanup.Activity(
        {process.pid: process} if failure != "unknown" else {},
        (cleanup.OpenPath(process.pid, "txt", artifact),),
    )
    monkeypatch.setattr(cleaner, "_activity", lambda: activity)
    changed = cleanup.Process(process.pid, 1, os.getuid(), "changed", "sleeper")
    monkeypatch.setattr(
        cleaner,
        "_processes",
        lambda pids: {process.pid: changed if failure == "changed" else process},
    )
    monkeypatch.setattr(cleanup, "OWNER_GRACE_SECONDS", 0)

    def signal_owner(pid, sig):
        if failure == "permission":
            raise PermissionError("cannot signal owner")
        if failure == "interrupted":
            raise cleanup.CleanupStopped("interrupted teardown")

    monkeypatch.setattr(cleanup.os, "kill", signal_owner)
    assert cleaner.run() == 1
    assert artifact.exists()
    assert cleaner.removed_bytes == 0


def test_usage_growth_is_bounded_and_unknown_is_not_zero(cleaner, monkeypatch) -> None:
    monkeypatch.setattr(cleanup, "HISTORY_LIMIT", 2)
    path = str(cleaner.src / "repo")
    for value in (10, 30, None):
        cleaner._record_usage({"at": time.time(), "roots": [{"path": path, "bytes": value}]})
    history = json.loads((cleaner.state / "usage.json").read_text())
    assert len(history) == 2
    assert history[0]["roots"][0]["delta_bytes"] == 20
    assert history[1]["roots"][0]["delta_bytes"] is None
    assert (cleaner.state / "usage.json").stat().st_mode & 0o777 == 0o600


def test_unknown_usage_is_reported_without_blocking_cleanup(cleaner, monkeypatch) -> None:
    _build_fixture(cleaner, 300)
    command = cleaner._command

    def denied(args, **kwargs):
        if args[0] == "du":
            raise PermissionError("measurement denied")
        return command(args, **kwargs)

    monkeypatch.setattr(cleaner, "_command", denied)
    assert cleaner.run() == 0
    receipt = json.loads(cleaner.log.with_suffix(".json").read_text())
    assert any(
        entry["error"] == "measurement denied" and entry["bytes"] is None
        for entry in receipt["usage"]["roots"]
    )
    assert cleaner.removed_bytes > 0


def test_mutating_run_requires_local_opt_in(tmp_path: Path) -> None:
    result = subprocess.run(
        [str(SCRIPT), "run"], env={"HOME": str(tmp_path)}, capture_output=True, text=True
    )
    assert result.returncode == 1
    assert "cleanup is disabled" in result.stderr


@pytest.mark.parametrize("retention", ["keep_forever", "project_owned"])
def test_accounting_labels_do_not_authorize_deletion_and_protection_vetoes(
    cleaner, retention
) -> None:
    repo = _build_fixture(cleaner, 300)
    other = cleaner.home / "Movies/movie"
    _write(other, 40)
    cleaner.usage_labels = {
        repo / "target/debug/output": (retention, "code/development"),
        other: ("emergency_reducible", "media"),
    }
    assert cleaner.run() == 0
    assert (repo / "target").exists()
    assert other.exists()
    receipt = json.loads(cleaner.log.with_suffix(".json").read_text())
    roots = {entry["path"]: entry for entry in receipt["usage"]["roots"]}
    assert roots[str(repo / "target/debug/output")]["retention"] == retention
    assert roots[str(other)]["content_type"] == "media"
    assert receipt["usage"]["unaccounted_physical_bytes"] is None


def test_swift_output_reclaimed_beside_dependency_source(cleaner) -> None:
    repo = _build_fixture(cleaner, 300)
    (repo / ".gitignore").write_text("target/\n.build/\n")
    source = repo / ".build/checkouts/package"
    source.mkdir(parents=True)
    _git(source, "init", "-q")
    _write(source / "unpublished.swift", 3)
    output = repo / ".build/arm64-apple-macosx/debug/output"
    _write(output, 3)
    _age_tree(output.parent.parent, 3)
    assert cleaner.run() == 0
    assert not output.exists()
    assert (source / "unpublished.swift").exists()
    assert (source / ".git").exists()


@pytest.mark.parametrize("root_removed", [False, True])
def test_concurrent_temp_removal_is_retained_without_false_failure(
    cleaner, monkeypatch, root_removed
) -> None:
    repo = _build_fixture(cleaner, 300)

    def vanished(candidate):
        if root_removed:
            shutil.rmtree(candidate.path)
            raise RuntimeError("content inspection exceeded its budget")
        raise FileNotFoundError("another process removed a child")

    monkeypatch.setattr(cleaner, "_measure", vanished)
    assert cleaner.run() == 0
    assert (repo / "target").exists() is not root_removed
    assert cleaner.entries[0]["reason"] == "changed during inspection"
    assert cleaner.removed_bytes == 0


def test_project_owned_state_preserves_its_output_owner(cleaner, monkeypatch) -> None:
    repo = _build_fixture(cleaner, 3000)
    reserved = cleaner.home / "project-state"
    cleaner.usage_labels[reserved] = ("project_owned", "code/development")
    process = cleanup.Process(999999, 1, os.getuid(), "fixture", "worker")
    activity = cleanup.Activity(
        {process.pid: process},
        (
            cleanup.OpenPath(process.pid, "txt", repo / "target/debug/output"),
            cleanup.OpenPath(process.pid, "3u", reserved / "experiment.db"),
        ),
    )
    monkeypatch.setattr(cleaner, "_activity", lambda: activity)
    assert cleaner.run() == 1
    assert (repo / "target/debug/output").exists()
    assert cleaner.stopped == []
    assert any("project-owned/deferred" in failure for failure in cleaner.failures)


def test_discovery_preserves_dirty_source_without_running_content_filters(cleaner) -> None:
    repo = _build_fixture(cleaner, 300)
    source = repo / "source.txt"
    source.write_text("original source\n")
    _git(repo, "add", "source.txt")
    _git(repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "base")
    (repo / ".gitattributes").write_text("source.txt filter=unavailable\n")
    _git(repo, "config", "filter.unavailable.clean", "false")
    _git(repo, "config", "filter.unavailable.required", "true")
    source.write_text("modified source\n")

    assert cleaner.run() == 0
    assert not (repo / "target").exists()
    assert source.read_text() == "modified source\n"


def test_ignored_data_is_not_walked_but_registered_nested_worktrees_are(
    cleaner, monkeypatch
) -> None:
    repo = _build_fixture(cleaner, 300)
    (repo / ".gitignore").write_text("target/\ncache/\nwork/\n")
    _git(repo, "add", ".gitignore")
    _git(repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "base")
    nested = repo / "work/nested"
    _git(repo, "worktree", "add", "-qb", "nested", str(nested))
    _write(nested / "target/output", 3)
    _age_tree(nested / "target", 3)
    for number in range(30):
        (repo / f"cache/{number}/data").mkdir(parents=True)
    monkeypatch.setattr(cleanup, "DISCOVERY_MAX_ENTRIES", 20)
    assert cleaner.run() == 0
    assert not (nested / "target").exists()
    assert not (repo / "target").exists()
    assert (repo / "cache/0/data").exists()


@pytest.mark.parametrize("nested_name", ["independent", "target"])
def test_discovery_inherits_checkout_above_source_root_and_switches_at_nested_repo(
    cleaner,
    nested_name,
) -> None:
    repo = _build_fixture(cleaner, 300)
    cleaner.src = repo / "packages"
    outer = cleaner.src / "one/two/target/output"
    nested = cleaner.src / nested_name
    _write(outer, 3)
    nested.mkdir()
    _git(nested, "init", "-q")
    (nested / ".gitignore").write_text("node_modules/\n")
    inner = nested / "one/two/node_modules/output"
    _write(inner, 3)
    unignored = nested / "target/source"
    _write(unignored, 3)

    candidates = {candidate.path: candidate for candidate in cleaner._builds()}
    assert candidates[outer.parent].checkout == repo
    assert candidates[inner.parent].checkout == nested
    assert cleaner._reason(candidates[unignored.parent], 0, _activity()) == "not ignored"
    assert not cleaner.failures


@pytest.mark.parametrize("owner_kind", ["chrome", "self", "foreign"])
def test_protected_process_owner_prevents_reclamation(cleaner, monkeypatch, owner_kind) -> None:
    repo = _build_fixture(cleaner, 3000)
    process = cleanup.Process(
        os.getpid() if owner_kind == "self" else 999999,
        1,
        os.getuid() + (1 if owner_kind == "foreign" else 0),
        "fixture",
        "Google Chrome" if owner_kind == "chrome" else "worker",
    )
    activity = cleanup.Activity(
        {process.pid: process},
        (cleanup.OpenPath(process.pid, "txt", repo / "target/debug/output"),),
    )
    monkeypatch.setattr(cleaner, "_activity", lambda: activity)
    assert cleaner.run() == 1
    assert (repo / "target/debug/output").exists()
    assert cleaner.stopped == []
    assert any(
        "Chrome owner" in failure or "cannot verify a stoppable owner" in failure
        for failure in cleaner.failures
    )


def test_activity_resolves_shared_symlink_parent_and_leaf(cleaner, monkeypatch) -> None:
    actual = cleaner.home / "actual"
    actual.mkdir()
    (actual / "output").write_text("generated")
    alias = cleaner.home / "alias"
    alias.symlink_to(actual)
    leaf = cleaner.home / "leaf"
    leaf.symlink_to(actual / "output")
    monkeypatch.delattr(cleaner, "_activity")
    process = cleanup.Process(123, 1, os.getuid(), "fixture", "worker")
    monkeypatch.setattr(cleaner, "_processes", lambda: {123: process})
    monkeypatch.setattr(
        cleaner,
        "_command",
        lambda *args, **kwargs: f"p123\nf3u\nn{alias / 'output'}\nf4u\nn{leaf}\nfcwd\nn/\n",
    )
    activity = cleaner._activity()
    assert activity.owners(cleanup.Candidate(actual, "build")) == {123}
    assert [file.path for file in activity.files] == [
        actual / "output",
        actual / "output",
        Path("/"),
    ]


def test_usage_history_respects_byte_limit_and_preserves_previous_on_oversize(
    cleaner, monkeypatch
) -> None:
    monkeypatch.setattr(cleanup, "HISTORY_BYTES", 400)
    for number in range(10):
        cleaner._record_usage({"at": number, "roots": [{"path": "/synthetic", "bytes": number}]})
    target = cleaner.state / "usage.json"
    before = target.read_bytes()
    assert len(before) <= 400
    assert json.loads(before)[-1]["roots"][0]["delta_bytes"] == 1
    with pytest.raises(RuntimeError, match="snapshot exceeds"):
        cleaner._record_usage({"at": 11, "roots": [{"path": "/" + "x" * 500, "bytes": 10}]})
    assert target.read_bytes() == before


def test_unmarked_demo_history_is_reported_and_preserved(cleaner) -> None:
    demo = cleaner.home / ".lf-machine/demos/retained"
    _write(demo / "history.jsonl", 40)
    _write(demo / "store.db", 40)
    _age_tree(demo, 40)
    assert cleaner.run() == 0
    assert (demo / "history.jsonl").exists()
    assert (demo / "store.db").exists()
    assert any(
        entry.get("reason") == "unmarked demo; preserve source and history"
        for entry in cleaner.entries
    )


def test_busy_uv_cache_is_retained_without_forcing_its_lock(cleaner) -> None:
    repo = _build_fixture(cleaner, 300)
    (cleaner.home / ".local/bin/uv").write_text(
        '#!/bin/sh\nif [ "$2" = dir ]; then echo "$HOME/.cache/uv"; exit 0; fi\n'
        "echo 'Cache is currently in-use' >&2\n"
        "echo 'Timeout (0s) when waiting for lock' >&2\nexit 2\n"
    )
    assert cleaner.run() == 0
    assert not (repo / "target").exists()
    receipt = json.loads(cleaner.log.with_suffix(".json").read_text())
    assert receipt["maintenance"][0]["status"] == "retained"
    assert "currently in-use" in receipt["maintenance"][0]["reason"]


@pytest.mark.parametrize(
    ("outcome", "age", "removed"),
    [
        ("passed", 0.5, False),
        ("passed", 2, True),
        ("failed", 2, False),
        ("failed", 8, True),
        ("unknown", 2, False),
        ("unknown", 8, True),
        ("saved", 40, False),
    ],
)
def test_evidence_retention_follows_observed_outcome(cleaner, outcome, age, removed):
    root = next(iter(cleaner.temp_roots)) / "pytest-of-fixture/pytest-100"
    _write(root / "screenshot.png", age)
    marker = root / ".cleanup-evidence.json"
    marker.write_text(json.dumps({"saved": True} if outcome == "saved" else {"outcome": outcome}))
    _age(marker, age)
    _age_tree(root, age)
    assert cleaner.run() == 0
    assert root.exists() is not removed
    entry = next(entry for entry in cleaner.entries if entry["path"] == str(root))
    assert entry["evidence_outcome"] == outcome


def test_saved_nested_evidence_survives_cleanup(cleaner):
    root = next(iter(cleaner.temp_roots)) / "pytest-of-fixture/pytest-100"
    _write(root / "case/.cleanup-keep", 40)
    _write(root / "case/screenshot.png", 40)
    _age_tree(root, 40)
    assert cleaner.run() == 0
    assert (root / "case/screenshot.png").exists()
    assert any(entry["reason"] == "explicitly saved evidence" for entry in cleaner.entries)


@pytest.mark.parametrize("passed", [True, False])
def test_pytest_records_its_actual_outcome(tmp_path, passed):
    suite = tmp_path / "suite"
    suite.mkdir()
    shutil.copyfile(Path(__file__).with_name("conftest.py"), suite / "conftest.py")
    (suite / "test_example.py").write_text(
        'def test_example(tmp_path):\n    (tmp_path / "evidence").write_text("proof")\n'
        f"    assert {passed}\n"
    )
    artifacts = tmp_path / "artifacts"
    env = {key: value for key, value in os.environ.items() if not key.startswith("LF_")}
    env["PYTHONPATH"] = str(SCRIPT.parent)
    result = subprocess.run(
        [sys.executable, "-m", "pytest", "-q", "--basetemp", str(artifacts), str(suite)],
        env=env,
        cwd=tmp_path,
        capture_output=True,
        text=True,
        stdin=subprocess.DEVNULL,
        timeout=30,
    )
    assert result.returncode == (0 if passed else 1), result.stdout + result.stderr
    assert cleanup.evidence_outcome(artifacts) == ("passed" if passed else "failed")


def test_uv_cache_sentinel_is_not_mistaken_for_a_checkout(cleaner):
    repo = _build_fixture(cleaner, 300)
    cache = repo / ".uv-cache"
    _write(cache / "CACHEDIR.TAG", 40)
    _write(cache / "sdists-v9/.git", 40)
    candidates = cleaner._builds()
    assert cache in cleaner.uv_caches
    assert any(candidate.path == repo / "target" for candidate in candidates)
    assert not cleaner.failures


def test_native_uv_cap_reclaims_unused_downloads_and_preserves_reserved_cache(cleaner, monkeypatch):
    cache = cleaner.home / ".cache/uv"
    reserved = cleaner.home / "reserved-cache"
    _write(cache / "download", 40)
    _write(reserved / "download", 40)
    cleaner.uv_caches.add(reserved)
    cleaner.usage_labels[reserved] = ("project_owned", "code/development")
    cleaner.cache_caps["uv_bytes"] = 1
    original = cleaner._command

    def command(args, **kwargs):
        if args[:3] == ["uv", "cache", "dir"]:
            return str(cache)
        if args[:3] == ["uv", "cache", "prune"]:
            return ""
        if args[:3] == ["uv", "cache", "clean"]:
            shutil.rmtree(Path(args[4]))
            return ""
        return original(args, **kwargs)

    monkeypatch.setattr(cleaner, "_command", command)
    cleaner._prune_uv()
    assert not cache.exists()
    assert (reserved / "download").exists()
    assert not cleaner.failures
    assert any(entry.get("bytes_after") == 0 for entry in cleaner.maintenance)


def test_docker_reclaims_only_old_reproducible_unused_images(cleaner, monkeypatch):
    images = {"age-unknown", "downloaded", "local", "used", "recent"}
    build_cache = ["old-cache"]

    def command(args, **kwargs):
        if args[1:3] == ["context", "show"]:
            return "desktop-linux"
        if args[1:3] == ["context", "inspect"]:
            return json.dumps("unix:///fixture/docker.sock")
        assert args[:3] == ["docker", "--context", "desktop-linux"]
        action = args[3:5]
        if action == ["buildx", "inspect"]:
            return "Driver: docker\nEndpoint: desktop-linux\n"
        if action == ["buildx", "prune"]:
            build_cache.clear()
            return ""
        if action == ["container", "ls"]:
            return "stopped-container"
        if action == ["container", "inspect"]:
            return "used"
        if action == ["image", "ls"]:
            return "\n".join(images)
        if action == ["image", "inspect"]:
            identifier = args[-1]
            created = (
                cleanup.datetime.now(cleanup.timezone.utc).isoformat()
                if identifier == "recent"
                else "2020-01-01T00:00:00.123456789Z"
            )
            metadata = {
                "Id": identifier,
                "RepoDigests": [] if identifier == "local" else ["registry@sha256:fixture"],
            }
            if identifier != "age-unknown":
                metadata["Created"] = created
            return json.dumps(metadata)
        if action == ["image", "rm"]:
            assert "--force" not in args
            images.remove(args[-1])
            return ""
        raise AssertionError(f"Unapproved Docker operation: {args}")

    monkeypatch.setattr(cleaner, "_command", command)
    cleaner._prune_docker()
    assert images == {"age-unknown", "local", "used", "recent"}
    assert {
        "step": "docker image",
        "image": "age-unknown",
        "status": "retained",
        "reason": "unknown creation time",
    } in cleaner.maintenance
    assert not build_cache
    assert not cleaner.failures


def test_remote_docker_context_is_observation_only(cleaner, monkeypatch):
    def command(args, **kwargs):
        if args[1:3] == ["context", "show"]:
            return "remote"
        if args[1:3] == ["context", "inspect"]:
            return json.dumps("ssh://fixture")
        raise AssertionError("Remote engine must not be changed")

    monkeypatch.setattr(cleaner, "_command", command)
    cleaner._prune_docker()
    assert cleaner.maintenance == [
        {"step": "docker", "status": "retained", "reason": "not a local Unix engine"}
    ]


@pytest.mark.parametrize("name", [".venv", "node_modules"])
def test_in_use_dependencies_survive_even_after_a_day(cleaner, name):
    root = cleaner.src / "repo" / name
    _write(root / "executable", 40)
    candidate = cleanup.Candidate(root, "build")
    activity = _activity({root / "executable"})
    assert cleaner._reason(candidate, time.time() - 40 * 86400, activity) == "held open"
    assert (root / "executable").exists()


def test_usage_rotates_to_previously_unmeasured_repositories(cleaner, monkeypatch):
    early = cleaner.src / "aaa"
    late = cleaner.src / "zzz"
    _write(early / "source", 0)
    _write(late / "source", 0)
    snapshot = cleaner._usage()
    for entry in snapshot["roots"]:
        if entry["path"] == str(late):
            entry.update(bytes=None, error="usage sampling budget exhausted")
    cleaner._record_usage(snapshot)
    now = [time.time()]
    monkeypatch.setattr(cleanup.time, "time", lambda: now[0])

    def measure(args, **kwargs):
        now[0] += 121
        return "42 fixture"

    monkeypatch.setattr(cleaner, "_command", measure)
    sample = {entry["path"]: entry for entry in cleaner._usage()["roots"]}
    assert sample[str(late)]["bytes"] == 42 * 1024
    assert sample[str(early)]["bytes"] is None
    assert sample[str(early)]["error"] == "usage sampling budget exhausted"


def test_growth_uses_last_known_measurement_across_unknown_samples(cleaner):
    path = str(cleaner.src / "repo")
    for at, value in [(1, 10), (2, None), (3, 30)]:
        cleaner._record_usage({"at": at, "roots": [{"path": path, "bytes": value}]})
    history = json.loads((cleaner.state / "usage.json").read_text())
    assert history[1]["roots"][0]["delta_bytes"] is None
    assert history[2]["roots"][0]["delta_bytes"] == 20
    assert history[2]["roots"][0]["previous_at"] == 1


def test_reserved_docker_storage_is_not_pruned(cleaner, monkeypatch):
    cleaner.usage_labels[cleaner.home / "Library/Containers/com.docker.docker"] = (
        "project_owned",
        "code/development",
    )

    def command(*args, **kwargs):
        raise AssertionError("Reserved storage must not be changed")

    monkeypatch.setattr(cleaner, "_command", command)
    cleaner._prune_docker()
    assert cleaner.maintenance[0]["reason"] == "explicit reservation"
