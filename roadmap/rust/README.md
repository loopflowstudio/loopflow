# Rust Roadmap

Source of truth for the Rust migration and path to hosted teams.

## North Star

A protocol-first, Rust-based control plane that can be driven locally or remotely (desktop + mobile), with strict isolation between control and execution.

**Design priorities:**
- Rust-only distribution (no Python runtime, binaries via PyPI like ruff/uv)
- Security by default (auth required for remote access)
- Claude Pro/Max support (no API keys required - uses existing subscriptions)
- Progressive complexity (local is simple, hosted adds features)
- Containers and Kubernetes optional but well-supported as path to hosted

## Phases

| Phase | Focus | lfd Runs | Agents Run | Claude Auth | User Auth |
|-------|-------|----------|------------|-------------|-----------|
| 1 | Rust release | Local | Local process | ~/.claude | Unix socket |
| 2 | Self-hosted | Self-hosted | Local/Container/K8s | ~/.claude (mounted) | WorkOS JWT |
| 3 | Hosted teams | Our cloud | K8s Jobs | Device flow | WorkOS JWT |

### Phase 1: Rust Release ✅

Ship `lf` and `lfd` as Rust binaries via PyPI. No Python runtime required.

| Task | Status |
|------|--------|
| Prompt parity (Rust matches Python output) | ✅ Done |
| Ops parity (commit, land, pr, next, etc.) | ✅ Done |
| Binary distribution via maturin | ✅ Done |
| Remove PyO3/Python bindings | ✅ Done |
| Merge lfd into lf crate | ✅ Done |
| Port `lf ops cp` and `lf ops doctor` | ✅ Done |
| CI: maturin-action builds wheels | ✅ Done |
| Add builtin steps: `add-prompt`, `setup` | 🔜 Next |
| First PyPI release | 🔜 Next |

**Python commands replaced by builtin steps:**

| Old Command | New Approach |
|-------------|--------------|
| `lf ops add` | `lf add-prompt <name>` - LLM generates real prompts |
| `lf init` | `lf setup` - LLM-guided repo setup |
| `lf ops summarize` | Moves to lfd (area summaries) |
| `lf ops trace` | Removed (testing-only) |

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

**Ready for release:**
- `lf` CLI: feature-complete (steps, flows, directions, context, agents)
- `lf ops`: complete (commit, land, pr, next, rebase, sync, abandon, wt, shell, cp, doctor)
- `lfd`: infrastructure ready (gRPC, storage, loops) - not yet primary execution path
- Distribution: maturin builds wheels with `lf` and `lfd` binaries

**Install method:**
```bash
uv tool install loopflow   # or: pip install loopflow
lf --help
lfd --help
```

## Principles

- **Rust-only:** No Python runtime. Binaries distributed via PyPI (like ruff, uv).
- **Self-extending:** Missing features become builtin steps, not code.
- **Protocol first:** Every project starts by validating the protocol surface.
- **UX invariants:** Prompts, flows, directions, and artifact paths must not change.
- **Security by default:** Auth required for any remote access.

## Distribution

| Artifact | Distribution | Contents |
|----------|--------------|----------|
| `loopflow` | PyPI wheel | `lf` + `lfd` binaries (no Python code) |

Wheels built for:
- `aarch64-apple-darwin` (Apple Silicon)
- `x86_64-apple-darwin` (Intel Mac)
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`

## Open Questions

| Question | Options | Decision |
|----------|---------|----------|
| Homebrew tap | Yes/no | Nice to have, not blocking |
| Windows support | Full, partial, none | TCP instead of Unix socket |
| Daemon auto-start | Service by default vs manual | `lf ops shell install` for now |
