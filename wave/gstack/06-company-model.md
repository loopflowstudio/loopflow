# Stage 6: Company as top-level object

The company is the root object in loopflow. Companies have Stripe Projects, codebases, and agents. Stripe Projects provides the provider graph; loopflow adds intelligence flows on top.

## Object hierarchy

```
Company
  ├── Stripe Projects (provider provisioning + credentials)
  │     ├── project: loopflow-app (Vercel, Supabase, Clerk, Stripe)
  │     ├── project: loopflow-studio (Cloudflare, Stripe)
  │     └── project: loopflow-docs (Vercel)
  ├── codebases
  │     ├── loopflowstudio/loopflow
  │     │     ├── waves
  │     │     ├── stripe-project: loopflow-app
  │     │     └── workstyle: lfjack
  │     └── loopflowstudio/loopflow.studio
  │           ├── waves
  │           ├── stripe-project: loopflow-studio
  │           └── workstyle: lfjack
  └── company-level flows (cron'd, cross-codebase)
        ├── executive-report
        ├── financial-health
        ├── shipping-velocity
        └── infra-health
```

## Company-level flows

These run on a cron, not inside any codebase. They query across all Stripe Projects and all codebases.

```yaml
# Company flow: weekly executive report
flow: executive-report
mode: cron
schedule: "0 9 * * 1"   # Monday 9am
```

### What they read

| Source | What the agent queries |
|---|---|
| Stripe (via Stripe Projects) | MRR, subscriptions, churn, revenue trends |
| Mercury | Cash position, burn rate, runway |
| GitHub | PRs merged, shipping velocity, contributor activity across all repos |
| PM (Linear/Notion) | Open work, completed work, blocked items across all waves |
| Anthropic/OpenAI | Token usage, cost per wave, cost per shipped feature |
| Stripe Projects vault | Secrets health, rotation status, provisioned services across all projects |
| Deployment providers (via Stripe Projects) | Uptime, incidents, deploy frequency |

The key shift: instead of loopflow maintaining its own provider credentials for company flows, it reads through Stripe Projects. One credential vault, one billing relationship, one place to rotate keys.

### What they produce

Reports. Written to a known location, pushed to a channel, or surfaced in Concerto.

```
Weekly executive report — 2026-03-24
─────────────────────────────────────
Revenue:  $42K MRR (+8% MoM), 12 new subscriptions
Cash:     $1.2M in Mercury, 14 months runway
Shipping: 23 PRs merged across 3 repos, 4 features shipped
AI spend: $340 across 8 waves, $85/shipped feature
Health:   all services green across 3 Stripe Projects
Alerts:   Stripe webhook secret rotated 45 days ago (rotate at 90)
```

## Company config

```yaml
# ~/.lf/company.yaml (or managed by lfd)
name: loopflow
infrastructure:
  backend: stripe-projects

projects:
  - id: proj_abc123
    name: loopflow-app
    repo: loopflowstudio/loopflow
    workstyle: lfjack
  - id: proj_def456
    name: loopflow-studio
    repo: loopflowstudio/loopflow.studio
    workstyle: lfjack

# Providers not managed by Stripe Projects
extra_providers:
  banking: mercury
  ai: [anthropic, openai, google]
  pm: [notion, linear]
```

Stripe Projects handles most provider relationships. `extra_providers` covers what it doesn't — banking, AI providers, PM tools. As Stripe Projects adds providers, things migrate from `extra_providers` into Stripe Projects naturally.

## Codebase vs company

| Level | Owns | Example query |
|---|---|---|
| Company | Stripe Projects, all codebases, company flows | "What's our MRR?" "What did we ship this week?" |
| Codebase | Waves, one Stripe Project, deployment | "Is the API healthy?" "What's blocking this PR?" |

Company-level agents query across codebases and Stripe Projects. Codebase-level agents (existing waves) work within one repo and one Stripe Project. The infrastructure model (stage 5) bridges them.

## Done when

1. Company config maps codebases to Stripe Projects
2. A company-level flow (executive-report) runs on cron with cross-project access
3. The report reads from Stripe (revenue) + GitHub (shipping) + AI providers (spend)
4. Company-level flows are distinct from codebase waves in lfd's model
5. `lfq` can show company-level flows alongside wave status
