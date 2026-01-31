# Rust Roadmap: Data Backend (Stage 5)

Migrate persistence from local SQLite to a service DB for multi-node clusters.

## Goal
Support multi-tenant managed clusters with durable state, concurrency safety, and auditability.

## Scope
- Postgres schema design
- Migrations and versioning
- Data access layer in Rust
- Operational backups and retention

## Non-goals
- Full data warehouse or analytics pipeline

## Schema needs
- Sessions, runs, triggers, agents
- Event streams or append-only logs
- Tenant and project scoping

## Migration strategy
- Dual-write or export/import tooling
- Compatibility layer for local mode (SQLite still supported)

## Success criteria
- Multi-node daemon workers share a consistent view.
- No run state corruption under concurrent access.
- Backups/restores are tested and documented.

## Open questions
- Do we keep SQLite for local dev long-term?
- Should we adopt an event-sourcing model?
