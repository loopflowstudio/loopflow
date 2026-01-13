# Agent Memory

## What this enables

Agents that learn from their work across iterations.

Currently, each agent iteration starts fresh. The agent reads its prompt file, sees the current codebase, and works. It has no knowledge of:

- What it did last iteration
- Whether that work succeeded or failed
- What problems it encountered
- What's already been addressed

This limits agents to reactive work. They can respond to changes ("main has new commits, review them") but can't pursue a thread of work ("continue improving test coverage until it reaches 80%").

With memory, an agent can:

1. **Learn from failures** - "Last time I tried X and it failed with error Y, try a different approach"
2. **Build on success** - "I've addressed auth and logging, now tackle error handling"
3. **Track progress** - "These 5 issues are done, these 3 remain"
4. **Avoid redundant work** - "I already fixed this in iteration 2, skip it"

## Core concept: The iteration journal

Each agent gets a journal file that persists across iterations. Before running, the agent reads its journal. After running, it appends an entry.

```
~/.lf/agents/{agent-id}/journal.md
```

The journal is injected into the prompt like the agent's main prompt file, but it's dynamic - each iteration adds a new entry.

### Journal structure

```markdown
# Agent: docs-quality
# Created: 2025-01-12T10:00:00

## Iteration 1 (2025-01-12T10:00:00)
**Status:** success
**Branch:** agent/docs-quality/1
**Summary:** Added docstrings to cli/run.py and cli/ops.py

### What I did
- Added module-level docstrings to 12 files
- Fixed 3 broken links in README

### What I learned
- The codebase prefers minimal docstrings (STYLE.md)
- Test files don't need docstrings

### Next iteration
- Continue with maestro/ module
- Check for missing README files

---

## Iteration 2 (2025-01-12T15:00:00)
**Status:** failed
**Branch:** agent/docs-quality/2
**Summary:** Attempted to add README to maestro/, tests failed

### What I did
- Created maestro/README.md
- Added docstrings to agent.py

### What went wrong
- Tests failed: test_agent_db.py assertion error on line 45
- The issue was in my code, not existing code

### Next iteration
- Fix the test failure before continuing
- Revert the changes that broke tests

---
```

### How it works

1. **Before each iteration**, read `journal.md` and inject it after the agent prompt
2. **After each iteration**, the agent writes a new journal entry (auto-commit to the worktree)
3. **Journal entries are structured** so the agent can parse them consistently
4. **The journal is stored outside the repo** so it persists even if worktrees are cleaned up

## Data structures

```python
@dataclass
class JournalEntry:
    """One iteration's record in the agent journal."""
    iteration: int
    timestamp: datetime
    status: str  # "success" | "failed" | "partial"
    branch: str
    summary: str
    what_i_did: list[str]
    what_i_learned: list[str]
    what_went_wrong: list[str] | None
    next_iteration: list[str]

    def to_markdown(self) -> str:
        """Format as markdown section."""
        ...

    @classmethod
    def from_markdown(cls, text: str) -> "JournalEntry":
        """Parse from markdown section."""
        ...


@dataclass
class AgentJournal:
    """Persistent memory for an agent."""
    agent_id: str
    agent_name: str
    created_at: datetime
    entries: list[JournalEntry]

    def add_entry(self, entry: JournalEntry) -> None:
        self.entries.append(entry)

    def to_markdown(self) -> str:
        """Format full journal as markdown."""
        ...

    @classmethod
    def load(cls, agent_id: str) -> "AgentJournal | None":
        """Load journal from disk."""
        ...

    def save(self) -> None:
        """Persist journal to disk."""
        ...

    def recent_entries(self, n: int = 5) -> list[JournalEntry]:
        """Get last N entries for prompt injection."""
        ...
```

## How journal entries are created

The agent writes its own journal entry. At the end of each pipeline, we add a "journal" task:

```python
# In run_agent_iteration, after pipeline completes:

# Gather iteration context for journal
iteration_context = {
    "iteration": agent.iteration + 1,
    "branch": branch_name,
    "status": "success" if exit_code == 0 else "failed",
    "commit_messages": _get_commits_on_branch(worktree_path),
}

# Ask the agent to reflect and write a journal entry
journal_prompt = _build_journal_prompt(agent, iteration_context)
journal_entry = _generate_journal_entry(journal_prompt, worktree_path)

# Append to journal
journal = AgentJournal.load(agent.id) or AgentJournal.new(agent)
journal.add_entry(journal_entry)
journal.save()
```

The journal prompt asks the agent to reflect:

```markdown
You just completed iteration {iteration} of your work as the {agent_name} agent.

Here's what happened:
- Branch: {branch}
- Status: {status}
- Commits: {commit_messages}

Write a journal entry for your future self. Include:
1. What you did (2-4 bullet points)
2. What you learned that will help next time
3. If the iteration failed: what went wrong
4. What you should focus on next iteration

Be specific and actionable. Your future self will read this before starting the next iteration.
```

## Prompt injection

The journal is injected between the agent prompt and the task:

```
[Agent prompt - persistent identity and goals]
---
[Recent journal entries - memory of past iterations]
---
[Task prompt - what to do this iteration]
```

To keep prompt size manageable:
- Only include last 5 entries by default
- Configurable via `journal_entries` in AgentLoopSpec
- Summarize older entries if journal grows large

## File storage

```
~/.lf/
  agents/
    {agent-id}/
      journal.md       # Full journal (markdown)
      journal.json     # Structured data for programmatic access
```

JSON format enables analytics and dashboards later; markdown is human-readable and can be directly injected into prompts.

## What it takes to build

### 1. Journal data structures (new file: `maestro/journal.py`)
- JournalEntry dataclass with markdown serialization
- AgentJournal with load/save/append operations
- Parse markdown back into structured data

### 2. Journal creation after iteration (modify `runner.py`)
- After pipeline completes, generate journal entry
- Use LLM to write the entry (same as commit messages)
- Append to journal file

### 3. Journal injection before iteration (modify `runner.py`)
- Load journal before gathering prompt components
- Inject recent entries into prompt

### 4. CLI for viewing journal (modify `cli/agent.py`)
- `lf agent journal <name>` - view agent's journal
- `lf agent journal <name> --tail 3` - last N entries
- `lf agent journal <name> --clear` - reset journal

### 5. AgentLoopSpec extensions
- `journal_entries: int` - how many entries to inject (default 5)
- `journal_enabled: bool` - opt-out for simple agents

## Risks and mitigations

**Context bloat**: Journal entries consume tokens. Mitigated by:
- Limiting to recent entries
- Keeping entries concise (structured format encourages brevity)
- Configurable limit

**Hallucinated history**: Agent might "remember" things that didn't happen. Mitigated by:
- Journal entries are written immediately after iteration
- They're based on actual git commits and exit codes
- Agent writes about concrete work, not abstract plans

**Journal rot**: Old entries become irrelevant. Mitigated by:
- Only inject recent entries
- Could add archival: after N entries, summarize and archive

**LLM cost**: Extra API call per iteration for journal. Mitigated by:
- Use fast/cheap model (Haiku or Sonnet)
- Could make journaling opt-in

## Path to build it

1. **JournalEntry and AgentJournal dataclasses** - Serialization to/from markdown
2. **Storage layer** - `~/.lf/agents/{id}/journal.md` read/write
3. **Journal generation** - After iteration, call LLM to write entry
4. **Prompt injection** - Load and inject journal before iteration
5. **CLI commands** - `lf agent journal show/clear`
6. **Config options** - `journal_enabled`, `journal_entries`

## Example usage

```bash
# Register an agent with journaling
lf ops agent register docs-agent \
  -p prompts/docs.md \
  --pipeline implement,review \
  --trigger main-changed \
  -c

# Run it
lf ops agent start docs-agent -c

# After a few iterations, check what it learned
lf ops agent journal docs-agent

# Agent: docs-agent
# Iterations: 5
#
# ## Iteration 5 (2025-01-12T18:00:00)
# **Status:** success
# **Branch:** agent/docs-agent/5
#
# ### What I did
# - Added missing type hints to llm_http.py
# - Fixed docstring formatting in config.py
#
# ### What I learned
# - The project uses `| None` syntax, not `Optional[]`
# - No Args/Returns docstrings per STYLE.md
#
# ### Next iteration
# - Continue with maestro/ module
# - Check for missing error handling
```

## What this doesn't solve

- **Cross-agent coordination**: Agents still can't communicate with each other
- **Long-term knowledge**: Journal is ephemeral; no permanent knowledge base
- **Active learning**: Agent doesn't update its base prompt, just accumulates entries

These could be future expansions, but journal-based memory is the foundation they'd build on.
