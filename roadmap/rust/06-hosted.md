# 06: Fully Hosted (Phase 3)

Full SaaS control plane. Same infrastructure self-hosters use, but we run it.

## What Phase 2 Delivers

- Self-hosted lfd with auth and container/K8s execution
- loopflow.studio relay for remote access (NAT traversal, TLS termination)
- JWT-based auth with local validation

Phase 3 adds the control plane, multi-tenancy, and user-facing features on top.

## Architecture

```
Mobile/Desktop ──TLS──▶ loopflow.studio ──tunnel──▶ customer lfd (K8s namespace)
```

Each customer gets an isolated K8s namespace with their own lfd, postgres, and agent Jobs. Resource quotas and network policies enforce isolation.

## Components

**Control plane (loopflow.studio):**
- Auth (WorkOS AuthKit, from Phase 2)
- Web UI for wave management
- Web terminal for Claude login (xterm.js + container)
- Git provider OAuth (GitHub/GitLab)
- Billing (Stripe, usage-based on agent-minutes)

**Data plane (K8s):**
- Per-customer namespace with lfd + postgres
- Agent Jobs with credential Secrets
- ResourceQuota + NetworkPolicy isolation

## Provisioning Flow

```
Sign up → Create namespace → Deploy lfd → Web terminal → claude login
→ Connect GitHub → Clone repos → Create waves → Waves run in namespace
```

## Plans

| Plan | Agent minutes/mo | Max concurrent | Price |
|------|-------------------|---------------|-------|
| Free | 100 | 1 | $0 |
| Pro | 1,000 | 4 | $29 |
| Team | 10,000 | 10 | $99 |

## Done When

- [ ] Web UI: wave list, status, logs, CRUD
- [ ] Web terminal: xterm.js, `claude login` captures credentials
- [ ] Git OAuth: GitHub + GitLab repo connection
- [ ] Multi-tenancy: namespace isolation, resource quotas
- [ ] Billing: Stripe integration, usage tracking, plan enforcement

## Future

- Multi-region data plane
- Custom domains
- SAML/OIDC enterprise SSO (built into WorkOS)
- Audit logs
- SLA tiers

## Dependencies

- Requires: All Phase 1 and Phase 2 work
