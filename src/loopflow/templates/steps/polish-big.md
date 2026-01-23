---
requires: none
produces: updated docs, scratch/polish-priorities.md
---
Survey the codebase's rough edges. What would make it feel more finished?

## Goal

Surface the polish work that matters most and make the case for it. Update documentation that's drifted. Identify friction points worth fixing. Produce evidence—specific examples, not vague concerns—that makes prioritization clear.

The output is both immediate (updated docs) and strategic (a prioritized list of polish projects with compelling evidence).

## Principle

If something can be fixed directly, fix it. Don't suggest work that could just be done. The priority list is for projects that require more scope, coordination, or decisions—things worth advocating for but outside what a single session can complete.

## Workflow

1. Read README and user-facing docs. Update anything obviously stale.
2. Run CLI commands. Note confusing help text, unhelpful errors.
3. Scan for inconsistencies: naming, patterns, structure.
4. Fix what can be fixed. Document what can't.
5. For remaining friction points, gather specific evidence:
   - Where it occurs
   - What a user would experience
   - What the fix would involve
6. Write `scratch/polish-priorities.md` with prioritized recommendations

## What to update immediately

**Stale documentation.** If a README describes old behavior, fix it now. Don't just note it.

**Obvious errors.** Typos, broken links, wrong examples in docs.

**Misleading help text.** CLI `--help` that doesn't match current behavior.

## What to document for future work

**Naming inconsistencies.** Same concept called different things in different places. Include specific examples.

**Error message gaps.** Errors that don't help users fix the problem. Quote the actual error, explain what's missing.

**Rough user flows.** Paths that work but feel unfinished. Describe the experience, not just the code.

**Documentation holes.** Features that exist but aren't explained. Note what a user would need to know.

## Output

1. **Immediate updates**: Make doc fixes directly. Commit them.

2. **Write `scratch/polish-priorities.md`**:

```markdown
# Polish Priorities

## Docs updated
- <file>: <what changed>
- ...

## Priority 1: <name>
**Evidence**:
- <specific example 1>
- <specific example 2>
**Impact**: <what users experience>
**Effort**: Low / Medium / High
**Recommendation**: <why this should be next>

## Priority 2: <name>
...

## Lower priority
<Issues worth tracking but not urgent. Brief notes, not full write-ups.>
```

The evidence section is the key. Specific examples make prioritization possible. "Error messages are bad" is not actionable. "Running `cmd foo` with no args shows 'Error: None' with no explanation" is.
