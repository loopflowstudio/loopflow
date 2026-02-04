# Rust Roadmap

Source of truth for the Rust migration and path to hosted teams.

## North Star

A protocol-first, Rust-based control plane that can be driven locally or remotely (desktop + mobile), with strict isolation between control and execution.

**Design priorities:**
- Rust-first implementation (performance, single binary distribution)
- Security by default (auth required for remote access)
- Claude Pro/Max support (no API keys required - uses existing subscriptions)
- Progressive complexity (local is simple, hosted adds features)
- Containers and Kubernetes optional but well-supported as path to hosted

## Phases

| Phase | Focus | lfd Runs | Agents Run | Claude Auth | User Auth |
|-------|-------|----------|------------|-------------|-----------|
| 1 | Rust port | Local | Local process | ~/.claude | Unix socket |
| 2 | Self-hosted | Self-hosted | Local/Container/K8s | ~/.claude (mounted) | WorkOS JWT |
| 3 | Hosted teams | Our cloud | K8s Jobs | Device flow | WorkOS JWT |

### Phase 1: Full Rust Port

Complete the migration to Rust. Single-binary distribution, local-only operation.

| Doc | Scope | Status |
|-----|-------|--------|
| [01a-prompt-parity](../../scratch/rust-parity.md) | Prompt assembly matches Python | ✅ Done |
| [01b-ops-parity](../../scratch/rust-parity.md) | Ops workflow logic matches Python | 🔜 Next |
| [01c-testing-and-rollout](01b-testing-and-rollout.md) | Rollout strategy, PyO3 bindings | Blocked on 01b |
| [02-lfd-primary](02-lfd-primary.md) | Wire lfd as primary execution path |
| [02b-summarize](02b-summarize.md) | Wave area summaries for LLM context |
| [03-service](03-service.md) | launchd/systemd integration |
| [04-distribution](04-distribution.md) | Binary distribution |

### Phase 2: Self-Hosted with Auth

Enable remote access with authentication. Containers optional but supported.

| Doc | Scope |
|-----|-------|
| [05-auth](05-auth.md) | loopflow.studio auth service, WorkOS AuthKit, JWT |
| [06-executors](06-executors.md) | Executor abstraction, container/K8s support |
| [07-deployment](07-deployment.md) | Docker Compose, Helm chart, images |

### Phase 3: Hosted Teams

Full SaaS control plane. Same infrastructure self-hosters use, but we run it.

| Doc | Scope |
|-----|-------|
| [08-hosted](08-hosted.md) | Control plane, multi-tenancy, web terminal, billing |

## Current State

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed documentation of what exists today.

**Summary:**
- `loopflow-engine` is feature-complete (flow parsing, context, agents, git, tick execution)
- `lfd` has infrastructure (gRPC, storage, loops) but isn't the primary execution path
- Prompt parity verified: Rust `lf run` produces identical prompts to Python
- Ops parity unverified: Rust `lf ops` runs but logic not compared to Python

**The gap:** Ops commands need mock-based parity testing before Rust can become primary. See [scratch/rust-parity.md](../../scratch/rust-parity.md).

## Principles

- **Protocol first:** every project starts by validating the protocol surface
- **UX invariants:** prompts, flows, directions, and artifact paths must not change
- **Control/execution isolation:** failures in execution must not destabilize control plane
- **Postgres in managed mode:** Postgres is the system of record for hosted lfd
- **Security by default:** auth required for any remote access

## Artifact Split

| Artifact | Language | Distribution | Purpose |
|----------|----------|--------------|---------|
| `lf` CLI | Rust | brew, cargo, binary | Primary user interface |
| `loopflow` library | Python | PyPI | Scripting, integration, contributor-friendly |

`uv tool install loopflow` remains the primary install method. The Python package bundles platform-specific Rust binaries (like ruff, uv do).

## Decision Criteria

| Metric | Target |
|--------|--------|
| Idle overhead | Reduced >50% vs Python daemon |
| Scheduling jitter | Reduced >30% under synthetic load |
| Protocol | Supports local + remote clients without UX drift |
| Parity | ≥95% on golden flow set |

## Open Questions

| Question | Options | Decision |
|----------|---------|----------|
| CLI distribution | Homebrew, cargo install, curl script | Need all three |
| Windows support | Full, partial, none | TCP instead of Unix socket |
| Daemon auto-start | Service by default vs manual | `lf daemon install` for service |
| Web terminal | xterm.js, Concerto, other | Phase 3 |
| Billing model | Per-agent-minute, flat rate, tiers | Phase 3 |
