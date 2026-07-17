#!/usr/bin/env python3
from __future__ import annotations

import os
import sqlite3
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ACCESS_PROFILES = (
    ("personal", "Profile 3", "primary@example.com"),
    ("engineering", "Profile 8", "engineering@example.com"),
    ("loopflow", "Default", "personal@example.com"),
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


def _seed_topology(database: Path, demo_home: Path) -> None:
    now = int(time.time())
    accounts = (
        ("claude", "primary-claude", "primary@example.com", "max"),
        ("codex", "primary-codex", "primary@example.com", "max"),
        ("claude", "personal-claude", "personal@example.com", "personal"),
        ("codex", "personal-codex", "personal@example.com", "personal"),
        (
            "codex",
            "engineering-codex",
            "engineering@example.com",
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
        connection.executemany(
            """
            INSERT INTO access_profiles (
                profile_id, chrome_directory, expected_login, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?)
            """,
            [
                (profile, directory, login, now, now)
                for profile, directory, login in ACCESS_PROFILES
            ],
        )


def _print_step(title: str, output: str) -> None:
    print(f"\n{title}\n")
    print(output)


def main() -> int:
    binary = _lf_binary()
    with tempfile.TemporaryDirectory(prefix="lf-account-demo-") as directory:
        demo_home = Path(directory)
        env = os.environ.copy()
        env["LF_HOME"] = str(demo_home)

        print("Isolated account-routing demo — fake metadata, no credentials")
        _run(binary, env, "auth", "accounts")
        database = demo_home / "loopflow.db"
        _seed_topology(database, demo_home)

        access = (
            ("claude", "primary-claude", ("personal",)),
            ("claude", "personal-claude", ("engineering", "loopflow")),
            ("codex", "primary-codex", ("personal",)),
            ("codex", "engineering-codex", ("engineering",)),
            ("codex", "personal-codex", ("loopflow",)),
        )
        for provider, account, profiles in access:
            _run(
                binary,
                env,
                "auth",
                "access",
                "set",
                provider,
                account,
                *[item for profile in profiles for item in ("--profile", profile)],
            )

        _run(
            binary,
            env,
            "route",
            "set",
            "claude",
            "primary-claude",
            "personal-claude",
        )
        _run(
            binary,
            env,
            "route",
            "set",
            "codex",
            "primary-codex",
            "engineering-codex",
            "personal-codex",
        )

        _print_step("Access profiles", _run(binary, env, "profile", "list"))
        _print_step(
            "Provider routes",
            _run(binary, env, "route", "show"),
        )
        _print_step("Account lifecycle", _run(binary, env, "auth", "accounts"))

        print("\nLook for:")
        print("  1. Claude and Codex have independent account orders.")
        print("  2. Accounts list ordered Chrome access venues.")
        print("  3. A venue may be shared without becoming account identity.")
        print("  4. The demo home is deleted on exit; live Loopflow is untouched.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (RuntimeError, subprocess.CalledProcessError, sqlite3.Error) as error:
        print(f"demo: {error}")
        raise SystemExit(1) from error
