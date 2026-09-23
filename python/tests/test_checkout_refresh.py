"""CLI proofs with local remotes; package/release side effects use executables in PATH."""

import json
import os
import select
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

Checkout = tuple[Path, Path, dict[str, str]]

LF = Path(os.environ.get("LOOPFLOW_TEST_LF", "target/debug/lf")).resolve()


def _run(cwd: Path, *args: str, env: dict[str, str] | None = None) -> str:
    result = subprocess.run(args, cwd=cwd, env=env, capture_output=True, text=True)
    assert result.returncode == 0, result.stdout + result.stderr
    return result.stdout.strip()


def _commit(repo: Path, name: str, text: str) -> str:
    (repo / name).write_text(text)
    _run(repo, "git", "add", name)
    _run(repo, "git", "commit", "-m", name)
    return _run(repo, "git", "rev-parse", "HEAD")


@pytest.fixture
def checkout(tmp_path: Path) -> tuple[Path, Path, dict[str, str]]:
    if not LF.exists():
        pytest.skip("build lf first: cargo build -p loopflow --bin lf")
    remote = tmp_path / "origin.git"
    main = tmp_path / "repo"
    author = tmp_path / "author"
    _run(tmp_path, "git", "init", "--bare", "-b", "main", str(remote))
    _run(tmp_path, "git", "clone", str(remote), str(main))
    for repo in [main]:
        _run(repo, "git", "config", "user.name", "Refresh proof")
        _run(repo, "git", "config", "user.email", "proof@example.com")
    _commit(main, "base.txt", "base\n")
    _run(main, "git", "push", "-u", "origin", "main")
    _run(tmp_path, "git", "clone", str(remote), str(author))
    _run(author, "git", "config", "user.name", "Refresh proof")
    _run(author, "git", "config", "user.email", "proof@example.com")
    env = {
        key: value for key, value in os.environ.items() if not key.startswith(("LF_", "LOOPFLOW_"))
    }
    env["LF_HOME"] = str(tmp_path / "lf-home")
    env["LF_DB_PATH"] = str(tmp_path / "lf-home/store.db")
    env["GIT_CONFIG_NOSYSTEM"] = "1"
    return main, author, env


def _advance(author: Path, name: str) -> str:
    sha = _commit(author, name, name)
    _run(author, "git", "push", "origin", "main")
    return sha


def test_main_refresh_repeats_and_observes_each_new_upstream(checkout: Checkout) -> None:
    main, author, env = checkout
    for name in ["first.txt", "second.txt"]:
        upstream = _advance(author, name)
        _run(main, str(LF), "rebase", env=env)
        assert _run(main, "git", "rev-parse", "HEAD") == upstream
        _run(main, str(LF), "rebase", env=env)
        assert _run(main, "git", "rev-parse", "HEAD") == upstream


def test_explicit_main_target_uses_one_refresh_snapshot(checkout: Checkout, tmp_path: Path) -> None:
    main, author, env = checkout
    local = _commit(main, "local.txt", "unpublished\n")
    first = _advance(author, "first.txt")
    second = _commit(author, "second.txt", "arrives during refresh\n")
    (main / "local.txt").write_text("staged\n")
    _run(main, "git", "add", "local.txt")
    (main / "local.txt").write_text("working\n")
    (main / "notes.txt").write_text("untracked\n")
    binaries = tmp_path / "bin"
    binaries.mkdir()
    wrapper = binaries / "git"
    # Publish upstream and interrupt transport after a successful fetch. That
    # invocation already has its snapshot; a later invocation recovers transport.
    wrapper.write_text(
        '#!/bin/sh\ncommand="$1"\n'
        'if [ "$command" = -C ]; then command="$3"; fi\n'
        'if [ "$command" = fetch ] && [ -e "$FETCH_MARKER" ]; then\n'
        '  echo "fetch transport became unavailable" >&2; exit 7\n'
        'fi\n"$REAL_GIT" "$@" || exit $?\n'
        'if [ "$command" = fetch ] && [ ! -e "$FETCH_MARKER" ]; then\n'
        '  touch "$FETCH_MARKER"\n'
        '  "$REAL_GIT" -C "$AUTHOR_REPO" push origin main\n'
        "fi\n"
    )
    wrapper.chmod(0o755)
    env["REAL_GIT"] = shutil.which("git") or "git"
    env["AUTHOR_REPO"] = str(author)
    env["FETCH_MARKER"] = str(tmp_path / "published")
    env["PATH"] = f"{binaries}:{env['PATH']}"
    for expected in [first, second]:
        _run(main, str(LF), "rebase", "origin/main", env=env)
        assert _run(main, "git", "rev-parse", "origin/main") == expected
        for sha in [local, expected]:
            _run(main, "git", "merge-base", "--is-ancestor", sha, "HEAD")
        assert _run(main, "git", "show", ":local.txt") == "staged"
        assert (main / "local.txt").read_text() == "working\n"
        assert (main / "notes.txt").read_text() == "untracked\n"
        assert _run(main, "git", "stash", "list") == ""
        (tmp_path / "published").unlink()


def test_main_preserves_unpublished_commits_and_index(checkout: Checkout) -> None:
    main, author, env = checkout
    local = _commit(main, "local.txt", "unpublished\n")
    (main / "local.txt").write_text("staged\n")
    _run(main, "git", "add", "local.txt")
    (main / "local.txt").write_text("working\n")
    (main / "untracked.txt").write_text("untracked\n")
    upstream = _advance(author, "upstream.txt")
    _run(main, str(LF), "rebase", env=env)
    # The next command must retain main's unpublished history too, including
    # when upstream advances between the rebase and sibling creation.
    upstream = _advance(author, "after-rebase.txt")
    _run(main, str(LF), "wt", "create", "next", env=env)
    sibling = main.with_name("repo.next")
    for repo in [main, sibling]:
        for sha in [local, upstream]:
            _run(repo, "git", "merge-base", "--is-ancestor", sha, "HEAD")
    assert _run(sibling, "git", "show", "HEAD:local.txt") == "unpublished"
    assert _run(sibling, "git", "status", "--porcelain") == ""
    assert _run(main, "git", "show", ":local.txt") == "staged"
    assert (main / "local.txt").read_text() == "working\n"
    assert (main / "untracked.txt").read_text() == "untracked\n"
    assert _run(author, "git", "ls-remote", "origin", "refs/heads/main").split()[0] == upstream


@pytest.mark.parametrize("command", [("list", "--sync", "--format", "json"), ("prune",)])
def test_worktree_refresh_preserves_local_main(
    checkout: Checkout, command: tuple[str, ...]
) -> None:
    main, author, env = checkout
    local = _commit(main, "local.txt", "unpublished\n")
    (main / "local.txt").write_text("staged\n")
    _run(main, "git", "add", "local.txt")
    (main / "local.txt").write_text("working\n")
    (main / "notes.txt").write_text("untracked\n")
    upstream = _advance(author, "upstream.txt")
    output = _run(main, str(LF), "wt", *command, env=env)
    if command[0] == "list":
        assert isinstance(json.loads(output), list)
    for sha in [local, upstream]:
        _run(main, "git", "merge-base", "--is-ancestor", sha, "HEAD")
    assert _run(main, "git", "show", ":local.txt") == "staged"
    assert (main / "local.txt").read_text() == "working\n"
    assert (main / "notes.txt").read_text() == "untracked\n"


@pytest.mark.parametrize("command", [("create", "next"), ("list", "--sync"), ("prune",)])
def test_worktree_refresh_reports_fetch_failure_then_recovers(
    checkout: Checkout, command: tuple[str, ...]
) -> None:
    main, author, env = checkout
    before = _commit(main, "local.txt", "unpublished\n")
    upstream = _advance(author, "upstream.txt")
    (main / "base.txt").write_text("caller edit\n")
    remote = _run(main, "git", "remote", "get-url", "origin")
    _run(main, "git", "remote", "set-url", "origin", str(main / "unavailable"))
    result = subprocess.run(
        [str(LF), "wt", *command], cwd=main, env=env, capture_output=True, text=True
    )
    assert result.returncode != 0
    assert "fetch" in result.stderr
    assert _run(main, "git", "rev-parse", "HEAD") == before
    assert (main / "base.txt").read_text() == "caller edit\n"
    assert not main.with_name("repo.next").exists()
    _run(main, "git", "remote", "set-url", "origin", remote)
    _run(main, str(LF), "wt", *command, env=env)
    for sha in [before, upstream]:
        _run(main, "git", "merge-base", "--is-ancestor", sha, "HEAD")
    assert (main / "base.txt").read_text() == "caller edit\n"


@pytest.mark.parametrize("ahead", [False, True])
def test_worktree_plans_leave_checkout_and_remote_refs_unchanged(
    checkout: Checkout, ahead: bool
) -> None:
    main, author, env = checkout
    if ahead:
        _commit(main, "local.txt", "unpublished\n")
    _advance(author, "not-fetched.txt")
    (main / "base.txt").write_text("staged\n")
    _run(main, "git", "add", "base.txt")
    (main / "base.txt").write_text("working\n")
    (main / "notes.txt").write_text("untracked\n")
    refs = _run(main, "git", "show-ref")
    for command in [("create", "next", "--plan"), ("prune", "--dry-run")]:
        _run(main, str(LF), "wt", *command, env=env)
        assert _run(main, "git", "show-ref") == refs
        assert _run(main, "git", "branch", "--show-current") == "main"
        assert _run(main, "git", "show", ":base.txt") == "staged"
        assert (main / "base.txt").read_text() == "working\n"
        assert (main / "notes.txt").read_text() == "untracked\n"
        assert not main.with_name("repo.next").exists()


@pytest.mark.parametrize("command", [("create", "next"), ("list", "--sync"), ("prune",)])
def test_worktree_refresh_conflict_preserves_main_and_stops(
    checkout: Checkout, command: tuple[str, ...]
) -> None:
    main, author, env = checkout
    before = _commit(main, "base.txt", "unpublished\n")
    _commit(author, "base.txt", "conflicting upstream\n")
    _run(author, "git", "push", "origin", "main")
    (main / "notes.txt").write_text("caller notes\n")
    result = subprocess.run(
        [str(LF), "wt", *command], cwd=main, env=env, capture_output=True, text=True
    )
    assert result.returncode != 0
    assert "could not update main" in result.stderr
    assert _run(main, "git", "rev-parse", "HEAD") == before
    assert (main / "base.txt").read_text() == "unpublished\n"
    assert (main / "notes.txt").read_text() == "caller notes\n"
    assert not (main / ".git/MERGE_HEAD").exists()
    assert not main.with_name("repo.next").exists()


def test_feature_refreshes_main_then_integrates_and_creates_sibling(checkout: Checkout) -> None:
    main, author, env = checkout
    _run(main, str(LF), "wt", "create", "feature", env=env)
    feature = main.with_name("repo.feature")
    _commit(feature, "feature.txt", "feature\n")
    (feature / "feature.txt").write_text("staged\n")
    _run(feature, "git", "add", "feature.txt")
    (feature / "feature.txt").write_text("working\n")
    for name in ["first.txt", "second.txt"]:
        upstream = _advance(author, name)
        _run(feature, str(LF), "rebase", env=env)
        assert _run(main, "git", "rev-parse", "HEAD") == upstream
        _run(feature, "git", "merge-base", "--is-ancestor", upstream, "HEAD")
        assert _run(feature, "git", "show", ":feature.txt") == "staged"
        assert (feature / "feature.txt").read_text() == "working\n"
    # A behind caller must still integrate when main was refreshed elsewhere.
    upstream = _advance(author, "main-first.txt")
    _run(main, str(LF), "rebase", env=env)
    _run(feature, str(LF), "rebase", env=env)
    _run(feature, "git", "merge-base", "--is-ancestor", upstream, "HEAD")
    _run(main, str(LF), "wt", "create", "next", env=env)
    sibling = main.with_name("repo.next")
    assert _run(sibling, "git", "rev-parse", "HEAD") == upstream
    _run(feature, str(LF), "rebase", env=env)
    assert (feature / "feature.txt").read_text() == "working\n"


def test_worktree_background_push_survives_cli_exit(checkout: Checkout, tmp_path: Path) -> None:
    main, author, env = checkout
    binaries = tmp_path / "bin"
    binaries.mkdir()
    gate = tmp_path / "push-gate"
    done = tmp_path / "push-done"
    directive = tmp_path / "shell-directive"
    os.mkfifo(gate)
    os.mkfifo(done)
    os.mkfifo(directive)
    wrapper = binaries / "git"
    wrapper.write_text(
        '#!/bin/sh\ncommand="$1"\n'
        'if [ "$command" = -C ]; then command="$3"; fi\n'
        'if [ "$command" = push ]; then\n'
        '  printf "started\\n" > "$PUSH_DONE"\n'
        '  read -r token < "$PUSH_GATE"\n'
        '  "$REAL_GIT" "$@"\n'
        "  result=$?\n"
        '  printf "%s\\n" "$result" > "$PUSH_DONE"\n'
        '  exit "$result"\n'
        'fi\nexec "$REAL_GIT" "$@"\n'
    )
    wrapper.chmod(0o755)
    delayed_env = {
        **env,
        "PATH": f"{binaries}:{env['PATH']}",
        "REAL_GIT": shutil.which("git") or "git",
        "PUSH_GATE": str(gate),
        "PUSH_DONE": str(done),
        "LOOPFLOW_DIRECTIVE_FILE": str(directive),
    }
    gate_fd = os.open(gate, os.O_RDWR)
    done_fd = os.open(done, os.O_RDWR)
    process = subprocess.Popen(
        [str(LF), "wt", "create", "delayed"],
        cwd=main,
        env=delayed_env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        # Hold the CLI at its final shell directive until Git has started.
        ready, _, _ = select.select([done_fd], [], [], 10)
        assert ready, "background push did not start"
        assert os.read(done_fd, 32).strip() == b"started"
        assert directive.read_text().startswith("cd ")
        stdout, stderr = process.communicate(timeout=10)
        assert process.returncode == 0, stdout + stderr
        # Let the real push publish only after the CLI has exited and closed
        # its descriptors, as can happen with any slow remote.
        os.write(gate_fd, b"continue\n")
        ready, _, _ = select.select([done_fd], [], [], 10)
        assert ready, "background push did not finish after CLI exit"
        assert os.read(done_fd, 32).strip() == b"0"
    finally:
        if process.poll() is None:
            process.kill()
            process.communicate()
        os.close(gate_fd)
        os.close(done_fd)
    feature = main.with_name("repo.delayed")
    assert _run(feature, "git", "rev-parse", "@{upstream}") == _run(
        feature, "git", "rev-parse", "HEAD"
    )
    _commit(feature, "feature.txt", "feature\n")
    upstream = _advance(author, "upstream.txt")
    _run(feature, str(LF), "rebase", env=env)
    _run(feature, "git", "merge-base", "--is-ancestor", upstream, "HEAD")
    assert _run(feature, "git", "rev-parse", "@{upstream}") == _run(
        feature, "git", "rev-parse", "HEAD"
    )


def test_fetch_failure_preserves_state_and_later_invocation_catches_up(checkout: Checkout) -> None:
    main, author, env = checkout
    before = _run(main, "git", "rev-parse", "HEAD")
    upstream = _advance(author, "after-missed-refresh.txt")
    (main / "base.txt").write_text("caller edit\n")
    remote = _run(main, "git", "remote", "get-url", "origin")
    _run(main, "git", "remote", "set-url", "origin", str(main / "unavailable"))
    result = subprocess.run([str(LF), "rebase"], cwd=main, env=env, capture_output=True, text=True)
    assert result.returncode != 0
    assert "fetch" in result.stderr
    assert _run(main, "git", "rev-parse", "HEAD") == before
    assert (main / "base.txt").read_text() == "caller edit\n"

    _run(main, "git", "remote", "set-url", "origin", remote)
    _run(main, str(LF), "rebase", env=env)
    assert _run(main, "git", "rev-parse", "HEAD") == upstream
    assert (main / "base.txt").read_text() == "caller edit\n"


@pytest.mark.skipif(sys.platform != "darwin", reason="Homebrew refresh is macOS-only")
def test_install_from_worktree_retries_packages_after_main_updated(
    checkout: Checkout, tmp_path: Path
) -> None:
    main, author, env = checkout
    _run(main, str(LF), "wt", "create", "installer", env=env)
    caller = main.with_name("repo.installer")
    upstream = _advance(author, "new-required-package.txt")
    binaries = tmp_path / "bin"
    binaries.mkdir()
    state = tmp_path / "install-state"
    state.mkdir()
    # Executable fakes model external installation, not Loopflow's orchestration.
    for name, source in {
        "brew": '#!/bin/sh\ncat > "$INSTALL_STATE/required"\n'
        'test ! -e "$INSTALL_STATE/offline" || exit 7\n'
        'touch "$INSTALL_STATE/packages-ready"\n',
        "uv": '#!/bin/sh\ntest -e "$INSTALL_STATE/packages-ready" || exit 8\n'
        'case "$1" in\nsync) touch "$INSTALL_STATE/environment-ready";;\n'
        'run) test -e "$INSTALL_STATE/environment-ready" || exit 9\n'
        'touch "$INSTALL_STATE/release-ready";;\nesac\n',
    }.items():
        path = binaries / name
        path.write_text(source)
        path.chmod(0o755)
    env["PATH"] = f"{binaries}:{env['PATH']}"
    env["INSTALL_STATE"] = str(state)
    (state / "offline").touch()
    result = subprocess.run(
        [str(LF), "install"], cwd=caller, env=env, capture_output=True, text=True
    )
    assert result.returncode != 0
    assert "package refresh failed" in result.stderr
    assert _run(main, "git", "rev-parse", "HEAD") == upstream
    assert not (state / "release-ready").exists()
    (state / "offline").unlink()
    _run(caller, str(LF), "install", env=env)
    assert (state / "release-ready").exists()
    assert 'brew "uv"' in (state / "required").read_text()
    _run(caller, str(LF), "install", env=env)
    assert _run(main, "git", "rev-parse", "HEAD") == upstream


def test_main_merge_conflict_restores_original_history_and_edits(checkout: Checkout) -> None:
    main, author, env = checkout
    before = _commit(main, "base.txt", "local commit\n")
    _commit(author, "base.txt", "upstream conflict\n")
    _run(author, "git", "push", "origin", "main")
    (main / "notes.txt").write_text("caller notes\n")
    result = subprocess.run([str(LF), "rebase"], cwd=main, env=env, capture_output=True, text=True)
    assert result.returncode != 0
    assert _run(main, "git", "rev-parse", "HEAD") == before
    assert (main / "base.txt").read_text() == "local commit\n"
    assert (main / "notes.txt").read_text() == "caller notes\n"
    assert not (main / ".git/MERGE_HEAD").exists()


def test_unchecked_main_retains_unpublished_history(checkout: Checkout) -> None:
    main, author, env = checkout
    local = _commit(main, "local.txt", "unpublished\n")
    _run(main, "git", "checkout", "-b", "feature")
    _commit(main, "feature.txt", "feature\n")
    upstream = _advance(author, "upstream.txt")
    _run(main, str(LF), "rebase", env=env)
    assert _run(main, "git", "branch", "--show-current") == "feature"
    for sha in [local, upstream]:
        _run(main, "git", "merge-base", "--is-ancestor", sha, "main")
        _run(main, "git", "merge-base", "--is-ancestor", sha, "HEAD")


def test_sibling_rebases_share_main_update_without_rejecting_each_other(checkout: Checkout) -> None:
    main, author, env = checkout
    callers = []
    for name in ["one", "two"]:
        _run(main, str(LF), "wt", "create", name, env=env)
        caller = main.with_name(f"repo.{name}")
        _commit(caller, f"{name}.txt", name)
        (caller / f"{name}.txt").write_text(f"{name} staged")
        _run(caller, "git", "add", f"{name}.txt")
        (caller / f"{name}.txt").write_text(f"{name} working")
        callers.append(caller)
    upstream = _advance(author, "upstream.txt")
    processes = [
        subprocess.Popen(
            [str(LF), "rebase"],
            cwd=caller,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        for caller in callers
    ]
    for process in processes:
        stdout, stderr = process.communicate(timeout=30)
        assert process.returncode == 0, stdout + stderr
    assert _run(main, "git", "rev-parse", "HEAD") == upstream
    for name, caller in zip(["one", "two"], callers):
        _run(caller, "git", "merge-base", "--is-ancestor", upstream, "HEAD")
        assert _run(caller, "git", "show", f":{name}.txt") == f"{name} staged"
        assert (caller / f"{name}.txt").read_text() == f"{name} working"
    assert _run(main, "git", "stash", "list") == ""
