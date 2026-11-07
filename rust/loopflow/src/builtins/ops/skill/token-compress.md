---
requires: text input and target token budget
produces: compressed text
---
Fit text into a token budget without silently losing what matters.

Compression is not truncation: keep the decisions, facts, risks, names, dates
and structure the next reader needs, and reshape them to fit. Spend the budget
on what can't be worked out again from code, git history or a fresh run: exact
quotes, what a run showed, outside
facts. Cut what a capable reader could recover.

## Workflow

1. Use the requested `N` tokens. Without one, ask in an interactive
   conversation; headless, pick a practical budget and state it.
2. Read the whole source before writing.
3. Pull out what lasts:
   - decisions and why
   - constraints and requirements
   - risks, blockers and open questions
   - names, dates, versions, paths, commands, URLs and identifiers
   - findings that would be expensive to rediscover
4. Group related details before cutting. Replace repetition with structure.
5. Write the compressed version under budget.
6. If the budget forces real cuts, add an `Omitted` line naming what went. Don't hide loss.

## Rules

- Keep causes: why something happened, not just what.
- Keep interfaces: commands, API names, file paths, config keys, environment variables.
- Keep uncertainty: open questions and weak assumptions are information.
- Merge duplicates; don't drop unique facts because they come late.
- Dense bullets beat vague prose. No "various fixes" when concrete categories fit.
- Don't shorten a list by keeping its first items. Recency, impact and risk matter more than order.

## Output

Return only the compressed text unless commentary was asked for. When useful:

```markdown
Budget: <N> tokens

<compressed text>

Omitted: <only if meaningful information was intentionally left out>
```
