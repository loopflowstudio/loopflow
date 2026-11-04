"""Build disposable LOO-672 review data around the already-built candidate."""

import hashlib
import json
import os
import plistlib
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from app_bundle import install_app_executable


def main() -> None:
    source = Path(__file__).resolve().parents[1]
    root = Path(tempfile.mkdtemp(prefix="loo-672-redemo-", dir="/tmp")).resolve()
    repo = root / "repo"
    repo.mkdir()
    home = root / "machine"
    home.mkdir()
    bundle = root / "Loopflow LOO-672 Redemo.app"
    contents = bundle / "Contents"
    resources = contents / "Resources"
    resources.mkdir(parents=True)
    plist = plistlib.loads((source / "swift/LoopflowMac/Info.plist").read_bytes())
    plist.update(CFBundleIdentifier="com.loopflow.mac.redemo.loo672",
                 CFBundleExecutable="Loopflow LOO-672 Redemo",
                 CFBundleName="Loopflow LOO-672 Redemo",
                 CFBundleDisplayName="Loopflow LOO-672 Redemo")
    (contents / "Info.plist").write_bytes(plistlib.dumps(plist))
    executable = install_app_executable(source / "swift/.build/debug", contents / "Info.plist", contents)
    cli = contents / "MacOS/lf"
    shutil.copy2(source / "target/debug/lf", cli)
    for name in ("LoopflowSwift_Loopflow.bundle", "LoopflowSwift_LoopflowMac.bundle"):
        shutil.copytree(source / "swift/.build/debug" / name, resources / name, symlinks=True)
    shutil.copy2(source / "swift/LoopflowMac/AppIcon.icns", resources)
    subprocess.run(["codesign", "--force", "--deep", "--sign", "-", str(bundle)], check=True)
    env = {key: value for key, value in os.environ.items() if not key.startswith(("LF_", "LOOPFLOW_UI_TEST_"))}
    env.update(LF_HOME=str(home), LF_BIN=str(cli), LF_USER_NAME="Jack Heart")

    def run(args: list[str]) -> str:
        return subprocess.check_output(args, cwd=repo, env=env, stdin=subprocess.DEVNULL, text=True)

    (repo / "README.md").write_text("# LOO-672 disposable conversation review\n")
    (repo / "AGENTS.md").write_text(
        "This repository is disposable demonstration data for Jack Heart’s LOO-672 review.\n"
        "Wait for a request in the conversation; no project work is assigned here.\n")
    for args in (["init", "-b", "main"], ["config", "user.name", "Jack Heart"],
                 ["config", "user.email", "loo672-demo@example.invalid"],
                 ["add", "."], ["commit", "-m", "Disposable review repository"]):
        run(["git", *args])
    run(["git", "init", "--bare", str(root / "origin.git")])
    run(["git", "remote", "add", "origin", str(root / "origin.git")])
    tasks = []
    for title, provider in (("Start a conversation", None), ("Claude continuity", "claude"), ("Codex continuity", "codex")):
        task = json.loads(run([str(cli), "task", "create", "--title", title, "--notes",
            "Disposable demonstration data for Jack Heart’s LOO-672 review; no project work.", "--json"]))
        task_id = task["id"]
        run([str(cli), "checkout", task_id, "--json"])
        session = None
        if provider:
            session = json.loads(run([str(cli), "--agent", provider, "resume", "--task", task_id, "--prepare", "--json"]))
            assert session["provider"] == provider, session["provider"]
        plan = json.loads(run([str(cli), "desktop", "plan", "--task", task_id, "--json"]))
        workspace = plan["tasks"]["items"][0]["reference"]["workspace"]
        if session:
            assert session["workspace"]["worktree"] == workspace["worktree"]
            assert session["workspace"]["machine_id"] == workspace["machine_id"]
        tasks.append({"id": task_id, "title": title, "workspace": workspace,
                      "session": session["id"] if session else None, "provider": provider})
    manifest = {"source_head": run(["git", "-C", str(source), "rev-parse", "HEAD"]).strip(),
                "source_diff_sha256": hashlib.sha256(run(["git", "-C", str(source), "diff", "HEAD", "--", "rust", "swift"]).encode()).hexdigest(),
                "candidate": str(bundle), "repository": str(repo), "machine": str(home), "tasks": tasks,
                "exercised": "CLI preparation only; no provider launched and no native acceptance claimed"}
    (root / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    # Drop inherited invocation markers without recording any credential values.
    launch = root / "launch.sh"
    launch.write_text("#!/bin/zsh\ncd " + shlex.quote(str(repo)) + "\n"
        + "exec env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') "
        + "LF_HOME=" + shlex.quote(str(home)) + " LF_BIN=" + shlex.quote(str(cli)) + " "
        + shlex.quote(str(executable)) + "\n")
    launch.chmod(0o755)
    print(root)


if __name__ == "__main__":
    main()
