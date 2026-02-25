# 06: Hosted SaaS

Run managed loopflow infrastructure so users do not self-host EC2/Mac Mini.

## What exists after this

Users sign in, connect GitHub, and run waves on managed infrastructure using the same protocol Concerto already speaks.

## Scope

### Start simple

- Reuse proven deployment shape from dogfood lanes
- Provision one managed runtime per customer (or controlled shared pool)
- Keep executor model unchanged (lfd + Docker agents)

### Scale later

- Move to orchestrated placement/isolation when demand requires it
- Keep protocol + auth contracts unchanged from earlier phases

### Dependencies

- Studio auth (Phase 04) stable in production
- Remote operational runbooks from EC2/Mac Mini dogfood are mature

## Done when

- New user can onboard without manual host provisioning
- Managed runtime is discoverable in Concerto via studio auth flow
- Wave execution reliability matches self-hosted baseline
