"""External skill source discovery and loading."""

import json
import re
import warnings
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import TYPE_CHECKING
from urllib.error import URLError
from urllib.request import urlopen

if TYPE_CHECKING:
    from loopflow.lf.config import SkillRegistryConfig, SkillSourceConfig


@dataclass
class SkillSource:
    """External skill library."""

    name: str
    prefix: str
    path: Path | None
    skills: list[str] = field(default_factory=list)
    kind: str = "local"
    base_url: str | None = None
    cache_dir: Path | None = None


@dataclass
class RegistrySkill:
    """SkillRegistry entry."""

    id: str
    name: str
    description: str
    updated_at: datetime | None


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

# Default rams location (single-file skill at ~/.claude/commands/rams.md)
_RAMS_PATH = Path.home() / ".claude" / "commands" / "rams.md"

_REGISTRY_DEFAULT_BASE_URL = "https://skillregistry.io"
_REGISTRY_DEFAULT_PREFIX = "sr"
_REGISTRY_DEFAULT_TTL_SECONDS = 86400
_REGISTRY_CACHE_DIR = Path.home() / ".lf" / "skills" / "skillregistry"
_REGISTRY_INDEX_FILE = "registry.json"


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


def _parse_registry_datetime(value: str | None) -> datetime | None:
    if not value:
        return None
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None


def _parse_registry_skills(payload: list[dict]) -> list[RegistrySkill]:
    skills = []
    for item in payload:
        skill_id = str(item.get("id") or item.get("name") or "").strip()
        if not skill_id:
            continue
        name = str(item.get("name") or skill_id)
        description = str(item.get("description") or "")
        updated_at = _parse_registry_datetime(item.get("updatedAt") or item.get("updated_at"))
        skills.append(
            RegistrySkill(
                id=skill_id,
                name=name,
                description=description,
                updated_at=updated_at,
            )
        )
    return skills


def _registry_cache_dir(config: "SkillRegistryConfig | None") -> Path:
    if config and config.cache_dir:
        return Path(config.cache_dir).expanduser()
    return _REGISTRY_CACHE_DIR


def _load_registry_cache(cache_path: Path) -> tuple[float, list[RegistrySkill]] | None:
    if not cache_path.exists():
        return None
    try:
        payload = json.loads(cache_path.read_text())
    except json.JSONDecodeError:
        return None
    fetched_at = payload.get("fetched_at")
    skills_payload = payload.get("skills")
    if not isinstance(fetched_at, (int, float)) or not isinstance(skills_payload, list):
        return None
    return float(fetched_at), _parse_registry_skills(skills_payload)


def _write_registry_cache(cache_path: Path, skills: list[RegistrySkill]) -> None:
    cache_path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "fetched_at": datetime.now(timezone.utc).timestamp(),
        "skills": [
            {
                "id": skill.id,
                "name": skill.name,
                "description": skill.description,
                "updated_at": skill.updated_at.isoformat() if skill.updated_at else None,
            }
            for skill in skills
        ],
    }
    cache_path.write_text(json.dumps(payload, indent=2, sort_keys=True))


def _fetch_registry_index(base_url: str) -> list[RegistrySkill]:
    url = f"{base_url.rstrip('/')}/api/skills"
    with urlopen(url, timeout=10) as response:
        data = json.loads(response.read().decode("utf-8"))
    if not isinstance(data, list):
        return []
    return _parse_registry_skills(data)


def _get_registry_skills(
    base_url: str,
    cache_path: Path,
    ttl_seconds: int,
) -> list[RegistrySkill]:
    cached = _load_registry_cache(cache_path)
    if cached:
        fetched_at, cached_skills = cached
        if datetime.now(timezone.utc).timestamp() - fetched_at < ttl_seconds:
            return cached_skills

    try:
        skills = _fetch_registry_index(base_url)
    except (URLError, OSError, json.JSONDecodeError) as exc:
        if cached:
            warnings.warn(f"SkillRegistry fetch failed, using cached list: {exc}")
            return cached[1]
        warnings.warn(f"SkillRegistry fetch failed: {exc}")
        return []

    _write_registry_cache(cache_path, skills)
    return skills


def _ensure_registry_skill_cached(
    base_url: str,
    cache_dir: Path,
    skill_id: str,
) -> Path:
    cache_dir.mkdir(parents=True, exist_ok=True)
    path = cache_dir / f"{skill_id}.md"
    if path.exists():
        return path

    url = f"{base_url.rstrip('/')}/skills/{skill_id}.md"
    with urlopen(url, timeout=10) as response:
        content = response.read().decode("utf-8")
    path.write_text(content)
    return path


def discover_skill_sources(
    config_sources: "list[SkillSourceConfig] | None" = None,
    repo_root: Path | None = None,
    *,
    auto_detect: bool = True,
    registry_config: "SkillRegistryConfig | None" = None,
) -> list[SkillSource]:
    """Find configured skill libraries.

    Checks:
    1. Explicit config sources
    2. Auto-detection at ~/.superpowers or ./superpowers (if auto_detect=True)
    3. SkillRegistry (if enabled)
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

    # Auto-detect rams if installed at ~/.claude/commands/rams.md
    if "rams" not in seen_prefixes and _RAMS_PATH.exists():
        sources.append(
            SkillSource(
                name="rams.ai",
                prefix="rams",
                path=_RAMS_PATH.parent,
                skills=["rams"],
                kind="single-file",
            )
        )
        seen_prefixes.add("rams")

    if registry_config and registry_config.enabled:
        base_url = registry_config.base_url or _REGISTRY_DEFAULT_BASE_URL
        prefix = registry_config.prefix or _REGISTRY_DEFAULT_PREFIX
        cache_dir = _registry_cache_dir(registry_config)
        cache_path = cache_dir / _REGISTRY_INDEX_FILE
        ttl_seconds = registry_config.cache_ttl_seconds or _REGISTRY_DEFAULT_TTL_SECONDS

        if prefix in seen_prefixes:
            warnings.warn(
                "SkillRegistry prefix "
                f"'{prefix}' conflicts with existing sources; skipping registry."
            )
        else:
            registry_skills = _get_registry_skills(base_url, cache_path, ttl_seconds)
            sources.append(
                SkillSource(
                    name="skillregistry",
                    prefix=prefix,
                    path=cache_dir,
                    skills=sorted({skill.id for skill in registry_skills}),
                    kind="registry",
                    base_url=base_url,
                    cache_dir=cache_dir,
                )
            )
            seen_prefixes.add(prefix)

    return sources


def find_skill(name: str, sources: list[SkillSource]) -> Skill | None:
    """Resolve 'sp:brainstorm' to a Skill.

    Returns None if not found.
    """
    if ":" not in name:
        return None

    prefix, skill_name = name.split(":", 1)

    for source in sources:
        if source.prefix != prefix or skill_name not in source.skills:
            continue

        prompt_path = None
        if source.kind == "registry":
            if not source.cache_dir or not source.base_url:
                return None
            prompt_path = _ensure_registry_skill_cached(
                source.base_url,
                source.cache_dir,
                skill_name,
            )
        elif source.kind == "single-file":
            # Single-file skill: path is parent dir, skill name matches filename
            if source.path:
                candidate = source.path / f"{skill_name}.md"
                if candidate.exists():
                    prompt_path = candidate
        elif source.path:
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
