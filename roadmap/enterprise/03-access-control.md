# Enterprise Roadmap: Access Control

Define authentication, authorization, and tenancy boundaries.

## Goal
Establish a security model that works for managed clusters and enterprise deployments.

## Scope
- Identity types (user, service, agent)
- Tenant and project scoping
- Authn mechanisms (API keys, OIDC)
- Authorization rules

## Key decisions
- Default authn for early adopters: API keys.
- Enterprise authn: OIDC with SSO.
- Authorization: role-based at tenant + project scopes.

## Success criteria
- All API calls scoped to a tenant + project.
- Clear role definitions and least-privilege defaults.
- Auditable access logs.

