"""The merge gate: a migration on origin/main can never change, and a new one is
additive or a dated contract."""

import datetime as dt
import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/check_migrations.py"
MIGRATIONS = Path("rust/loopflow/src/store/migrations")


def _git(repo: Path, *args: str, when: dt.datetime | None = None) -> None:
    """Run git in `repo`; `when` dates a commit."""
    env = dict(os.environ)
    if when:
        env |= {"GIT_AUTHOR_DATE": when.isoformat(), "GIT_COMMITTER_DATE": when.isoformat()}
    subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@example.com", *args],
        cwd=repo,
        check=True,
        capture_output=True,
        env=env,
    )


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """origin/main carries one released batch and one merged slug."""
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts/check_migrations.py").write_bytes(SCRIPT.read_bytes())
    (tmp_path / MIGRATIONS).mkdir(parents=True)
    (tmp_path / MIGRATIONS / "0.10.1.001_release.sql").write_text(
        "-- draft: initial\nCREATE TABLE waves (id TEXT);\n"
    )
    (tmp_path / MIGRATIONS / "task_graphs.sql").write_text("CREATE TABLE graphs (id TEXT);\n")
    _git(tmp_path, "init", "-q", "-b", "main")
    _git(tmp_path, "add", ".")
    _git(tmp_path, "commit", "-qm", "main")
    _git(tmp_path, "update-ref", "refs/remotes/origin/main", "HEAD")
    return tmp_path


def check(repo: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, "scripts/check_migrations.py"],
        cwd=repo,
        capture_output=True,
        text=True,
    )


def merge(repo: Path, name: str, sql: str, days_ago: int) -> None:
    """Land a migration on origin/main `days_ago` days back."""
    (repo / MIGRATIONS / name).write_text(sql)
    _git(repo, "add", ".")
    when = dt.datetime.now(dt.timezone.utc) - dt.timedelta(days=days_ago)
    _git(repo, "commit", "-qm", name, when=when)
    _git(repo, "update-ref", "refs/remotes/origin/main", "HEAD")


CONTRACT = "-- expand: add_title\n-- exports: ~/.lf/exports/waves-name.json\n"


def test_merged_history_left_alone_passes(repo: Path):
    result = check(repo)
    assert result.returncode == 0, result.stderr
    assert "no unmerged migration" in result.stdout


def test_new_slugs_pass_and_are_named(repo: Path):
    (repo / MIGRATIONS / "add_note.sql").write_text("ALTER TABLE waves ADD note TEXT;\n")
    (repo / MIGRATIONS / "add_colour.sql").write_text("ALTER TABLE waves ADD colour TEXT;\n")
    result = check(repo)
    assert result.returncode == 0, result.stderr
    assert "unmerged: add_colour.sql, add_note.sql" in result.stdout


@pytest.mark.parametrize("name", ["task_graphs.sql", "0.10.1.001_release.sql"])
def test_an_edited_merged_migration_fails(repo: Path, name: str):
    (repo / MIGRATIONS / name).write_text("SELECT 1;\n")
    result = check(repo)
    assert result.returncode == 1
    assert "differs from origin/main" in result.stderr


def test_a_deleted_merged_migration_fails(repo: Path):
    (repo / MIGRATIONS / "task_graphs.sql").unlink()
    result = check(repo)
    assert result.returncode == 1
    assert "never deleted" in result.stderr


def test_a_new_released_batch_fails(repo: Path):
    (repo / MIGRATIONS / "0.10.2.001_release.sql").write_text("SELECT 1;\n")
    result = check(repo)
    assert result.returncode == 1
    assert "adds a released batch" in result.stderr


@pytest.mark.parametrize("name", ["Add-Note.sql", "drafts"])
def test_misplaced_files_fail(repo: Path, name: str):
    path = repo / MIGRATIONS / name
    if name == "drafts":
        path.mkdir()
    else:
        path.write_text("SELECT 1;\n")
    assert check(repo).returncode == 1


def test_a_slug_reusing_a_released_name_fails(repo: Path):
    (repo / MIGRATIONS / "initial.sql").write_text("SELECT 1;\n")
    result = check(repo)
    assert result.returncode == 1
    assert "reuses the released migration name" in result.stderr


def test_runtime_sql_names_its_columns(repo: Path):
    (repo / "rust/loopflow/src/writer.rs").write_text(
        'const SQL: &str = "INSERT INTO waves VALUES (?1)";\n'
    )
    result = check(repo)
    assert result.returncode == 1
    assert "positional INSERT" in result.stderr


def test_a_rebuild_that_keeps_every_column_is_additive(repo: Path):
    merge(repo, "add_name.sql", "ALTER TABLE waves ADD COLUMN name TEXT;\n", 40)
    (repo / MIGRATIONS / "rebuild_waves.sql").write_text(
        "CREATE TABLE waves_rebuilt (name TEXT, id TEXT CHECK (id <> ''));\n"
        "INSERT INTO waves_rebuilt(id, name) SELECT id, name FROM waves;\n"
        "DROP TABLE waves;\nALTER TABLE waves_rebuilt RENAME TO waves;\n"
        "CREATE INDEX waves_name ON waves(name);\n"
    )
    result = check(repo)
    assert result.returncode == 0, result.stderr


def test_a_redefined_view_that_keeps_its_columns_is_additive(repo: Path):
    merge(repo, "wave_names.sql", "CREATE VIEW wave_names AS SELECT id FROM waves;\n", 40)
    (repo / MIGRATIONS / "widen_view.sql").write_text(
        "ALTER TABLE waves ADD COLUMN name TEXT;\n"
        "DROP VIEW wave_names;\n"
        "CREATE VIEW wave_names AS SELECT w.id, w.name FROM waves w;\n"
    )
    result = check(repo)
    assert result.returncode == 0, result.stderr


def test_a_view_losing_a_column_needs_a_contract(repo: Path):
    merge(repo, "add_name.sql", "ALTER TABLE waves ADD COLUMN name TEXT;\n", 40)
    merge(repo, "wave_names.sql", "CREATE VIEW wave_names AS SELECT id, name FROM waves;\n", 40)
    merge(repo, "add_title.sql", "ALTER TABLE waves ADD COLUMN title TEXT;\n", 31)
    narrow = repo / MIGRATIONS / "narrow_view.sql"
    sql = "DROP VIEW wave_names;\nCREATE VIEW wave_names AS SELECT id FROM waves;\n"

    narrow.write_text(sql)
    result = check(repo)
    assert result.returncode == 1
    assert "drops column wave_names.name without naming its expand step" in result.stderr

    narrow.write_text(CONTRACT + sql)
    result = check(repo)
    assert result.returncode == 0, result.stderr


def test_a_dropped_column_needs_a_dated_contract_that_exports(repo: Path):
    merge(repo, "add_name.sql", "ALTER TABLE waves ADD COLUMN name TEXT;\n", 40)
    merge(repo, "add_title.sql", "ALTER TABLE waves ADD COLUMN title TEXT;\n", 31)
    contract = repo / MIGRATIONS / "drop_name.sql"

    contract.write_text("ALTER TABLE waves DROP COLUMN name;\n")
    result = check(repo)
    assert result.returncode == 1
    assert "drops column waves.name without naming its expand step" in result.stderr

    contract.write_text("-- expand: add_title\nALTER TABLE waves DROP COLUMN name;\n")
    result = check(repo)
    assert result.returncode == 1
    assert "-- exports:" in result.stderr

    contract.write_text(CONTRACT + "ALTER TABLE waves DROP COLUMN name;\n")
    result = check(repo)
    assert result.returncode == 0, result.stderr
    assert "drop_name.sql: contract of add_title (31 days)" in result.stdout


def test_a_contract_waits_for_its_expand_step_to_age(repo: Path):
    merge(repo, "add_name.sql", "ALTER TABLE waves ADD COLUMN name TEXT;\n", 40)
    merge(repo, "add_title.sql", "ALTER TABLE waves ADD COLUMN title TEXT;\n", 3)
    (repo / MIGRATIONS / "drop_name.sql").write_text(
        CONTRACT + "ALTER TABLE waves DROP COLUMN name;\n"
    )
    result = check(repo)
    assert result.returncode == 1
    assert "merged 3 days ago; its contract clears on" in result.stderr


def test_a_contract_drops_only_what_its_expand_step_left_behind(repo: Path):
    merge(repo, "add_title.sql", "ALTER TABLE waves ADD COLUMN title TEXT;\n", 31)
    (repo / MIGRATIONS / "drop_title.sql").write_text(
        CONTRACT + "ALTER TABLE waves DROP COLUMN title;\n"
    )
    result = check(repo)
    assert result.returncode == 1
    assert "did not leave behind" in result.stderr

    (repo / MIGRATIONS / "drop_title.sql").write_text(
        "-- expand: never_merged\n-- exports: x\nALTER TABLE waves DROP COLUMN title;\n"
    )
    result = check(repo)
    assert result.returncode == 1
    assert "not on origin/main" in result.stderr


@pytest.mark.parametrize(
    ("sql", "reason"),
    [
        ("ALTER TABLE waves ADD COLUMN rank INTEGER NOT NULL;", "required without a default"),
        (
            "CREATE TABLE waves_rebuilt (id INTEGER);\n"
            "INSERT INTO waves_rebuilt SELECT id FROM waves;\n"
            "DROP TABLE waves;\nALTER TABLE waves_rebuilt RENAME TO waves;\n",
            "changed its type",
        ),
        ("CREATE UNIQUE INDEX waves_id ON waves(id);", "unique index"),
    ],
)
def test_changes_that_break_older_sql_are_not_additive(repo: Path, sql: str, reason: str):
    (repo / MIGRATIONS / "reshape.sql").write_text(sql + "\n")
    result = check(repo)
    assert result.returncode == 1
    assert "is not additive" in result.stderr and reason in result.stderr
