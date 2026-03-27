# Stage 6: Company as top-level object

The company is the root object in loopflow. Companies have providers, codebases, and agents. This stage introduces the company model and company-level flows.

## Object hierarchy

```
Company
  ├── providers (Stripe, Doppler, AWS, Mercury, GitHub, ...)
  ├── codebases
  │     ├── codebase A
  │     │     ├── waves
  │     │     ├── uses: [stripe, doppler, github, postgres]
  │     │     └── workstyle: lfjack
  │     └── codebase B
  │           ├── waves
  │           ├── uses: [cloudflare, github]
  │           └── workstyle: gstack
  └── company-level flows (cron'd, full access)
        ├── executive-report
        ├── financial-health
        ├── shipping-velocity
        └── infra-health
```

## Company-level flows

These run on a cron, not inside any codebase. They have full access to all company providers and produce reports.

```yaml
# Company flow: weekly executive report
flow: executive-report
mode: cron
schedule: "0 9 * * 1"   # Monday 9am
providers: [stripe, mercury, github, linear, anthropic]
```

### What they read

| Provider | What the agent queries |
|---|---|
| Stripe | MRR, subscriptions, churn, revenue trends |
| Mercury | Cash position, burn rate, runway |
| GitHub | PRs merged, shipping velocity, contributor activity across all repos |
| PM (Linear/Notion) | Open work, completed work, blocked items across all waves |
| Anthropic/OpenAI | Token usage, cost per wave, cost per shipped feature |
| Doppler | Secrets health, rotation status |
| Fly/Railway/Vercel | Deployment health, uptime, incidents |

### What they produce

Reports. Written to a known location, pushed to a channel, or surfaced in Concerto.

```
Weekly executive report — 2026-03-24
─────────────────────────────────────
Revenue:  $42K MRR (+8% MoM), 12 new subscriptions
Cash:     $1.2M in Mercury, 14 months runway
Shipping: 23 PRs merged across 3 repos, 4 features shipped
AI spend: $340 across 8 waves, $85/shipped feature
Health:   all services green
Alerts:   Stripe webhook secret rotated 45 days ago (rotate at 90)
```

## Company config

```yaml
# ~/.lf/company.yaml (or managed by lfd)
name: loopflow
providers:
  ai: [anthropic, openai, google]
  source: github
  pm: [notion, linear]
  secrets: doppler
  payments: stripe
  banking: mercury
  cloud: [aws, cloudflare]
  deployment: [fly]
  ci: github-actions
  database: postgres

codebases:
  - repo: loopflowstudio/loopflow
    uses: [anthropic, openai, github, doppler, cloudflare, postgres]
    workstyle: lfjack
  - repo: loopflowstudio/loopflow.studio
    uses: [cloudflare, stripe, github]
    workstyle: lfjack
```

## Codebase vs company

| Level | Owns | Example query |
|---|---|---|
| Company | Provider accounts, credentials, all codebases | "What's our MRR?" "What did we ship this week?" |
| Codebase | Waves, service connections, deployment | "Is the API healthy?" "What's blocking this PR?" |

Company-level agents query across codebases. Codebase-level agents (existing waves) work within one repo. The infrastructure model (stage 5) bridges them — company defines providers, codebase declares which ones it uses.

## Done when

1. Company config exists and can be read by lfd
2. A company-level flow (executive-report) runs on cron with provider access
3. The report reads from at least Stripe + GitHub + the AI providers
4. Company-level flows are distinct from codebase waves in lfd's model
5. `lfq` can show company-level flows alongside wave status
