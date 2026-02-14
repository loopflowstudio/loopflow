---
requires: none
produces: scratch/scan-upstream.md
model: claude:sonnet
---
Check external APIs and services you depend on for breaking changes.

## Workflow

1. **Identify external dependencies.** Scan the area for:
   - API client code (REST calls, SDK usage, GraphQL queries)
   - Service integrations (cloud providers, SaaS APIs, webhooks)
   - SDK imports (e.g., `anthropic`, `openai`, `stripe`, `aws-sdk`)
   - Configuration referencing external endpoints

2. **Check for changes.** For each external dependency, search for:
   - Recent changelog entries or release notes
   - API version deprecations or sunset dates
   - Breaking changes in recent releases
   - New API versions available
   - Migration guides or upgrade paths

3. **Assess relevance.** Filter findings to what actually affects this codebase:
   - Does the code use the affected endpoints or features?
   - Is the current API version being deprecated?
   - Are there new capabilities worth adopting?

## Output

Write `scratch/scan-upstream.md`:

```markdown
# Upstream Scan — <date>

## Breaking changes

### <service/API> — <change summary>
- **What changed**: <description>
- **Affects**: <which files/features in our code>
- **Deadline**: <deprecation date, if any>
- **Migration**: <URL or summary>

## Notable updates

### <service/API> — <update summary>
- **What's new**: <description>
- **Relevant to us**: <yes/no — why>

## Stable
<Services checked with no relevant changes>
```

## What to avoid

**Reporting every changelog entry.** Only surface changes that affect code in the area.

**Missing context.** Always connect the upstream change to specific code that uses it. An API deprecation is only relevant if we call that endpoint.
