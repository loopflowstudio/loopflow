# SkillRegistry Integration Design

> "lets integrate with https://skillregistry.io/"

## What to build

Add SkillRegistry as a remote skill source so `lf` can list and run registry skills via a prefix (default `sr:`) with local caching.

## Data structures

```python
@dataclass
class RegistrySkill:
    id: str
    name: str
    description: str
    updated_at: datetime | None

class SkillRegistryConfig(BaseModel):
    enabled: bool = False
    base_url: str = "https://skillregistry.io"
    prefix: str = "sr"
    cache_ttl_seconds: int = 86400
    cache_dir: Optional[str] = None  # default: ~/.lf/skills/skillregistry
```

```python
@dataclass
class SkillSource:
    name: str
    prefix: str
    path: Path
    skills: list[str]
    kind: str = "local"  # "local" or "registry"
    base_url: str | None = None
```

```python
@dataclass
class Skill:
    name: str
    source: str
    prompt_path: Path  # path to cached SKILL.md
```

## Key functions

```python
def fetch_registry_index(base_url: str) -> list[RegistrySkill]:
    """GET {base_url}/api/skills and return parsed entries."""


def load_registry_cache(cache_path: Path) -> list[RegistrySkill] | None:
    """Return cached list if present and parseable."""


def write_registry_cache(cache_path: Path, skills: list[RegistrySkill]) -> None:
    """Persist list to JSON."""


def get_registry_skills(
    base_url: str,
    cache_path: Path,
    ttl_seconds: int,
) -> list[RegistrySkill]:
    """Use cache if fresh; otherwise fetch and cache. Fall back to stale cache on errors."""


def ensure_registry_skill_cached(
    base_url: str,
    cache_dir: Path,
    skill_id: str,
) -> Path:
    """Download {base_url}/skills/{id}.md into cache_dir and return path."""
```

```python
def discover_skill_sources(..., config_sources, registry_config, repo_root) -> list[SkillSource]:
    """Existing local discovery + optional SkillRegistry source."""


def find_skill(name: str, sources: list[SkillSource]) -> Skill | None:
    """If source.kind == 'registry', ensure cached file and return Skill."""
```

## UI changes

- CLI listing (`lf --list`) should show an **EXTERNAL SKILLS** section for SkillRegistry when enabled.
- Config docs: add `skill_registry` block and show `lf sr:<skill>` usage.

## Constraints

- **Network optional:** If SkillRegistry is disabled, no network calls. If enabled and the network fails, fall back to cached list or skip registry gracefully.
- **No extra dependencies:** Use stdlib (`urllib.request`, `json`) for HTTP.
- **Cache location:** Default to `~/.lf/skills/skillregistry/` with a `registry.json` index and per-skill `SKILL.md` files.
- **Prefix conflicts:** If `skill_registry.prefix` matches an existing configured source prefix, skip registry and warn.

## Done when

- `lf --list` shows SkillRegistry skills when `skill_registry.enabled: true` in `.lf/config.yaml`.
- `lf sr:gog` downloads `https://skillregistry.io/skills/gog.md` once, caches it, and runs the skill prompt.
- `uv run pytest tests/test_skills.py` passes with new registry tests.
