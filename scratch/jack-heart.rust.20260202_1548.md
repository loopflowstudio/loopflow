# lf ops parity: worktrees + next

## Problem

Rust `lf ops` lacks the worktree-centered workflow that makes Loopflow fast: create/switch/prune/list worktrees, preserve worktrees on `next`, and auto-cd into the right directory. Power users (and the Rust parity roadmap) need a first-class, low-friction worktree experience that matches Python behavior without changing UX invariants.

## Approach

Ship a single, coherent worktree system in Rust that backs `lf ops wt *` and `lf ops next` with the same primitives. The core is a new `loopflow-engine::worktrees` module that owns:

- Worktree discovery: parse `git worktree list --porcelain`, enrich with repo metadata and merge status.
- Worktree creation: deterministic naming from config schema, optional stacking, and consistent filesystem layout (`../<branch>` default).
- Worktree preservation: move current worktree to a timestamped path on `next` before creating the next branch.
- Auto-cd: shell directive written to a known temp path on every command that changes worktree, with `lf ops shell install` wiring the shell to source it.

CLI behavior is front-loaded: `lf ops wt create` and `lf ops next` should be 1-command flows with sensible defaults, no extra flags for the common path. Worktrees should be self-describing (branch, base, stack info, last commit, PR status) and listable in JSON for editor integrations. The design assumes no external `wt` dependency, but can consume it if present for events.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep parity by shelling out to Python `lf` | Fast to implement | Breaks Rust-first path, adds hidden dependency, violates the Rust roadmap priority |
| Minimal git-only worktree ops (no metadata, no shell integration) | Low risk | Fails UX invariants (auto-cd, rich list output), doesn't unlock `next` workflows |
| Persist worktree state in lfd | Powerful for hosted | Over-scoped for Phase 1; adds auth/storage coupling before core parity |

## Key decisions

- **Rust-first implementation**: all worktree logic lives in `loopflow-engine`, aligned with the principle "Rust-first implementation (performance, single binary distribution)".
- **Preserve UX invariants**: CLI flags and prompt artifacts must match Python behavior per "UX invariants: prompts, flows, directions, and artifact paths must not change".
- **Protocol-first surfaces**: `wt list --format json` is treated as an API, reflecting "Protocol first" so editor/daemon integrations can rely on a stable contract.
- **No new background services**: worktrees are file-system and git based, honoring "Control/execution isolation" by avoiding daemon coupling in Phase 1.

## Scope

- In scope: `lf ops wt create/switch/list/prune/ci`, `lf ops next` worktree preservation + auto-cd directive, `loopflow-engine::worktrees` module, JSON output schema, branch naming from config, merge detection.
- Out of scope: wave<->worktree registry, hosted multi-user coordination, fish shell integration, lfd-backed worktree inventory.

## Done when

- `lf ops wt create feature-x` creates `../feature-x` and prints a shell directive that auto-cds.
- `lf ops wt list --format json` returns structured metadata including branch, path, base, stack, merged status.
- `lf ops wt prune --dry-run` lists prunable worktrees; `--force` removes them.
- `lf ops next` preserves the current worktree under a timestamped path and creates a new worktree from the correct base.
