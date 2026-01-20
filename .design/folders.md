# folders: Unified file storage conventions

**What to build:** Document and enforce consistent folder conventions for prompts, config, and agent output across loopflow.

## Current state

Loopflow uses several dot-directories with overlapping purposes. This creates confusion about where things belong.

| Location | Current purpose |
|----------|-----------------|
| `.claude/commands/` | Task prompts (Claude Code compatible) |
| `.lf/` | Config, voices, summaries, task prompts |
| `.design/` | Per-PR scratchpad (cleared on merge) |
| `docs/` | Public documentation |

## Target state

### Folder hierarchy

```
.claude/commands/     # Task prompts (primary location)
  review.md
  implement.md

.lf/                  # Loopflow config and extensions
  config.yaml         # Repo configuration
  voices/             # Personas for agent responses
    concise.md
    architect.md
  goals/              # Directives for autonomous agent loops
    test-coverage.md
    reduce-debt.md
  summaries/          # Generated codebase summaries (gitignored)

.design/              # Current PR scratchpad (cleared on merge)
  <branch>.md         # Design spec for this branch
  questions.md        # Open questions captured during runs
  review.md           # Review verdict

.docs/                # Internal documentation (persists)
  architecture.md     # How the system works
  decisions/          # ADRs, design decisions
  context/            # Background for agents

docs/                 # Public documentation (persists)
  getting-started.md
  api.md
```

### Principles

1. **`.claude/commands/` for prompts** — Primary location. Portable across tools that support Claude Code format. `.lf/*.md` supported as fallback.

2. **`.lf/` for config and extensions** — Everything loopflow-specific: `config.yaml`, `voices/`, `goals/`, `summaries/`. Not prompts (prefer `.claude/commands/`).

3. **`.design/` is ephemeral** — Per-PR working space. `lfops land` clears contents. Never merged to main.

4. **`.docs/` is persistent internal docs** — For maintainers, not users. Agents read and evolve this. Architecture, decisions, context that helps future work.

5. **`docs/` is public docs** — For users of the repo. Human-written, human-maintained.

### Goals

Goals are high-level directives for autonomous agent loops. Unlike tasks (single-purpose prompts), goals describe ongoing objectives that an agent works toward across multiple iterations.

```markdown
# .lf/goals/test-coverage.md
Improve test coverage across the codebase.

## Focus areas
- Untested utility functions
- Edge cases in API handlers

## Constraints
- Don't delete existing tests
- Keep each test focused on one behavior
```

Agent definitions in `~/.lf/agents/` reference goals via the `goal` field:

```yaml
---
repo: /path/to/repo
pipeline: ship
trigger: loop
goal: test-coverage
---
```

## Data structures

No new data structures. This is documentation and convention enforcement.

## Key changes

### 1. Documentation updates

- `docs/storage.md` — **New page** explaining all folder conventions (created)
- `docs/index.md` — Link to storage page (done)
- `docs/_config.yml` — Add to nav (done)

### 2. `lfops land` behavior

Already clears `.design/` contents. No change needed—verify it doesn't touch `.docs/`.

### 3. `lf add` command

Currently creates prompts in `.claude/commands/`. No change needed.

### 4. Built-in prompt updates

Update built-in prompts to reference `.docs/` where appropriate:

- `design.md` — Mention checking `.docs/` for context
- `implement.md` — Read `.docs/` for architecture guidance
- `review.md` — Consider `.docs/decisions/` when evaluating choices

### 5. Goal loading in agent runner

Update `lfd/agents.py` to load goals from `.lf/goals/`:

```python
def load_goal(name: str, repo_root: Path) -> str | None:
    """Load goal content from .lf/goals/{name}.md"""
    goal_path = repo_root / ".lf" / "goals" / f"{name}.md"
    if goal_path.exists():
        return goal_path.read_text()
    return None
```

The agent runner injects the goal content into the prompt when spawning iterations.

## Out of scope

- **Summaries location** — Current location (`.lf/summaries/`) works. No change.

## Constraints

- `.claude/commands/` must remain the primary prompt location for Claude Code compatibility
- `.design/` clearing on merge is existing behavior—don't break it
- No migration needed—these are conventions, not schema changes

## Done when

1. ✅ `docs/storage.md` documents all folder conventions with philosophy
2. ✅ Navigation updated to include storage page
3. ✅ "Context over memory" philosophy documented
4. ✅ Summarize module has unit tests (29 tests)
5. ✅ Demo tape for context visualization created
6. Built-in prompts reference `.docs/` appropriately
7. Running `lfops land` on a branch with `.docs/` changes merges them (doesn't clear)
8. Goal loading works: agent with `goal: test-coverage` reads `.lf/goals/test-coverage.md`

Verification:
```bash
# Confirm .design/ is cleared but .docs/ is not
git checkout -b test-folders
mkdir -p .design .docs
echo "test" > .design/test.md
echo "test" > .docs/test.md
git add . && git commit -m "test"
# After land, .design/test.md should be gone, .docs/test.md should remain
```
