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
        env = {key: value for key, value in os.environ.items() if not key.startswith("LF_")}
        env["LF_HOME"] = str(demo_home)

        print("Isolated account-routing demo — fake metadata, no credentials")
        _run(binary, env, "account", "set", "linear", "--clear-chrome-profiles")
        database = demo_home / "loopflow.db"
        _seed_topology(database, demo_home)

        access = (
            ("claude", "primary@example.com", ("personal",)),
            ("claude", "personal@example.com", ("engineering", "loopflow")),
            ("codex", "primary@example.com", ("personal",)),
            ("codex", "engineering@example.com", ("engineering",)),
            ("codex", "personal@example.com", ("loopflow",)),
        )
        for provider, account, profiles in access:
            _run(
                binary,
                env,
                "account",
                "set",
                provider,
                account,
                *[item for profile in profiles for item in ("--chrome-profile", profile)],
            )

        _run(
            binary,
            env,
            "account",
            "route",
            "set",
            "claude",
            "primary@example.com",
            "personal@example.com",
        )
        _run(
            binary,
            env,
            "account",
            "route",
            "set",
            "codex",
            "primary@example.com",
            "engineering@example.com",
            "personal@example.com",
        )

        _print_step("Access profiles", _run(binary, env, "account", "--cached", "--details"))
        _print_step(
            "Provider routes",
            _run(binary, env, "account", "route"),
        )
        _print_step("Account lifecycle", _run(binary, env, "account", "--cached"))

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
