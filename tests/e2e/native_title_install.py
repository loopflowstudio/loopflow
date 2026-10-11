"""Prove title hook installation and retry in the disposable installation container."""

import json
import os
import subprocess
import sys
from pathlib import Path


def _run(*args: str) -> str:
    result = subprocess.run(args, capture_output=True, text=True, timeout=180)
    assert result.returncode == 0, result.stdout + result.stderr
    return result.stdout + result.stderr


def main() -> None:
    assert Path("/.dockerenv").exists() and os.getuid() == 0, "requires disposable container"
    binary = str(Path(sys.argv[1]).resolve())
    user = "lf-title-install"
    _run("useradd", "--create-home", user)
    home = Path("/home") / user
    claude = home / ".claude/settings.json"
    codex = home / ".codex/hooks.json"
    for path in [claude, codex]:
        path.parent.mkdir()
    original = {
        "permissions": {"allow": ["Read"]},
        "hooks": {
            "UserPromptSubmit": [
                {"hooks": [{"type": "command", "command": "existing-message-hook"}]}
            ]
        },
    }
    claude.write_text(json.dumps(original))
    claude.chmod(0o640)
    shared = home / "shared-hooks.json"
    shared.write_text("invalid JSON")
    codex.symlink_to(shared)
    _run("chown", "-R", f"{user}:{user}", str(home))

    def promote(*options: str) -> str:
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
            "self",
            "install",
            "promote",
            "--cli-target",
            str(home / ".local/bin/lf"),
            "--sync-skills",
            *options,
        )

    before = claude.read_bytes()
    promote("--preview")
    assert claude.read_bytes() == before and shared.read_text() == "invalid JSON"
    first = promote()
    assert "promoted published" in first, first
    assert "native title hook installation failed" in first, first
    assert shared.read_text() == "invalid JSON" and codex.is_symlink()
    # Repair the caller's malformed settings, then retry the exact candidate.
    shared.write_text("{}")
    retry = promote()
    assert "already installed" in retry, retry
    assert "native title hook installation failed" not in retry, retry
    for provider, path in [("claude", claude), ("codex", codex)]:
        settings = json.loads(path.read_text())
        handlers = [
            handler for entry in settings["hooks"]["UserPromptSubmit"] for handler in entry["hooks"]
        ]
        assert (
            sum(handler.get("command") == f"lf __session-title {provider}" for handler in handlers)
            == 1
        )
    settings = json.loads(claude.read_text())
    assert settings["permissions"] == original["permissions"]
    assert settings["hooks"]["UserPromptSubmit"][0] == original["hooks"]["UserPromptSubmit"][0]
    assert claude.stat().st_mode & 0o777 == 0o640
    assert codex.is_symlink()
    before = [path.read_bytes() for path in [claude, codex]]
    promote()
    assert before == [path.read_bytes() for path in [claude, codex]]
    print("PASS: published promotion, preview, failed-hook retry, preservation and idempotence")


if __name__ == "__main__":
    main()
