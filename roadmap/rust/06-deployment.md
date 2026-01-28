# Rust Roadmap: Deployment + Operations (Stage 6)

Harden deployment for managed clusters and enterprise environments.

## Goal
Make Loopflow easy to deploy, observe, and operate in production.

## Scope
- Container images and Helm charts
- Service discovery and config management
- Secrets handling
- Monitoring and alerting

## Non-goals
- Full SaaS control plane

## Operational requirements
- Zero-downtime upgrades
- Clear health checks and readiness probes
- Structured logs and trace IDs

## Success criteria
- One-command install for a dev cluster.
- Upgrade path with documented rollback.
- Sane defaults for resource limits.

## Open questions
- Do we ship a single binary + config, or a full container stack?
- How much ops code lives in repo vs separate infra repo?

