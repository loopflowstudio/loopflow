"""Back up a dense SQLite store read-only into a private benchmark Home."""

import argparse
import json
import os
import sqlite3
import uuid
from pathlib import Path


def require_fixture(home: Path) -> None:
    if not (home / "startup-fixture.json").is_file():
        raise ValueError("Use fixture.py to create a private benchmark Home first")


def read_session_ids(home: Path) -> set[str]:
    with sqlite3.connect(f"file:{home / 'loopflow.db'}?mode=ro", uri=True) as db:
        return {row[0] for row in db.execute("SELECT id FROM agent_sessions")}


def prepare(source: Path, home: Path, receipts: int = 0) -> dict:
    home.mkdir(mode=0o700, parents=True, exist_ok=False)
    destination = home / "loopflow.db"
    with sqlite3.connect(f"file:{source.resolve()}?mode=ro", uri=True) as original:
        with sqlite3.connect(destination) as copy:
            original.backup(copy)
    destination.chmod(0o600)
    # The selected CLI migrates this private Home when it first opens it.
    with sqlite3.connect(destination) as copy:
        copy.executescript("PRAGMA foreign_keys=OFF; BEGIN;")
        # Copied account rows contain absolute provider-home paths. Remove the
        # routes before any launch so this fixture uses only native provider
        # selection and cannot refresh an installed managed account.
        for table in (
            "provider_routes",
            "provider_session_accounts",
            "auth_browser_bindings",
            "provider_accounts",
        ):
            copy.execute(f"DELETE FROM {table}")
        if copy.execute("PRAGMA foreign_key_check").fetchone() is not None:
            raise ValueError("Benchmark fixture has invalid foreign keys")
        copy.commit()
        copy.execute("PRAGMA foreign_keys=ON")
        counts = {
            table: copy.execute(f"SELECT count(*) FROM {table}").fetchone()[0]
            for table in ("agent_sessions", "processes", "session_events", "provider_accounts")
        }
    root = home / "runtime/exec-processes"
    root.mkdir(parents=True)
    for index in range(receipts):
        identity = str(uuid.UUID(int=index + 1))
        (root / f"{identity}.json").write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "trace_id": "synthetic-startup-benchmark",
                    "exec_id": identity,
                    "pid": 2147483647,
                    "started_at": 1,
                }
            )
        )
    receipt = {
        "counts": counts,
        "credential_files_copied": False,
        "managed_account_routes_removed": True,
        "synthetic_process_receipts": receipts,
    }
    (home / "startup-fixture.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return receipt


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--home", type=Path, required=True)
    parser.add_argument("--receipts", type=int, default=0)
    args = parser.parse_args()
    os.umask(0o077)
    print(json.dumps(prepare(args.source, args.home, args.receipts)))


if __name__ == "__main__":
    main()
