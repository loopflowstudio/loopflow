---
requires: scratch/ analysis
produces: scratch/roadmap-proposal.md
---
Synthesize analysis into a proposal for work or reference.

## Scope

The included context defines your area. Propose items that belong to this area. When promoted via `add-to-roadmap`, actionable items go to `roadmap/<wave>/` and reference material goes to `reports/`.

## Workflow

1. Read analysis in `scratch/` (research, simplification opportunities, polish priorities, etc.)
2. Read `reports/` to understand project direction and existing items
3. Identify the highest-leverage proposal that emerges from the analysis
4. Write `scratch/roadmap-proposal.md`

## Output format

Write `scratch/roadmap-proposal.md`:

```markdown
---
status: proposed
---

# Title

One paragraph describing what and why.

## Context

What analysis led to this proposal. Reference specific findings from scratch/.

## Scope

- What's included
- What's explicitly not included

## Approach

Technical direction. Not a full design doc—just enough to unblock building.
```

## Guidelines

- Focus on substantial work, not small fixes
- Be honest about scope—what's in, what's out
- The approach section should have enough detail that someone could start building
- If the analysis doesn't clearly point to a proposal, write `scratch/roadmap-proposal.md` explaining why and what's missing
