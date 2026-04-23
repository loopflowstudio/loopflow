---
requires: dependency state, security advisories, upstream changelogs, recent main changes, optional external feeds
produces: scratch/vsm-s4-scan.md
---
Scan the environment around the chord.

## Goal

Look outward, not inward.

Surface environmental changes that could affect member waves: dependency drift,
security advisories, upstream API changes, and cross-wave main-branch movement.
Record what changed and where it might matter.

## Workflow

1. Check dependency state (`cargo outdated`, `uv pip list --outdated`, lockfiles).
2. Check security advisories (`cargo audit`, dependabot alerts, similar sources).
3. Read upstream API changelogs and other subscribed external feeds.
4. Review recent main-branch changes that cut across wave boundaries.
5. Record relevant external signals and their likely owning waves.

## Output

Write `scratch/vsm-s4-scan.md`:

```markdown
# VSM S4 Scan — <date>

## Dependency State
<outdated or breaking changes>

## Security and Advisory Signals
<advisories, severity, affected areas>

## Upstream and API Changes
<relevant contracts or changelog items>

## Cross-Boundary Main Changes
<recent changes that affect multiple waves>

## Raw Signals
<facts only>
```

## What to avoid

**Wishlist research.** Every signal should be plausibly relevant to this chord.
