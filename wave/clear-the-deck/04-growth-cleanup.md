# 04: Growth Infrastructure Cleanup

**Finish line:** Waitlist, tier gating, and growth infrastructure deleted. Ship product, not marketing.

## Context

Growth infrastructure (waitlist signup, tier-based feature gating, marketing pages) exists across the codebase and deployment config. None of it is earning its keep. Delete it.

## What to build

1. **Delete waitlist infrastructure.** Signup flows, email collection, waitlist management.

2. **Delete tier gating** (if not already done in auth consolidation). Feature flags tied to subscription tiers, the tier model, any remaining checks.

3. **Delete marketing/growth code.** Landing pages, conversion tracking, anything that isn't the product itself.

4. **Audit for orphans.** Check for env vars, config keys, database tables, and API endpoints that only existed for growth infra.

## Done when

- No waitlist, tier, or marketing code in the codebase
- No orphaned config, env vars, or database tables
- Tests pass
