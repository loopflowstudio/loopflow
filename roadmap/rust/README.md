# Rust Roadmap

This folder tracks the Rust program of work. It is the source of truth for the staged plan, migration rules, and enterprise posture it enables.

## Why now
Loopflow is evolving beyond single‑developer macOS use to managed, multi‑tenant clusters. The daemon is the long‑lived, critical path service; it needs a stable protocol boundary, predictable runtime, and portable deployment.

## Goals
- Improve daemon reliability (24/7 operation, fewer runtime failures).
- Improve portability for managed clusters (Linux‑friendly, container‑ready, easy distribution).
- Preserve Loopflow UX: prompts, flows, directions, and artifact paths remain unchanged.
- Establish a protocol‑first architecture for remote clients (desktop + mobile).
- Support session connect for interactive steps (tick‑based flow execution with pause/resume).

## Non‑goals
- Full redesign of flows/steps/prompts.
- UX changes beyond remote connectivity and observability.

## North star
A protocol‑first, Rust‑based control plane that can be driven locally or remotely (desktop + mobile), with strict isolation between control and execution.

## Principles
- **Protocol first:** every project starts by validating the protocol surface.
- **UX invariants:** prompts, flows, directions, and artifact paths must not change.
- **Control/execution isolation:** failures in execution must not destabilize control plane.
- **Postgres in managed mode:** Postgres is the system of record for hosted `lfd`.

## Control vs execution isolation
- **Control plane:** long‑lived daemon for scheduling, run state, policy enforcement.
- **Execution plane:** isolated workers (process/container/job) that run agents.
- Failures in execution must not destabilize control plane.

## Recommendation
Commit to **protocol‑first** Rust `lfd` with remote client support. The protocol is the primary design artifact; every project starts by validating that the protocol surface fits the intended behavior.

## Ideal end state (ignoring transition costs)
- Rust control plane + Rust execution engine
- Postgres as system of record
- Protocol‑first APIs (gRPC/JSON)
- Per‑run sandboxing with strict resource limits
- Remote clients (desktop + mobile) as first‑class

## Decision criteria
- Idle overhead reduced by >50% vs Python daemon.
- Scheduling jitter reduced by >30% under synthetic load.
- Protocol supports local + remote clients without UX drift.
- Parity ≥ 95% on golden flow set.

## Stages
- Stage 1: `roadmap/rust/01-protocol.md`
- Stage 2: `roadmap/rust/02-core-engine.md`
- Stage 3: `roadmap/rust/03-daemon-service.md`
- Stage 4: `roadmap/rust/04-lf-client.md`
- Stage 5: `roadmap/rust/05-data-backend.md`
- Stage 6: `roadmap/rust/06-deployment.md`
- Stage 7: `roadmap/rust/07-validation.md`

## Parity + test story (stage by stage)
### Stage 1 — Protocol
- **Keep:** Python `lf` + Python `lfd` as default.
- **Parity focus:** protocol schemas mirror current behavior.
- **Tests:** schema compatibility tests + golden event payloads.

### Stage 2 — Core engine
- **Keep:** Python stack remains default; Rust core is opt‑in behind a flag.
- **Parity focus:** prompt assembly + flow parsing output equivalence.
- **Tests:** golden prompt fixtures; diff‑based tests on context assembly.

### Stage 3 — Rust daemon
- **Keep:** Python `lfd` stays default; Rust `lfd` runs in shadow mode.
- **Parity focus:** scheduling decisions and run state transitions.
- **Tests:** shadow‑mode trace comparison; fault‑injection tests.

### Stage 4 — lf client
- **Keep:** local `lf` uses in‑process `lfd‑core` by default.
- **Parity focus:** local vs remote `lf` behavior is identical.
- **Tests:** end‑to‑end CLI parity suite; remote integration tests.

### Stage 5 — Data backend
- **Keep:** local stays on SQLite; managed mode uses Postgres.
- **Parity focus:** run state persistence and event ordering.
- **Tests:** dual‑backend tests; migration tests; consistency checks.

### Stage 6 — Deployment
- **Keep:** single‑machine dev workflow intact.
- **Parity focus:** behavior unchanged when containerized.
- **Tests:** container smoke tests; upgrade/rollback tests.

### Stage 7 — Validation
- **Keep:** Python stack available as fallback until gates are met.
- **Parity focus:** golden flow set ≥ 95% parity.
- **Tests:** full validation suite; performance + reliability benchmarks.

## Library transitions (when and how)
### Token counting
- **Stage 2:** evaluate Rust tokenizer options.
- **Rule:** if no accurate/maintained tokenizer exists, fall back to byte‑based limits with explicit guardrails.

### Cron parsing
- **Stage 2/3:** validate Rust cron parsing against existing croniter behavior.
- **Rule:** parity for supported expressions; document any intentional reductions.

### SQLite → Postgres
- **Stage 5:** move managed/cluster mode to Postgres.
- **Rule:** local mode may stay on SQLite, but hosted `lfd` uses Postgres by default.

### Pydantic → Serde
- **Stage 2:** replace validation with serde‑based types in Rust core.
- **Rule:** keep protocol schema as the source of truth; validation errors map to protocol error codes.

### YAML parsing
- **Stage 2:** serde_yaml replaces pyyaml for flow/step parsing in Rust core.
- **Rule:** preserve parsing semantics; document any differences.

### HTTP/Socket server
- **Stage 3:** Rust server replaces Python asyncio/FastAPI.
- **Rule:** protocol compatibility is required; no UX changes.

## CI + release expectations
- Rust builds on macOS + Linux.
- Protocol compatibility tests run in CI.
- If Python bindings exist, wheels are built for supported platforms.

## UX compatibility matrix
- **Must not change:** prompt/flow semantics, artifact paths, CLI affordances.
- **Should not change:** direction composition, flow execution order, local defaults.
- **Ambiguous:** token counting heuristics, scheduling jitter, minor error strings.
