# Simplification Opportunities

## Product intent

Loopflow is a **prompt orchestration layer** for AI coding agents. It assembles context, stores prompts as reusable artifacts, and passes them to Claude Code, Codex, or Gemini. The core value: reproducible, composable prompts that chain together—each step reads what the previous one wrote, then hands off cleanly.

## Opportunity 1: Goals and Voices are the same concept

**Misalignment**: The product exposes one concept ("voice") but the codebase maintains two parallel implementations.

**Symptom**:
- `lf/goals.py` (276 lines) and `lf/voices.py` (235 lines) are nearly identical
- Both have `GoalKind`/`VoiceKind` enums with same values (ROLE, MODE)
- Both have the same loading logic: repo → global → builtin
- Both have identical heuristic detection (`_detect_goal_kind` / `_detect_voice_kind`)
- Both have `needs_adaptive()`, `resolve_*()`, `build_effective_*()`, `render_*()`
- `templates/goals/` and `templates/voices/` contain identical files (adaptive.md is byte-for-byte the same)
- The only difference: `goals.py` imports from `voices.py` for `_parse_frontmatter`

**Realignment**:
- Delete `goals.py` entirely
- Remove `templates/goals/` directory
- All code uses `voices.py` exclusively

**Cascade**:
- ~275 lines of code removed
- No more confusion about whether to use "goal" or "voice"
- One search path instead of two
- Config validation gets simpler (no `goal` vs `voice` ambiguity)
- Migration references in `lfd/migrations/` can drop goal-related schema

## Opportunity 2: Context assembly has too many flags doing the same thing

**Misalignment**: Context assembly evolved organically with features added via boolean flags. The product wants "assemble relevant context automatically"—but the architecture is "check 8 conditionals to see what's included."

**Symptom**: `ContextConfig` has flags that create exponential combinations:
```python
class ContextConfig(BaseModel):
    pathset: list[str] = []        # files to add
    exclude: list[str] = []        # files to remove
    lfdocs: bool = True            # include root .md + roadmap/ + scratch/
    diff: bool = False             # include raw diff
    diff_files: bool = True        # include files touched by branch
    summaries: bool = True         # include pre-generated summaries
    clipboard: bool = False        # include pasted content
```

These flags combine in `gather_prompt_components()` (90+ lines of conditional gathering) and `format_prompt()` (85+ lines of conditional formatting). The trimming logic in `trim_prompt_components()` has to reason about all combinations.

Meanwhile, the product's mental model is simpler:
- **docs**: static project knowledge (STYLE.md, README.md, roadmap/)
- **work**: what you're changing (scratch/, branch files)
- **extra**: clipboard, explicit paths

**Realignment**: Replace flags with a single `ContextMode` enum:
```python
class ContextMode(Enum):
    FULL = "full"        # docs + work + summaries (default for steps)
    MINIMAL = "minimal"  # work only (for commits)
    CUSTOM = "custom"    # explicit pathset, no auto-gather
```

Most users want `FULL`. The complexity exists to support edge cases that could be handled by `CUSTOM` + explicit paths.

**Cascade**:
- `gather_prompt_components()` becomes 3 clear branches instead of flag soup
- Token trimming gets simpler: drop summaries first, then docs, then work
- CLI flags collapse: `--mode minimal` instead of `--no-lfdocs --no-diff-files --no-summaries`
- Factory methods (`for_commit`, `for_interactive`) become mode selection
- Context display shows mode, not flag matrix

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
