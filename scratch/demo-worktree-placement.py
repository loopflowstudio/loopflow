"""Demonstrate placement through the built lf CLI in disposable Git repositories."""

import json
import os
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
LF = ROOT / "target/debug/lf"


def main() -> None:
    results = []
    with tempfile.TemporaryDirectory(prefix="lf-placement-demo-") as temporary:
        fixture = Path(temporary).resolve()
        home = fixture / "home"
        home.mkdir()
        env = {
            "PATH": os.environ["PATH"], "HOME": str(home),
            "LF_HOME": str(home / ".lf"), "LF_BIN": str(LF),
            "GIT_CONFIG_NOSYSTEM": "1", "GIT_TERMINAL_PROMPT": "0",
        }

        def run(cwd: Path, *args: str) -> str:
            result = subprocess.run(args, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                    capture_output=True, text=True, timeout=60)
            if result.returncode:
                raise RuntimeError(f"{args}: {result.stdout}\n{result.stderr}")
            return result.stdout.strip()

        for host in ["Conductor", "Herdr", "Worktrunk", "Claude"]:
            case = fixture / host.lower()
            case.mkdir()
            repo = case / "app"
            remote = case / "remote.git"
            run(case, "git", "init", "--bare", "--initial-branch=main", str(remote))
            run(case, "git", "clone", str(remote), str(repo))
            run(repo, "git", "config", "user.name", "Placement Demo")
            run(repo, "git", "config", "user.email", "fixture@example.invalid")
            (repo / "README.md").write_text("Demo project\n")
            run(repo, "git", "add", "README.md")
            run(repo, "git", "commit", "-m", "Fixture baseline")
            run(repo, "git", "push", "-u", "origin", "main")
            run(repo, "git", "remote", "set-head", "origin", "main")
            if host == "Conductor":
                parent = case / "conductor/workspaces/app"
                names = [("warsaw", "alice/fix-login"), ("tokyo", "bob/add-search")]
                expected = parent / "next"
            elif host == "Herdr":
                parent = case / ".herdr/worktrees/app"
                names = [("feature-login", "feature/login"), ("fix-search", "fix/search")]
                expected = parent / "next"
            elif host == "Worktrunk":
                parent = case
                names = [("app.feature-login", "feature/login"), ("app.fix-search", "fix/search")]
                expected = parent / "app.next"
            else:
                parent = repo / ".claude/worktrees"
                names = [("login", "worktree-login"), ("search", "worktree-search")]
                expected = parent / "next"
            parent.mkdir(parents=True, exist_ok=True)
            # Git seeds foreign-tool registrations; every operation being demoed uses lf.
            for directory, branch in names:
                run(repo, "git", "worktree", "add", "-b", branch, str(parent / directory), "HEAD")
            first = parent / names[0][0]
            (first / "unfinished.txt").write_text("Preserve teammate work\n")
            before = run(repo, "git", "rev-parse", "HEAD")
            plan = run(repo, str(LF), "wt", "create", "next", "--plan")
            assert f"worktree: {expected}" in plan and "strategy: create" in plan, plan
            assert not expected.exists(), "planning created a checkout"
            created = run(repo, str(LF), "wt", "create", "next")
            assert (expected / ".git").is_file(), created
            (expected / "unfinished.txt").write_text("Preserve new work\n")
            registrations = run(repo, "git", "worktree", "list", "--porcelain")
            reused_plan = run(repo, str(LF), "wt", "create", "next", "--plan")
            assert "strategy: use_existing_worktree" in reused_plan, reused_plan
            reused = run(repo, str(LF), "wt", "create", "next")
            assert f"Using existing worktree: {expected}" in reused, reused
            selected = run(repo, str(LF), "wt", "switch", names[0][1])
            assert str(first) in selected, selected
            assert run(repo, "git", "worktree", "list", "--porcelain") == registrations
            assert registrations.count("worktree ") == 4
            assert run(repo, "git", "rev-parse", "HEAD") == before
            assert (first / "unfinished.txt").read_text() == "Preserve teammate work\n"
            assert (expected / "unfinished.txt").read_text() == "Preserve new work\n"
            assert not (repo / ".lf/config.yaml").exists()
            results.append({"layout": host, "expected": str(expected), "plan": plan,
                            "create": created, "reuse_plan": reused_plan, "reuse": reused,
                            "select_existing_branch": selected, "registrations": 4,
                            "uncommitted_files_preserved": True, "config_required": False})
            print(f"PASS {host}: {expected.relative_to(case)}; create, reuse, existing-branch selection, dirty files preserved")
        evidence = json.dumps(results, indent=2).replace(str(fixture), "$FIXTURE")
        (ROOT / "scratch/worktree-demo.json").write_text(evidence + "\n")


if __name__ == "__main__":
    main()
