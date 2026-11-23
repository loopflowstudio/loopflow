"""Leaving marks become dated removals, and a dropped member stops being listed."""

import datetime as dt
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))

import due_removals  # noqa: E402

MERGED = dt.datetime(2026, 11, 17, tzinfo=dt.timezone.utc)
FILES = {
    "0.10.1.001_release.sql": b"CREATE TABLE waves (id TEXT, hue TEXT);\n"
    b"CREATE TABLE pools (n TEXT);\n",
    "stop_hue.sql": b"-- leaving: waves.hue LOO-1\n-- leaving: pools LOO-2\n",
}


def test_a_mark_is_due_staging_days_after_its_migration_merged():
    removals = due_removals.removals(FILES, {"stop_hue.sql": MERGED})
    assert [(r.task, r.members, r.drop_after) for r in removals] == [
        ("LOO-1", ["waves.hue"], dt.date(2026, 12, 17)),
        ("LOO-2", ["pools"], dt.date(2026, 12, 17)),
    ]
    assert not removals[0].due(dt.date(2026, 12, 16))
    assert removals[0].due(dt.date(2026, 12, 17))


def test_an_unmerged_mark_is_never_due():
    (removal, _) = due_removals.removals(FILES, {})
    assert not removal.merged and not removal.due(dt.date(2030, 1, 1))


def test_a_member_the_contract_dropped_is_no_longer_listed():
    files = FILES | {"drop_pools.sql": b"-- expand: stop_hue\nDROP TABLE pools;\n"}
    removals = due_removals.removals(files, {"stop_hue.sql": MERGED})
    assert [r.task for r in removals] == ["LOO-1"]
