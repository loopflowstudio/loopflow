# Global Config & Auto-Prune

**What to build:** Global `~/.lf/` directory for user-wide config, steps, goals, and flows—plus automatic worktree cleanup when PRs merge.

## Data Structures

```python
# In config.py - extend existing Config model

@dataclass
class AutopruneConfig:
    enabled: bool = False
    poll_interval_seconds: int = 60  # 1 minute

class Config(BaseModel):
    # ... existing fields ...
    autoprune: AutopruneConfig | bool = False  # bool for simple enable
```

Config merge logic:

```python
def load_config(repo_root: Path) -> Config:
    """Load config, merging global (~/.lf/config.yaml) with repo (.lf/config.yaml)."""
    global_config = _load_yaml(Path.home() / ".lf" / "config.yaml")
    repo_config = _load_yaml(repo_root / ".lf" / "config.yaml")
    return _merge_configs(global_config, repo_config)

def _merge_configs(global_cfg: dict, repo_cfg: dict) -> Config:
    """Repo overrides global. Additive lists combine."""
    merged = {**global_cfg}

    ADDITIVE_KEYS = {"context", "exclude", "skill_sources", "summaries"}

    for key, value in repo_cfg.items():
        if key in ADDITIVE_KEYS and key in merged:
            merged[key] = merged[key] + value  # combine lists
        else:
            merged[key] = value  # repo wins

    return Config(**merged)
```

## Key Functions

### Global Content Discovery

Terminology: "steps" everywhere in loopflow code. "commands" only refers to Claude Code's `.claude/commands/` directory.

```python
# In context.py - extend existing discovery

def gather_step(repo_root: Path, name: str, config: Config) -> StepFile:
    """Search order: external skills → repo → global → builtin."""
    # Search order:
    # 1. External skills (prefix:name format)
    # 2. .claude/commands/{name}.md  (repo, Claude Code compat)
    # 3. .lf/steps/{name}.md         (repo)
    # 4. .lf/{name}.md               (repo, legacy)
    # 5. ~/.lf/steps/{name}.md       (global) ← NEW
    # 6. ~/.claude/commands/{name}.md (global, Claude Code compat)
    # 7. templates/steps/{name}.md   (builtin)

def load_goal(repo_root: Path, goal_name: str) -> Goal:
    """Search order: repo → global → builtin."""
    # 1. .lf/goals/{name}.md         (repo)
    # 2. ~/.lf/goals/{name}.md       (global) ← NEW
    # 3. templates/goals/{name}.md   (builtin)

def load_flow(repo_root: Path, flow_name: str) -> Flow:
    """Search order: repo → global → builtin."""
    # 1. .lf/flows/{name}.py         (repo)
    # 2. ~/.lf/flows/{name}.py       (global) ← NEW
    # 3. templates/flows/{name}.py   (builtin)
```

### Auto-Prune

Existing `lfops wt prune` is one-shot: syncs main, finds merged worktrees, prompts, removes. No polling.

Auto-prune adds polling to lfd daemon. When enabled, lfd periodically checks for merged worktrees and removes them automatically.

```python
# In lfd/server.py - extend periodic check

async def _periodic_check(self):
    """Run periodic maintenance tasks."""
    while True:
        await asyncio.sleep(60)
        await self._cleanup_stale_sessions()
        await self._check_autoprune()  # NEW

async def _check_autoprune(self):
    """Prune merged worktrees if enabled."""
    # For each repo with active loops/sessions:
    #   1. Load config, check autoprune.enabled
    #   2. Check if poll_interval has elapsed since last check
    #   3. Call find_merged() to get pruneable worktrees
    #   4. Remove each (skip dirty)
    #   5. Log pruned worktrees
```

```python
# In lfd/autoprune.py (new file)

@dataclass
class PruneState:
    repo: Path
    last_check: datetime | None = None

def check_and_prune(repo: Path, config: Config) -> list[str]:
    """Check for merged worktrees and prune them. Returns pruned branch names."""
    merged = find_merged(repo, base_branch="main")
    pruned = []

    for wt in merged:
        if wt.is_dirty:
            continue
        remove(wt)
        pruned.append(wt.branch)
        log.info(f"Auto-pruned worktree: {wt.branch}")

    return pruned
```

## Config Examples

Simple enable:

```yaml
# ~/.lf/config.yaml
autoprune: true
```

With options:

```yaml
# ~/.lf/config.yaml
autoprune:
  enabled: true
  poll_interval_seconds: 600  # 10 minutes
```

Global defaults with repo override:

```yaml
# ~/.lf/config.yaml (global)
agent_model: claude:opus
voice: concise
context:
  - ~/notes/coding-style.md
skill_sources:
  - name: superpowers
    prefix: sp
    path: ~/.superpowers

# .lf/config.yaml (repo)
agent_model: codex  # overrides global
context:
  - docs/api.md     # combined with global
```

## Constraints

- **Repo always wins for scalars.** If both global and repo set `agent_model`, repo's value is used.
- **Additive keys combine.** `context`, `exclude`, `skill_sources`, `summaries` merge both lists.
- **Auto-prune skips dirty worktrees.** Uncommitted work is never lost.
- **Auto-prune requires lfd running.** Polling happens in daemon's periodic check.
- **No grace period.** Prune as soon as merged is detected.

## Done When

```bash
# Global config works
echo "agent_model: codex" > ~/.lf/config.yaml
cd /tmp/test-repo && lf review -c  # shows codex as model

# Global steps work
mkdir -p ~/.lf/steps
echo "# My step" > ~/.lf/steps/mystep.md
lf --list | grep mystep

# Auto-prune works
# 1. Enable autoprune
echo "autoprune: true" >> ~/.lf/config.yaml

# 2. Create worktree, merge its PR
lfops wt create test-prune
echo "test" > test.txt && git add . && git commit -m "test"
lfops pr && lfops land
# Wait for merge...

# 3. Verify auto-removal (within poll interval)
lfd status  # shows daemon running
sleep 60    # wait for poll
wt list     # test-prune worktree is gone
```

## Open Questions

1. **Webhook later?** Polling works for now. Webhook subscription (GitHub App or manual) could reduce latency from 60s to instant. Worth adding later if polling proves too slow.
