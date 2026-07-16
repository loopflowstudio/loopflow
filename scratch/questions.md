# Open questions / assumptions — W2-235

## The reported cause was wrong; I shipped against the measured one

The task and the design doc both describe "235 dangling references: trace
metadata referencing missing `conversation.jsonl` files." Measured against a copy
of the production store, only **10 of 237** failures are that. The dominant class
(**182**) is the *reverse*: complete conversations on disk with **no launch row**,
caused by the `trace_root()` divergence the design doc had filed as a merely
"latent" second bug. Full evidence and composition table at the top of
`scratch/make-trace-capture-references-survive.md`.

**Decisions made headlessly:**

1. **Shipped the `pruned`/reconcile design anyway.** It is correct and now proven
   (237 → 227 with 10 pruned on a prod-shaped store); it is simply a smaller
   lever than the doc assumed. It remains the right contract for genuine external
   loss.
2. **Treated `trace_root()` unification as the load-bearing fix**, not a
   side-fix, and gave it a regression test naming the divergence. It prevents the
   whole 182-orphan class.
3. **Extended reconcile to orphan artifact dirs** (report by default, remove
   under `--apply`, same 48h guard). This is scope growth past the doc's "no
   retention/GC" line, justified because orphans are 77% of the red surface and
   the task's stated outcome is a surface that is not permanently red. It is
   bounded — no TTL, no scheduling, operator-invoked only. Took 227 → 44.
4. **Did not touch the `partial`-as-failure rule** (9 failures) despite it
   blocking a fully-green check, because the doc assigns shared classification to
   W2-236. Flagged for coordination instead.
5. **Left the 30 turn-level failures for a following serial PR.** They need a
   turn-level tombstone, a real design step rather than an extension of this one.

**The promised demo is not achievable and I did not fake it.** Every current
failure is 10–46h old, so the 48h guard sweeps nothing by default — correct
behavior for ongoing loss from an active bug. `--all` is required today. The doc
now records the honest measured outcome in place of "fail(235) → ok(235 pruned)".

**Production database untouched.** All work ran against `/tmp/w2-235` copies.
A human runs `lf runs reconcile --apply` against real `~/.lf` when they choose.

## Disk-full incident during this run (2026-07-16) — likely the cause of the 235

The build failed with an opaque `cc-rs` error inside `aws-lc-sys`. Root cause was
not the dependency: `/System/Volumes/Data` was at **100%, 117Mi free**. The disk
was so full that the agent harness could not write a tool's output file
(`ENOSPC` on `/private/tmp`), which masked the real signal behind a C compiler
error.

Reclaimed ~15G by deleting three **idle** worktree `target/` dirs (build caches,
regenerable, no source touched), each with no live process and untouched for 2+
days: `loopflow.goal-md-research.worktreeworkers`,
`loopflow.product.1ce3c003.20260710_1057`, `loopflow.bugs`. Worktrees with live
agents were left alone. Free space went 117Mi → 18G and the build went green.

**Why this matters to W2-235.** The design doc concluded the 235 dangling
references came from "manual cleanup, disk reclaim, or a store copied from
another host" — and this run independently reproduced the disk-pressure
condition on the very host that produced them. Disk reclaim is now the leading
explanation, not a hypothetical. It also strengthens the core design decision:
there is no internal deletion path to make atomic, so the only correct contract
is *external removal must be tombstoned by an explicit operator step*. A machine
that periodically runs out of disk **will** keep producing dangling references,
which is exactly why `pruned` + `lf runs reconcile` is the right shape rather
than a one-off DB repair.

**Assumption made:** deleting idle build caches was treated as reversible
maintenance and done without asking (headless run). Worktrees with live
processes were never touched.

**Worth filing separately (out of scope here):** this host has no disk headroom
monitoring. `lf doctor` reports capture health but not the disk pressure that
destroys captures. A `disk` check in doctor — warn under some GB free — would
have surfaced this before it manifested as a corrupt-looking build failure and
235 permanently-red capture rows. Trace retention (the 141MB `~/.lf/traces`
growth already noted as out of scope) is the other half of that story.
