# Stage 5: Infrastructure model

Loopflow's model of the world outside the codebase. Accounts, secrets, services — and the invariant that they should match.

## Why this is core

Agents can write code now. The hard part has shifted to managing and distributing code and turning on services. A codebase is inert without the machinery around it — deployment targets, databases, DNS, payment processors, secrets. Loopflow needs a representation of all of this to maintain homeostasis for a codebase serving customers.

This isn't a workstyle feature. It's foundational — every workstyle inherits it. gstack's `/ship` assumes deployment exists. Autoresearch assumes a GPU and training data. vsm assumes PM providers. The infrastructure model is what makes "run this flow end-to-end" actually work instead of hitting a wall at ship time.

## Three layers

### Accounts

Organizational relationships with providers. Above any single codebase.

```yaml
# .lf/accounts.yaml (or repo-level, or org-level)
accounts:
  - provider: doppler
    type: secrets
  - provider: vercel
    type: deployment
  - provider: aws
    type: cloud
    region: us-east-1
  - provider: google-domains
    type: dns
  - provider: stripe
    type: payments
  - provider: railway
    type: deployment
  - provider: github
    type: source
```

### Secrets

Credentials that connect code to accounts. Managed by a secrets provider (Doppler). The bridge between accounts and services.

```yaml
# Secrets are stored in Doppler, not in loopflow
# Loopflow knows what should exist and can verify
secrets:
  provider: doppler
  project: myapp
  environments: [dev, staging, production]
  expected:
    - STRIPE_SECRET_KEY        # account: stripe
    - STRIPE_WEBHOOK_SECRET    # account: stripe
    - DATABASE_URL             # account: railway
    - VERCEL_TOKEN             # account: vercel
```

### Services

What's actually running for this codebase. The health layer.

```yaml
services:
  - name: api
    account: railway
    check: curl -s https://api.myapp.com/health
    required: true
  - name: db
    account: railway
    type: postgres
    check: pg_isready -h $DB_HOST
    required: true
  - name: payments
    account: stripe
    secrets: [STRIPE_SECRET_KEY, STRIPE_WEBHOOK_SECRET]
    required: false    # degraded without it
  - name: web
    account: vercel
    check: curl -s https://myapp.com
    kind: deployment
```

## The invariant

Accounts, secrets, and services should be in agreement. When they drift, loopflow detects it:

- **Missing secret**: service `payments` needs `STRIPE_SECRET_KEY`, Doppler doesn't have it → flag
- **Orphaned secret**: Doppler has `OLD_API_KEY`, no service references it → flag
- **Account disconnected**: Vercel account exists but auth token expired → flag
- **Service unhealthy**: deployment target returns non-200 → flag

This is the homeostasis check. Like VSM watches wave health, this watches infrastructure health. The garden/govern flows can incorporate it — s4 (intelligence) already watches for environmental changes.

## Workstyle integration

Each workstyle can declare what infrastructure it expects:

```yaml
# gstack workstyle declares
needs:
  accounts: [deployment, ci]
  services: [deployment-target, test-runner]

# autoresearch declares
needs:
  services: [gpu, training-data]

# vsm declares
needs:
  accounts: [pm-provider]
```

When syncing a workstyle, loopflow checks whether the required infrastructure exists. Missing pieces surface as setup tasks, not runtime failures.

## Done when

1. `lf ops infra check` verifies accounts, secrets, and services are in agreement
2. Drift between Doppler secrets and declared service needs is detected
3. Workstyles can declare infrastructure requirements
4. Missing infrastructure surfaces during `lf init --workstyle` not during flow execution
5. The infrastructure model is available to all flows (garden, VSM, gstack ship)
