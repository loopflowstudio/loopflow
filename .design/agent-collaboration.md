# Agent Collaboration: Coordination Between Multiple Agents

## The Problem

With personal-main branches, multiple agents can now work continuously on the same repo. But they're unaware of each other:

```
origin/main
    ↑
security-agent-main ←─ security-fix-1
    ↑
refactor-agent-main ←─ cleanup-utils
```

Both agents might touch the same files. Merge conflicts pile up. Rebases fail silently. Work gets duplicated or contradicts.

## What This Expansion Adds

**Conflict awareness**: Before an agent starts, check if its personal-main has diverged from another agent's work. Surface conflicts early, not at land time.

**Coordination protocol**: Agents can declare what files/areas they're working on. Other agents see this and route around.

**Stale detection**: If an agent's personal-main is far behind origin/main, pause and rebase before doing more work.

## Key Functions

```python
# loopflow/lfd/coordination.py

@dataclass
class AgentClaim:
    """Files an agent intends to modify."""
    agent_name: str
    paths: list[str]  # glob patterns like "src/auth/**"
    claimed_at: datetime
    expires_at: datetime | None = None

def check_conflicts(agent: AgentSpec) -> list[str]:
    """Return list of files that conflict with other agents' claims.

    Checks:
    1. Files claimed by other running agents
    2. Files modified on other agents' personal-mains since last sync
    """
    ...

def claim_paths(agent: AgentSpec, paths: list[str], duration_minutes: int = 60) -> bool:
    """Claim paths for this agent. Returns False if conflict."""
    ...

def check_staleness(agent: AgentSpec, threshold_commits: int = 10) -> bool:
    """Return True if personal-main is too far behind origin/main."""
    commits_behind = _count_commits_behind(agent.personal_main, "origin/main")
    return commits_behind > threshold_commits

def auto_rebase_if_clean(agent: AgentSpec) -> bool:
    """Attempt automatic rebase. Returns True if successful."""
    # Only if no conflicts
    # Only if personal-main has no uncommitted work
    ...
```

## Data Storage

Claims stored in `~/.lf/lfd.db`:

```sql
CREATE TABLE agent_claims (
    id TEXT PRIMARY KEY,
    agent_name TEXT NOT NULL,
    path_pattern TEXT NOT NULL,  -- glob like "src/auth/**"
    claimed_at TEXT NOT NULL,
    expires_at TEXT,
    UNIQUE(agent_name, path_pattern)
);
```

## Integration Points

### Pre-run check in runner.py

```python
def run_agent_iteration(...):
    # NEW: Check coordination before starting
    conflicts = check_conflicts(agent)
    if conflicts:
        print(f"Skipping iteration: conflicts with {conflicts}")
        return 0  # Not an error, just skip

    stale = check_staleness(agent)
    if stale:
        if auto_rebase_if_clean(agent):
            print("Auto-rebased personal-main")
        else:
            print("Personal-main is stale; run lfops rebase")
            return 0

    # ... rest of iteration
```

### Design task instructs agent to claim

Inject into agent prompt:

```markdown
## Coordination

Before modifying files, declare your intent:

    lfops claim src/auth/**

This prevents other agents from touching those files until you're done.
When your iteration completes, claims auto-expire.
```

### New CLI commands

```bash
lfops claim src/auth/**      # claim paths for current agent
lfops claims                 # show all active claims
lfops claims --release       # release all claims for current agent
```

## Workflow Example

```bash
# Agent A starts, claims auth files
lfd start security-agent
# → claims src/auth/**, starts iteration

# Agent B starts, sees conflict
lfd start refactor-agent
# → "Waiting: src/auth/** claimed by security-agent"
# → skips iteration, tries again next trigger

# Agent A finishes, claims expire
# Agent B's next trigger succeeds
```

## Scope

This is intentionally minimal:
- Claims are advisory, not locks
- Conflicts skip the iteration rather than queue
- No inter-agent messaging beyond the claim table
- Staleness triggers user action, not automatic complex merges

The goal is surfacing problems early, not solving all coordination automatically.

## Done When

```bash
# Create two agents targeting same files
lfd new agent-a --repo . --pipeline review
lfd new agent-b --repo . --pipeline review

# Start both
lfd start agent-a
# → claims some files, runs iteration

lfd start agent-b
# → sees conflict, skips with message

# After agent-a finishes
lfd start agent-b
# → no conflicts, runs normally

# Staleness check
# (artificially advance origin/main)
lfd start agent-a
# → "Personal-main is 15 commits behind; run lfops rebase"
```

## Open Questions

1. Should claims persist across daemon restarts? (Probably yes, with expiration)
2. How granular should path patterns be? (Start with globs, can add file-level later)
3. Should agents wait/queue or just skip? (Skip is simpler, queue adds complexity)

