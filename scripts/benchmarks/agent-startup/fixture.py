"""Back up a dense SQLite store read-only; apply source drafts only to the copy."""

import argparse
import json
import os
import sqlite3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from canonicalize_migrations import _order, _read_drafts


def require_fixture(home: Path) -> None:
    if not (home / "startup-fixture.json").is_file():
        raise ValueError("Use fixture.py to create a private benchmark Home first")


def prepare(source: Path, home: Path, apply_drafts: bool) -> dict:
    home.mkdir(mode=0o700, parents=True, exist_ok=False)
    destination = home / "loopflow.db"
    with sqlite3.connect(f"file:{source.resolve()}?mode=ro", uri=True) as original:
        with sqlite3.connect(destination) as copy:
            original.backup(copy)
    destination.chmod(0o600)
    drafts = _order(_read_drafts()) if apply_drafts else []
    with sqlite3.connect(destination) as copy:
        # A fresh copy gets the draft set once. An incompatible released
        # frontier fails validation when the selected CLI opens this fixture.
        copy.executescript(
            "PRAGMA foreign_keys=OFF; BEGIN;\n" + "\n".join(draft.sql for draft in drafts)
        )
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
    receipt = {
        "counts": counts,
        "drafts": [draft.name for draft in drafts],
        "credential_files_copied": False,
        "managed_account_routes_removed": True,
    }
    (home / "startup-fixture.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return receipt


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--home", type=Path, required=True)
    parser.add_argument("--apply-drafts", action="store_true")
    args = parser.parse_args()
    os.umask(0o077)
    print(json.dumps(prepare(args.source, args.home, args.apply_drafts)))


if __name__ == "__main__":
    main()
