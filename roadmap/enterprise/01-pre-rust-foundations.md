# Enterprise Roadmap: Pre‑Rust Foundations

Work that should happen before the Rust port because it clarifies requirements and is language‑independent.

## 1) OS portability policy
**Goal:** Support Linux and macOS with identical behavior.
- Define supported OS versions and filesystem assumptions.
- Remove macOS-only assumptions in paths, process management, and file watchers.
- Standardize environment variables and config discovery across OSes.

## 2) Container runtime model
**Goal:** Decide how runs are executed in managed clusters.
- Define a container/job runner model (sidecar, job queue, or worker pool).
- Document isolation boundaries (per-run, per-tenant, per-agent).
- Establish resource limit defaults (CPU, memory, disk).

## 3) Access control model
**Goal:** Define authentication and authorization before protocol changes.
- Identify user, team, and service identities.
- Define project scoping rules and tenancy boundaries.
- Decide on API key vs OIDC for early managed clusters.

## 4) Observability requirements
**Goal:** Make runs auditable and debuggable in production.
- Structured logs with trace IDs across processes.
- Metrics: run latency, error rate, queue depth, resource usage.
- Retention and redaction policies.

## 5) Secrets and config management
**Goal:** Make secrets deployable and safe.
- Define secret storage (env, file, secret manager).
- Establish config layering (global, project, user, runtime overrides).

## Deliverables
- A written portability and runtime policy.
- A containerized execution PoC.
- A minimal access control spec.
- An observability schema (log fields + metrics list).

