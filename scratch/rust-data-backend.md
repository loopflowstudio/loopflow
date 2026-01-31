# Rust Data Backend: Postgres + SQLite

## Problem

Loopflow is moving toward managed, multi-tenant clusters. The current SQLite-only persistence model cannot safely support multi-node concurrency, durable auditing, or enterprise backups. We need a Rust data layer that uses Postgres as the system of record for hosted `lfd`, while keeping local dev fast and unchanged.

## Approach

Build a Rust data backend with a single, explicit data access API and two storage engines: Postgres for managed mode and SQLite for local mode. The API expresses the real domain concepts (tenant, project, session, run, trigger, agent, event) and makes data flow explicit: append events, project state, query summaries.

**Core elements**

- **Schema-first backend API**: Define a Rust trait (or module boundary) that exposes domain operations (`create_run`, `append_event`, `set_run_status`, `list_active_runs`, `record_trigger_fire`). Both Postgres and SQLite implement the same contract.
- **Postgres schema (managed)**: Multi-tenant tables with strict foreign keys and `tenant_id`+`project_id` everywhere. Use `uuid` IDs to avoid cross-node collisions. Store timestamps in UTC.
- **SQLite schema (local)**: Same logical shape, single-tenant, no `tenant_id` column (or fixed `tenant_id = 'local'`), WAL mode enabled for reliability.
- **Event log + state tables**: Append-only `events` table for auditability plus materialized state tables for fast queries. State tables update in the same transaction that appends the event.
- **Migrations**: SQL migrations for both backends with a shared version graph. Use the same migration names and numbers across engines to guarantee parity.
- **Backups/retention**: For Postgres, document a baseline backup strategy (daily snapshot + WAL archiving). Add a retention policy for `events` with configurable TTL in managed mode.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Postgres everywhere (no SQLite) | Simpler code, fewer branches | Breaks local UX and introduces heavyweight dependency for single-user dev |
| SQLite only with WAL + file locks | Minimal change | Doesn’t meet multi-tenant concurrency, durability, or managed ops requirements |
| Event-sourcing only (no state tables) | Strong audit trail | Query performance suffers; read paths become complex and latency-sensitive |
| Dual-write to Postgres + SQLite long-term | Migration safety | Long-term complexity and divergence risk; make SQLite local-only |

## Key decisions

- **"Postgres in managed mode: Postgres is the system of record for hosted `lfd`."** We make Postgres the only backing store for managed clusters. SQLite remains for local-only mode.
- **"UX invariants: prompts, flows, directions, and artifact paths must not change."** Storage switches are config-driven only; all CLI behavior, paths, and user workflows stay identical.
- **"Control/execution isolation: failures in execution must not destabilize control plane."** Only the control plane writes authoritative state; execution workers report events. DB writes happen in control-plane transactions.
- **"Protocol first: every project starts by validating the protocol surface."** The data access API is the protocol: versioned, explicit, and tested across both backends before higher-level features depend on it.

## Scope

- In scope:
  - Postgres schema (tenants, projects, sessions, runs, triggers, agents, events)
  - SQLite schema for local mode with identical logical model
  - Migration tooling and version tracking
  - Rust data access layer with shared API + per-backend implementations
  - Event log + state tables with atomic updates
  - Backup and retention guidance for managed mode
- Out of scope:
  - Analytics warehouse or BI pipeline
  - New user-facing CLI commands or flow semantics
  - Full event-sourcing rewrite with derived projections only
  - Managed SaaS control plane

## Done when

- `lfd` can run with `storage=postgres` in managed mode and `storage=sqlite` in local mode without any CLI or workflow changes.
- Postgres and SQLite schemas are functionally equivalent (same domain objects, same invariant checks).
- Migrations run cleanly on both backends and the version table matches.
- Dual-backend tests pass via `cargo test` and cover: create/run lifecycle, event append, and concurrent updates.
- Managed-mode backup + retention guidance exists and is verified against a sample Postgres instance.
