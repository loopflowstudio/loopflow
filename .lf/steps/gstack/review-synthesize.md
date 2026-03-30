---
description: 'Synthesize parallel review outputs from gstack:review, gstack:cso, and gstack:codex into a unified assessment.'
tools:
- Read
- Grep
- Glob
- Write
- Edit
---
# Review Synthesis

Three reviewers worked in parallel on the same branch. Combine their findings into one actionable assessment.

## Inputs

Resolve the current branch first:

```bash
BRANCH=$(git rev-parse --abbrev-ref HEAD)
DESIGN_DOC="scratch/${BRANCH}.md"
```

Read the fork manifest at `.lf/fork-manifest.json` to find each branch's worktree. Then read each reviewer's output:

1. **gstack:review** — code quality, architecture, test coverage
2. **gstack:cso** — security analysis, vulnerability assessment
3. **gstack:codex** — cross-model review (different perspective, different blind spots)

Also read the design doc at `$DESIGN_DOC` and the review log at `.gstack/reviews.jsonl` for context on what was intended.

## Synthesis

### Reconcile overlapping findings

Multiple reviewers often flag the same issue with different framing. Deduplicate:
- Same root cause → merge into one finding, credit all perspectives
- Similar symptoms, different causes → keep separate, note the distinction

### Calibrate severity

Reviewers have different thresholds. Normalize:
- **Blocking**: breaks correctness, introduces security vulnerability, or violates a stated constraint
- **Important**: degrades quality, misses edge cases, or creates maintenance burden
- **Minor**: style, naming, documentation, or nice-to-have improvements

A security finding from CSO that review called "minor" should be escalated. A style nit that codex flagged as "important" should be downgraded.

### Distinguish substance from flavor

Model-flavor differences are not real disagreements. If one reviewer says "extract a helper" and another says "inline is fine," that's a judgment call, not a conflict. Note the tradeoff and move on.

Real disagreements — different conclusions about correctness, different architectural preferences with concrete tradeoffs — those need explicit callout.

## Output

Update `$DESIGN_DOC` with a `## Review Synthesis` section containing:

1. **Blocking issues** (if any) — with clear description of what to fix
2. **Important findings** — grouped by theme
3. **Minor items** — brief list
4. **Conflicts** — where reviewers genuinely disagreed, present both sides
5. **Verdict**: ITERATE (has blocking issues) or SHIP (no blockers)

Also append a summary entry to `.gstack/reviews.jsonl`:

```bash
BRANCH=$(git rev-parse --abbrev-ref HEAD)
echo '{"step":"review-synthesize","branch":"'"$BRANCH"'","timestamp":"'"$(date -u +%Y-%m-%dT%H:%M:%SZ)"'","verdict":"SHIP_OR_ITERATE","blocking":N,"important":N,"minor":N}' >> .gstack/reviews.jsonl
```
