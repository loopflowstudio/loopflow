"""The release cut: drafts become one ordered, release-scoped batch."""

import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/canonicalize_migrations.py"
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


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts/canonicalize_migrations.py").write_bytes(SCRIPT.read_bytes())
    (tmp_path / MIGRATIONS).mkdir(parents=True)
    (tmp_path / MIGRATIONS / "0.11.001_initial.sql").write_text("CREATE TABLE waves (id TEXT);\n")
    (tmp_path / MIGRATIONS_RS).write_text(REGISTRY)
    (tmp_path / DRAFTS).mkdir()
    return tmp_path


def _token(name: str) -> str:
    # Deterministic per name so re-run/two-repo tests build identical draft files.
    # 128-bit (32 hex chars), matching the immutable id new_migration.py mints.
    import hashlib

    return hashlib.sha256(name.encode()).hexdigest()[:32]


def draft(
    repo: Path,
    name: str,
    depends_on: str = "",
    body: str = "SELECT 1;\n",
    token: str | None = None,
) -> None:
    token = token or _token(name)
    (repo / DRAFTS / f"{name}__{token}.sql").write_text(
        f"-- name: {name}\n-- id: {token}\n-- depends_on: {depends_on}\n{body}"
    )


def draft_names(repo: Path) -> set[str]:
    import re

    pattern = re.compile(r"^([a-z][a-z0-9_]*)__[0-9a-f]{32}\.sql$")
    return {
        match.group(1)
        for path in (repo / DRAFTS).glob("*.sql")
        if (match := pattern.match(path.name))
    }


def run(repo: Path, *args: str) -> subprocess.CompletedProcess:
    arguments = list(args)
    authorities = {"--check", "--dry-run", "--release-cut", "--materialize-for-tests"}
    if not authorities.intersection(arguments):
        arguments.append("--release-cut")
    return subprocess.run(
        [sys.executable, "scripts/canonicalize_migrations.py", *arguments],
        cwd=repo,
        capture_output=True,
        text=True,
    )


def canonical_files(repo: Path) -> list[str]:
    return sorted(p.name for p in (repo / MIGRATIONS).glob("*.sql"))


def test_empty_draft_set_is_a_noop(repo: Path) -> None:
    before = (repo / MIGRATIONS_RS).read_text()
    result = run(repo, "0.11.30")
    assert result.returncode == 0, result.stderr
    assert "no drafts" in result.stdout
    assert (repo / MIGRATIONS_RS).read_text() == before


def test_two_independent_drafts_become_one_release_batch_ordered_by_name(repo: Path) -> None:
    draft(repo, "add_wave_colour")
    draft(repo, "add_task_priority")

    result = run(repo, "0.11.30")

    assert result.returncode == 0, result.stderr
    assert canonical_files(repo) == [
        "0.11.001_initial.sql",
        "0.11.30.001_release.sql",
    ]
    # Drafts are consumed, leaving none behind.
    assert list((repo / DRAFTS).glob("*.sql")) == []
    registry = (repo / MIGRATIONS_RS).read_text()
    assert "patch: Some(30)" in registry
    assert 'name: "release"' in registry
    batch = (repo / MIGRATIONS / "0.11.30.001_release.sql").read_text()
    assert batch.index("-- draft: add_task_priority") < batch.index("-- draft: add_wave_colour")
    assert registry.rstrip().endswith("];")


def test_a_dependency_orders_before_its_dependent_against_name_order(repo: Path) -> None:
    # By name, `a_backfill` sorts before `z_setup`; the dependency inverts that.
    draft(repo, "z_setup")
    draft(repo, "a_backfill", depends_on="z_setup")

    result = run(repo, "0.11.30")

    assert result.returncode == 0, result.stderr
    assert canonical_files(repo) == [
        "0.11.001_initial.sql",
        "0.11.30.001_release.sql",
    ]
    batch = (repo / MIGRATIONS / "0.11.30.001_release.sql").read_text()
    assert batch.index("-- draft: z_setup") < batch.index("-- draft: a_backfill")


def test_the_canonical_file_carries_the_body_without_the_header(repo: Path) -> None:
    draft(repo, "add_wave_colour", body="ALTER TABLE waves ADD COLUMN colour TEXT;\n")

    run(repo, "0.11.30")

    written = (repo / MIGRATIONS / "0.11.30.001_release.sql").read_text()
    assert written == ("-- draft: add_wave_colour\nALTER TABLE waves ADD COLUMN colour TEXT;\n")
    assert "-- name:" not in written


def test_a_minor_release_uses_its_full_package_version(repo: Path) -> None:
    draft(repo, "add_wave_colour")

    result = run(repo, "0.12.0")

    assert result.returncode == 0, result.stderr
    assert (repo / MIGRATIONS / "0.12.0.001_release.sql").exists()


def test_a_cycle_fails_before_writing_anything(repo: Path) -> None:
    draft(repo, "a", depends_on="b")
    draft(repo, "b", depends_on="a")
    before = (repo / MIGRATIONS_RS).read_text()

    result = run(repo, "0.11.30")

    assert result.returncode == 1
    assert "cycle" in result.stderr
    assert (repo / MIGRATIONS_RS).read_text() == before
    assert canonical_files(repo) == ["0.11.001_initial.sql"]
    assert draft_names(repo) == {"a", "b"}


def test_check_mode_writes_nothing(repo: Path) -> None:
    draft(repo, "add_wave_colour")
    before = (repo / MIGRATIONS_RS).read_text()

    result = run(repo, "0.11.30", "--check")

    assert result.returncode == 0, result.stderr
    assert "0.11.30.001_release" in result.stdout
    assert (repo / MIGRATIONS_RS).read_text() == before
    assert draft_names(repo) == {"add_wave_colour"}


def test_plain_invocation_cannot_create_canonical_migrations(repo: Path) -> None:
    draft(repo, "add_wave_colour")
    before = (repo / MIGRATIONS_RS).read_text()

    result = subprocess.run(
        [sys.executable, "scripts/canonicalize_migrations.py", "0.11.30"],
        cwd=repo,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 2
    assert "outside the release cut" in result.stderr
    assert (repo / MIGRATIONS_RS).read_text() == before
    assert canonical_files(repo) == ["0.11.001_initial.sql"]
    assert draft_names(repo) == {"add_wave_colour"}


def test_test_materialization_has_explicit_disposable_authority(repo: Path) -> None:
    draft(repo, "add_wave_colour")

    result = run(repo, "0.12.0", "--materialize-for-tests")

    assert result.returncode == 0, result.stderr
    assert (repo / MIGRATIONS / "0.12.0.001_release.sql").exists()


def test_test_materialization_models_the_next_patch_after_a_release(repo: Path) -> None:
    (repo / "Cargo.toml").write_text(
        '[workspace]\n\n[workspace.package]\nversion = "0.12.2"\nedition = "2021"\n'
    )
    draft(repo, "first_patch")
    assert run(repo, "0.12.2").returncode == 0
    draft(repo, "next_patch")

    result = run(repo, "0.12.2", "--materialize-for-tests")

    assert result.returncode == 0, result.stderr
    assert "0.12.2 -> 0.12.3" in result.stdout
    assert (repo / MIGRATIONS / "0.12.3.001_release.sql").exists()
    assert 'version = "0.12.3"' in (repo / "Cargo.toml").read_text()


def test_re_running_the_same_release_is_deterministic(tmp_path: Path) -> None:
    def build(name: str) -> Path:
        repo = tmp_path / name
        (repo / "scripts").mkdir(parents=True)
        (repo / "scripts/canonicalize_migrations.py").write_bytes(SCRIPT.read_bytes())
        (repo / MIGRATIONS).mkdir(parents=True)
        (repo / MIGRATIONS / "0.11.001_initial.sql").write_text("CREATE TABLE waves (id TEXT);\n")
        (repo / MIGRATIONS_RS).write_text(REGISTRY)
        (repo / DRAFTS).mkdir()
        draft(repo, "add_wave_colour")
        draft(repo, "add_task_priority")
        return repo

    first, second = build("first"), build("second")
    assert run(first, "0.11.30").returncode == 0
    assert run(second, "0.11.30").returncode == 0

    assert canonical_files(first) == canonical_files(second)
    assert (first / MIGRATIONS_RS).read_text() == (second / MIGRATIONS_RS).read_text()


def test_a_release_cannot_publish_a_second_batch(repo: Path) -> None:
    draft(repo, "add_wave_colour")
    assert run(repo, "0.11.30").returncode == 0
    draft(repo, "add_task_priority")

    result = run(repo, "0.11.30")

    assert result.returncode == 1
    assert "already has canonical migration" in result.stderr
    assert draft_names(repo) == {"add_task_priority"}


def test_an_abandoned_release_leaves_drafts_regenerable(repo: Path) -> None:
    draft(repo, "add_wave_colour")
    # A failed release never merged its ids; --check proves the plan without
    # publishing, and a later real run regenerates the same id.
    assert "0.11.30.001_release" in run(repo, "0.11.30", "--check").stdout
    run(repo, "0.11.30")
    assert (repo / MIGRATIONS / "0.11.30.001_release.sql").exists()


def test_a_dependency_on_a_released_migration_canonicalizes_after_it(repo: Path) -> None:
    # `initial` is the already-released 0.11.001; depending on it across the
    # release boundary is legal and imposes no in-cut ordering.
    draft(repo, "add_wave_colour", depends_on="initial")

    result = run(repo, "0.11.30")

    assert result.returncode == 0, result.stderr
    assert (repo / MIGRATIONS / "0.11.30.001_release.sql").exists()


def test_a_dependency_can_name_a_draft_from_an_earlier_release_batch(repo: Path) -> None:
    draft(repo, "add_wave_colour")
    assert run(repo, "0.11.30").returncode == 0
    draft(repo, "backfill_colour", depends_on="add_wave_colour")

    result = run(repo, "0.11.31")

    assert result.returncode == 0, result.stderr
    assert (repo / MIGRATIONS / "0.11.31.001_release.sql").exists()


def test_a_dependency_on_an_unknown_name_fails(repo: Path) -> None:
    draft(repo, "add_wave_colour", depends_on="nonexistent")
    before = (repo / MIGRATIONS_RS).read_text()

    result = run(repo, "0.11.30")

    assert result.returncode == 1
    assert "neither a draft" in result.stderr
    assert (repo / MIGRATIONS_RS).read_text() == before
    assert draft_names(repo) == {"add_wave_colour"}


def test_a_draft_cannot_forge_release_provenance(repo: Path) -> None:
    draft(repo, "add_wave_colour", body="-- draft: invented\nSELECT 1;\n")

    result = run(repo, "0.11.30")

    assert result.returncode == 1
    assert "reserved `-- draft:`" in result.stderr
    assert draft_names(repo) == {"add_wave_colour"}


def test_two_drafts_sharing_a_readable_name_fail(repo: Path) -> None:
    # Distinct tokens keep the files distinct at authoring; a shared readable
    # name only ever surfaces as one release-cut failure, never a merge conflict.
    draft(repo, "dup", token="a" * 32)
    draft(repo, "dup", token="b" * 32)
    before = (repo / MIGRATIONS_RS).read_text()

    result = run(repo, "0.11.30")

    assert result.returncode == 1
    assert "share the readable name" in result.stderr
    assert (repo / MIGRATIONS_RS).read_text() == before


def test_a_write_failure_leaves_the_tree_byte_identical(repo: Path) -> None:
    # Force the atomic registry replace to fail (its parent dir is read-only)
    # after the canonical files are written, and prove the rollback restores the
    # tree byte-for-byte: registry, canonical dir, and drafts all unchanged.
    draft(repo, "add_wave_colour", body="ALTER TABLE waves ADD COLUMN colour TEXT;\n")
    before_registry = (repo / MIGRATIONS_RS).read_text()
    before_canonical = canonical_files(repo)
    before_drafts = {p.name: p.read_text() for p in (repo / DRAFTS).glob("*.sql")}

    store_dir = (repo / MIGRATIONS_RS).parent
    store_dir.chmod(0o500)
    try:
        result = run(repo, "0.11.30")
    finally:
        store_dir.chmod(0o700)

    assert result.returncode == 1
    assert "restored byte-for-byte" in result.stderr
    assert (repo / MIGRATIONS_RS).read_text() == before_registry
    assert canonical_files(repo) == before_canonical
    assert {p.name: p.read_text() for p in (repo / DRAFTS).glob("*.sql")} == before_drafts
