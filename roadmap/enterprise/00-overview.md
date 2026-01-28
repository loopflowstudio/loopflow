# Enterprise Roadmap: Overview

Identify infra and architectural changes needed for managed clusters that are **outside** the Rust port. Split work into pre‑Rust and post‑Rust phases.

## Principles
- Rust port covers **core engine and daemon** changes.
- Enterprise roadmap covers **infra, deployment, security, and ops** that are independent of language.
- If it benefits Python and Rust equally, it belongs here.

## Phases
### Pre‑Rust (can start now)
- Containerization and runtime isolation
- Authn/z and access control
- Observability and audit logging
- Configuration and secrets management

### Post‑Rust (can be done later)
- Cluster scheduling and multi-tenant quotas
- Managed control plane features
- Enterprise onboarding and compliance tooling

## What stays in the Rust roadmap
- Protocol + engine + daemon implementation
- Core scheduling logic
- Internal state model

## Success criteria
- Clear separation of responsibilities between Rust and enterprise tracks.
- Every enterprise requirement mapped to a phase and owner.
- Hosted `lfd` trial path defined for remote clients (desktop + mobile).
