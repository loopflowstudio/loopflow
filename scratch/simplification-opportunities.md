# Simplification Opportunities

## Product intent

Loopflow is a **prompt orchestration layer** for AI coding agents. It assembles context, stores prompts as reusable artifacts, and passes them to Claude Code, Codex, or Gemini. The core value: reproducible, composable prompts that chain together—each step reads what the previous one wrote, then hands off cleanly.

## ~~Opportunity 1: Goals and Voices are the same concept~~ DONE

Consolidated in this branch. Deleted `goals.py`, `templates/goals/`, and `test_goals.py`. All code now uses `voices.py` exclusively with global voice support (~/.lf/voices/).

## ~~Opportunity 2: Context assembly has too many flags doing the same thing~~ DONE

Consolidated in this branch. Replaced scattered boolean flags with structured config:

```python
class DiffMode(str, Enum):
    FILES = "files"    # full content of changed files (default)
    DIFF = "diff"      # raw unified diff (for commits)
    NONE = "none"      # neither

class FilesetConfig(BaseModel):
    paths: list[str] = []           # additive to defaults (scratch/, roadmap/, *.md)
    exclude: list[str] = []         # removes from defaults + paths
    token_limit: int | None = None  # if set, summarize files exceeding this

class ContextConfig(BaseModel):
    diff_mode: DiffMode = DiffMode.FILES
    files: FilesetConfig = FilesetConfig()
    lfdocs: bool = True             # bundled LOOPFLOW.md system doc
    clipboard: bool = False
```

Key simplifications:
- `diff` + `diff_files` booleans → single `diff_mode` enum (FILES, DIFF, NONE)
- `pathset` + `exclude` + `summaries` → hierarchical `FilesetConfig` with `token_limit`
- Default paths (scratch/, roadmap/, *.md) are implicit, user adds/removes
- CLI stays flat (`--diff-mode`, `--paths`, `--exclude`), config is structured

## Opportunity 3: Three layers of step resolution

**Misalignment**: Steps are "just markdown files" in the product's model. But resolution walks multiple hierarchies with skill sources as a special case.

**Symptom**: `gather_step()` does this:
1. Check if name contains ":" → external skill discovery (SkillRegistry, Superpowers)
2. Check `.lf/steps/{name}.md`
3. Check `.claude/commands/{name}.md`
4. Check `~/.lf/steps/{name}.md`
5. Check `~/.claude/commands/{name}.md`
6. Check `templates/steps/{name}.md` (builtins)

External skills have their own discovery (`skills.py`, 398 lines) with caching, prefixes, HTTP fetching. The frontmatter parsing then applies per-step config overrides for model, voice, context.

The product model: "steps live in `.lf/steps/`, builtins are fallbacks." That's 2 levels, not 6+.

**Realignment**:
- Collapse to: repo steps → global steps → builtins
- Skills become a separate concept invoked explicitly (`lf skill sp:brainstorm`)
- Remove `.claude/commands/` special-casing (symlink if you need compatibility)

**Cascade**:
- Step resolution is a simple chain, not a conditional tree
- Skills are opt-in, not checked on every step lookup
- `list_all_steps()` doesn't need 4 return values
- Global step paths reduce to one location

## Opportunity 4: FlowRun/StepRun/Agent have unclear boundaries

**Misalignment**: The daemon tracks execution at three levels, but the product concept is "an agent runs steps." The middle layer (FlowRun) exists for implementation reasons.

**Symptom**: `lfd/models.py` defines:
- `Agent`: persistent config (flow, area, voice, triggers)
- `FlowRun`: execution instance with worktree, branch, current_step, pr_url
- `StepRun`: single step execution with pid, model, run_mode

But:
- Interactive steps (`lf debug -c`) create StepRuns without FlowRuns or Agents
- Flow execution creates StepRuns directly, optionally linked to FlowRuns
- The daemon server has handlers for all three, with different query patterns
- Concerto's UI cares about "what's running" (StepRun) and "worktree status" (derived from FlowRun state)

**Realignment**: Two concepts:
- `Session`: an execution (replaces StepRun), can be standalone or part of a flow
- `Agent`: persistent config + current session reference

FlowRun merges into Agent as runtime state. A flow is just a list of step names—no separate "execution instance" needed.

**Cascade**:
- Database schema simplifies: agents + sessions
- Daemon handlers reduce: `sessions.list`, `sessions.history`, `agents.status`
- Concerto queries one thing for "what's running"
- Flow execution tracks progress in-memory, not in separate FlowRun records

## Aligned areas

**The core prompt model is clean.** Step + Voice + Context → Prompt → Agent. The composition makes sense.

**Worktree management is solid.** Each feature gets isolation. The staleness detection and pruning logic reflects real user needs.

**The daemon's push-event architecture is right.** Socket for events, HTTP for queries. lfd maintains state, Concerto renders it.

**Token budgeting works.** The greedy trimming with diff_files as last resort matches how users think about context importance.

**Template system is sound.** Builtin steps as fallbacks, user overrides win. The frontmatter config pattern is extensible without complexity.
