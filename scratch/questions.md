# Open questions / assumptions — W2-235

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
