# Sprint 01: fast-path + `lf land` + worktree rotation

## What was implemented

Three features shipped as one unit:

1. **`fast-path` step runner** — New `fast_path` field in step frontmatter. Both CLI (`lf`) and daemon (`lfd`) try the command before spinning up an agent. Exit 0 skips the agent entirely. Non-zero injects failure output into the prompt and falls through to agent.

2. **Worktree rotation in `lf ops land`** — After landing a PR, `rotate_worktree()` preserves the current worktree (appending `.{unix_ts}`), checks for remaining wave items, and creates a fresh shortname worktree if items remain. CLI emits a shell directive to `cd` into the new worktree.

3. **`lf land` step** — New builtin step with `fast-path: lf ops land`. On the happy path, no agent needed. On failure, agent gets error context and the ops API reference.

Plus: shortname worktrees are now protected from `wt prune`.

## Key choices

**`fast-path` is a step-level feature, not a hook system.** One field in frontmatter, one code path in each runner. Hooks would need event types, ordering, error handling — over-engineered for the pattern we actually need (try command, fall back to agent).

**Rotation happens on intent, not merge.** The PR enables auto-merge or merges locally — either way, the branch's work is done. Waiting for CI to finish merging would block the user unnecessarily.

**Dot heuristic for shortname detection.** `preserve_worktree()` always produces `{name}.{unix_ts}`, so `name.contains('.')` reliably distinguishes preserved from active worktrees. No filesystem lookups.

**Failure context injected as `<lf:fast-path-failure>` XML tag.** The agent sees command, exit code, stdout, and stderr immediately at the top of the prompt.

**post_step_sync skipped on fast-path success.** The fast-path command (e.g. `lf ops land`) handles its own side effects. Running commit+push afterward would fail (branch merged, worktree renamed).

## How it fits together

```
engine/fast_path.rs          — try_fast_path() + FailureContext Display
engine/flow.rs               — fast_path field on Step + StepFrontmatter
engine/builtins/steps/ops/land.md  — lf land step prompt

lf/commands/run.rs           — CLI fast-path integration (before agent launch)
lf/commands/flow.rs          — auto-commit between flow steps
lf/commands/ops/mod.rs       — shell directive after rotation
lf/discovery.rs              — "land" in builtin categories

lfd/executor/wave/mod.rs     — daemon fast-path integration
lfd/executor/helpers.rs      — post_step_sync uses commit_workflow

ops/land.rs                  — rotate_worktree(), has_wave_items(), RotationResult
engine/worktrees.rs          — shortname prune protection
```

## Risks and bottlenecks

- **Worktree rename while cwd is inside it.** Mitigated: CLI uses `write_shell_directive` to move the user's shell. Daemon renames post-run; `cleanup_run_worktree()` handles missing paths gracefully.
- **fast-path runs arbitrary shell commands.** By design — step authors control the command. Same trust boundary as the agent itself.
- **`has_wave_items()` only checks for `.md` files, ignoring README.** If a wave uses non-markdown items, they won't be detected. Acceptable since all current wave items are markdown.

## What's not included

- Concerto UI for rotation or land status
- `lf rebase` step (sprint 04 — just a step file that consumes fast-path)
- Release notes improvements (sprints 02/03 — separate dependency chain)
- `lfd` scheduling changes (daemon already handles step advancement)

## Wave alignment

Goals advanced:
- "`lf land` lands the PR, rotates the shortname worktree, advances to next wave item — fast-path, no agent" — done
- "`fast-path` as a general step feature — any step can declare a fast command that skips the agent on success" — done

Wave README updated to reflect sprint 01 completion. Goals and risks that are now resolved have been removed.
