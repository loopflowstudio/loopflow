import importlib
import json
import os
import plistlib
import shlex
import shutil
import subprocess
import sys
import time
from pathlib import Path
from types import SimpleNamespace

import pytest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "disk-cleanup.sh"
sys.path.insert(0, str(SCRIPT.parent))
cleanup = importlib.import_module("disk_cleanup")

pytestmark = pytest.mark.skipif(sys.platform != "darwin", reason="macOS maintenance script")


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
    for tool, body in {"lf": "exit 0", "uv": "exit 0", "xcrun": "exit 0", **(stubs or {})}.items():
        stub = bin_dir / tool
        stub.write_text(f"#!/bin/sh\n{body}\n")
        stub.chmod(0o755)
    public = tmp_path / "home/.local/bin"
    public.mkdir(parents=True, exist_ok=True)
    (public / "lf").unlink(missing_ok=True)
    (public / "lf").symlink_to(bin_dir / "lf")
    env = {
        "HOME": str(tmp_path / "home"),
        "LF_HOME": str(tmp_path / "home/.lf"),
        "LOOPFLOW_SRC": str(tmp_path / "home/src"),
        "TMPDIR": str(tmp_path),
        "PATH": f"{bin_dir}:/usr/bin:/bin:/usr/sbin:/sbin",
    }
    # Redirect only the host-wide OS lock into this disposable fixture. The
    # runner and flock behavior are unchanged; concurrent real recovery cannot
    # make preservation assertions pass merely because cleanup skipped.
    launcher = (
        "import sys; from pathlib import Path; "
        f"sys.path.insert(0, {str(SCRIPT.parent)!r}); "
        "import resource_envelope, disk_cleanup; "
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


def test_home_rotation_removes_week_old_material_and_keeps_the_store(tmp_path: Path) -> None:
    lf = tmp_path / "home/.lf"
    _write(lf / "loopflow.db", 30)
    _write(lf / "loopflow.db-wal", 30)
    _write(lf / "accounts/codex/a/sessions/old.jsonl", 60)
    _write(lf / "loopflow.db.backup-0.14.7-0123456789abcdef", 20)
    _write(lf / "loopflow.db.backup-0.14.7-0123456789abcdef-shm", 20)
    _write(lf / "loopflow.db.backup-0.14.8-fedcba9876543210", 10)
    _write(lf / "runs/ab/oldest/events.jsonl", 40)
    _write(lf / "runs/ef/fresh/events.jsonl", 0)
    _write(lf / "logs/old.log", 30)
    _write(lf / "logs/held.log", 30)
    _write(lf / "copies/.tmpAbC123/loopflow.db", 3)
    _age_tree(lf / "runs/ab", 40)

    with open(lf / "logs/held.log", "a"):
        out = _run(tmp_path)

    for gone in (
        "runs/ab/oldest",
        "logs/old.log",
        "loopflow.db.backup-0.14.7-0123456789abcdef",
        "loopflow.db.backup-0.14.7-0123456789abcdef-shm",
    ):
        assert not (lf / gone).exists(), f"{gone} survived:\n{out}"
    for kept in (
        "loopflow.db",
        "loopflow.db-wal",
        "accounts/codex/a/sessions/old.jsonl",
        "loopflow.db.backup-0.14.8-fedcba9876543210",
        "runs/ef/fresh/events.jsonl",
        "logs/held.log",
        "copies/.tmpAbC123",
    ):
        assert (lf / kept).exists(), f"{kept} was removed:\n{out}"
    assert "removed captures" in (lf / "logs/disk.log").read_text()


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
    assert "== repository kata" in out


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


@pytest.mark.parametrize("held", [False, True])
def test_backup_with_recent_or_open_sidecar_is_kept(tmp_path: Path, held: bool) -> None:
    lf = tmp_path / "home/.lf"
    old = lf / "loopflow.db.backup-old"
    sidecar = Path(f"{old}-wal")
    _write(old, 30)
    _write(sidecar, 30 if held else 0)
    _write(lf / "loopflow.db.backup-new", 10)

    # A read-only handle leaves the sidecar's old timestamp intact.
    with sidecar.open("rb") if held else open(os.devnull, "rb"):
        out = _run(tmp_path)

    assert old.exists(), out
    assert sidecar.exists(), out


def test_cache_and_trash_keep_open_or_recent_contents_and_log_removals(tmp_path: Path) -> None:
    home = tmp_path / "home"
    for root in (home / "Library/Caches", home / ".Trash"):
        for name, age in (("old", 40), ("held", 40), ("recent", 0)):
            _write(root / name / "data", age)
            _age_tree(root / name, 40)

    with (home / "Library/Caches/held/data").open("rb"), (home / ".Trash/held/data").open("rb"):
        out = _run(tmp_path)

    for root in (home / "Library/Caches", home / ".Trash"):
        assert not (root / "old").exists(), out
        assert (root / "held/data").exists(), out
        assert (root / "recent/data").exists(), out
    log = (home / ".lf/logs/disk.log").read_text()
    assert "removed cache" in log
    assert "removed trash" in log


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


def test_real_cleanup_preserves_unpublished_work_and_removes_finished_checkouts(
    tmp_path: Path,
) -> None:
    root = SCRIPT.parent.parent
    build_env = {key: value for key, value in os.environ.items() if not key.startswith("LF_")}
    subprocess.run(
        ["cargo", "build", "-p", "loopflow", "--bin", "lf"],
        cwd=root,
        env=build_env,
        stdin=subprocess.DEVNULL,
        check=True,
    )
    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=root, env=build_env
        )
    )
    binary = Path(metadata["target_directory"]) / "debug/lf"

    # HOME does not redirect getpwuid_r. Interpose only that external account
    # lookup in the fixture CLI so production registry/installation reads also
    # resolve to this disposable Home; the production executable is unchanged.
    source = tmp_path / "account.c"
    source.write_text("""
#include <pwd.h>
#include <stdlib.h>
static int fixture_getpwuid_r(uid_t uid, struct passwd *pwd, char *buf,
                             size_t size, struct passwd **result) {
    int status = getpwuid_r(uid, pwd, buf, size, result);
    if (status == 0 && *result) pwd->pw_dir = getenv("LOOPFLOW_TEST_ACCOUNT_HOME");
    return status;
}
__attribute__((used)) static struct { const void *replacement; const void *original; }
interpose __attribute__((section("__DATA,__interpose"))) = {
    (const void *)fixture_getpwuid_r, (const void *)getpwuid_r
};
""")
    library = tmp_path / "account.dylib"
    subprocess.run(["cc", "-dynamiclib", str(source), "-o", str(library)], check=True)
    home = tmp_path / "home"
    (tmp_path / "account").mkdir()
    repo = home / "src/kata"
    repo.mkdir(parents=True)
    bare = tmp_path / "origin.git"
    env = {
        "HOME": str(home),
        "LF_HOME": str(home / ".lf"),
        "LF_BIN": str(binary),
        "LOOPFLOW_TEST_ACCOUNT_HOME": str(tmp_path / "account"),
        "DYLD_INSERT_LIBRARIES": str(library),
        "PATH": f"{tmp_path / 'bin'}:/usr/bin:/bin:/usr/sbin:/sbin",
        "GIT_ALLOW_PROTOCOL": "file",
        "GIT_TERMINAL_PROMPT": "0",
    }
    _git(repo, "init", "-qb", "main", env=env)
    _git(repo, "config", "user.name", "Cleanup Fixture", env=env)
    _git(repo, "config", "user.email", "fixture@example.com", env=env)
    (repo / ".gitignore").write_text("target/\n")
    _git(repo, "add", ".", env=env)
    _git(repo, "commit", "-qm", "base", env=env)
    _git(repo, "init", "--bare", str(bare), env=env)
    _git(bare, "symbolic-ref", "HEAD", "refs/heads/main", env=env)
    _git(repo, "remote", "add", "origin", "https://github.com/fixture/kata.git", env=env)
    _git(repo, "config", f"url.{bare}.insteadOf", "https://github.com/fixture/kata.git", env=env)
    _git(repo, "push", "-u", "origin", "main", env=env)
    pulls = {}
    kept = {}
    finished = []
    for branch in ["unpushed", "old-unpushed", "merged", "closed", "moved", "dirty"]:
        path = repo.with_name(f"kata.{branch}")
        _git(repo, "worktree", "add", "-b", branch, str(path), "main", env=env)
        (path / "work.txt").write_text(branch)
        _git(path, "add", ".", env=env)
        date_env = (
            {
                **env,
                "GIT_AUTHOR_DATE": "2020-01-01T00:00:00Z",
                "GIT_COMMITTER_DATE": "2020-01-01T00:00:00Z",
            }
            if branch == "old-unpushed"
            else env
        )
        _git(path, "commit", "-qm", branch, env=date_env)
        head = _git(path, "rev-parse", "HEAD", env=env)
        if branch in {"merged", "closed", "moved", "dirty"}:
            pulls[branch] = [
                {
                    "number": len(pulls) + 1,
                    "headRefOid": head,
                    "state": "CLOSED" if branch == "closed" else "MERGED",
                }
            ]
        if branch == "merged":
            _git(repo, "merge", "--no-edit", branch, env=env)
            _git(repo, "push", "origin", "main", env=env)
        if branch == "moved":
            (path / "work.txt").write_text("unpublished follow-up")
            _git(path, "commit", "-am", "follow-up", env=env)
        if branch == "dirty":
            (path / "notes.txt").write_text("unfinished notes")
        if branch == "old-unpushed":
            # No Task or PR and idle for years: removed, its commit saved first.
            saved = (branch, _git(path, "rev-parse", "HEAD", env=env))
        if branch in {"merged", "closed", "old-unpushed"}:
            finished.append((branch, path))
        else:
            kept[branch] = (
                path,
                _git(path, "rev-parse", "HEAD", env=env),
                (path / "work.txt").read_bytes(),
            )
    gh = rf"""#!{sys.executable}
import json, re, sys
pulls = {pulls!r}
if sys.argv[1:3] == ["api", "rate_limit"]:
    budget = {{"limit": 5000, "remaining": 4900, "reset": 2000000000}}
    print(json.dumps({{"resources": {{"graphql": budget}}}}))
else:
    query = next(arg[6:] for arg in sys.argv if arg.startswith("query="))
    repo = {{}}
    pattern = r'b(\d+): pullRequests\(first: 100, headRefName: "([^"]+)"'
    for index, branch in re.findall(pattern, query):
        repo["b" + index] = {{"nodes": pulls.get(branch, [])}}
        repo["r" + index] = None
    print(json.dumps({{"data": {{"repository": repo}}}}))
"""
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    (bin_dir / "gh").write_text(gh)
    (bin_dir / "gh").chmod(0o755)
    dry = subprocess.run(
        [str(binary), "wt", "prune", "--dry-run"],
        cwd=repo,
        env=env,
        capture_output=True,
        text=True,
        check=True,
    )
    assert "merged" in dry.stdout and "closed" in dry.stdout
    assert all(path.exists() for _, path in finished)

    # Set dyld variables at exec: macOS strips them while starting system shells.
    lf = "exec env " + " ".join(
        shlex.quote(f"{key}={env[key]}")
        for key in [
            "LF_BIN",
            "DYLD_INSERT_LIBRARIES",
            "LOOPFLOW_TEST_ACCOUNT_HOME",
            "GIT_ALLOW_PROTOCOL",
        ]
    )
    lf += f' {shlex.quote(str(binary))} "$@"'
    out = _run(tmp_path, {"lf": lf})
    for branch, path in finished:
        assert not path.exists(), out
        assert not _git(repo, "branch", "--list", branch, env=env), out
    for branch, (path, head, content) in kept.items():
        assert (path / "work.txt").read_bytes() == content, out
        assert _git(path, "rev-parse", "HEAD", env=env) == head, out
        assert _git(repo, "rev-parse", branch, env=env) == head, out
    assert (kept["dirty"][0] / "notes.txt").read_text() == "unfinished notes"
    saved_refs = _git(
        repo, "for-each-ref", "--format=%(objectname)", f"refs/lf/pruned/{saved[0]}", env=env
    )
    assert saved_refs == saved[1], out
    again = _run(tmp_path, {"lf": lf})
    assert "0 removed," in again, again


@pytest.fixture
def cleaner(tmp_path: Path, monkeypatch):
    home = tmp_path / "home"
    home.mkdir()
    monkeypatch.setenv("HOME", str(home))
    monkeypatch.setenv("LF_HOME", str(home / ".lf"))
    monkeypatch.setenv("LOOPFLOW_SRC", str(home / "src"))
    public = home / ".local/bin"
    public.mkdir(parents=True)
    for name in ("lf", "uv", "xcrun"):
        tool = public / name
        tool.write_text("#!/bin/sh\nexit 0\n")
        tool.chmod(0o755)
    monkeypatch.setenv("PATH", f"{public}:/usr/bin:/bin:/usr/sbin:/sbin")
    runner = cleanup.Cleaner(False, 1800)
    monkeypatch.setattr(runner, "_activity", lambda: cleanup.Activity(frozenset(), frozenset()))
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
        monkeypatch.setattr(
            cleaner, "_activity", lambda: cleanup.Activity(frozenset({output}), frozenset())
        )
    elif protection == "builder":
        monkeypatch.setattr(
            cleaner, "_activity", lambda: cleanup.Activity(frozenset(), frozenset({worktree}))
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
        _write(derived / name / "Build/output", 3, 1024 * 1024)
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
    _write(live, 5)
    _age_tree(repo / "target", 5)
    source = repo / "unpublished.rs"
    source.write_text("unfinished work")
    monkeypatch.setattr(
        cleaner, "_activity", lambda: cleanup.Activity(frozenset({live}), frozenset())
    )
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
        monkeypatch.setattr(
            cleaner, "_activity", lambda: cleanup.Activity(frozenset(), frozenset({repo}))
        )
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
    cleaner.lf.write_text("#!/bin/sh\nexit 99\n")
    cleaner.dry_run = True
    assert cleaner.run() == 0
    assert (repo / "target/debug/output").exists()
    assert any(entry["status"] == "eligible" for entry in cleaner.entries)
    assert cleaner.removed_bytes == 0


def test_partial_native_failure_is_nonzero_after_reclamation(cleaner) -> None:
    repo = _build_fixture(cleaner, 300)
    cleaner.lf.write_text("#!/bin/sh\necho broken >&2\nexit 7\n")
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


def test_public_cli_wins_over_stale_inherited_selection(cleaner, monkeypatch) -> None:
    monkeypatch.setenv("LF_BIN", "/Applications/Stale.app/Contents/MacOS/lf")
    monkeypatch.setenv("LF_AGENT_CALLER", "stale")
    cleaner.lf.write_text('#!/bin/sh\n[ -z "$LF_BIN$LF_AGENT_CALLER" ] || exit 42\n')
    runner = cleanup.Cleaner(False, 1800)
    monkeypatch.setattr(runner, "_activity", lambda: cleanup.Activity(frozenset(), frozenset()))
    assert runner.run() == 0


def test_child_deadline_kills_its_process_group(cleaner) -> None:
    marker = cleaner.home / "child-finished"
    start = time.time()
    with pytest.raises(subprocess.TimeoutExpired):
        cleaner._command(
            ["/bin/sh", "-c", f"(sleep 1; touch {shlex.quote(str(marker))}) & wait"], timeout=0.1
        )
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
    assert cleaner._activity().builders == frozenset({repo})


@pytest.fixture
def schedule(cleaner, monkeypatch):
    main = cleaner.src / "loopflow"
    scripts = main / "scripts"
    scripts.mkdir(parents=True)
    _git(main, "init", "-q")
    for name in (
        "disk-cleanup.sh",
        "disk_cleanup.py",
        "resource_envelope.py",
        "performance-budgets.json",
    ):
        shutil.copyfile(SCRIPT.parent / name, scripts / name)
    monkeypatch.setattr(cleanup, "__file__", str(scripts / "disk_cleanup.py"))
    public = cleaner.lf.parent
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
    assert args[-3:] == ["/bin/bash", str(cleaner.src / "loopflow/scripts/disk-cleanup.sh"), "run"]
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
        cleaner, "_activity", lambda: cleanup.Activity(frozenset(), frozenset({Path("/")}))
    )
    assert cleaner.run() == 0
    assert not (repo / "target").exists()


def test_output_opened_after_inventory_is_retained(cleaner, monkeypatch) -> None:
    repo = _build_fixture(cleaner, 300)
    artifact = repo / "target/debug/output"
    snapshots = iter(
        [
            cleanup.Activity(frozenset(), frozenset()),
            cleanup.Activity(frozenset({artifact}), frozenset()),
        ]
    )
    monkeypatch.setattr(cleaner, "_activity", lambda: next(snapshots))
    assert cleaner.run() == 0
    assert artifact.exists()


def test_launchd_bootstrap_failure_restores_existing_job(schedule, cleaner) -> None:
    assert cleanup._schedule("install") == 0
    original = schedule.read_bytes()
    (cleaner.home / "cron").write_text("17 * * * * old # loopflow:disk-cleanup\n")
    launchctl = cleaner.lf.parent / "launchctl"
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


def test_newer_backup_directory_or_symlink_cannot_displace_newest_backup(cleaner) -> None:
    backup = cleaner.lf_home / "loopflow.db.backup-current"
    _write(backup, 30)
    (cleaner.lf_home / "loopflow.db.backup-directory").mkdir()
    (cleaner.lf_home / "loopflow.db.backup-link").symlink_to(backup)
    assert cleaner.run() == 0
    assert backup.exists()
