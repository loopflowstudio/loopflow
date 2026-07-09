# lf command API redesign

Full surface redesign — all 15 top-level + 20 op commands rethought as one system.
`op` dissolves entirely. Built-ins own the top: promoted names are reserved words;
colliding skills/flows need the explicit `lf skill`/`lf flow` form.

## Settled grammar

- `lf pr` — noun suite for the PR lifecycle:
  - `lf pr` (bare) / `lf pr status` — show current branch's PR state
  - `lf pr open`      (was `lf op pr`)
  - `lf pr submit`    (was `lf op submit` — human-gated land)
  - `lf pr land`      (was `lf op land`)
  - `lf pr abandon`   (was `lf op abandon`)
  - no `push` verb — open/submit already push; bare push dies with op
- `lf wt` — worktree suite (was `lf op wt`)
- `lf rebase` — bare verb (was `lf op rebase`)
- `lf commit` — bare verb (was `lf op commit`)
- `lf auth` — top-level, at least for now (was `lf op auth`)
- `lf release` — top-level suite (run, check, notes, bump, tag, status); the
  built-in overrides the `release` flow, which stays reachable via `lf flow release`
- `lf pm` — top-level suite (show, update, init, status; was `lf op pm`);
  heaviest skill sweep at ×27 builtin mentions

Usage backs the cut: op pr ×122, op wt ×100, op land ×42, op auth ×29,
op rebase ×25 in shell history — then single digits.

## lfd stops shelling out (settled)

Machine paths (queue reconcile on PR-merge hooks, next/land dispatch) call the
library in-process instead of spawning `lf`. This kills the argv-stability
constraint: the CLI grammar exists for humans only.

- `lfd/queue.rs` — replace `lf ... pr` / `lf op queue reconcile` spawns with
  direct library calls
- `lf_exec.rs` / `POST /v0/exec` — the remote door (lfq run) stays, but its
  validated argv follows the new human grammar; internal callers stop using it
- flow YAMLs that shell `lf op ...` in steps get the new names

## Plumbing: most of it dies

- `cp`, `shell` — delete unless a caller turns up
- `doctor` — folds into a setup/status surface (exact home open)
- `reset-waves` — `lf wave reset`
- `sync-skills` — moves into the install/refresh path (install.py already calls it)
- Survivors rehome under their noun; nothing keeps an `op`-style drawer

## Dies with op

- `next`, `advance`, `branches` — branch/wave rotation and remote-branch ops;
  no CLI home. Machine callers (lfd exec-door validates `["op", "next", ...]`)
  go in-process; anything land needs from advance/next gets absorbed into
  `lf pr land` / `lf wt` internals.
- `cp`, `shell`, `push`, `reset-waves` (→ `lf wave reset`), `sync-skills`
  (→ install path) — per plumbing section above

## Resolved dualities

- `sync` — subsumed by `lf rebase`. Already ~true in code: `rebase_with_recovery`
  calls the same `sync_main()` that `op sync` wraps (`ops/rebase.rs:218`).
  Two gaps to close before deleting the command:
  (a) rebase's sync is best-effort (`let _ =`) and silently skips a dirty main
  worktree where `op sync` hard-fails — surface the skip in progress output;
  (b) `lf rebase` while standing on main must short-circuit to sync-only
  instead of classifying main as a rebasable branch.
  Built-in `sync` dies; the `sync` flow keeps the bare name.
- `queue` — bare name stays with the flow (×44 in history); reconcile goes
  in-process, so no CLI home needed
- `release` — the exception: built-in suite wins the name, flow needs
  `lf flow release`

## Sweep inventory (rename cost)

- 79 `lf op` mentions across 136 builtin skill files (pm ×27, submit ×12, land ×12, wt ×9, pr ×7)
- Rust: queue.rs spawns, lf_exec.rs validation fixtures, onboarding.rs help text
- Docs: `docs/lfop.md`, `LOOPFLOW.md`, READMEs, goldens
- No alias shims — one name, migrate everything (house style)

## Demo

From a worktree: `lf pr open` opens the PR, `lf pr` shows its state,
`lf pr land` lands it, `lf wt create next-thing` rotates.
No `op` anywhere in the muscle memory.
