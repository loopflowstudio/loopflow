"""Tests for loopflow.lf.skills module."""

import pytest

from loopflow.lf.config import SkillRegistryConfig, SkillSourceConfig
from loopflow.lf.skills import (
    _REGISTRY_INDEX_FILE,
    RegistrySkill,
    Skill,
    SkillSource,
    _discover_superpowers_skills,
    _get_registry_skills,
    _normalize_skill_name,
    _write_registry_cache,
    discover_skill_sources,
    find_skill,
    list_all_skills,
    load_skill_prompt,
)

# =============================================================================
# Skill name normalization
# =============================================================================


def test_normalize_skill_name_removes_ing_suffix():
    assert _normalize_skill_name("brainstorming") == "brainstorm"
    assert _normalize_skill_name("writing") == "writ"


def test_normalize_skill_name_removes_s_suffix():
    assert _normalize_skill_name("plans") == "plan"
    assert _normalize_skill_name("tests") == "test"


def test_normalize_skill_name_replaces_underscores():
    assert _normalize_skill_name("write_plan") == "write-plan"


def test_normalize_skill_name_lowercases():
    assert _normalize_skill_name("BrainStorming") == "brainstorm"


def test_normalize_skill_name_tdd_special_case():
    assert _normalize_skill_name("test-driven-development") == "tdd"


def test_normalize_skill_name_combined_transformations():
    # "-ing" removal only works on trailing "ing" of the whole string
    # "writing-plans" -> remove trailing "s" -> "writing-plan"
    assert _normalize_skill_name("writing-plans") == "writing-plan"


# =============================================================================
# Skill discovery
# =============================================================================


@pytest.fixture
def superpowers_dir(tmp_path):
    """Create a superpowers-style skill directory."""
    skills_dir = tmp_path / "skills"
    skills_dir.mkdir()

    # Create brainstorming skill
    brainstorm = skills_dir / "brainstorming"
    brainstorm.mkdir()
    (brainstorm / "SKILL.md").write_text("# Brainstorm skill\n\nBrainstorm ideas.")

    # Create writing-plans skill
    write_plan = skills_dir / "writing-plans"
    write_plan.mkdir()
    (write_plan / "SKILL.md").write_text("# Write Plan skill\n\nCreate plans.")

    # Create directory without SKILL.md (should be ignored)
    no_skill = skills_dir / "incomplete"
    no_skill.mkdir()
    (no_skill / "README.md").write_text("Not a skill")

    return tmp_path


def test_discover_superpowers_skills_finds_skills(superpowers_dir):
    skills = _discover_superpowers_skills(superpowers_dir)
    assert "brainstorm" in skills
    assert "writing-plan" in skills  # "-ing" removal works on trailing "ing" only
    assert "incomplete" not in skills


def test_discover_superpowers_skills_returns_sorted(superpowers_dir):
    skills = _discover_superpowers_skills(superpowers_dir)
    assert skills == sorted(skills)


def test_discover_superpowers_skills_empty_if_no_skills_dir(tmp_path):
    skills = _discover_superpowers_skills(tmp_path)
    assert skills == []


def test_discover_superpowers_skills_empty_if_no_valid_skills(tmp_path):
    skills_dir = tmp_path / "skills"
    skills_dir.mkdir()
    (skills_dir / "empty").mkdir()
    skills = _discover_superpowers_skills(tmp_path)
    assert skills == []


# =============================================================================
# discover_skill_sources
# =============================================================================


def test_discover_skill_sources_from_config(superpowers_dir):
    config_sources = [SkillSourceConfig(name="superpowers", prefix="sp", path=str(superpowers_dir))]
    sources = discover_skill_sources(config_sources)

    # Filter out auto-detected sources like rams
    sp_sources = [s for s in sources if s.prefix == "sp"]
    assert len(sp_sources) == 1
    assert sp_sources[0].name == "superpowers"
    assert "brainstorm" in sp_sources[0].skills


def test_discover_skill_sources_skips_nonexistent_path(tmp_path):
    config_sources = [
        SkillSourceConfig(name="missing", prefix="ms", path=str(tmp_path / "nonexistent"))
    ]
    sources = discover_skill_sources(config_sources, auto_detect=False)
    # Filter out auto-detected sources like rams
    ms_sources = [s for s in sources if s.prefix == "ms"]
    assert len(ms_sources) == 0


def test_discover_skill_sources_multiple_sources(tmp_path):
    # Create two skill directories
    sp1 = tmp_path / "skills1"
    sp1.mkdir()
    skills1 = sp1 / "skills"
    skills1.mkdir()
    skill1 = skills1 / "alpha"
    skill1.mkdir()
    (skill1 / "SKILL.md").write_text("Alpha skill")

    sp2 = tmp_path / "skills2"
    sp2.mkdir()
    skills2 = sp2 / "skills"
    skills2.mkdir()
    skill2 = skills2 / "beta"
    skill2.mkdir()
    (skill2 / "SKILL.md").write_text("Beta skill")

    config_sources = [
        SkillSourceConfig(name="source1", prefix="s1", path=str(sp1)),
        SkillSourceConfig(name="source2", prefix="s2", path=str(sp2)),
    ]
    sources = discover_skill_sources(config_sources, auto_detect=False)

    # Filter to only the explicitly configured sources
    configured_sources = [s for s in sources if s.prefix in {"s1", "s2"}]
    assert len(configured_sources) == 2
    prefixes = {s.prefix for s in configured_sources}
    assert prefixes == {"s1", "s2"}


def test_discover_skill_sources_auto_detects_local_superpowers(tmp_path):
    # Create repo-local superpowers
    sp = tmp_path / "superpowers"
    sp.mkdir()
    skills = sp / "skills"
    skills.mkdir()
    skill = skills / "local-skill"
    skill.mkdir()
    (skill / "SKILL.md").write_text("Local skill")

    sources = discover_skill_sources(None, repo_root=tmp_path)

    # Should include the local superpowers
    sp_sources = [s for s in sources if s.prefix == "sp"]
    assert len(sp_sources) == 1
    assert "local-skill" in sp_sources[0].skills


def test_discover_skill_sources_config_prevents_auto_detection(tmp_path, superpowers_dir):
    # Create repo-local superpowers (should be ignored if sp already configured)
    local_sp = tmp_path / "superpowers"
    local_sp.mkdir()
    skills = local_sp / "skills"
    skills.mkdir()
    skill = skills / "local-only"
    skill.mkdir()
    (skill / "SKILL.md").write_text("Local only")

    # Configure sp prefix explicitly
    config_sources = [SkillSourceConfig(name="superpowers", prefix="sp", path=str(superpowers_dir))]
    sources = discover_skill_sources(config_sources, repo_root=tmp_path)

    # Should use configured source, not local
    sp_sources = [s for s in sources if s.prefix == "sp"]
    assert len(sp_sources) == 1
    assert "brainstorm" in sp_sources[0].skills
    assert "local-only" not in sp_sources[0].skills


def test_discover_skill_sources_no_sources_returns_empty():
    sources = discover_skill_sources(None, repo_root=None)
    # May be empty or may find ~/.superpowers if it exists
    # Just verify it doesn't crash
    assert isinstance(sources, list)


def test_discover_skill_sources_auto_detects_rams(tmp_path, monkeypatch):
    """rams.ai is auto-detected when ~/.claude/commands/rams.md exists."""
    rams_dir = tmp_path / ".claude" / "commands"
    rams_dir.mkdir(parents=True)
    rams_file = rams_dir / "rams.md"
    rams_file.write_text("# Rams\nAccessibility and design review.")

    monkeypatch.setattr("loopflow.lf.skills._RAMS_PATH", rams_file)

    sources = discover_skill_sources(None, repo_root=None, auto_detect=False)

    rams_sources = [s for s in sources if s.prefix == "rams"]
    assert len(rams_sources) == 1
    assert rams_sources[0].name == "rams.ai"
    assert rams_sources[0].kind == "single-file"
    assert "rams" in rams_sources[0].skills


# =============================================================================
# SkillRegistry discovery
# =============================================================================


def test_discover_skill_sources_registry_enabled(tmp_path, monkeypatch):
    registry_skills = [
        RegistrySkill(id="alpha", name="Alpha", description="alpha", updated_at=None),
        RegistrySkill(id="beta", name="Beta", description="beta", updated_at=None),
    ]

    def fake_get_registry_skills(base_url, cache_path, ttl_seconds):
        return registry_skills

    monkeypatch.setattr("loopflow.lf.skills._get_registry_skills", fake_get_registry_skills)

    config = SkillRegistryConfig(
        enabled=True,
        base_url="https://example.com",
        prefix="sr",
        cache_dir=str(tmp_path),
    )

    sources = discover_skill_sources(
        None,
        repo_root=None,
        auto_detect=False,
        registry_config=config,
    )

    # Find the registry source
    sr_sources = [s for s in sources if s.prefix == "sr"]
    assert len(sr_sources) == 1
    source = sr_sources[0]
    assert source.kind == "registry"
    assert "alpha" in source.skills


def test_get_registry_skills_uses_cache(tmp_path, monkeypatch):
    cache_path = tmp_path / _REGISTRY_INDEX_FILE
    sample = [RegistrySkill(id="alpha", name="Alpha", description="alpha", updated_at=None)]
    _write_registry_cache(cache_path, sample)

    def fail_fetch(base_url):
        raise AssertionError("should not fetch")

    monkeypatch.setattr("loopflow.lf.skills._fetch_registry_index", fail_fetch)

    skills = _get_registry_skills("https://example.com", cache_path, ttl_seconds=86400)
    assert [skill.id for skill in skills] == ["alpha"]


# =============================================================================
# find_skill
# =============================================================================


def test_find_skill_returns_skill_when_found(superpowers_dir):
    sources = [
        SkillSource(
            name="superpowers",
            prefix="sp",
            path=superpowers_dir,
            skills=["brainstorm", "writ-plan"],
        )
    ]
    skill = find_skill("sp:brainstorm", sources)

    assert skill is not None
    assert skill.name == "brainstorm"
    assert skill.source == "superpowers"
    assert skill.prompt_path.name == "SKILL.md"


def test_find_skill_returns_none_for_unknown_skill(superpowers_dir):
    sources = [
        SkillSource(
            name="superpowers",
            prefix="sp",
            path=superpowers_dir,
            skills=["brainstorm"],
        )
    ]
    skill = find_skill("sp:unknown", sources)
    assert skill is None


def test_find_skill_returns_none_for_unknown_prefix(superpowers_dir):
    sources = [
        SkillSource(
            name="superpowers",
            prefix="sp",
            path=superpowers_dir,
            skills=["brainstorm"],
        )
    ]
    skill = find_skill("xx:brainstorm", sources)
    assert skill is None


def test_find_skill_returns_none_without_colon():
    sources = []
    skill = find_skill("brainstorm", sources)
    assert skill is None


def test_find_skill_handles_multiple_sources(tmp_path):
    # Create two sources with different prefixes
    sp1 = tmp_path / "source1" / "skills"
    sp1.mkdir(parents=True)
    skill1 = sp1 / "alpha"
    skill1.mkdir()
    (skill1 / "SKILL.md").write_text("Alpha from s1")

    sp2 = tmp_path / "source2" / "skills"
    sp2.mkdir(parents=True)
    skill2 = sp2 / "alpha"
    skill2.mkdir()
    (skill2 / "SKILL.md").write_text("Alpha from s2")

    sources = [
        SkillSource(name="s1", prefix="s1", path=tmp_path / "source1", skills=["alpha"]),
        SkillSource(name="s2", prefix="s2", path=tmp_path / "source2", skills=["alpha"]),
    ]

    skill1_result = find_skill("s1:alpha", sources)
    skill2_result = find_skill("s2:alpha", sources)

    assert skill1_result.source == "s1"
    assert skill2_result.source == "s2"


def test_find_skill_handles_registry_source(tmp_path, monkeypatch):
    cached = tmp_path / "alpha.md"
    cached.write_text("# Alpha")

    def fake_cache(base_url, cache_dir, skill_id):
        return cached

    monkeypatch.setattr("loopflow.lf.skills._ensure_registry_skill_cached", fake_cache)

    sources = [
        SkillSource(
            name="skillregistry",
            prefix="sr",
            path=tmp_path,
            skills=["alpha"],
            kind="registry",
            base_url="https://example.com",
            cache_dir=tmp_path,
        )
    ]

    skill = find_skill("sr:alpha", sources)

    assert skill is not None
    assert skill.name == "alpha"
    assert skill.prompt_path == cached


def test_find_skill_handles_single_file_source(tmp_path):
    """Single-file skills (like rams) work correctly."""
    skill_file = tmp_path / "rams.md"
    skill_file.write_text("# Rams skill\nReview accessibility and design.")

    sources = [
        SkillSource(
            name="rams.ai",
            prefix="rams",
            path=tmp_path,
            skills=["rams"],
            kind="single-file",
        )
    ]

    skill = find_skill("rams:rams", sources)

    assert skill is not None
    assert skill.name == "rams"
    assert skill.source == "rams.ai"
    assert skill.prompt_path == skill_file


# =============================================================================
# load_skill_prompt
# =============================================================================


def test_load_skill_prompt_returns_content(superpowers_dir):
    skill = Skill(
        name="brainstorm",
        source="superpowers",
        prompt_path=superpowers_dir / "skills" / "brainstorming" / "SKILL.md",
    )
    content = load_skill_prompt(skill)
    assert "Brainstorm skill" in content
    assert "Brainstorm ideas." in content


# =============================================================================
# list_all_skills
# =============================================================================


def test_list_all_skills_returns_prefixed_names():
    sources = [
        SkillSource(name="sp", prefix="sp", path=None, skills=["brainstorm", "plan"]),
    ]
    skills = list_all_skills(sources)

    assert ("sp:brainstorm", "sp") in skills
    assert ("sp:plan", "sp") in skills


def test_list_all_skills_includes_multiple_sources():
    sources = [
        SkillSource(name="sp", prefix="sp", path=None, skills=["alpha"]),
        SkillSource(name="custom", prefix="cx", path=None, skills=["beta"]),
    ]
    skills = list_all_skills(sources)

    assert ("sp:alpha", "sp") in skills
    assert ("cx:beta", "custom") in skills


def test_list_all_skills_returns_sorted():
    sources = [
        SkillSource(name="z", prefix="zz", path=None, skills=["zzz"]),
        SkillSource(name="a", prefix="aa", path=None, skills=["aaa"]),
    ]
    skills = list_all_skills(sources)

    names = [name for name, _ in skills]
    assert names == sorted(names)


def test_list_all_skills_empty_sources():
    skills = list_all_skills([])
    assert skills == []
