# Agents: Future Work

Notes from the `agentstab` design session. These are out of scope for v1 but should be built eventually.

## State Sync

Agent definitions can exist locally (`~/.lf/agents/`) or in repo (`.lf/agents/`). Future: sync between them.

**Behavior:**
- When user creates/modifies locally, auto-create PR to sync to main
- Pull from main when possible
- Stop syncing if merge conflict, notify user
- Default: auto-land sync PRs (configurable)

**Data structures:**

```python
@dataclass
class AgentState:
    """Local state in ~/.lf/agents/{name}.state.json"""
    iteration: int
    last_run: datetime | None
    last_main_sha: str | None               # Track main for sync
    pending_sync: bool                      # True if local changes need PR

@dataclass
class SharedAgentDef:
    """In repo: .lf/agents/{name}.yaml - synced via PR"""
    # Same as AgentDef but lives in repo
```

**Functions:**

```python
def sync_agent_to_main(agent: str) -> None:
    """Create PR to push local agent def to repo"""

def pull_agent_from_main(agent: str) -> bool:
    """Update local from repo, return False if conflict"""
```

## Merge Conflict Handling

When an agent can't cleanly merge its work:
- Stop pulling from main
- Notify user (how? desktop notification? CLI warning on next `lfd status`?)
- User resolves conflict manually
- Agent resumes syncing

## Pipeline Visualization

Once DAG pipelines exist, a way to visualize them:
- `lfd show-pipeline ship` prints ASCII DAG
- Or: generate mermaid diagram

## Agent Collaboration

Agents noticing things for other agents:
- Security agent notices performance issue, leaves note for perf agent
- Structured format in `.research/` (e.g., `{emoji}-observations.md`)
- Agents could be configured to read specific `.research/` files as part of their context

## Rate Limiting

For looping agents:
- `max_iterations_per_day: int` - prevent runaway
- `min_interval_seconds: int` - floor between iterations
- Cost tracking per agent
