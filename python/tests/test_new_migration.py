"""Authoring a draft is merge-independent: file-only, immutable id, no shared edit."""

import re
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/new_migration.py"
MIGRATIONS = Path("rust/loopflow/src/store/migrations")
DRAFTS = MIGRATIONS / "drafts"
MIGRATIONS_RS = Path("rust/loopflow/src/store/migrations.rs")

REGISTRY = """const MIGRATIONS: &[Migration] = &[
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 1,
        },
        name: "initial",
        sql: include_str!("migrations/0.11.001_initial.sql"),
    },
];
"""

DRAFT_FILE = re.compile(r"^([a-z][a-z0-9_]*)__([0-9a-f]{32})\.sql$")


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """A repo with one released migration, a Rust registry, and no drafts."""
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts/new_migration.py").write_bytes(SCRIPT.read_bytes())
    (tmp_path / MIGRATIONS).mkdir(parents=True)
    (tmp_path / MIGRATIONS / "0.11.001_initial.sql").write_text("CREATE TABLE waves (id TEXT);\n")
    (tmp_path / MIGRATIONS_RS).write_text(REGISTRY)
    return tmp_path


def run(repo: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, "scripts/new_migration.py", *args],
        cwd=repo,
        capture_output=True,
        text=True,
    )


def draft_files(repo: Path) -> list[str]:
    return sorted(p.name for p in (repo / DRAFTS).glob("*.sql"))


def parse(filename: str) -> tuple[str, str]:
    match = DRAFT_FILE.match(filename)
    assert match, filename
    return match.group(1), match.group(2)


def test_new_migration_writes_a_file_only_draft_with_no_ordinal(repo: Path) -> None:
    result = run(repo, "add_wave_colour")

    assert result.returncode == 0, result.stderr
    files = draft_files(repo)
    assert len(files) == 1
    name, token = parse(files[0])
    # A 128-bit token (32 hex chars) — materially collision-resistant, not the
    # earlier 32-bit token that two same-name branches could realistically clash.
    assert name == "add_wave_colour" and re.fullmatch(r"[0-9a-f]{32}", token)
    body = (repo / DRAFTS / files[0]).read_text()
    assert body.startswith(f"-- name: add_wave_colour\n-- id: {token}\n-- depends_on: \n")
    # No canonical ordinal moved, and no Rust registry entry to paste anywhere.
    assert list((repo / MIGRATIONS).glob("0.11.*.sql")) == [
        repo / MIGRATIONS / "0.11.001_initial.sql"
    ]
    assert "DraftMigration" not in result.stdout
    assert "DRAFTS" not in result.stdout


def test_two_branches_same_name_have_no_shared_registry_edit(repo: Path, tmp_path: Path) -> None:
    # Author the same readable name from two independent trees; prove distinct
    # files, distinct ids, and a byte-identical Rust registry — nothing to contend on.
    other = tmp_path / "other"
    other.mkdir()
    (other / "scripts").mkdir()
    (other / "scripts/new_migration.py").write_bytes(SCRIPT.read_bytes())
    (other / MIGRATIONS).mkdir(parents=True)
    (other / MIGRATIONS / "0.11.001_initial.sql").write_text("CREATE TABLE waves (id TEXT);\n")
    (other / MIGRATIONS_RS).write_text(REGISTRY)

    registry_before = (repo / MIGRATIONS_RS).read_text()
    assert run(repo, "add_wave_colour").returncode == 0
    assert run(other, "add_wave_colour").returncode == 0

    _, id_a = parse(draft_files(repo)[0])
    _, id_b = parse(draft_files(other)[0])
    # Distinct, and each a full 128-bit token: if the id ever narrows back to a
    # weak width, one of these fullmatches fails before the collision ever could.
    assert id_a != id_b
    assert re.fullmatch(r"[0-9a-f]{32}", id_a) and re.fullmatch(r"[0-9a-f]{32}", id_b)
    assert (repo / MIGRATIONS_RS).read_text() == registry_before
    assert (other / MIGRATIONS_RS).read_text() == registry_before


def test_the_same_name_authored_twice_here_keeps_both(repo: Path) -> None:
    assert run(repo, "add_wave_colour").returncode == 0
    assert run(repo, "add_wave_colour").returncode == 0
    files = draft_files(repo)
    assert len(files) == 2
    assert {parse(f)[0] for f in files} == {"add_wave_colour"}
    assert len({parse(f)[1] for f in files}) == 2


def test_many_concurrent_same_name_drafts_never_collide(repo: Path) -> None:
    # The collision-free claim, exercised: author one readable name many times
    # (the concurrent-branch worst case) and prove every minted id is a distinct
    # 128-bit token. A narrowed id would repeat here or fail the width check —
    # a 32-bit token has a ~50% chance of colliding within ~77k of these.
    count = 64
    for _ in range(count):
        assert run(repo, "add_wave_colour").returncode == 0
    ids = [parse(f)[1] for f in draft_files(repo)]
    assert len(ids) == count
    assert all(re.fullmatch(r"[0-9a-f]{32}", token) for token in ids)
    assert len(set(ids)) == count, "minted ids collided"


def test_new_migration_never_fetches_or_touches_git(repo: Path) -> None:
    assert not (repo / ".git").exists()
    result = run(repo, "add_task_priority")
    assert result.returncode == 0, result.stderr
    assert {parse(f)[0] for f in draft_files(repo)} == {"add_task_priority"}


def test_depends_on_records_a_draft_dependency(repo: Path) -> None:
    run(repo, "add_wave_colour")
    result = run(repo, "backfill_colour", "--depends-on", "add_wave_colour")

    assert result.returncode == 0, result.stderr
    backfill = next(p for p in (repo / DRAFTS).glob("backfill_colour__*.sql"))
    assert "-- depends_on: add_wave_colour\n" in backfill.read_text()


def test_depends_on_accepts_a_released_migration(repo: Path) -> None:
    result = run(repo, "backfill_colour", "--depends-on", "initial")
    assert result.returncode == 0, result.stderr


def test_depends_on_accepts_a_draft_published_inside_a_release_batch(repo: Path) -> None:
    (repo / MIGRATIONS / "0.11.2.001_release.sql").write_text(
        "-- draft: add_wave_colour\nSELECT 1;\n"
    )

    result = run(repo, "backfill_colour", "--depends-on", "add_wave_colour")

    assert result.returncode == 0, result.stderr


def test_depends_on_rejects_an_unknown_name(repo: Path) -> None:
    result = run(repo, "backfill_colour", "--depends-on", "nope")
    assert result.returncode == 1
    assert "no draft or released migration" in result.stderr


def test_new_migration_rejects_a_name_colliding_with_a_released_migration(repo: Path) -> None:
    result = run(repo, "initial")
    assert result.returncode == 1
    assert "already a released migration name" in result.stderr


def test_new_migration_rejects_a_double_underscore_name(repo: Path) -> None:
    # `__` is reserved as the name/id separator in the filename.
    result = run(repo, "add__colour")
    assert result.returncode == 2
    assert "reserved" in result.stderr
