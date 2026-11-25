---
requires: none
produces: scratch/polish-priorities.md and unstarted Tasks for the top priorities
---
Survey the rough edges of the area you are given. What would make it feel more finished?

Read `scratch/` first. Your area is whatever the request names: a
subsystem, a surface, a directory, recent work. In a Wave with no request, it
is what the Wave's goal says it owns. Otherwise it is the whole codebase.

"We did a lot of work on accounts lately" names an area, not a diff. Find the
PRs that did the work, see which part of the system or infrastructure they
kept touching, and widen to the next cohesive whole. Work on that whole: the
PRs may have been patches on a larger problem. Open the output by naming the
area you took.

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

## Area
<What you looked at, and why that>

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

Then file the priorities as Tasks, following `lf help capture-tasks`. Group
rough edges that share a cause or a surface into one Task someone would notice
finishing; a Task per typo buries the backlog. "Lower priority" stays in the
note. Tasks wait unstarted until someone picks them.
