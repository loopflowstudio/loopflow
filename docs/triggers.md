---
layout: default
title: Triggers (Subscribe & Schedule)
---

# Triggers: Subscribe & Schedule

Beyond continuous loops, `lfd` supports reactive triggers that run when specific conditions are met.

![triggers demo](triggers-demo.gif)

**Prerequisite**: The daemon must be running. See [Starting the Daemon](loops.md#starting-the-daemon).

## Subscribe

Watch for file changes on main and trigger automatically.

```bash
lfd subscribe "src/api/**" api-docs-updater
```

When files matching the pathset change on main, the goal runs one iteration.

### Examples

```bash
lfd subscribe "src/api/routes/**" openapi-updater
lfd subscribe "src/models/*.py" type-generator
lfd subscribe "schema.graphql,src/resolvers/**" graphql-codegen
```

### How It Works

1. Daemon periodically fetches `origin/main`
2. Compares current SHA to last seen SHA
3. If pathset files changed, triggers one iteration
4. Updates the baseline SHA

First run establishes a baseline without triggering.

## Schedule

Run on a cron schedule. Designed for laptop use.

```bash
lfd schedule "0 9 * * *" daily-triage
```

### Grace Period

Schedules have a 24-hour grace period for laptop use:

- **9am schedule, computer on at 9am** → runs immediately
- **9am schedule, computer wakes at 2pm** → still runs (within grace)
- **9am schedule, computer wakes next week** → skipped (too stale)

This handles laptops that sleep overnight or during commutes.

### Cron Syntax

```
┌───────────── minute (0-59)
│ ┌───────────── hour (0-23)
│ │ ┌───────────── day of month (1-31)
│ │ │ ┌───────────── month (1-12)
│ │ │ │ ┌───────────── day of week (0-6, SUN-SAT)
│ │ │ │ │
* * * * *
```

Examples:
- `0 9 * * *` — 9am daily
- `0 9 * * MON-FRI` — 9am weekdays
- `*/30 * * * *` — every 30 minutes
- `0 0 * * 0` — midnight on Sundays

## Managing Triggers

Check status of all loops including triggers:

```bash
lfd status
```

Output shows type and trigger info:

```
GOAL              TYPE       STATUS   ITER  TRIGGER
product-engineer  loop       running  12    continuous
api-docs          subscribe  idle     3     src/api/**
morning-triage    schedule   idle     5     0 9 * * *
```

Stop or remove triggers:

```bash
lfd stop api-docs-updater
lfd rm morning-triage
```

## Options

### Area Override

Focus on specific paths when triggered:

```bash
lfd subscribe "src/api/**" api-docs -r docs/api/
lfd schedule "0 9 * * *" triage -r src/core/
```

### Project File

Add context from a file:

```bash
lfd schedule "0 9 * * *" reviewer -p review-checklist.md
```

## Combining Triggers

A goal can have multiple trigger types:

```bash
# Same goal, different triggers
lfd loop product-engineer           # continuous
lfd subscribe "src/api/**" product-engineer -r src/api/
lfd schedule "0 9 * * *" product-engineer
```

Each creates a separate loop instance with its own state.

## See Also

[Loops](loops.md) · [`lfd` reference](lfd.md) · [Configuration](config.md)
