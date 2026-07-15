#!/usr/bin/env python3
from __future__ import annotations

import os
import sqlite3
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROFILES = (
    "jack@loopflow.studio",
    "loopflow-eng@loopflow.studio",
    "jackstah@gmail.com",
)


def _lf_binary() -> Path:
    subprocess.run(
        ["cargo", "build", "-q", "-p", "loopflow", "--bin", "lf"],
        cwd=ROOT,
        check=True,
    )
    return ROOT / "target" / "debug" / "lf"


def _run(binary: Path, env: dict[str, str], *args: str) -> str:
    result = subprocess.run(
        [str(binary), *args],
        cwd=ROOT,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        message = result.stderr.strip() or result.stdout.strip()
        raise RuntimeError(message or f"lf {' '.join(args)} exited {result.returncode}")
    return result.stdout.rstrip()


def _seed_accounts(database: Path, demo_home: Path) -> None:
    now = int(time.time())
    accounts = (
        ("claude", "jack-claude", "jack@loopflow.studio", "max"),
        ("codex", "jack-codex", "jack@loopflow.studio", "max"),
        ("claude", "personal-claude", "jackstah@gmail.com", "personal"),
        ("codex", "personal-codex", "jackstah@gmail.com", "personal"),
        (
            "codex",
            "loopflow-eng-codex",
            "loopflow-eng@loopflow.studio",
            "max",
        ),
    )
    with sqlite3.connect(database) as connection:
        connection.executemany(
            """
            INSERT INTO provider_accounts (
                provider, account_id, home, login_email, credential_state,
                routing_state, plan, paid_through, utilization_percent,
                cooldown_until, cooldown_reason, last_selected_at, created_at,
                updated_at
            ) VALUES (?, ?, ?, ?, 'connected', 'automatic', ?, NULL, NULL,
                      NULL, NULL, NULL, ?, ?)
            """,
            [
                (
                    provider,
                    account_id,
                    str(demo_home / "accounts" / provider / account_id),
                    email,
                    plan,
                    now,
                    now,
                )
                for provider, account_id, email, plan in accounts
            ],
        )


def _print_step(title: str, output: str) -> None:
    print(f"\n{title}\n")
    print(output)


def main() -> int:
    binary = _lf_binary()
    with tempfile.TemporaryDirectory(prefix="lf-profile-demo-") as directory:
        demo_home = Path(directory)
        env = os.environ.copy()
        env["LF_HOME"] = str(demo_home)

        print("Isolated profile-routing demo — fake metadata, no credentials")
        for profile in PROFILES:
            _run(binary, env, "profile", "create", profile)

        database = demo_home / "loopflow.db"
        _seed_accounts(database, demo_home)

        mappings = (
            ("jack@loopflow.studio", "claude", "jack-claude"),
            ("jack@loopflow.studio", "codex", "jack-codex"),
            ("loopflow-eng@loopflow.studio", "claude", "personal-claude"),
            (
                "loopflow-eng@loopflow.studio",
                "codex",
                "loopflow-eng-codex",
            ),
            ("jackstah@gmail.com", "claude", "personal-claude"),
            ("jackstah@gmail.com", "codex", "personal-codex"),
        )
        for profile, provider, account in mappings:
            _run(
                binary,
                env,
                "profile",
                "account",
                "set",
                profile,
                provider,
                account,
            )

        _run(
            binary,
            env,
            "profile",
            "route",
            "set",
            "--default",
            "jack@loopflow.studio",
            "--backup",
            "loopflow-eng@loopflow.studio",
            "--backup",
            "jackstah@gmail.com",
        )

        _print_step("Profiles", _run(binary, env, "profile", "list"))
        _print_step(
            "Repository route",
            _run(binary, env, "profile", "route", "show"),
        )
        _print_step("Account lifecycle", _run(binary, env, "auth", "accounts"))

        print("\nLook for:")
        print("  1. Profiles are the Google/Chrome login emails.")
        print("  2. Claude and Codex map independently per profile.")
        print("  3. jackstah@gmail.com Claude is marked shared with loopflow-eng@loopflow.studio.")
        print("  4. The demo home is deleted on exit; live Loopflow is untouched.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (RuntimeError, subprocess.CalledProcessError, sqlite3.Error) as error:
        print(f"demo: {error}")
        raise SystemExit(1) from error
