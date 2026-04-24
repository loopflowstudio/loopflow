# Stage 5: Infrastructure model

Loopflow's model of the world outside the codebase. Built on Stripe Projects for provisioning, with loopflow's own drift detection and health layer on top.

## Why Stripe Projects

Agents can write code now. The hard part has shifted to managing the machinery around it — deployment targets, databases, DNS, payment processors, secrets. Stripe Projects already handles provisioning and credentials for 10+ providers (Vercel, Supabase, Clerk, etc.) with a single CLI. Building our own provisioning layer would reinvent what they've already negotiated.

Stripe Projects provisions. Loopflow watches, reasons, and acts.

## Architecture

```
Stripe Projects (provisioning layer)
  ├── provider accounts (Vercel, Supabase, Clerk, AWS, ...)
  ├── credential vault + env sync
  ├── multi-environment (dev/staging/prod)
  └── billing (one payment method for all services)

Loopflow (intelligence layer)
  ├── drift detection (declared vs actual)
  ├── health monitoring (is it running?)
  ├── workstyle defaults (which services a workstyle expects)
  ├── governance (VSM s4 watches infrastructure changes)
  └── company flows (stage 6) query across all of it
```

## Two backends

```yaml
# .lf/config.yaml
infrastructure:
  backend: stripe-projects    # uses Stripe Projects CLI/API
  project: proj_abc123        # Stripe Projects project ID
```

```yaml
# For teams not on Stripe
infrastructure:
  backend: manual
```

With `stripe-projects` backend, loopflow reads state from Stripe Projects' API. With `manual`, loopflow reads from local config (the fallback).

## What loopflow models

### Services (from Stripe Projects)

Stripe Projects knows what's provisioned. Loopflow reads this and adds health checks:

```yaml
# .lf/infra.yaml — loopflow's view of the project
services:
  - name: api
    provider: vercel          # provisioned via Stripe Projects
    check: curl -s https://api.myapp.com/health
    required: true
  - name: db
    provider: supabase        # provisioned via Stripe Projects
    check: pg_isready -h $DB_HOST
    required: true
  - name: auth
    provider: clerk           # provisioned via Stripe Projects
    required: true
  - name: payments
    provider: stripe
    secrets: [STRIPE_SECRET_KEY, STRIPE_WEBHOOK_SECRET]
    required: false           # degraded without it
```

### Secrets (vault sync)

Stripe Projects manages the credential vault. Loopflow declares what each service needs and checks for drift:

```yaml
secrets:
  expected:
    - STRIPE_SECRET_KEY        # service: payments
    - STRIPE_WEBHOOK_SECRET    # service: payments
    - DATABASE_URL             # service: db
    - CLERK_SECRET_KEY         # service: auth
```

`lf ops infra check` compares expected secrets against what Stripe Projects' vault actually contains.

## The invariant

Services, secrets, and providers should be in agreement. When they drift, loopflow detects it:

- **Missing secret**: service `payments` needs `STRIPE_SECRET_KEY`, vault doesn't have it
- **Orphaned secret**: vault has `OLD_API_KEY`, no service references it
- **Provider disconnected**: Vercel account auth expired in Stripe Projects
- **Service unhealthy**: health check returns non-200
- **Provisioning gap**: workstyle expects Supabase but `stripe projects list` shows no database

This is the homeostasis check. VSM's s4 (intelligence) already watches for environmental changes — the infrastructure model gives it something concrete to watch.

## Workstyle integration

Workstyles declare which services they expect. `lf init --workstyle gstack` provisions them via Stripe Projects:

```yaml
# gstack workstyle
providers:
  deployment: vercel
  ci: github-actions
  payments: stripe
  auth: clerk
  database: supabase
  secrets: stripe-projects    # vault, not Doppler
```

```bash
lf init --workstyle gstack
# → stripe projects add vercel
# → stripe projects add supabase
# → stripe projects add clerk
# Already has: stripe (it's a Stripe project)
# CI stays on GitHub Actions (not a Stripe Projects provider)
```

Workstyles declare preferences. Stripe Projects handles provisioning. Loopflow bridges them.

## LLM context

Stripe Projects generates LLM context about provisioned services. Loopflow can consume this as part of area/context assembly — agents working on payment code get Stripe's service context, agents working on auth get Clerk's.

```yaml
# .lf/config.yaml
context:
  - source: stripe-projects   # auto-generated service context
```

## CLI

```bash
lf ops infra check              # drift detection: secrets, health, provisioning gaps
lf ops infra status             # current state: providers, services, health
lf ops infra provision           # run stripe projects add for missing services
lf ops infra env                # sync env vars from Stripe Projects vault
```

Under the hood, `lf ops infra provision` calls `stripe projects add <provider>`. `lf ops infra env` calls `stripe projects env --pull`. Loopflow is the orchestrator, Stripe Projects is the engine.

## Done when

1. `lf ops infra check` reads from Stripe Projects and reports drift
2. `lf ops infra status` shows provisioned services + health checks
3. Workstyles can declare expected services; `lf init` provisions via Stripe Projects
4. Missing infrastructure surfaces during `lf init --workstyle` not during flow execution
5. Manual backend works for teams without Stripe Projects
6. The infrastructure model is available to all flows (garden, VSM, gstack ship)
