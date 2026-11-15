"""Prove locator enrollment from a populated 0.14.3 installation in a container."""

import hashlib
import json
import os
import platform
import sqlite3
import subprocess
import sys
import tarfile
import urllib.request
from pathlib import Path
from urllib.parse import urlparse

PRIOR = Path("/tmp/lf-repository-prior")


def _fetch_prior() -> None:
    archive = f"lf-{platform.machine()}-unknown-linux-gnu.tar.gz"
    base = "https://github.com/loopflowstudio/loopflow/releases/download/v0.14.3"
    PRIOR.mkdir(exist_ok=True)
    for name in [archive, "SHA256SUMS"]:
        with urllib.request.urlopen(f"{base}/{name}", timeout=120) as response:
            (PRIOR / name).write_bytes(response.read())
    sums = dict(line.split()[::-1] for line in (PRIOR / "SHA256SUMS").read_text().splitlines())
    assert hashlib.sha256((PRIOR / archive).read_bytes()).hexdigest() == sums[archive]
    with tarfile.open(PRIOR / archive) as package:
        member = package.getmember("lf")
        assert member.isfile()
        package.extract(member, PRIOR, filter="data")


def _run(*args: str, cwd: Path | None = None) -> str:
    result = subprocess.run(
        args, cwd=cwd, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=180
    )
    assert result.returncode == 0, result.stdout + result.stderr
    return result.stdout


def main() -> None:
    assert Path("/.dockerenv").exists() and os.getuid() == 0, "requires disposable container"
    if sys.argv[1] == "--fetch-prior":
        _fetch_prior()
        return
    candidate = str(Path(sys.argv[1]).resolve())
    user = "lf-repository-upgrade"
    _run("useradd", "--create-home", user)
    home = Path("/home") / user
    repo = home / "retained"
    repo.mkdir()
    _run("chown", "-R", f"{user}:{user}", str(home))

    def invoke(binary: str, *args: str, cwd: Path = repo) -> str:
        return _run(
            "runuser",
            "-u",
            user,
            "--",
            "env",
            "-i",
            f"HOME={home}",
            f"PATH={home}/.local/bin:/usr/bin:/bin",
            binary,
            *args,
            cwd=cwd,
        )

    def git(*args: str) -> str:
        return invoke("/usr/bin/git", *args)

    git("init", "-q")
    git(
        "-c",
        "user.name=Proof",
        "-c",
        "user.email=proof@example.invalid",
        "commit",
        "--allow-empty",
        "-qm",
        "Initial",
    )
    old = str(PRIOR / "lf")
    entry = str(home / ".local/bin/lf")
    invoke(old, "self", "install", "promote", "--cli-target", entry)
    assert invoke(entry, "--version").strip() == "lf 0.14.3"
    task = json.loads(invoke(entry, "task", "create", "--title", "Upgrade continuity", "--json"))
    placed = json.loads(invoke(entry, "task", "checkout", task["id"], "--json"))
    checkout = Path(placed["worktree"])
    session = json.loads(invoke(entry, "session", "ensure", "--task", task["id"], "--json"))
    identity = json.loads(invoke(entry, "repo", "identity", "--json"))
    unpublished = checkout / "unpublished.txt"
    unpublished.write_text("retained edit\n")
    git("remote", "add", "origin", "https://example.invalid/retained.git")
    # 0.14.3 has no locator writer: this is a populated released store, not a
    # fresh current-schema fixture with its locator artificially removed.
    assert not list((repo / ".git").glob("loopflow/*/loopflow-repository"))
    with sqlite3.connect(home / ".lf/loopflow.db") as db:
        before = db.execute("SELECT id,trace_id,cwd FROM processes ORDER BY id").fetchall()
    invoke(candidate, "self", "install", "promote", "--cli-target", entry)
    for cwd in [repo, checkout]:
        assert json.loads(invoke(entry, "config", "repository", "--json", cwd=cwd)) == identity
        after = json.loads(invoke(entry, "desktop", "task", task["id"], "--json", cwd=cwd))
        assert after["task"]["id"] == task["id"]
        assert after["execution"]["worktree"] == str(checkout)
        invoke(entry, "session", "list", "--json", cwd=cwd)
        link = json.loads(
            invoke(entry, "--task", task["id"], "open", "--link", "--json", cwd=cwd)
        )[0]["url"]
        assert urlparse(link).path.startswith("/task/")
        # The upgraded store resolves the Work address back to the same Work.
        again = json.loads(invoke(entry, "open", link, "--link", "--json", cwd=cwd))
        assert again[0]["url"] == link
    resumed = json.loads(invoke(entry, "resume", "--prepare", "--task", task["id"], "--json"))
    for field in ["id", "workspace", "work", "provider", "title", "cwd"]:
        assert resumed[field] == session[field]
    assert unpublished.read_text() == "retained edit\n"
    with sqlite3.connect(home / ".lf/loopflow.db") as db:
        after = dict(
            (row[0], row[1:]) for row in db.execute("SELECT id,trace_id,cwd FROM processes")
        )
    assert all(after[row[0]] == row[1:] for row in before)
    moved = home / "renamed"
    repo.rename(moved)
    assert json.loads(invoke(entry, "config", "repository", "--json", cwd=moved)) == identity
    after_move = json.loads(invoke(entry, "desktop", "task", task["id"], "--json", cwd=moved))
    assert after_move["execution"]["worktree"] == str(checkout)
    assert unpublished.read_text() == "retained edit\n"
    # Promotion retry and ordinary reads retain the same identity and checkout.
    invoke(candidate, "self", "install", "promote", "--cli-target", entry, cwd=home)
    assert json.loads(invoke(entry, "config", "repository", "--json", cwd=moved)) == identity
    print("PASS: populated 0.14.3 promotion, Task/Session identity, history, edits, move and retry")
    print(
        "Native provider resume and macOS Desktop installation are separate acceptance boundaries."
    )


if __name__ == "__main__":
    main()
