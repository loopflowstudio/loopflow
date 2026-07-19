"""The release gate: a migration that already shipped can never change.

Each test builds a throwaway repo with a release tag, so the check runs against a
real tag rather than the one this repo happens to be on.
"""

import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/check_migrations.py"
MIGRATIONS = "rust/loopflow/src/store/migrations"
MIGRATIONS_RS = "rust/loopflow/src/store/migrations.rs"


def _git(repo: Path, *args: str) -> None:
    subprocess.run(["git", *args], cwd=repo, check=True, capture_output=True)


def _registry(*entries: tuple) -> str:
    """A MIGRATIONS registry in Rust, shaped as migrations.rs writes it."""
    rendered = []
    for entry in entries:
        if len(entry) == 4:
            major, minor, ordinal, name = entry
            patch = None
            filename = f"{major}.{minor}.{ordinal:03d}_{name}.sql"
        else:
            major, minor, patch, ordinal, name = entry
            filename = f"{major}.{minor}.{patch}.{ordinal:03d}_{name}.sql"
        patch_value = "None" if patch is None else f"Some({patch})"
        rendered.append(
            f"""Migration {{
    id: MigrationId {{
        major: {major},
        minor: {minor},
        patch: {patch_value},
        ordinal: {ordinal},
    }},
    name: "{name}",
    sql: include_str!("migrations/{filename}"),
}}, """
        )
    return f"const MIGRATIONS: &[Migration] = &[{''.join(rendered)}];\n"


def _register(repo: Path, *entries: tuple) -> None:
    (repo / MIGRATIONS_RS).write_text(_registry(*entries))


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """A repo tagged v0.10.1 with one shipped migration, `0.10.001_initial.sql`."""
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts/check_migrations.py").write_bytes(SCRIPT.read_bytes())
    (tmp_path / MIGRATIONS).mkdir(parents=True)
    (tmp_path / "Cargo.toml").write_text('[workspace.package]\nversion = "0.10.1"\n')
    (tmp_path / "pyproject.toml").write_text('[project]\nversion = "0.10.1"\n')
    (tmp_path / MIGRATIONS / "0.10.001_initial.sql").write_text("CREATE TABLE waves (id TEXT);\n")
    _register(tmp_path, (0, 10, 1, "initial"))

    _git(tmp_path, "init", "-q")
    _git(tmp_path, "config", "user.email", "test@example.com")
    _git(tmp_path, "config", "user.name", "test")
    _git(tmp_path, "add", ".")
    _git(tmp_path, "commit", "-qm", "release")
    _git(tmp_path, "tag", "v0.10.1")
    return tmp_path


def check(repo: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, "scripts/check_migrations.py"],
        cwd=repo,
        capture_output=True,
        text=True,
    )


def test_a_shipped_migration_left_alone_passes(repo: Path):
    result = check(repo)
    assert result.returncode == 0, result.stderr
    assert "unchanged since v0.10.1" in result.stdout


def test_a_batch_matching_the_active_package_release_passes(repo: Path):
    (repo / MIGRATIONS / "0.10.1.001_release.sql").write_text(
        "-- draft: add_note\nALTER TABLE waves ADD note TEXT;\n"
    )
    _register(repo, (0, 10, 1, "initial"), (0, 10, 1, 1, "release"))

    result = check(repo)
    assert result.returncode == 0, result.stderr


def test_a_migration_file_nobody_registered_fails(repo: Path):
    """An unregistered migration never runs — shipping one is shipping a no-op."""
    (repo / MIGRATIONS / "0.10.1.001_release.sql").write_text(
        "-- draft: add_note\nALTER TABLE waves ADD note TEXT;\n"
    )

    result = check(repo)
    assert result.returncode == 1
    assert "not in the MIGRATIONS registry" in result.stderr


def test_a_registry_entry_without_its_file_fails(repo: Path):
    _register(repo, (0, 10, 1, "initial"), (0, 10, 2, "add_note"))

    result = check(repo)
    assert result.returncode == 1
    assert "has no file" in result.stderr


def test_duplicate_registry_ids_fail(repo: Path):
    _register(repo, (0, 10, 1, "initial"), (0, 10, 1, "initial"))

    result = check(repo)
    assert result.returncode == 1
    assert "collides" in result.stderr


def test_a_registry_entry_whose_id_disagrees_with_its_file_fails(repo: Path):
    (repo / MIGRATIONS_RS).write_text(
        _registry((0, 10, 2, "initial")).replace(
            "migrations/0.10.002_initial.sql", "migrations/0.10.001_initial.sql"
        )
    )

    result = check(repo)
    assert result.returncode == 1
    assert "must agree" in result.stderr


def test_a_registry_entry_whose_name_disagrees_with_its_file_fails(repo: Path):
    (repo / MIGRATIONS_RS).write_text(
        _registry((0, 10, 1, "setup")).replace(
            "migrations/0.10.001_setup.sql", "migrations/0.10.001_initial.sql"
        )
    )

    result = check(repo)
    assert result.returncode == 1
    assert "must agree" in result.stderr


def test_a_registry_out_of_id_order_fails(repo: Path):
    (repo / MIGRATIONS / "0.10.002_add_note.sql").write_text("ALTER TABLE waves ADD note TEXT;\n")
    _register(repo, (0, 10, 2, "add_note"), (0, 10, 1, "initial"))

    result = check(repo)
    assert result.returncode == 1
    assert "not in id order" in result.stderr


def test_editing_a_shipped_migration_fails(repo: Path):
    (repo / MIGRATIONS / "0.10.001_initial.sql").write_text("CREATE TABLE waves (id INTEGER);\n")

    result = check(repo)
    assert result.returncode == 1
    assert "has been edited" in result.stderr


def test_renaming_a_shipped_migration_fails(repo: Path):
    """Even a rename the registry agrees with: the id is what shipped."""
    (repo / MIGRATIONS / "0.10.001_initial.sql").rename(repo / MIGRATIONS / "0.10.001_setup.sql")
    _register(repo, (0, 10, 1, "setup"))

    result = check(repo)
    assert result.returncode == 1
    assert "is now missing" in result.stderr


def test_moving_the_migration_directory_does_not_void_the_check(repo: Path):
    """Relocating the directory cannot quietly retire the shipped ids inside it."""
    (repo / MIGRATIONS).rename(repo / "rust/loopflow/src/store/elsewhere")

    result = check(repo)
    assert result.returncode == 1
    assert "missing" in result.stderr


def test_a_migration_ahead_of_the_package_version_fails(repo: Path):
    (repo / MIGRATIONS / "0.11.0.001_release.sql").write_text("-- draft: too_new\nSELECT 1;\n")

    result = check(repo)
    assert result.returncode == 1
    assert "namespaced ahead" in result.stderr


def test_a_new_canonical_migration_behind_the_active_namespace_fails(repo: Path):
    (repo / "Cargo.toml").write_text('[workspace.package]\nversion = "0.11.0"\n')
    (repo / "pyproject.toml").write_text('[project]\nversion = "0.11.0"\n')
    (repo / MIGRATIONS / "0.10.1.001_release.sql").write_text("-- draft: too_old\nSELECT 1;\n")
    _register(repo, (0, 10, 1, "initial"), (0, 10, 1, 1, "release"))

    result = check(repo)

    assert result.returncode == 1
    assert "namespaced behind the active package namespace 0.11.0" in result.stderr
    assert "ordinal-free draft" in result.stderr


def test_a_batch_from_an_older_patch_release_fails(repo: Path):
    (repo / "Cargo.toml").write_text('[workspace.package]\nversion = "0.10.2"\n')
    (repo / "pyproject.toml").write_text('[project]\nversion = "0.10.2"\n')
    (repo / MIGRATIONS / "0.10.1.001_release.sql").write_text("-- draft: too_old\nSELECT 1;\n")
    _register(repo, (0, 10, 1, "initial"), (0, 10, 1, 1, "release"))

    result = check(repo)

    assert result.returncode == 1
    assert "active package namespace 0.10.2" in result.stderr


def test_a_new_legacy_three_part_migration_fails(repo: Path):
    (repo / MIGRATIONS / "0.10.002_add_note.sql").write_text("SELECT 1;\n")
    _register(repo, (0, 10, 1, "initial"), (0, 10, 2, "add_note"))

    result = check(repo)

    assert result.returncode == 1
    assert "legacy three-part format" in result.stderr


def test_a_release_cannot_publish_more_than_its_single_batch(repo: Path):
    (repo / MIGRATIONS / "0.10.1.002_release.sql").write_text("-- draft: add_note\nSELECT 1;\n")
    _register(repo, (0, 10, 1, "initial"), (0, 10, 1, 2, "release"))

    result = check(repo)

    assert result.returncode == 1
    assert "single `<version>.001_release.sql`" in result.stderr


def test_a_release_batch_requires_draft_provenance(repo: Path):
    (repo / MIGRATIONS / "0.10.1.001_release.sql").write_text("SELECT 1;\n")
    _register(repo, (0, 10, 1, "initial"), (0, 10, 1, 1, "release"))

    result = check(repo)

    assert result.returncode == 1
    assert "carries no `-- draft:` provenance" in result.stderr


def test_a_malformed_migration_name_fails(repo: Path):
    (repo / MIGRATIONS / "002_oops.sql").write_text("SELECT 1;\n")

    result = check(repo)
    assert result.returncode == 1
    assert "is not" in result.stderr


def test_manifests_disagreeing_on_the_version_fails(repo: Path):
    (repo / "pyproject.toml").write_text('[project]\nversion = "0.11.0"\n')

    result = check(repo)
    assert result.returncode == 1
    assert "disagree" in result.stderr


def test_a_branch_cannot_reuse_an_ordinal_owned_by_current_main(repo: Path):
    (repo / MIGRATIONS / "0.10.002_main_change.sql").write_text("SELECT 'main';\n")
    _register(repo, (0, 10, 1, "initial"), (0, 10, 2, "main_change"))
    _git(repo, "add", ".")
    _git(repo, "commit", "-qm", "main migration")
    _git(repo, "update-ref", "refs/remotes/origin/main", "HEAD")
    _git(repo, "checkout", "-qb", "feature", "HEAD~1")

    (repo / MIGRATIONS / "0.10.002_branch_change.sql").write_text("SELECT 'branch';\n")
    _register(repo, (0, 10, 1, "initial"), (0, 10, 2, "branch_change"))

    result = check(repo)
    assert result.returncode == 1
    assert "collides with 0.10.002_main_change.sql on origin/main" in result.stderr


# -- Drafts -------------------------------------------------------------------
#
# A draft carries a stable name and no ordinal, so two branches never contend for
# one and a behind-main branch that adds only a draft stays green. The check only
# proves the drafts are well-formed and orderable; the release cut assigns ids.

DRAFTS = MIGRATIONS + "/drafts"


def _draft(repo: Path, name: str, depends_on: str = "", body: str = "SELECT 1;\n") -> None:
    import hashlib

    token = hashlib.sha256(name.encode()).hexdigest()[:32]
    (repo / DRAFTS).mkdir(parents=True, exist_ok=True)
    (repo / DRAFTS / f"{name}__{token}.sql").write_text(
        f"-- name: {name}\n-- id: {token}\n-- depends_on: {depends_on}\n{body}"
    )


def test_a_well_formed_draft_passes(repo: Path):
    _draft(repo, "add_wave_colour")

    result = check(repo)
    assert result.returncode == 0, result.stderr
    assert "1 draft migration(s): add_wave_colour" in result.stdout


def test_two_independent_drafts_pass_and_do_not_collide(repo: Path):
    _draft(repo, "add_wave_colour")
    _draft(repo, "add_task_priority")

    result = check(repo)
    assert result.returncode == 0, result.stderr


def test_a_draft_only_advance_on_main_does_not_fail_a_behind_branch(repo: Path):
    # In the new model, ordinary merges add drafts, not canonical migrations, so
    # main's canonical set does not move between releases. Drafts are never
    # compared against origin/main, so a branch behind main's drafts that adds its
    # own draft stays green — where branch-time ordinal allocation used to collide.
    _draft(repo, "main_draft")
    _git(repo, "add", ".")
    _git(repo, "commit", "-qm", "main draft")
    _git(repo, "update-ref", "refs/remotes/origin/main", "HEAD")
    _git(repo, "checkout", "-qb", "feature", "HEAD~1")  # drops main_draft
    _draft(repo, "feature_draft")

    result = check(repo)
    assert result.returncode == 0, result.stderr


def test_a_draft_depending_on_a_declared_draft_passes(repo: Path):
    _draft(repo, "add_wave_colour")
    _draft(repo, "backfill_colour", depends_on="add_wave_colour")

    result = check(repo)
    assert result.returncode == 0, result.stderr


def test_a_draft_depending_on_no_draft_fails(repo: Path):
    _draft(repo, "backfill_colour", depends_on="add_wave_colour")

    result = check(repo)
    assert result.returncode == 1
    assert "neither a draft" in result.stderr


def test_a_draft_dependency_cycle_fails(repo: Path):
    _draft(repo, "a", depends_on="b")
    _draft(repo, "b", depends_on="a")

    result = check(repo)
    assert result.returncode == 1
    assert "cycle" in result.stderr


def test_a_draft_cannot_forge_release_provenance(repo: Path):
    _draft(repo, "add_wave_colour", body="-- draft: invented\nSELECT 1;\n")

    result = check(repo)

    assert result.returncode == 1
    assert "reserved `-- draft:`" in result.stderr


def test_a_draft_colliding_with_a_released_name_fails(repo: Path):
    _draft(repo, "initial")

    result = check(repo)
    assert result.returncode == 1
    assert "collides with a released migration" in result.stderr


def test_a_draft_header_disagreeing_with_its_filename_fails(repo: Path):
    (repo / DRAFTS).mkdir(parents=True)
    (repo / DRAFTS / "add_wave_colour__deadbeefdeadbeefdeadbeefdeadbeef.sql").write_text(
        "-- name: something_else\n-- id: deadbeefdeadbeefdeadbeefdeadbeef\n-- depends_on: \n"
    )

    result = check(repo)
    assert result.returncode == 1
    assert "not 'add_wave_colour'" in result.stderr


def test_a_draft_without_a_name_header_fails(repo: Path):
    (repo / DRAFTS).mkdir(parents=True)
    (repo / DRAFTS / "add_wave_colour__deadbeefdeadbeefdeadbeefdeadbeef.sql").write_text(
        "ALTER TABLE waves ADD colour TEXT;\n"
    )

    result = check(repo)
    assert result.returncode == 1
    assert "no `-- name:` header" in result.stderr


def test_the_drafts_readme_is_ignored(repo: Path):
    _draft(repo, "add_wave_colour")
    (repo / DRAFTS / "README.md").write_text("# Draft migrations\n")

    result = check(repo)
    assert result.returncode == 0, result.stderr
