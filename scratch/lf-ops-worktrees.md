# lf ops parity: worktrees + next

## Status

Implemented in Rust with worktree management, worktree preservation on `lf ops next`, and shell auto-cd integration (zsh/bash).

## Summary

- Added Rust-first worktree management (`lf ops wt create/switch/list/prune/ci`) with JSON list output.
- Implemented worktree primitives in `loopflow-engine::worktrees` (list, create with naming schema, preserve move).
- Updated `lf ops next` to preserve the current worktree and create a fresh branch/worktree.
- Added tests for config parsing, context assembly, and git/worktree behavior.
- Updated `docs/lfops.md` to match worktree paths and shell integration behavior.

## Problem

Rust `lf ops` lacked the worktree-centered workflow that makes Loopflow fast: create/switch/prune/list worktrees, preserve worktrees on `next`, and auto-cd into the right directory. Power users (and the Rust parity roadmap) need a first-class, low-friction worktree experience that matches Python behavior without changing UX invariants.

## Approach

Ship a single, coherent worktree system in Rust that backs `lf ops wt *` and `lf ops next` with the same primitives. The core is a new `loopflow-engine::worktrees` module that owns:

- Worktree discovery: parse `git worktree list --porcelain`, enrich with repo metadata and merge status.
- Worktree creation: deterministic naming from config schema, optional stacking, and consistent filesystem layout (`../<branch>` default).
- Worktree preservation: move current worktree to a timestamped path on `next` before creating the next branch.
- Auto-cd: shell directive written to a known temp path on every command that changes worktree, with `lf ops shell install` wiring the shell to source it.

CLI behavior is front-loaded: `lf ops wt create` and `lf ops next` are 1-command flows with sensible defaults, no extra flags for the common path. Worktrees are self-describing (branch, base, stack info, last commit, PR status) and listable in JSON for editor integrations.

## Key decisions

- **Rust-first implementation**: all worktree logic lives in `loopflow-engine`.
- **Preserve UX invariants**: CLI flags and prompt artifacts match Python behavior.
- **Protocol-first surfaces**: `wt list --format json` is treated as an API for editor/daemon integrations.
- **No new background services**: worktrees are file-system and git based.
- **Worktree paths**: keep sibling layout (`../<short-name>`) even when branch names are schema-expanded.

## How it fits together

`lf ops wt *` and `lf ops next` call into `loopflow-engine::worktrees`, which shells out to git for worktree discovery and creation. Shell integration writes directives to a temp file that a wrapper sources after `lf` returns.

## Risks and bottlenecks

- Merge detection depends on `origin/<default-branch>` being up to date; stale remotes can misclassify prunability.
- Shell wrapper redefines `lf`; unusual shell setups may require manual sourcing or disable auto-cd.
- Worktree pruning skips dirty trees silently; users may want explicit visibility of skipped paths.

## Out of scope

- PR auto-merge, stack retargeting, or wave/worktree registry integration.
- Fish shell integration.
- Any daemon-backed worktree state or `wt` CLI event emission.

## Done when

- `lf ops wt create feature-x` creates `../feature-x` and prints a shell directive that auto-cds.
- `lf ops wt list --format json` returns structured metadata including branch, path, base, stack, merged status.
- `lf ops wt prune --dry-run` lists prunable worktrees; `--force` removes them.
- `lf ops next` preserves the current worktree under a timestamped path and creates a new worktree from the correct base.
