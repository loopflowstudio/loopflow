# One system

The subtraction program: one binary, one fold, one language. Audited
2026-07-08 — each section carries its receipts.

## One binary

Collapse the three binaries (`lf`, `lfd`, `lfq`) into one story. Most lfd
mutation handlers already delegate to the same `crate::ops::*` code `lf`
calls; the exec door re-execs `lf` argv; `lfq` is the live door client
(bin/lfq.rs) and needs its own collapse story.

- The named collapse-blockers each get a home or die: the resident HTTP
  listener socket; webhook ingress + signature verification + CI dedupe;
  the machine-global token-refresh loop; OS service management; boot
  hygiene; the bearer-auth boundary (including the exec-door verb
  allowlist, reviewed for flowloop-era commands); control-session tmux
  lifecycle.
- `lfq` folds into `lf` or is explicitly re-chartered.
- No second execution path survives: git, tmux, vendor, and worktree paths
  all route through `lf`.

## One fold

- The two independent read models become one: the journal fold
  (`fold_thread`/`fold_workers`, feeds the wave server) and the SQLite
  store interpretation (`lf ls`/`status`/`runs` + lfd HTTP routes) never
  share a fold today. Concerto, CLI, and server read the same projection.
  A steward that curates from two disagreeing read models curates lies —
  this precedes it.
- The journal scales honestly when it starts to hurt: rotation,
  segmentation, compaction (ff593217).

## One language

- One way to reach a flow/skill (today: `lf flow X`, `lf skill X`, and the
  unchecked bare `lf X`); ledger reads get one rule (`lf usage` is HTTP,
  `lf ls`/`runs` are direct); the doubled `-w/--wave` semantics collapse;
  `lf -l` vs `lf ls` becomes one listing mechanism (Linear 8e01041f).
- The govern skill's vocabulary converts: 61 chord/member references
  across 12 skill files, plus ~30 chord-tree mentions in Rust and one in
  Swift; keyboard-chord and workspace-member false positives stay fenced.
- The operating prompt is unified: one document, assembled once (cfa35ff).
