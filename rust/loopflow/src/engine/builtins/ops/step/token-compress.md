---
requires: text input and target token budget
produces: compressed text
---
Compress text into a target token budget without silently dropping important information.

## Goal

Produce a smaller artifact that preserves the decisions, facts, risks, names, dates, and structure a downstream agent or human needs. Compression is not truncation. Shape the information so it fits.

## Workflow

1. Identify the target budget. Use the user's requested `N` tokens when provided. If no budget is provided, ask for one in interactive mode; in headless mode choose a practical budget and state it.
2. Read the source text fully before writing the compressed version.
3. Extract the durable content:
   - decisions and rationale
   - constraints and requirements
   - risks, blockers, and open questions
   - names, dates, versions, paths, commands, URLs, and identifiers
   - evidence that would be expensive to rediscover
4. Group related details before cutting. Replace repetition with structure.
5. Write the compressed version under the target budget.
6. If the budget forces meaningful omissions, add an `Omitted` line naming the categories omitted. Do not hide loss.

## Compression rules

- Preserve causality: keep why something happened, not just what happened.
- Preserve interfaces: commands, API names, file paths, config keys, environment variables, and external contracts usually matter.
- Preserve uncertainty: open questions and weak assumptions are first-class information.
- Merge duplicates; do not delete unique facts just because they appear late.
- Prefer dense bullets over vague prose.
- Do not use placeholders like "various fixes" when concrete categories fit.
- Do not summarize a list by taking the first items. Recency, impact, and risk matter more than order.

## Output

Return only the compressed text unless the user asked for commentary.

When useful, use this shape:

```markdown
Budget: <N> tokens

<compressed text>

Omitted: <only if meaningful information was intentionally left out>
```
