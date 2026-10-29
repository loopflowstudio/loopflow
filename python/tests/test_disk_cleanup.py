import json
import os
import shlex
import subprocess
import sys
import time
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "disk-cleanup.sh"

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


def _run(tmp_path: Path, stubs: dict[str, str] | None = None, exit_code: int = 0) -> str:
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir(exist_ok=True)
    for tool, body in {"lf": "exit 0", "uv": "exit 0", "xcrun": "exit 0", **(stubs or {})}.items():
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
    result = subprocess.run(
        ["bash", str(SCRIPT)], env=env, capture_output=True, text=True, timeout=600
    )
    assert result.returncode == exit_code, result.stdout + result.stderr
    return result.stdout


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
    _write(recent / "target/debug/kata", 0.1)
    _age_tree(recent / "target", 0.1)

    out = _run(tmp_path)

    assert not (idle / "target").exists(), out
    assert (recent / "target").exists(), out
    assert "written in the last 4 hours" in out


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


def test_failed_modification_inspection_keeps_material(tmp_path: Path) -> None:
    cache = tmp_path / "home/Library/Caches/old/data"
    _write(cache, 40)
    _age_tree(cache.parent, 40)

    out = _run(
        tmp_path, {"find": 'case "$1" in */Caches/old) exit 1;; esac\nexec /usr/bin/find "$@"'}
    )

    assert cache.exists(), out
    assert "modification inspection failed" in out


def test_process_in_checkout_keeps_its_idle_build_output(tmp_path: Path) -> None:
    repo = tmp_path / "home/src/kata"
    repo.mkdir(parents=True)
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    (repo / ".gitignore").write_text("target/\n")
    _write(repo / "target/debug/kata", 3)
    _age_tree(repo / "target", 3)
    process = subprocess.Popen(["/bin/sleep", "600"], cwd=repo)
    try:
        out = _run(tmp_path)
        assert (repo / "target/debug/kata").exists(), out
        assert f"kept {repo} (held open)" in out
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
    assert "lf wt prune failed" not in out, out
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
