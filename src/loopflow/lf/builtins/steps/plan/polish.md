---
requires: none
produces: scratch/polish-priorities.md
---
Survey your area's rough edges. What would make it feel more finished?

## Scope

The included context defines your area of responsibility. Polish within that scope. If given `src/cli/`, focus on CLI polish—help text, error messages, user flows. Don't audit unrelated areas. Own your area's quality.

## Goal

Surface the polish work that matters most and make the case for it. Identify friction points worth fixing. Produce evidence—specific examples, not vague concerns—that makes prioritization clear.

## Workflow

1. Read README and user-facing docs. Note what's stale or misleading.
2. Run CLI commands. Note confusing help text, unhelpful errors.
3. Scan for inconsistencies: naming, patterns, structure.
4. For each friction point, gather specific evidence:
   - Where it occurs
   - What a user would experience
   - What the fix would involve
5. Write `scratch/polish-priorities.md` with prioritized recommendations

## What to document

**Stale documentation.** READMEs that describe old behavior.

**Naming inconsistencies.** Same concept called different things in different places. Include specific examples.

**Error message gaps.** Errors that don't help users fix the problem. Quote the actual error, explain what's missing.

**Rough user flows.** Paths that work but feel unfinished. Describe the experience, not just the code.

**Documentation holes.** Features that exist but aren't explained. Note what a user would need to know.

## Output

Write `scratch/polish-priorities.md`:

```markdown
# Polish Priorities

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
