"""External skill source discovery and loading."""

import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from loopflow.lf.config import SkillSourceConfig


@dataclass
class SkillSource:
    """External skill library."""

    name: str
    prefix: str
    path: Path
    skills: list[str] = field(default_factory=list)


@dataclass
class Skill:
    """A skill from an external source."""

    name: str
    source: str
    prompt_path: Path


# Default superpowers locations to check
_SUPERPOWERS_PATHS = [
    Path.home() / ".superpowers",
    Path.home() / "superpowers",
]


def _normalize_skill_name(dir_name: str) -> str:
    """Normalize directory name to skill name.

    brainstorming -> brainstorm
    writing-plans -> write-plan
    test-driven-development -> tdd
    """
    name = dir_name.lower()
    name = re.sub(r"ing$", "", name)  # brainstorming -> brainstorm
    name = re.sub(r"s$", "", name)  # writing-plans -> writing-plan

    name = name.replace("_", "-")

    if name == "test-driven-development":
        return "tdd"

    return name


def _discover_superpowers_skills(source_path: Path) -> list[str]:
    """Discover skills in a superpowers installation."""
    skills_dir = source_path / "skills"
    if not skills_dir.exists():
        return []

    skills = []
    for entry in skills_dir.iterdir():
        if entry.is_dir():
            skill_file = entry / "SKILL.md"
            if skill_file.exists():
                skills.append(_normalize_skill_name(entry.name))

    return sorted(skills)


def _find_skill_prompt_path(source_path: Path, skill_name: str) -> Path | None:
    """Find the prompt file for a skill."""
    skills_dir = source_path / "skills"
    if not skills_dir.exists():
        return None

    for entry in skills_dir.iterdir():
        if entry.is_dir():
            normalized = _normalize_skill_name(entry.name)
            if normalized == skill_name:
                skill_file = entry / "SKILL.md"
                if skill_file.exists():
                    return skill_file

    return None


def discover_skill_sources(
    config_sources: "list[SkillSourceConfig] | None" = None,
    repo_root: Path | None = None,
    *,
    auto_detect: bool = True,
) -> list[SkillSource]:
    """Find configured skill libraries.

    Checks:
    1. Explicit config sources
    2. Auto-detection at ~/.superpowers or ./superpowers (if auto_detect=True)
    """
    sources = []
    seen_prefixes = set()

    if config_sources:
        for source_config in config_sources:
            path = Path(source_config.path).expanduser()
            if not path.exists():
                continue

            skills = _discover_superpowers_skills(path)
            sources.append(
                SkillSource(
                    name=source_config.name,
                    prefix=source_config.prefix,
                    path=path,
                    skills=skills,
                )
            )
            seen_prefixes.add(source_config.prefix)

    # Auto-detect superpowers if not explicitly configured
    if auto_detect and "sp" not in seen_prefixes:
        # Check repo-local first
        if repo_root:
            local_path = repo_root / "superpowers"
            if local_path.exists():
                skills = _discover_superpowers_skills(local_path)
                if skills:
                    sources.append(
                        SkillSource(
                            name="superpowers",
                            prefix="sp",
                            path=local_path,
                            skills=skills,
                        )
                    )
                    seen_prefixes.add("sp")

        # Then check default locations
        if "sp" not in seen_prefixes:
            for default_path in _SUPERPOWERS_PATHS:
                if default_path.exists():
                    skills = _discover_superpowers_skills(default_path)
                    if skills:
                        sources.append(
                            SkillSource(
                                name="superpowers",
                                prefix="sp",
                                path=default_path,
                                skills=skills,
                            )
                        )
                        break

    return sources


def find_skill(name: str, sources: list[SkillSource]) -> Skill | None:
    """Resolve 'sp:brainstorm' to a Skill.

    Returns None if not found.
    """
    if ":" not in name:
        return None

    prefix, skill_name = name.split(":", 1)

    for source in sources:
        if source.prefix == prefix and skill_name in source.skills:
            prompt_path = _find_skill_prompt_path(source.path, skill_name)
            if prompt_path:
                return Skill(
                    name=skill_name,
                    source=source.name,
                    prompt_path=prompt_path,
                )

    return None


def load_skill_prompt(skill: Skill) -> str:
    """Extract prompt content from skill definition."""
    return skill.prompt_path.read_text()


def list_all_skills(sources: list[SkillSource]) -> list[tuple[str, str]]:
    """Return all skills as (prefixed_name, source_name) tuples."""
    skills = []
    for source in sources:
        for skill_name in source.skills:
            prefixed = f"{source.prefix}:{skill_name}"
            skills.append((prefixed, source.name))
    return sorted(skills)
