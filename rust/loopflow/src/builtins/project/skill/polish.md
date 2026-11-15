---
requires: none
produces: scratch/polish-priorities.md
---
Survey your area's rough edges. What would make it feel more finished?

Read `scratch/` first. The included context is your area: given `src/cli/`,
polish the CLI — help text, error messages, user flows — and leave everything
else alone.

## Workflow

1. Read the README and user-facing docs. Note what's stale or misleading.
2. Run the commands. Note confusing help text and unhelpful errors.
3. Scan for inconsistencies in naming, patterns and structure.
4. For each rough edge, write down where it happens, what someone using it
   sees, and what the fix involves.
5. Write `scratch/polish-priorities.md`, most important first.

## What to look for

**Stale docs.** READMEs that describe old behavior.

**Naming drift.** One concept with different names in different places. Quote them.

**Unhelpful errors.** Errors that don't say how to fix the problem. Quote the
error and say what's missing.

**Rough flows.** Paths that work but feel unfinished. Describe what it's like to
use, not just the code.

**Doc holes.** Features that exist but aren't explained. Say what someone would
need to know.

## Output

```markdown
# Polish Priorities

## Priority 1: <name>
**Examples**:
- <specific example 1>
- <specific example 2>
**Impact**: <what people run into>
**Effort**: Low / Medium / High
**Recommendation**: <why this should be next>

## Priority 2: <name>
...

## Lower priority
<Worth tracking, not urgent. Brief notes.>
```

Specific examples make the list worth reading. "Error messages are bad" isn't
actionable. "Running `cmd foo` with no args shows 'Error: None' with no
explanation" is.
